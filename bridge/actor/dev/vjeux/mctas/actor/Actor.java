package dev.vjeux.mctas.actor;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardOpenOption;
import java.util.Comparator;
import java.util.Locale;
import java.util.function.Supplier;
import java.util.stream.Stream;
import net.minecraft.client.CameraType;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.ContainerInput;
import net.minecraft.world.inventory.Slot;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.BlockHitResult;
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
                float f = (float) k / (float) ticks;
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
                            f[1].equals("third") ? CameraType.THIRD_PERSON_BACK : CameraType.FIRST_PERSON);
                    return null;
                });
                return "ok cam=" + f[1];
            }
            case "fillinto":
                return fillInto(new BlockPos(Integer.parseInt(f[1]), Integer.parseInt(f[2]), Integer.parseInt(f[3])), f[4],
                        f.length > 5 ? Integer.parseInt(f[5]) : 99);
            case "digbox":
                return digBox2(
                        Integer.parseInt(f[1]), Integer.parseInt(f[2]), Integer.parseInt(f[3]),
                        Integer.parseInt(f[4]), Integer.parseInt(f[5]), Integer.parseInt(f[6]),
                        f.length > 7 ? f[7] : "any",
                        f.length > 8 ? Integer.parseInt(f[8]) : 100);
            case "dropall": {
                // dropall <item-substr> <keep-stacks>: throw away whole junk stacks (mined
                // deepslate/netherrack) so item entities never pile up and lag the server.
                waitPlayer();
                int keep = Integer.parseInt(f[2]);
                return onClient(() -> {
                    Minecraft mc = Minecraft.getInstance();
                    var inv = mc.player.getInventory();
                    int seen = 0;
                    int dropped = 0;
                    for (int i = 0; i < inv.getNonEquipmentItems().size(); i++) {
                        var stack = inv.getItem(i);
                        String id = String.valueOf(BuiltInRegistries.ITEM.getKey(stack.getItem()));
                        if (stack.getCount() > 0 && id.contains(f[1])) {
                            if (++seen <= keep) {
                                continue;
                            }
                            if (i >= 9) {
                                final int containerSlot = i;
                                mc.gameMode.handleContainerInput(mc.player.inventoryMenu.containerId,
                                        mc.player.inventoryMenu.slots.stream()
                                                .filter(s -> s.container == inv && s.getContainerSlot() == containerSlot)
                                                .findFirst().orElseThrow().index,
                                        8, ContainerInput.SWAP, mc.player);
                                inv.setSelectedSlot(8);
                            } else {
                                inv.setSelectedSlot(i);
                            }
                            dropped += stack.getCount();
                            mc.gameMode.dropItem(mc.player, true);
                        }
                    }
                    return "ok dropped=" + dropped;
                });
            }
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
                return walk(Double.parseDouble(f[1]) + 0.5, Double.parseDouble(f[2]) + 0.5);
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
     * A lean mine for `digBox`: aim + first hit in ONE client submit, then tick-gated
     * direct calls (the same survival timing as `mine`'s precise path — `mine`'s own
     * prelude costs ~5 extra frame-gated submits a block). Slow blocks (obsidian) defer
     * to the full `mine` (held key).
     */
    private static String mineFast(BlockPos pos) throws Exception {
        float perTick = onClient(() -> {
            Minecraft mc = Minecraft.getInstance();
            if (mc.level.getBlockState(pos).isAir()) {
                return 1.0F;
            }
            float[] aim = anglesTo(mc.player, pos.getX() + 0.5, pos.getY() + 0.5, pos.getZ() + 0.5);
            setLook(mc.player, aim[0], aim[1]);
            float prog = mc.level.getBlockState(pos).getDestroyProgress(mc.player, mc.player.level(), pos);
            if (prog >= 0.2F) {
                Direction dir = faceToward(mc.player, pos);
                if (mc.gameMode.startDestroyBlock(pos, dir)) {
                    mc.player.swing(InteractionHand.MAIN_HAND, mc.player.getMainHandItem().getAttackAnimation(), false);
                }
            }
            return prog;
        });
        if (perTick >= 1.0F) {
            Thread.sleep(40);
            String now = onClient(() -> blockId(Minecraft.getInstance().level.getBlockState(pos)));
            return now.contains("air") ? "ok mined" : "error instamine left " + now;
        }
        if (perTick < 0.2F) {
            return mine(pos, false);
        }
        Direction dir = onClient(() -> faceToward(Minecraft.getInstance().player, pos));
        int[] lastTick = {-1};
        long deadline = System.currentTimeMillis() + 15_000;
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
                    return false;
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
            Thread.sleep(25);
        }
        onClient(() -> {
            Minecraft.getInstance().gameMode.stopDestroyBlock();
            return null;
        });
        return "error mine timed out";
    }

    /** One box scan: target cells for `mode`, plus the eye position they were measured from. */
    private static java.util.List<BlockPos> scanTargets(int x0, int y0, int z0, int x1, int y1, int z1, String mode) throws Exception {
        return onClient(() -> {
            Minecraft mc = Minecraft.getInstance();
            java.util.List<BlockPos> out = new java.util.ArrayList<>();
            for (int y = y0; y <= y1; y++) {
                for (int z = z0; z <= z1; z++) {
                    for (int x = x0; x <= x1; x++) {
                        BlockPos p = new BlockPos(x, y, z);
                        BlockState s = mc.level.getBlockState(p);
                        if (s.isAir() || !s.getFluidState().isEmpty()) {
                            continue;
                        }
                        String id = blockId(s);
                        boolean want = switch (mode) {
                            case "obsidian" -> id.equals("minecraft:obsidian");
                            case "noobs" -> !id.equals("minecraft:bedrock") && !id.equals("minecraft:obsidian");
                            default -> !id.equals("minecraft:bedrock");
                        };
                        if (want && !s.getCollisionShape(mc.level, p).isEmpty()) {
                            out.add(p);
                        }
                    }
                }
            }
            return out;
        });
    }

    /**
     * Digs a box STATIONARY-first: mine everything in reach of the current stand (one
     * scan, then a pure mining loop), then take ONE step to the standable cell with the
     * most remaining targets in reach. At this client's frame rate every walk and every
     * client submit is precious: a stand is worth ~200 blocks here, where the previous
     * frontier-chasing version spent its whole budget walking between single blocks.
     */
    private static String digBox2(int x0, int y0, int z0, int x1, int y1, int z1, String mode, int secs) throws Exception {
        waitPlayer();
        long deadline = System.currentTimeMillis() + secs * 1000L;
        int mined = 0;
        int walks = 0;
        java.util.Set<BlockPos> triedStands = new java.util.HashSet<>();
        while (System.currentTimeMillis() < deadline) {
            // Stationary phase: mine everything the current eye can reach.
            Object[] scan = onClient(() -> {
                Minecraft mc = Minecraft.getInstance();
                java.util.List<BlockPos> reach = new java.util.ArrayList<>();
                int total = 0;
                var eye = mc.player.getEyePosition();
                BlockPos feet = mc.player.blockPosition();
                for (int y = y0; y <= y1; y++) {
                    for (int z = z0; z <= z1; z++) {
                        for (int x = x0; x <= x1; x++) {
                            BlockPos p = new BlockPos(x, y, z);
                            BlockState s = mc.level.getBlockState(p);
                            if (s.isAir() || !s.getFluidState().isEmpty()) {
                                continue;
                            }
                            String id = blockId(s);
                            boolean want = switch (mode) {
                                case "obsidian" -> id.equals("minecraft:obsidian");
                                case "noobs" -> !id.equals("minecraft:bedrock") && !id.equals("minecraft:obsidian");
                                default -> !id.equals("minecraft:bedrock");
                            };
                            if (!want || s.getCollisionShape(mc.level, p).isEmpty()) {
                                continue;
                            }
                            total++;
                            // Never the support block under our own feet (we would drop).
                            if (p.equals(feet.below())) {
                                continue;
                            }
                            if (p.distToCenterSqr(eye.x, eye.y, eye.z) <= 4.2 * 4.2) {
                                reach.add(p);
                            }
                        }
                    }
                }
                return new Object[] {reach, total};
            });
            @SuppressWarnings("unchecked")
            java.util.List<BlockPos> reach = (java.util.List<BlockPos>) scan[0];
            int total = (Integer) scan[1];
            if (total == 0) {
                log("digbox2 done: mined=" + mined + " walks=" + walks + " left=0");
                return "ok left=0";
            }
            boolean dug = false;
            for (BlockPos cell : reach) {
                if (System.currentTimeMillis() > deadline) {
                    break;
                }
                String r = mineFast(cell);
                if (r.startsWith("ok")) {
                    mined++;
                    dug = true;
                }
            }
            if (dug) {
                continue;
            }
            // Move phase: the standable cell (box ±1) with the most targets in reach,
            // preferring near ones; walk there once.
            java.util.List<BlockPos> rest = scanTargets(x0, y0, z0, x1, y1, z1, mode);
            BlockPos stand = onClient(() -> {
                Minecraft mc = Minecraft.getInstance();
                LocalPlayer p = mc.player;
                BlockPos best = null;
                int bestCover = -1;
                double bestDist = 1e9;
                for (int y = y0; y <= y1 + 1; y++) {
                    for (int z = z0 - 1; z <= z1 + 1; z++) {
                        for (int x = x0 - 1; x <= x1 + 1; x++) {
                            BlockPos s = new BlockPos(x, y, z);
                            if (triedStands.contains(s) || s.equals(p.blockPosition())) {
                                continue;
                            }
                            BlockState below = mc.level.getBlockState(s.below());
                            if (below.getCollisionShape(mc.level, s.below()).isEmpty() || !below.getFluidState().isEmpty()) {
                                continue;
                            }
                            if (!mc.level.getBlockState(s).getCollisionShape(mc.level, s).isEmpty()
                                    || !mc.level.getBlockState(s.above()).getCollisionShape(mc.level, s.above()).isEmpty()) {
                                continue;
                            }
                            int cover = 0;
                            for (BlockPos t : rest) {
                                if (t.distToCenterSqr(s.getX() + 0.5, s.getY() + 1.62, s.getZ() + 0.5) <= 4.2 * 4.2) {
                                    cover++;
                                }
                            }
                            double pd = Math.abs(s.getX() - p.getBlockX()) + Math.abs(s.getZ() - p.getBlockZ()) + 2.0 * Math.abs(s.getY() - p.getBlockY());
                            if (cover > bestCover || (cover == bestCover && pd < bestDist)) {
                                bestCover = cover;
                                bestDist = pd;
                                best = s;
                            }
                        }
                    }
                }
                return bestCover > 0 ? best : null;
            });
            if (stand == null) {
                log("digbox2 stuck: mined=" + mined + " walks=" + walks + " left=" + rest.size());
                return "ok left=" + rest.size() + " stuck";
            }
            triedStands.add(stand);
            walks++;
            walk(stand.getX() + 0.5, stand.getZ() + 0.5);
        }
        int left = scanTargets(x0, y0, z0, x1, y1, z1, mode).size();
        log("digbox2 done: mined=" + mined + " walks=" + walks + " left=" + left);
        return "ok left=" + left;
    }

    /**
     * Digs out a whole box from inside the client: repeatedly mine the nearest target the
     * crosshair can reach, stepping to live-verified stands near the player when it cannot.
     * One command instead of one file round trip per block (the per-block IPC plus stale
     * snapshot stands made big clears crawl at this client's frame rate). `mode` "any"
     * mines every solid block except bedrock; "obsidian" only obsidian. Runs at most
     * `secs`, answers how many target cells remain.
     */
    private static String digBox(int x0, int y0, int z0, int x1, int y1, int z1, String mode, int secs) throws Exception {
        waitPlayer();
        long deadline = System.currentTimeMillis() + secs * 1000L;
        int stuckRounds = 0;
        int mineOk = 0;
        int mineFail = 0;
        int walkFail = 0;
        String firstFails = "";
        while (System.currentTimeMillis() < deadline) {
            // Scan (client thread): targets sorted by distance to the player's eye.
            java.util.List<BlockPos> targets = onClient(() -> {
                Minecraft mc = Minecraft.getInstance();
                java.util.List<BlockPos> out = new java.util.ArrayList<>();
                for (int y = y0; y <= y1; y++) {
                    for (int z = z0; z <= z1; z++) {
                        for (int x = x0; x <= x1; x++) {
                            BlockPos p = new BlockPos(x, y, z);
                            BlockState s = mc.level.getBlockState(p);
                            if (s.isAir() || !s.getFluidState().isEmpty()) {
                                continue;
                            }
                            String id = blockId(s);
                            boolean want = switch (mode) {
                                case "obsidian" -> id.equals("minecraft:obsidian");
                                case "noobs" -> !id.equals("minecraft:bedrock") && !id.equals("minecraft:obsidian");
                                default -> !id.equals("minecraft:bedrock");
                            };
                            if (want && !s.getCollisionShape(mc.level, p).isEmpty()) {
                                out.add(p);
                            }
                        }
                    }
                }
                var eye = mc.player.getEyePosition();
                out.sort(Comparator.comparingDouble(p -> p.distToCenterSqr(eye.x, eye.y, eye.z)));
                return out;
            });
            if (targets.isEmpty()) {
                return "ok left=0";
            }
            boolean progress = false;
            int walkTries = 0;
            for (BlockPos cell : targets) {
                if (System.currentTimeMillis() > deadline) {
                    break;
                }
                double d = onClient(() -> {
                    var eye = Minecraft.getInstance().player.getEyePosition();
                    return Math.sqrt(cell.distToCenterSqr(eye.x, eye.y, eye.z));
                });
                if (d <= 4.2) {
                    String r = mineFast(cell);
                    if (r.startsWith("ok")) {
                        mineOk++;
                        progress = true;
                        break;
                    }
                    mineFail++;
                    if (firstFails.length() < 200) {
                        firstFails += " [" + cell + " d=" + String.format(Locale.ROOT, "%.1f", d) + " " + r + "]";
                    }
                    continue;
                }
                // Step to a live stand within reach of the cell, close to where we are.
                BlockPos stand = onClient(() -> {
                    Minecraft mc = Minecraft.getInstance();
                    LocalPlayer p = mc.player;
                    BlockPos best = null;
                    double bestD = 1e9;
                    for (int dx = -3; dx <= 3; dx++) {
                        for (int dz = -3; dz <= 3; dz++) {
                            for (int dy = -4; dy <= 1; dy++) {
                                BlockPos s = cell.offset(dx, dy, dz);
                                if (s.equals(cell) || s.equals(cell.above())) {
                                    continue;
                                }
                                BlockState below = mc.level.getBlockState(s.below());
                                if (below.getCollisionShape(mc.level, s.below()).isEmpty() || !below.getFluidState().isEmpty()) {
                                    continue;
                                }
                                if (!mc.level.getBlockState(s).getCollisionShape(mc.level, s).isEmpty()
                                        || !mc.level.getBlockState(s.above()).getCollisionShape(mc.level, s.above()).isEmpty()) {
                                    continue;
                                }
                                double eyeD = Math.sqrt(cell.distToCenterSqr(s.getX() + 0.5, s.getY() + 1.62, s.getZ() + 0.5));
                                if (eyeD > 4.2) {
                                    continue;
                                }
                                double pd = Math.abs(s.getX() - p.getBlockX()) + Math.abs(s.getZ() - p.getBlockZ())
                                        + 3.0 * Math.abs(s.getY() - p.getBlockY());
                                if (pd < bestD) {
                                    bestD = pd;
                                    best = s;
                                }
                            }
                        }
                    }
                    return best != null && bestD <= 7 ? best : null;
                });
                if (stand == null || ++walkTries > 4) {
                    continue;
                }
                String wr = walk(stand.getX() + 0.5, stand.getZ() + 0.5);
                if (wr.startsWith("ok")) {
                    String r = mineFast(cell);
                    if (r.startsWith("ok")) {
                        mineOk++;
                        progress = true;
                        break;
                    }
                    mineFail++;
                    if (firstFails.length() < 200) {
                        firstFails += " [walked " + cell + " " + r + "]";
                    }
                } else {
                    walkFail++;
                }
            }
            if (!progress && ++stuckRounds >= 3) {
                log("digbox stuck: mined=" + mineOk + " mineFail=" + mineFail + " walkFail=" + walkFail + " firstFails:" + firstFails + " | " + onClient(Actor::posLine));
                return "ok left=" + targets.size() + " stuck";
            }
            if (progress) {
                stuckRounds = 0;
            }
        }
        int left = onClient(() -> {
            Minecraft mc = Minecraft.getInstance();
            int n = 0;
            for (int y = y0; y <= y1; y++) {
                for (int z = z0; z <= z1; z++) {
                    for (int x = x0; x <= x1; x++) {
                        BlockState s = mc.level.getBlockState(new BlockPos(x, y, z));
                        String id = blockId(s);
                        boolean want = switch (mode) {
                            case "obsidian" -> id.equals("minecraft:obsidian");
                            case "noobs" -> !id.equals("minecraft:bedrock") && !id.equals("minecraft:obsidian");
                            default -> !id.equals("minecraft:bedrock");
                        };
                        if (!s.isAir() && s.getFluidState().isEmpty() && want) {
                            n++;
                        }
                    }
                }
            }
            return n;
        });
        log("digbox done: mined=" + mineOk + " mineFail=" + mineFail + " walkFail=" + walkFail + " left=" + left + " firstFails:" + firstFails);
        return "ok left=" + left;
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
    private static String walk(double tx, double tz) throws Exception {
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
                    mc.options.keyJump.setDown(stuck[0] > 8);
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
}
