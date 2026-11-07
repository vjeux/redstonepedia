package dev.vjeux.mctas.actor;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardOpenOption;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.function.Supplier;
import java.util.stream.Stream;
import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.CameraType;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.AbstractButton;
import net.minecraft.client.gui.components.events.ContainerEventHandler;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.inventory.BeaconScreen;
import net.minecraft.client.gui.screens.options.OptionsScreen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.ContainerInput;
import net.minecraft.world.inventory.Slot;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;

/**
 * The scripted survival player of `mctas bedrock video`: a daemon thread in the REAL client that
 * plays like a person — turns the player's head, holds attack (one `continueDestroyBlock` per
 * client tick, exactly like holding the mouse: survival break times, swings included), places
 * blocks with `useItemOn` (the use key's own code), picks hotbar slots and walks (the movement
 * keys' `setDown`, read by `KeyboardInput` each tick; jumping when stuck against a step).
 *
 * <p>Commands arrive one per file in `mctas-actor/` of the game directory (`cmd-<seq>.txt`, one
 * line); each answer lands in `done-<seq>.txt` (one line, `ok ...` or `error ...`) and the
 * command file is removed. Commands: `ping`, `pos`, `look x y z [ticks]`, `lookdir yaw pitch
 * [ticks]`, `select slot`, `mine x y z`, `place x y z FACE` (click that block's FACE with the
 * selected item), `placef x y z FACE yaw pitch` (the same, but looking along yaw/pitch instead
 * of at the face — how a dropper gets its facing), `use`, `toss` (the drop key: one item from
 * the selected slot), `cam first|third`, `fillinto x y z substr` (open the container there and
 * shift-click every matching stack of the player's inventory into it), `invcount substr`,
 * `walk x z`, `sneak 0|1`, `wait ticks`, `waitdim substr`, `waitblock x y z substr`, `swing`,
 * `keys_off`.
 */
public final class Actor {
    private Actor() {}

    private static final Path DIR = Paths.get("mctas-actor");
    private static volatile boolean started;

    public static void start() {
        if (started) {
            return;
        }
        started = true;
        Thread t = new Thread(Actor::loop, "mctas-actor");
        t.setDaemon(true);
        t.start();
    }

    private static void loop() {
        log("actor up");
        while (true) {
            try {
                Thread.sleep(40);
                if (!Files.isDirectory(DIR)) {
                    continue;
                }
                Path cmd;
                try (Stream<Path> s = Files.list(DIR)) {
                    cmd = s.filter(p -> p.getFileName().toString().matches("cmd-\\d+\\.txt"))
                            .min(Comparator.comparing(p -> p.getFileName().toString()))
                            .orElse(null);
                }
                if (cmd == null) {
                    continue;
                }
                String name = cmd.getFileName().toString();
                String seq = name.substring(4, name.length() - 4);
                String line = Files.readString(cmd, StandardCharsets.UTF_8).trim();
                String result;
                try {
                    result = run(line);
                } catch (Throwable e) {
                    result = "error " + e;
                    log("cmd " + seq + " [" + line + "] -> " + result);
                }
                // Atomic publish (write + rename): the driver's 50 ms poll must never see a
                // half-written (empty) answer — that race killed a take under CPU load.
                Path doneTmp = DIR.resolve("done-" + seq + ".part");
                Files.writeString(doneTmp, result + "\n", StandardCharsets.UTF_8);
                Files.move(doneTmp, DIR.resolve("done-" + seq + ".txt"), java.nio.file.StandardCopyOption.ATOMIC_MOVE);
                Files.deleteIfExists(cmd);
            } catch (InterruptedException e) {
                return;
            } catch (Throwable e) {
                log("loop: " + e);
            }
        }
    }

    private static void log(String line) {
        try {
            Files.createDirectories(DIR);
            Files.writeString(DIR.resolve("actor.log"), line + "\n", StandardCharsets.UTF_8,
                    StandardOpenOption.CREATE, StandardOpenOption.APPEND);
        } catch (Throwable ignored) {
        }
    }

    // ---- client-thread helpers -------------------------------------------------------------

    private static <T> T onClient(Supplier<T> f) throws Exception {
        Minecraft mc = Minecraft.getInstance();
        if (mc == null) {
            throw new IllegalStateException("no instance yet");
        }
        return mc.submit(f).get();
    }

    /** Blocks (actor thread) until the player exists (world joined / dimension loaded). */
    private static void waitPlayer() throws Exception {
        long deadline = System.currentTimeMillis() + 60_000;
        while (System.currentTimeMillis() < deadline) {
            Boolean there = onClient(() -> {
                Minecraft mc = Minecraft.getInstance();
                return mc.player != null && mc.level != null && mc.gameMode != null;
            });
            if (there) {
                return;
            }
            Thread.sleep(100);
        }
        throw new IllegalStateException("player never appeared");
    }

    private static int tickCount() throws Exception {
        return onClient(() -> Minecraft.getInstance().player == null ? -1 : Minecraft.getInstance().player.tickCount);
    }

    /** Waits until the client's tick count passes `from` + n (one gameMode call per tick = honest timing). */
    private static int waitTicks(int from, int n) throws Exception {
        int target = from + n;
        long deadline = System.currentTimeMillis() + 30_000L + n * 200L;
        while (System.currentTimeMillis() < deadline) {
            int now = tickCount();
            // A fresh player (dimension change / respawn) restarts tickCount: count it as passed.
            if (now >= target || now < from) {
                return now;
            }
            Thread.sleep(4);
        }
        throw new IllegalStateException("ticks stalled at " + tickCount() + " (wanted " + target + ")");
    }

    private static float wrapDeg(float a) {
        while (a <= -180.0F) {
            a += 360.0F;
        }
        while (a > 180.0F) {
            a -= 360.0F;
        }
        return a;
    }

    private static void setLook(LocalPlayer p, float yaw, float pitch) {
        p.setYRot(yaw);
        p.setXRot(Math.max(-90.0F, Math.min(90.0F, pitch)));
        p.yHeadRot = yaw;
    }

    /** Yaw/pitch from the player's eyes toward a point (Minecraft degrees). */
    private static float[] anglesTo(LocalPlayer p, double x, double y, double z) {
        Vec3 eye = p.getEyePosition();
        double dx = x - eye.x;
        double dy = y - eye.y;
        double dz = z - eye.z;
        float yaw = (float) Math.toDegrees(Math.atan2(-dx, dz));
        float pitch = (float) Math.toDegrees(Math.atan2(-dy, Math.hypot(dx, dz)));
        return new float[] {yaw, pitch};
    }

    private static void lookSmooth(float yaw, float pitch, int ticks) throws Exception {
        float[] cur = onClient(() -> {
            LocalPlayer p = Minecraft.getInstance().player;
            return new float[] {p.getYRot(), p.getXRot()};
        });
        int from = tickCount();
        for (int i = 1; i <= ticks; i++) {
            final int k = i;
            onClient(() -> {
                LocalPlayer p = Minecraft.getInstance().player;
                float t = (float) k / (float) ticks;
                // Ease in and out (smoothstep): camera turns that start and stop softly on video.
                float f = t * t * (3.0F - 2.0F * t);
                setLook(p, cur[0] + wrapDeg(yaw - cur[0]) * f, cur[1] + (pitch - cur[1]) * f);
                return null;
            });
            from = waitTicks(from, 1);
        }
    }

    private static Direction faceToward(LocalPlayer p, BlockPos pos) {
        Vec3 eye = p.getEyePosition();
        double dx = eye.x - (pos.getX() + 0.5);
        double dy = eye.y - (pos.getY() + 0.5);
        double dz = eye.z - (pos.getZ() + 0.5);
        double ax = Math.abs(dx);
        double ay = Math.abs(dy);
        double az = Math.abs(dz);
        if (ax >= ay && ax >= az) {
            return dx > 0 ? Direction.EAST : Direction.WEST;
        }
        if (ay >= az) {
            return dy > 0 ? Direction.UP : Direction.DOWN;
        }
        return dz > 0 ? Direction.SOUTH : Direction.NORTH;
    }

    private static String blockId(BlockState s) {
        return String.valueOf(BuiltInRegistries.BLOCK.getKey(s.getBlock()));
    }

    private static String heldDesc() {
        Minecraft mc = Minecraft.getInstance();
        if (mc.player == null) {
            return "none";
        }
        var stack = mc.player.getMainHandItem();
        return String.valueOf(BuiltInRegistries.ITEM.getKey(stack.getItem())) + "x" + stack.getCount();
    }

    private static String posLine() {
        Minecraft mc = Minecraft.getInstance();
        LocalPlayer p = mc.player;
        if (p == null || mc.level == null) {
            return "ok noplayer";
        }
        return String.format(Locale.ROOT, "ok dim=%s pos=%.2f,%.2f,%.2f look=%.1f,%.1f mode=%s creative=%s slot=%d held=%s tick=%d health=%.1f fps=%d grabbed=%s",
                mc.level.dimension().identifier(), p.getX(), p.getY(), p.getZ(), p.getYRot(), p.getXRot(),
                mc.gameMode == null ? "?" : mc.gameMode.getPlayerMode(), p.isCreative(),
                p.getInventory().getSelectedSlot(), heldDesc(), p.tickCount, p.getHealth(), mc.getFps(), mc.mouseHandler.isMouseGrabbed());
    }

    private static void keysOff() {
        Minecraft mc = Minecraft.getInstance();
        if (mc.options == null) {
            return;
        }
        mc.options.keyUp.setDown(false);
        mc.options.keyDown.setDown(false);
        mc.options.keyLeft.setDown(false);
        mc.options.keyRight.setDown(false);
        mc.options.keyJump.setDown(false);
        mc.options.keySprint.setDown(false);
    }

    // ---- commands --------------------------------------------------------------------------

    private static String run(String line) throws Exception {
        String[] f = line.split("\\s+");
        switch (f[0]) {
            case "ping":
                return onClient(Actor::posLine);
            case "pos":
                waitPlayer();
                return onClient(Actor::posLine);
            case "wait": {
                waitPlayer();
                waitTicks(tickCount(), Integer.parseInt(f[1]));
                return "ok waited";
            }
            case "look": {
                waitPlayer();
                double x = Double.parseDouble(f[1]);
                double y = Double.parseDouble(f[2]);
                double z = Double.parseDouble(f[3]);
                int ticks = f.length > 4 ? Integer.parseInt(f[4]) : 8;
                float[] a = onClient(() -> anglesTo(Minecraft.getInstance().player, x, y, z));
                lookSmooth(a[0], a[1], ticks);
                return "ok looking";
            }
            case "lookdir": {
                waitPlayer();
                int ticks = f.length > 3 ? Integer.parseInt(f[3]) : 8;
                lookSmooth(Float.parseFloat(f[1]), Float.parseFloat(f[2]), ticks);
                return "ok looking";
            }
            case "select": {
                waitPlayer();
                int slot = Integer.parseInt(f[1]);
                onClient(() -> {
                    Minecraft.getInstance().player.getInventory().setSelectedSlot(slot);
                    return null;
                });
                waitTicks(tickCount(), 2);
                return onClient(() -> "ok held=" + heldDesc());
            }
            case "mine":
                return mine(new BlockPos(Integer.parseInt(f[1]), Integer.parseInt(f[2]), Integer.parseInt(f[3])), f.length > 4 && f[4].equals("sweep"));
            case "place":
                return place(new BlockPos(Integer.parseInt(f[1]), Integer.parseInt(f[2]), Integer.parseInt(f[3])),
                        Direction.valueOf(f[4].toUpperCase(Locale.ROOT)), null);
            case "placef":
                return place(new BlockPos(Integer.parseInt(f[1]), Integer.parseInt(f[2]), Integer.parseInt(f[3])),
                        Direction.valueOf(f[4].toUpperCase(Locale.ROOT)),
                        new float[] {Float.parseFloat(f[5]), Float.parseFloat(f[6])});
            case "toss": {
                waitPlayer();
                onClient(() -> {
                    Minecraft mc = Minecraft.getInstance();
                    mc.gameMode.dropItem(mc.player, false);
                    return null;
                });
                return "ok tossed";
            }
            case "cam": {
                waitPlayer();
                onClient(() -> {
                    Minecraft.getInstance().options.setCameraType(
                            f[1].equals("third") ? CameraType.THIRD_PERSON_BACK
                                    : f[1].equals("front") ? CameraType.THIRD_PERSON_FRONT : CameraType.FIRST_PERSON);
                    return null;
                });
                return "ok cam=" + f[1];
            }
            case "fillinto":
                return fillInto(new BlockPos(Integer.parseInt(f[1]), Integer.parseInt(f[2]), Integer.parseInt(f[3])), f[4],
                        f.length > 5 ? Integer.parseInt(f[5]) : 99);
            case "hotbar": {
                // hotbar <slot> <item-substr>: end with a stack of that item in hotbar slot
                // <slot> (a number-key SWAP in the inventory screen, the GUI's own action);
                // answers ok slot=<where it now sits>.
                waitPlayer();
                int want = Integer.parseInt(f[1]);
                return onClient(() -> {
                    Minecraft mc = Minecraft.getInstance();
                    AbstractContainerMenu menu = mc.player.inventoryMenu;
                    Slot found = null;
                    for (Slot slot : menu.slots) {
                        if (slot.container != mc.player.getInventory()) {
                            continue;
                        }
                        String id = String.valueOf(BuiltInRegistries.ITEM.getKey(slot.getItem().getItem()));
                        if (id.contains(f[2]) && slot.getItem().getCount() > 0) {
                            if (slot.getContainerSlot() < 9) {
                                return "ok slot=" + slot.getContainerSlot();
                            }
                            found = slot;
                        }
                    }
                    if (found == null) {
                        return "error no *" + f[2] + "* in inventory";
                    }
                    mc.gameMode.handleContainerInput(menu.containerId, found.index, want, ContainerInput.SWAP, mc.player);
                    return "ok slot=" + want;
                });
            }
            case "invcount": {
                waitPlayer();
                return onClient(() -> {
                    Minecraft mc = Minecraft.getInstance();
                    int n = 0;
                    for (var stack : mc.player.getInventory().getNonEquipmentItems()) {
                        if (String.valueOf(BuiltInRegistries.ITEM.getKey(stack.getItem())).contains(f[1])) {
                            n += stack.getCount();
                        }
                    }
                    return "ok count=" + n;
                });
            }
            case "use": {
                waitPlayer();
                return onClient(() -> {
                    Minecraft mc = Minecraft.getInstance();
                    InteractionResult r = mc.gameMode.useItem(mc.player, InteractionHand.MAIN_HAND);
                    return "ok use=" + r;
                });
            }
            case "walk":
                // walk x z [jump]: `jump` hops a step as soon as it blocks (climbing a pyramid).
                return walk(Double.parseDouble(f[1]) + 0.5, Double.parseDouble(f[2]) + 0.5, f.length > 3 && f[3].equals("jump") ? 1 : 8);
            case "sneak": {
                waitPlayer();
                boolean on = f[1].equals("1");
                onClient(() -> {
                    Minecraft.getInstance().options.keyShift.setDown(on);
                    return null;
                });
                return "ok sneak=" + on;
            }
            case "swing": {
                waitPlayer();
                onClient(() -> {
                    LocalPlayer p = Minecraft.getInstance().player;
                    p.swing(InteractionHand.MAIN_HAND, p.getMainHandItem().getAttackAnimation(), false);
                    return null;
                });
                return "ok swung";
            }
            case "keys_off": {
                onClient(() -> {
                    keysOff();
                    if (Minecraft.getInstance().options != null) {
                        Minecraft.getInstance().options.keyShift.setDown(false);
                    }
                    if (Minecraft.getInstance().gameMode != null) {
                        Minecraft.getInstance().gameMode.stopDestroyBlock();
                    }
                    return null;
                });
                return "ok off";
            }
            case "waitdim": {
                long deadline = System.currentTimeMillis() + 60_000;
                while (System.currentTimeMillis() < deadline) {
                    String dim = onClient(() -> {
                        Minecraft mc = Minecraft.getInstance();
                        return mc.level == null ? "" : String.valueOf(mc.level.dimension().identifier());
                    });
                    if (dim.contains(f[1])) {
                        waitPlayer();
                        return onClient(Actor::posLine);
                    }
                    Thread.sleep(100);
                }
                return "error never reached dimension *" + f[1] + "*";
            }
            case "waitblock": {
                waitPlayer();
                BlockPos pos = new BlockPos(Integer.parseInt(f[1]), Integer.parseInt(f[2]), Integer.parseInt(f[3]));
                long deadline = System.currentTimeMillis() + 60_000;
                String seen = "?";
                while (System.currentTimeMillis() < deadline) {
                    seen = onClient(() -> blockId(Minecraft.getInstance().level.getBlockState(pos)));
                    if (seen.contains(f[4])) {
                        return "ok block=" + seen;
                    }
                    Thread.sleep(100);
                }
                return "error " + pos + " stayed " + seen;
            }
            case "tap": {
                // tap <key name>: one press + release of a physical key inside one frame, as the
                // game's key callback does it: KeyMapping.click on EVERY mapping bound to that key
                // (a key bound to Attack and Use clicks both), consumed by the next client tick.
                waitPlayer();
                InputConstants.Key key = InputConstants.getKey(f[1]);
                int t0 = tickCount();
                long ms = System.currentTimeMillis();
                onClient(() -> {
                    KeyMapping.click(key);
                    return null;
                });
                waitTicks(t0, 2);
                return "ok tapped " + f[1] + " at " + ms;
            }
            case "click":
                return click(f[1]);
            case "hit": {
                waitPlayer();
                return onClient(Actor::hitLine);
            }
            case "hud": {
                waitPlayer();
                boolean show = f[1].equals("1");
                onClient(() -> {
                    Minecraft mc = Minecraft.getInstance();
                    if (mc.gui.hud.isHidden() == show) {
                        mc.gui.hud.toggle();
                    }
                    return null;
                });
                return "ok hud=" + show;
            }
            case "pose": {
                waitPlayer();
                return onClient(() -> {
                    Minecraft mc = Minecraft.getInstance();
                    LocalPlayer p = mc.player;
                    Vec3 e = p.getEyePosition();
                    Vec3 c = mc.gameRenderer.mainCamera().position();
                    return String.format(Locale.ROOT, "ok eye=%.4f,%.4f,%.4f cam=%.4f,%.4f,%.4f yaw=%.3f pitch=%.3f fov=%d win=%dx%d pose=%s onground=%s",
                            e.x, e.y, e.z, c.x, c.y, c.z, p.getYRot(), p.getXRot(), mc.options.fov().get(),
                            mc.getWindow().getWidth(), mc.getWindow().getHeight(), p.getPose(), p.onGround());
                });
            }
            case "hold": {
                // hold <forward|back|left|right|jump|sneak> <ticks>: a movement key held down.
                waitPlayer();
                KeyMapping k = onClient(() -> mapping(f[1]));
                int n = Integer.parseInt(f[2]);
                onClient(() -> {
                    k.setDown(true);
                    return null;
                });
                try {
                    waitTicks(tickCount(), n);
                } finally {
                    onClient(() -> {
                        k.setDown(false);
                        return null;
                    });
                }
                return onClient(Actor::posLine);
            }
            case "pressat": {
                // pressat <wall ms> <key>: tap the key at that wall-clock time, from this thread
                // (no frame wait): the click is consumed by the client's next tick.
                long at = Long.parseLong(f[1]);
                InputConstants.Key key = InputConstants.getKey(f[2]);
                while (System.currentTimeMillis() < at - 3) {
                    Thread.sleep(1);
                }
                while (System.currentTimeMillis() < at) {
                    Thread.onSpinWait();
                }
                long ms = System.currentTimeMillis();
                KeyMapping.click(key);
                waitTicks(tickCount(), 2);
                return "ok pressed at " + ms;
            }
            case "holduse": {
                // holduse <ticks>: the use key held down (drinking a potion: 32 ticks).
                waitPlayer();
                int n = Integer.parseInt(f[1]);
                onClient(() -> {
                    Minecraft.getInstance().options.keyUse.setDown(true);
                    return null;
                });
                try {
                    waitTicks(tickCount(), n);
                } finally {
                    onClient(() -> {
                        Minecraft.getInstance().options.keyUse.setDown(false);
                        return null;
                    });
                }
                return "ok held use " + n;
            }
            case "gui":
                return gui(f);
            case "beacon":
                return beacon(f.length > 1 ? f[1] : "iron_ingot");
            case "track":
                return track(f);
            case "presscue":
                return pressCue(f);
            case "toasts": {
                // toasts off: keep the toast corner empty (recipe/advancement pop-ups from the kit
                // would cover the video); toasts on: let them show again.
                boolean off = f[1].equals("off");
                noToasts = off;
                if (off && !toastThread) {
                    toastThread = true;
                    Thread t = new Thread(() -> {
                        while (true) {
                            try {
                                if (noToasts) {
                                    onClient(() -> {
                                        Minecraft.getInstance().gui.toastManager().clear();
                                        return null;
                                    });
                                }
                                Thread.sleep(40);
                            } catch (Throwable e) {
                                log("toasts: " + e);
                            }
                        }
                    }, "mctas-toasts");
                    t.setDaemon(true);
                    t.start();
                }
                return "ok toasts off=" + off;
            }
            case "state": {
                // state x y z: the block state the client sees there.
                waitPlayer();
                BlockPos pos = new BlockPos(Integer.parseInt(f[1]), Integer.parseInt(f[2]), Integer.parseInt(f[3]));
                return onClient(() -> "ok " + Minecraft.getInstance().level.getBlockState(pos));
            }
            default:
                return "error unknown command " + f[0];
        }
    }

    /**
     * Looks at the block and holds the attack key until it is air: the client's own per-tick
     * `continueAttack` does the work (starting the break, swinging, survival timing), exactly
     * like a held mouse button. The actor only aims, presses, and polls a few times a second
     * (client-thread tasks run once per frame, so driving every tick from outside would mine
     * at frame rate, not tick rate). Needs the mouse grabbed, like real play: on a headless
     * display the window may never take focus, so grab it (and as a last resort set the flag
     * the grab would set — the same state a focused window has).
     */
    private static String mine(BlockPos pos, boolean sweep) throws Exception {
        waitPlayer();
        float[] a = onClient(() -> anglesTo(Minecraft.getInstance().player, pos.getX() + 0.5, pos.getY() + 0.5, pos.getZ() + 0.5));
        lookSmooth(a[0], a[1], sweep ? 1 : 3);
        // Fast blocks (a few ticks: netherrack, deepslate) are mined with direct per-tick calls:
        // precise (no overshoot into whatever is behind) and only a handful of round trips.
        // Slow blocks (obsidian: ~31 ticks) hold the attack key instead, so the client's own
        // per-tick path runs at full tick rate regardless of frame rate; overshoot there only
        // starts a crack in the next obsidian, which resets when the key is released.
        // `sweep` (bulk clearing, digging down): always the held key — overshoot just mines
        // more of the same rock, exactly like a player sweeping an instamine pick.
        float perTickF = onClient(() -> {
            Minecraft mc = Minecraft.getInstance();
            BlockState s = mc.level.getBlockState(pos);
            return s.isAir() ? 1.0F : s.getDestroyProgress(mc.player, mc.player.level(), pos);
        });
        float perTick = sweep ? 0.0F : perTickF;
        if (sweep && perTickF >= 1.0F && onClient(() -> Minecraft.getInstance().level.getBlockState(pos).isAir())) {
            return "ok mined";
        }
        if (perTick >= 1.0F) {
            // already air, or one direct call breaks it (instamine)
            onClient(() -> {
                Minecraft mc = Minecraft.getInstance();
                if (!mc.level.getBlockState(pos).isAir()) {
                    Direction dir = faceToward(mc.player, pos);
                    if (mc.gameMode.startDestroyBlock(pos, dir)) {
                        mc.player.swing(InteractionHand.MAIN_HAND, mc.player.getMainHandItem().getAttackAnimation(), false);
                    }
                }
                return null;
            });
            Thread.sleep(60);
            String now = onClient(() -> blockId(Minecraft.getInstance().level.getBlockState(pos)));
            return now.contains("air") ? "ok mined" : "error instamine left " + now;
        }
        if (perTick >= 0.2F) {
            Direction dir = onClient(() -> faceToward(Minecraft.getInstance().player, pos));
            int[] lastTick = {-1};
            long deadline = System.currentTimeMillis() + 20_000;
            while (System.currentTimeMillis() < deadline) {
                Boolean done = onClient(() -> {
                    Minecraft mc = Minecraft.getInstance();
                    if (mc.player == null || mc.level == null) {
                        return false;
                    }
                    if (mc.level.getBlockState(pos).isAir()) {
                        mc.gameMode.stopDestroyBlock();
                        return true;
                    }
                    if (mc.player.tickCount == lastTick[0]) {
                        return false; // same game tick: a second call would mine faster than a held button
                    }
                    lastTick[0] = mc.player.tickCount;
                    float[] aim = anglesTo(mc.player, pos.getX() + 0.5, pos.getY() + 0.5, pos.getZ() + 0.5);
                    setLook(mc.player, aim[0], aim[1]);
                    if (mc.gameMode.continueDestroyBlock(pos, dir)) {
                        mc.player.swing(InteractionHand.MAIN_HAND, mc.player.getMainHandItem().getAttackAnimation(), false);
                    }
                    return false;
                });
                if (done) {
                    return "ok mined";
                }
                Thread.sleep(30);
            }
            onClient(() -> {
                Minecraft.getInstance().gameMode.stopDestroyBlock();
                return null;
            });
            return "error mine timed out";
        }
        onClient(() -> {
            Minecraft mc = Minecraft.getInstance();
            if (!mc.mouseHandler.isMouseGrabbed()) {
                mc.mouseHandler.grabMouse();
            }
            if (!mc.mouseHandler.isMouseGrabbed()) {
                try {
                    java.lang.reflect.Field f = mc.mouseHandler.getClass().getDeclaredField("mouseGrabbed");
                    f.setAccessible(true);
                    f.setBoolean(mc.mouseHandler, true);
                    log("mine: mouse grab forced (window never focused)");
                } catch (Throwable e) {
                    log("mine: grab failed: " + e);
                }
            }
            mc.options.keyAttack.setDown(true);
            return null;
        });
        try {
            // The held key mines whatever the crosshair touches: if this block is occluded,
            // it makes no progress — give it a few seconds, then finish with direct per-tick
            // calls (which name the block, like the precise path).
            long deadline = System.currentTimeMillis() + 6_000;
            while (System.currentTimeMillis() < deadline) {
                Boolean done = onClient(() -> {
                    Minecraft mc = Minecraft.getInstance();
                    if (mc.player == null || mc.level == null) {
                        return false;
                    }
                    if (mc.level.getBlockState(pos).isAir()) {
                        return true;
                    }
                    float[] aim = anglesTo(mc.player, pos.getX() + 0.5, pos.getY() + 0.5, pos.getZ() + 0.5);
                    setLook(mc.player, aim[0], aim[1]);
                    return false;
                });
                if (done) {
                    return "ok mined";
                }
                Thread.sleep(120);
            }
        } finally {
            onClient(() -> {
                Minecraft mc = Minecraft.getInstance();
                mc.options.keyAttack.setDown(false);
                if (mc.gameMode != null) {
                    mc.gameMode.stopDestroyBlock();
                }
                return null;
            });
        }
        // Fallback for an occluded target: direct per-tick calls naming this block.
        Direction dir2 = onClient(() -> faceToward(Minecraft.getInstance().player, pos));
        int[] lastTick2 = {-1};
        long deadline2 = System.currentTimeMillis() + 30_000;
        while (System.currentTimeMillis() < deadline2) {
            Boolean done = onClient(() -> {
                Minecraft mc = Minecraft.getInstance();
                if (mc.player == null || mc.level == null) {
                    return false;
                }
                if (mc.level.getBlockState(pos).isAir()) {
                    mc.gameMode.stopDestroyBlock();
                    return true;
                }
                if (mc.player.tickCount == lastTick2[0]) {
                    return false;
                }
                lastTick2[0] = mc.player.tickCount;
                float[] aim = anglesTo(mc.player, pos.getX() + 0.5, pos.getY() + 0.5, pos.getZ() + 0.5);
                setLook(mc.player, aim[0], aim[1]);
                if (mc.gameMode.continueDestroyBlock(pos, dir2)) {
                    mc.player.swing(InteractionHand.MAIN_HAND, mc.player.getMainHandItem().getAttackAnimation(), false);
                }
                return false;
            });
            if (done) {
                return "ok mined";
            }
            Thread.sleep(30);
        }
        String held = onClient(() -> blockId(Minecraft.getInstance().level.getBlockState(pos)));
        return "error mine timed out, block still " + held;
    }

    /** Looks at the clicked face, then the use key's own call: place (or light) with the held item. */
    private static String place(BlockPos pos, Direction face, float[] fixedLook) throws Exception {
        waitPlayer();
        double hx = pos.getX() + 0.5 + face.getStepX() * 0.5;
        double hy = pos.getY() + 0.5 + face.getStepY() * 0.5;
        double hz = pos.getZ() + 0.5 + face.getStepZ() * 0.5;
        if (fixedLook != null) {
            // A dropper's facing comes from where the player LOOKS, not from the clicked face:
            // hold the given yaw/pitch for the click (the hit result still names the same face).
            lookSmooth(fixedLook[0], fixedLook[1], 4);
        } else {
            float[] a = onClient(() -> anglesTo(Minecraft.getInstance().player, hx, hy, hz));
            lookSmooth(a[0], a[1], 4);
        }
        String r = onClient(() -> {
            Minecraft mc = Minecraft.getInstance();
            BlockHitResult hit = new BlockHitResult(new Vec3(hx, hy, hz), face, pos, false);
            InteractionResult res = mc.gameMode.useItemOn(mc.player, InteractionHand.MAIN_HAND, hit);
            if (res.consumesAction()) {
                mc.player.swing(InteractionHand.MAIN_HAND, mc.player.getMainHandItem().getAttackAnimation(), false);
            }
            return String.valueOf(res);
        });
        waitTicks(tickCount(), 2);
        BlockPos target = pos.relative(face);
        String there = onClient(() -> blockId(Minecraft.getInstance().level.getBlockState(target)));
        return "ok place=" + r + " target=" + there;
    }

    /**
     * Opens the container at `pos` (a right-click, the use key's own call), shift-clicks up
     * to `maxStacks` player-inventory stacks whose item id contains `substr` into it, and
     * closes it: how a player loads a dropper (capped so the player keeps their own supply).
     */
    private static String fillInto(BlockPos pos, String substr, int maxStacks) throws Exception {
        waitPlayer();
        float[] a = onClient(() -> anglesTo(Minecraft.getInstance().player,
                pos.getX() + 0.5, pos.getY() + 0.5, pos.getZ() + 0.5));
        lookSmooth(a[0], a[1], 4);
        onClient(() -> {
            Minecraft mc = Minecraft.getInstance();
            BlockHitResult hit = new BlockHitResult(
                    new Vec3(pos.getX() + 0.5, pos.getY() + 0.5, pos.getZ() + 0.5), Direction.UP, pos, false);
            mc.gameMode.useItemOn(mc.player, InteractionHand.MAIN_HAND, hit);
            return null;
        });
        long deadline = System.currentTimeMillis() + 10_000;
        AbstractContainerMenu menu = null;
        while (System.currentTimeMillis() < deadline) {
            menu = onClient(() -> {
                Minecraft mc = Minecraft.getInstance();
                AbstractContainerMenu m = mc.player.containerMenu;
                return m != null && m != mc.player.inventoryMenu ? m : null;
            });
            if (menu != null) {
                break;
            }
            Thread.sleep(60);
        }
        if (menu == null) {
            return "error container at " + pos + " never opened";
        }
        AbstractContainerMenu open = menu;
        int moved = onClient(() -> {
            Minecraft mc = Minecraft.getInstance();
            int n = 0;
            int stacks = 0;
            // Slots of the player's own inventory inside a container menu hold stacks whose
            // container is the Inventory; everything else is the opened container's side.
            for (Slot slot : open.slots) {
                if (stacks >= maxStacks) {
                    break;
                }
                var stack = slot.getItem();
                String id = String.valueOf(BuiltInRegistries.ITEM.getKey(stack.getItem()));
                if (slot.container == mc.player.getInventory() && id.contains(substr) && stack.getCount() > 0) {
                    n += stack.getCount();
                    stacks++;
                    mc.gameMode.handleContainerInput(open.containerId, slot.index, 0, ContainerInput.QUICK_MOVE, mc.player);
                }
            }
            return n;
        });
        waitTicks(tickCount(), 3);
        onClient(() -> {
            Minecraft mc = Minecraft.getInstance();
            mc.player.closeContainer(); // also clears the screen (clientSideCloseContainer)
            return null;
        });
        return "ok moved=" + moved;
    }

    /** Walks (forward key, steering each tick) until over (x, z); jumps when stuck against a step. */
    private static String walk(double tx, double tz, int jumpAfter) throws Exception {
        waitPlayer();
        Object startLevel = onClient(() -> Minecraft.getInstance().level);
        try {
            int from = tickCount();
            double[] last = onClient(() -> {
                LocalPlayer p = Minecraft.getInstance().player;
                return new double[] {p.getX(), p.getZ()};
            });
            int[] stuck = {0};
            int[] sinceLog = {0};
            long deadline = System.currentTimeMillis() + 25_000;
            while (System.currentTimeMillis() < deadline) {
                Boolean arrived = onClient(() -> {
                    Minecraft mc = Minecraft.getInstance();
                    LocalPlayer p = mc.player;
                    if (p == null || mc.level != startLevel) {
                        // Stepped through a portal mid-walk: the walk is over by definition.
                        keysOff();
                        return true;
                    }
                    double dx = tx - p.getX();
                    double dz = tz - p.getZ();
                    if (Math.hypot(dx, dz) < 0.35) {
                        keysOff();
                        return true;
                    }
                    float yaw = (float) Math.toDegrees(Math.atan2(-dx, dz));
                    setLook(p, yaw, p.getXRot());
                    mc.options.keyUp.setDown(true);
                    double moved = Math.hypot(p.getX() - last[0], p.getZ() - last[1]);
                    if (moved < 0.01 && p.onGround()) {
                        stuck[0]++;
                    } else {
                        stuck[0] = 0;
                    }
                    last[0] = p.getX();
                    last[1] = p.getZ();
                    mc.options.keyJump.setDown(stuck[0] > jumpAfter);
                    if (stuck[0] > 14) {
                        stuck[0] = 0;
                    }
                    if (++sinceLog[0] >= 40) {
                        sinceLog[0] = 0;
                        log(String.format(Locale.ROOT, "walk -> (%.1f, %.1f): at %.2f, %.2f, %.2f onGround=%s fwd=%s screen=%s",
                                tx, tz, p.getX(), p.getY(), p.getZ(), p.onGround(), mc.options.keyUp.isDown(),
                                mc.gui.screen() == null ? "none" : mc.gui.screen().getClass().getSimpleName()));
                    }
                    return false;
                });
                if (arrived) {
                    return onClient(Actor::posLine);
                }
                from = waitTicks(from, 1);
            }
            return "error walk timed out";
        } finally {
            onClient(() -> {
                keysOff();
                return null;
            });
        }
    }

    // ---- the roof-machine tutorial (`mctas bedrock yt`) ----------------------------------------

    /** `click use|attack|both`: the use / attack key's own handler (Minecraft.startUseItem /
     * startAttack, the code a key press runs): whatever the crosshair targets is what is clicked. */
    private static String click(String which) throws Exception {
        waitPlayer();
        String r = onClient(() -> {
            Minecraft mc = Minecraft.getInstance();
            try {
                String before = hitLine();
                if (which.equals("attack") || which.equals("both")) {
                    Method m = Minecraft.class.getDeclaredMethod("startAttack");
                    m.setAccessible(true);
                    m.invoke(mc);
                }
                if (which.equals("use") || which.equals("both")) {
                    Method m = Minecraft.class.getDeclaredMethod("startUseItem");
                    m.setAccessible(true);
                    m.invoke(mc);
                }
                return before;
            } catch (Exception e) {
                return "error " + e;
            }
        });
        waitTicks(tickCount(), 2);
        return r;
    }

    private static String hitLine() {
        Minecraft mc = Minecraft.getInstance();
        HitResult h = mc.hitResult;
        if (h == null) {
            return "ok type=NONE";
        }
        if (h instanceof BlockHitResult b && h.getType() == HitResult.Type.BLOCK) {
            BlockPos p = b.getBlockPos();
            Vec3 l = b.getLocation();
            return String.format(Locale.ROOT, "ok type=BLOCK pos=%d,%d,%d face=%s loc=%.3f,%.3f,%.3f block=%s",
                    p.getX(), p.getY(), p.getZ(), b.getDirection().getName(), l.x, l.y, l.z, blockId(mc.level.getBlockState(p)));
        }
        return "ok type=" + h.getType();
    }

    private static KeyMapping mapping(String name) {
        Minecraft mc = Minecraft.getInstance();
        switch (name) {
            case "forward":
                return mc.options.keyUp;
            case "back":
                return mc.options.keyDown;
            case "left":
                return mc.options.keyLeft;
            case "right":
                return mc.options.keyRight;
            case "jump":
                return mc.options.keyJump;
            case "sneak":
                return mc.options.keyShift;
            default:
                throw new IllegalArgumentException("no movement key " + name);
        }
    }

    private static Object fieldOf(Object o, String name) throws Exception {
        for (Class<?> c = o.getClass(); c != null; c = c.getSuperclass()) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(o);
            } catch (NoSuchFieldException ignored) {
            }
        }
        throw new NoSuchFieldException(name + " in " + o.getClass());
    }

    private static Method methodOf(Object o, String name, Class<?>... args) throws Exception {
        for (Class<?> c = o.getClass(); c != null; c = c.getSuperclass()) {
            try {
                Method m = c.getDeclaredMethod(name, args);
                m.setAccessible(true);
                return m;
            } catch (NoSuchMethodException ignored) {
            }
        }
        throw new NoSuchMethodException(name + " in " + o.getClass());
    }

    private static AbstractButton findButton(List<? extends GuiEventListener> kids, String want) {
        for (GuiEventListener l : kids) {
            if (l instanceof AbstractButton b && b.visible && b.getMessage().getString().contains(want)) {
                return b;
            }
            if (l instanceof ContainerEventHandler c) {
                AbstractButton b = findButton(c.children(), want);
                if (b != null) {
                    return b;
                }
            }
        }
        return null;
    }

    /** The KeyBindsList entry whose mapping is `name` (`key.attack`), from the open key binds screen. */
    private static Object keyEntry(Screen s, String name) throws Exception {
        Object list = fieldOf(s, "keyBindsList");
        for (Object e : (List<?>) methodOf(list, "children").invoke(list)) {
            if (e.getClass().getSimpleName().equals("KeyEntry") && ((KeyMapping) fieldOf(e, "key")).getName().equals(name)) {
                return e;
            }
        }
        throw new IllegalStateException("no key entry " + name);
    }

    /**
     * `gui options` (open the options screen), `gui press <label>` (press the visible button
     * whose label contains it: the button's own onPress, as a click on it), `gui scrollto
     * <mapping>` / `gui select <mapping>` (key binds screen: scroll its entry into view / press its
     * binding button), `gui key <code>` (a key press delivered to the open screen: how the key
     * binds screen reads the new key), `gui close`, `gui name` (the open screen's class).
     */
    private static String gui(String[] f) throws Exception {
        waitPlayer();
        String rest = String.join(" ", Arrays.copyOfRange(f, 2, f.length));
        return onClient(() -> {
            Minecraft mc = Minecraft.getInstance();
            Screen s = mc.gui.screen();
            try {
                switch (f[1]) {
                    case "options":
                        mc.gui.setScreen(new OptionsScreen(null, mc.options));
                        return "ok";
                    case "name":
                        return "ok screen=" + (s == null ? "none" : s.getClass().getSimpleName());
                    case "close":
                        if (s != null) {
                            s.onClose();
                        }
                        mc.gui.setScreen(null);
                        return "ok";
                    case "press": {
                        if (s == null) {
                            return "error no screen";
                        }
                        AbstractButton b = findButton(s.children(), rest);
                        if (b == null) {
                            return "error no button *" + rest + "*";
                        }
                        b.onPress(new KeyEvent(InputConstants.KEY_RETURN, 0, 0));
                        return "ok pressed " + b.getMessage().getString();
                    }
                    case "scrollto": {
                        Object e = keyEntry(s, rest);
                        Object list = fieldOf(s, "keyBindsList");
                        for (Class<?> c = list.getClass(); c != null; c = c.getSuperclass()) {
                            for (Method m : c.getDeclaredMethods()) {
                                if (m.getName().equals("centerScrollOn") && m.getParameterCount() == 1) {
                                    m.setAccessible(true);
                                    m.invoke(list, e);
                                    return "ok";
                                }
                            }
                        }
                        return "error no centerScrollOn";
                    }
                    case "select": {
                        Object e = keyEntry(s, rest);
                        AbstractButton b = (AbstractButton) fieldOf(e, "changeButton");
                        b.onPress(new KeyEvent(InputConstants.KEY_RETURN, 0, 0));
                        return "ok selected " + rest;
                    }
                    case "key": {
                        if (s == null) {
                            return "error no screen";
                        }
                        boolean used = s.keyPressed(new KeyEvent(Integer.parseInt(f[2]), 0, 0));
                        return "ok key used=" + used;
                    }
                    case "binds": {
                        StringBuilder sb = new StringBuilder("ok");
                        for (KeyMapping k : mc.options.keyMappings) {
                            if (k.getName().equals("key.attack") || k.getName().equals("key.use")) {
                                sb.append(' ').append(k.getName()).append('=').append(k.saveString());
                            }
                        }
                        return sb.toString();
                    }
                    default:
                        return "error unknown gui command " + f[1];
                }
            } catch (Exception e) {
                return "error " + e;
            }
        });
    }

    /**
     * With the beacon's screen open: shift-click one `pay` item into the payment slot, then press
     * the Haste button (primary), the "II" upgrade button and the confirm button, each the screen's
     * own button press, a short pause between them (the video shows each step).
     */
    private static String beacon(String pay) throws Exception {
        waitPlayer();
        String r1 = onClient(() -> {
            Minecraft mc = Minecraft.getInstance();
            if (!(mc.gui.screen() instanceof BeaconScreen)) {
                return "error no beacon screen";
            }
            AbstractContainerMenu menu = mc.player.containerMenu;
            for (Slot slot : menu.slots) {
                String id = String.valueOf(BuiltInRegistries.ITEM.getKey(slot.getItem().getItem()));
                if (slot.container == mc.player.getInventory() && id.contains(pay) && slot.getItem().getCount() > 0) {
                    mc.gameMode.handleContainerInput(menu.containerId, slot.index, 0, ContainerInput.QUICK_MOVE, mc.player);
                    return "ok";
                }
            }
            return "error no " + pay + " in the inventory";
        });
        if (r1.startsWith("error")) {
            return r1;
        }
        StringBuilder log = new StringBuilder("ok");
        for (String step : new String[] {"primary", "upgrade", "confirm"}) {
            waitTicks(tickCount(), 16);
            String r = onClient(() -> {
                Screen s = Minecraft.getInstance().gui.screen();
                if (!(s instanceof BeaconScreen)) {
                    return "error the beacon screen closed before " + step;
                }
                try {
                    for (Object b : (List<?>) fieldOf(s, "beaconButtons")) {
                        String cls = b.getClass().getSimpleName();
                        AbstractButton ab = (AbstractButton) b;
                        boolean hit = switch (step) {
                            case "primary" -> cls.equals("BeaconPowerButton") && (Boolean) fieldOf(b, "isPrimary")
                                    && fieldOf(b, "effect").equals(MobEffects.HASTE);
                            case "upgrade" -> cls.equals("BeaconUpgradePowerButton");
                            default -> cls.equals("BeaconConfirmButton");
                        };
                        if (hit) {
                            if (!ab.active || !ab.visible) {
                                return "error " + step + " button inactive (active=" + ab.active + " visible=" + ab.visible + ")";
                            }
                            ab.onPress(new KeyEvent(InputConstants.KEY_RETURN, 0, 0));
                            return "ok " + step;
                        }
                    }
                    return "error no " + step + " button";
                } catch (Exception e) {
                    return "error " + e;
                }
            });
            if (r.startsWith("error")) {
                return r;
            }
            log.append(' ').append(step);
        }
        return log.toString();
    }

    private static volatile boolean tracking;
    private static volatile boolean noToasts;
    private static boolean toastThread;

    /**
     * `track start <file> x,y,z [x,y,z ...]`: log every change the client sees in those cells
     * (`<wall ms> <client tick> <cell index> <state>`, one line per change, sampled every frame)
     * until `track stop`: the machine's rhythm as the player sees it, for the video's cue overlay.
     */
    private static String track(String[] f) throws Exception {
        if (f[1].equals("stop")) {
            tracking = false;
            Thread.sleep(100);
            return "ok stopped";
        }
        waitPlayer();
        tracking = false;
        Thread.sleep(100);
        Path file = DIR.resolve(f[2]);
        List<BlockPos> cells = new ArrayList<>();
        for (int i = 3; i < f.length; i++) {
            String[] c = f[i].split(",");
            cells.add(new BlockPos(Integer.parseInt(c[0]), Integer.parseInt(c[1]), Integer.parseInt(c[2])));
        }
        Files.writeString(file, "", StandardCharsets.UTF_8);
        tracking = true;
        Thread t = new Thread(() -> {
            String[] last = new String[cells.size()];
            StringBuilder buf = new StringBuilder();
            long flushed = System.currentTimeMillis();
            while (tracking) {
                try {
                    String[] now = onClient(() -> {
                        Minecraft mc = Minecraft.getInstance();
                        String[] out = new String[cells.size() + 1];
                        out[0] = String.valueOf(mc.player == null ? -1 : mc.player.tickCount);
                        for (int i = 0; i < cells.size(); i++) {
                            out[i + 1] = mc.level == null ? "?" : String.valueOf(mc.level.getBlockState(cells.get(i)));
                        }
                        return out;
                    });
                    long ms = System.currentTimeMillis();
                    for (int i = 0; i < cells.size(); i++) {
                        if (!now[i + 1].equals(last[i])) {
                            last[i] = now[i + 1];
                            buf.append(ms).append(' ').append(now[0]).append(' ').append(i).append(' ').append(now[i + 1]).append('\n');
                        }
                    }
                    if (ms - flushed > 500 && buf.length() > 0) {
                        Files.writeString(file, buf.toString(), StandardCharsets.UTF_8, StandardOpenOption.APPEND);
                        buf.setLength(0);
                        flushed = ms;
                    }
                    Thread.sleep(2);
                } catch (Throwable e) {
                    log("track: " + e);
                    return;
                }
            }
            try {
                Files.writeString(file, buf.toString(), StandardCharsets.UTF_8, StandardOpenOption.APPEND);
            } catch (Throwable ignored) {
            }
        }, "mctas-track");
        t.setDaemon(true);
        t.start();
        return "ok tracking " + cells.size();
    }

    /**
     * `presscue x y z <key> <delay ticks> <timeout ms>`: watch the cell (the swap piston's head
     * cell) until the client sees a piston head LAND there (the arm fully out: the cue), wait
     * `delay` client ticks, then tap the key (KeyMapping.click, as `tap`). With delay 0 the tap
     * happens in the same frame the cue is seen. Answers the cue/tap wall times.
     */
    private static String pressCue(String[] f) throws Exception {
        waitPlayer();
        BlockPos pos = new BlockPos(Integer.parseInt(f[1]), Integer.parseInt(f[2]), Integer.parseInt(f[3]));
        InputConstants.Key key = InputConstants.getKey(f[4]);
        int delay = Integer.parseInt(f[5]);
        long deadline = System.currentTimeMillis() + Long.parseLong(f[6]);
        boolean sawOther = false;
        while (System.currentTimeMillis() < deadline) {
            final boolean armedToClick = sawOther && delay == 0;
            String s = onClient(() -> {
                Minecraft mc = Minecraft.getInstance();
                String id = blockId(mc.level.getBlockState(pos));
                if (armedToClick && id.equals("minecraft:piston_head")) {
                    KeyMapping.click(key);
                    return "clicked " + mc.player.tickCount;
                }
                return id;
            });
            if (s.startsWith("clicked")) {
                long ms = System.currentTimeMillis();
                waitTicks(tickCount(), 2);
                return "ok cue+tap at " + ms + " tick " + s.substring(8);
            }
            if (!s.equals("minecraft:piston_head")) {
                sawOther = true;
            } else if (sawOther) {
                long cue = System.currentTimeMillis();
                int t0 = tickCount();
                waitTicks(t0, delay);
                long ms = System.currentTimeMillis();
                onClient(() -> {
                    KeyMapping.click(key);
                    return null;
                });
                waitTicks(tickCount(), 2);
                return "ok cue at " + cue + " tap at " + ms;
            }
            Thread.sleep(2);
        }
        return "error no cue within the timeout";
    }
}
