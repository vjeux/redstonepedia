package dev.vjeux.mctas.bridge;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import java.io.BufferedOutputStream;
import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Base64;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import net.minecraft.SharedConstants;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.TagParser;
import net.minecraft.server.commands.data.BlockDataAccessor;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.piston.PistonMovingBlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.Vec3;

/**
 * Line-delimited JSON protocol. Requests are read on a background thread and executed in order on
 * the server thread; each request gets exactly one response line carrying the same {@code id}.
 *
 * <p>See docs/internals.md (bridge protocol) for the full reference.
 */
public final class Protocol {
    private static final JsonObject EOF = new JsonObject();
    private final InputStream in;
    private final OutputStream out;
    private final LinkedBlockingQueue<JsonObject> queue = new LinkedBlockingQueue<>();
    private final Gson gson = new GsonBuilder().disableHtmlEscaping().create();
    private final Map<Integer, Sandbox> sandboxes = new HashMap<>();
    private final Map<String, BlockState> parsedStates = new HashMap<>();
    private final Map<BlockState, String> serializedStates = new IdentityHashMap<>();
    private SimServer server;
    private HolderLookup<Block> blockLookup;
    private int nextSandboxId = 1;

    Protocol(InputStream in, OutputStream out) {
        this.in = in;
        this.out = new BufferedOutputStream(out, 1 << 16);
    }

    /** Called on the server thread once the levels exist. */
    void start(SimServer server) {
        this.server = server;
        this.blockLookup = server.registryAccess().lookupOrThrow(Registries.BLOCK);
        Thread reader = new Thread(this::readLoop, "mctas-protocol-reader");
        reader.setDaemon(true);
        reader.start();
        JsonObject ready = new JsonObject();
        ready.addProperty("event", "ready");
        ready.addProperty("version", SharedConstants.getCurrentVersion().name());
        write(ready);
        flush();
    }

    private void readLoop() {
        try (BufferedReader reader = new BufferedReader(new InputStreamReader(this.in, StandardCharsets.UTF_8), 1 << 16)) {
            String line;
            while ((line = reader.readLine()) != null) {
                if (line.isBlank()) {
                    continue;
                }
                JsonObject request;
                try {
                    request = JsonParser.parseString(line).getAsJsonObject();
                } catch (RuntimeException e) {
                    request = new JsonObject();
                    request.addProperty("op", "__invalid");
                    request.addProperty("error", "invalid JSON: " + e.getMessage());
                }
                this.queue.put(request);
            }
        } catch (IOException | InterruptedException e) {
            // fall through: treat as EOF
        }
        this.queue.add(EOF);
    }

    /** Runs on the server thread: wait briefly for work, then drain the queue. */
    void serve() {
        JsonObject request;
        try {
            this.server.viewerTick();
            request = this.queue.poll(this.server.listening() ? 50 : 200, TimeUnit.MILLISECONDS);
        } catch (InterruptedException e) {
            return;
        }
        while (request != null) {
            if (request == EOF) {
                flush();
                Runtime.getRuntime().halt(0);
            }
            write(handle(request));
            request = this.queue.poll();
        }
        flush();
    }

    private JsonObject handle(JsonObject request) {
        JsonObject response = new JsonObject();
        if (request.has("id")) {
            response.add("id", request.get("id"));
        }
        try {
            String op = request.has("op") ? request.get("op").getAsString() : "";
            JsonObject result = dispatch(op, request);
            response.addProperty("ok", true);
            for (Map.Entry<String, JsonElement> e : result.entrySet()) {
                response.add(e.getKey(), e.getValue());
            }
        } catch (Throwable t) {
            response.addProperty("ok", false);
            String message = t.getMessage() == null ? t.toString() : t.getClass().getSimpleName() + ": " + t.getMessage();
            response.addProperty("error", message);
        }
        return response;
    }

    private JsonObject dispatch(String op, JsonObject r) throws Exception {
        return switch (op) {
            case "ping" -> new JsonObject();
            case "blockinfo" -> BlockInfo.run(this, sandbox(r), r);
            case "spawnrules" -> BlockInfo.spawnRules(this, sandbox(r), r);
            case "blockstates" -> BlockInfo.states(this, r);
            case "path" -> MobAiProbe.path(sandbox(r), r);
            case "path_nodes" -> MobAiProbe.pathNodes(sandbox(r), r);
            case "math" -> MobAiProbe.math(r);
            case "mobstate" -> MobAiProbe.mobState(sandbox(r));
            case "info" -> info();
            case "create" -> create(r);
            case "nether_create" -> netherCreate(r);
            case "drop" -> drop(r);
            case "clear" -> clear(r);
            case "reload" -> reload(r);
            case "unload" -> unload(r);
            case "load" -> load(r);
            case "set" -> set(r);
            case "fill" -> fill(r);
            case "get" -> get(r);
            case "unsupported" -> unsupported(r);
            case "rotate" -> rotate(r);
            case "use" -> use(r);
            case "use_entity" -> useEntity(r);
            case "step" -> step(r);
            case "status" -> status(sandbox(r));
            case "events" -> events(sandbox(r));
            case "entities" -> entities(r);
            case "summon" -> summon(r);
            case "world" -> world(r);
            case "chunk_lists" -> chunkLists(r);
            case "heights" -> heights(r);
            case "identity_hashes" -> identityHashes();
            case "memory" -> memory(r);
            case "teleport" -> teleport(r);
            case "poi_take" -> poiTake(r);
            case "poi_free" -> poiFree(r);
            case "home_release" -> homeRelease(r);
            case "sensor_delay" -> sensorDelay(r);
            case "golem_spots" -> golemSpots(r);
            case "trace" -> trace(r);
            case "terrain" -> Terrain.load(this, sandbox(r), r);
            case "light_box" -> Terrain.lightBox(sandbox(r), ints(r.getAsJsonArray("from")), ints(r.getAsJsonArray("to")));
            case "normalize_states" -> {
                // each state as the game writes it back (parse, then serialize: default properties
                // spelled out), for comparing state strings from other sources
                JsonArray out = new JsonArray();
                for (JsonElement s : r.getAsJsonArray("states")) {
                    out.add(serialize(parse(s.getAsString())));
                }
                JsonObject o = new JsonObject();
                o.add("states", out);
                yield o;
            }
            case "leash" -> leash(r);
            case "update" -> update(r);
            case "build" -> build(r);
            case "signals" -> signals(r);
            case "exec" -> exec(r);
            case "bedata" -> beData(r);
            case "gameevent" -> gameEvent(r);
            case "random_state" -> randomState(r);
            case "ticking_chunks" -> tickingChunks(r);
            case "spawn_state" -> spawnState(r);
            case "ticking_sections" -> tickingSections(r);
            case "viewers" -> viewers();
            case "camera" -> camera(r);
            case "viewer_mode" -> viewerMode(r);
            case "view" -> view(r);
            case "play" -> play(r);
            case "hold" -> hold(r);
            case "__invalid" -> throw new IllegalArgumentException(r.get("error").getAsString());
            default -> throw new IllegalArgumentException("unknown op '" + op + "'");
        };
    }

    // ---- ops -------------------------------------------------------------------------------

    private JsonObject info() {
        JsonObject o = new JsonObject();
        o.addProperty("version", SharedConstants.getCurrentVersion().name());
        o.addProperty("sandboxes", this.sandboxes.size());
        Runtime rt = Runtime.getRuntime();
        o.addProperty("heap_used", rt.totalMemory() - rt.freeMemory());
        o.addProperty("heap_max", rt.maxMemory());
        return o;
    }

    private JsonObject create(JsonObject r) {
        int minCX = -1, minCZ = -1, maxCX = 0, maxCZ = 0;
        if (r.has("chunks")) {
            JsonArray c = r.getAsJsonArray("chunks");
            minCX = c.get(0).getAsInt();
            minCZ = c.get(1).getAsInt();
            maxCX = c.get(2).getAsInt();
            maxCZ = c.get(3).getAsInt();
        }
        long seed = r.has("seed") ? r.get("seed").getAsLong() : 0L;
        boolean customSpawners = r.has("custom_spawners") && r.get("custom_spawners").getAsBoolean();
        String biome = r.has("biome") && !r.get("biome").isJsonNull() ? r.get("biome").getAsString() : null;
        int id = this.nextSandboxId++;
        Integer seaLevel = r.has("sea_level") && !r.get("sea_level").isJsonNull() ? r.get("sea_level").getAsInt() : null;
        Sandbox sb = Sandbox.create(this.server, id, minCX, minCZ, maxCX, maxCZ, seed, customSpawners, biome, seaLevel);
        this.sandboxes.put(id, sb);
        JsonObject o = new JsonObject();
        o.addProperty("sandbox", id);
        o.add("min", pos(sb.minX(), sb.level.getMinY(), sb.minZ()));
        o.add("max", pos(sb.maxX(), sb.level.getMaxY(), sb.maxZ()));
        // (tickexact 2026-11-10, farmopt's station) {"nether": {"chunks": [x0, z0, x1, z1], "seed":
        // N}}: its Nether sandbox (Sandbox.createNether), addressed by its own id for blocks and
        // block entities, ticked by this one
        if (r.has("nether") && r.get("nether").isJsonObject()) {
            JsonObject n = r.getAsJsonObject("nether");
            n.addProperty("sandbox", id);
            o.add("nether", netherCreate(n).get("nether"));
        }
        return o;
    }

    /**
     * (tickexact 2026-11-10, farmopt's station) {@code nether_create}: the Nether of sandbox
     * {@code sandbox} ({@link Sandbox#createNether}): {@code chunks} [x0, z0, x1, z1] kept loaded,
     * {@code seed} its level random's; answers its own sandbox id ({@code nether}) for blocks and
     * block entities (it ticks with its overworld sandbox).
     */
    private JsonObject netherCreate(JsonObject r) {
        Sandbox sb = sandbox(r);
        JsonArray c = r.getAsJsonArray("chunks");
        int nid = this.nextSandboxId++;
        long nseed = r.has("seed") ? r.get("seed").getAsLong() : sb.seed;
        Sandbox nsb = Sandbox.createNether(this.server, sb, nid, c.get(0).getAsInt(), c.get(1).getAsInt(), c.get(2).getAsInt(), c.get(3).getAsInt(), nseed);
        this.sandboxes.put(nid, nsb);
        JsonObject o = new JsonObject();
        o.addProperty("nether", nid);
        return o;
    }

    private JsonObject drop(JsonObject r) {
        Sandbox sb = sandbox(r);
        sb.drop(this.server);
        this.sandboxes.remove(sb.id);
        return new JsonObject();
    }

    private JsonObject clear(JsonObject r) {
        sandbox(r).clear();
        return new JsonObject();
    }

    /** Save, unload and load again every chunk of a sandbox ({@link Sandbox#reload}). */
    private JsonObject reload(JsonObject r) {
        sandbox(r).reload(this.server);
        return new JsonObject();
    }

    /**
     * (tickexact 2026-11-10, ironspec's fault rule) {@code unload}: every chunk of a sandbox saved
     * and unloaded ({@link Sandbox#unloadAll}); the steps after it tick the level with them away
     * until {@code load} ({@link Sandbox#loadAll}) brings them back.
     */
    private JsonObject unload(JsonObject r) {
        Sandbox sb = sandbox(r);
        if (r.has("chunks")) {
            int[] b = ints(r.getAsJsonArray("chunks"));
            sb.unloadSome(this.server, b[0], b[1], b[2], b[3]);
        } else {
            sb.unloadAll(this.server);
        }
        return new JsonObject();
    }

    private JsonObject load(JsonObject r) {
        Sandbox sb = sandbox(r);
        if (r.has("chunks")) {
            int[] b = ints(r.getAsJsonArray("chunks"));
            sb.loadSome(this.server, b[0], b[1], b[2], b[3]);
            JsonObject o = new JsonObject();
            o.add("tick_list", sb.tickList());
            return o;
        }
        sb.loadAll(this.server, r.has("order") ? r.get("order").getAsString() : "save");
        JsonObject o = new JsonObject();
        o.addProperty("order", sb.lastLoadOrder);
        o.addProperty("sequence", sb.lastLoadSequence);
        // each entity back under its new number: [old, new]
        JsonArray re = new JsonArray();
        for (int[] p : sb.lastRenumbered) {
            JsonArray a = new JsonArray();
            a.add(p[0]);
            a.add(p[1]);
            re.add(a);
        }
        o.add("renumbered", re);
        return o;
    }

    private static int flags(JsonObject r) {
        if (!r.has("mode")) {
            return Sandbox.RAW_FLAGS;
        }
        JsonElement mode = r.get("mode");
        if (mode.isJsonPrimitive() && mode.getAsJsonPrimitive().isNumber()) {
            return mode.getAsInt();
        }
        return switch (mode.getAsString()) {
            case "raw" -> Sandbox.RAW_FLAGS;
            case "update" -> Block.UPDATE_ALL;
            // shape updates only, no block update to the neighbors (how crops grow): the BUD judge
            case "shape" -> Block.UPDATE_CLIENTS;
            default -> throw new IllegalArgumentException("unknown mode " + mode);
        };
    }

    private JsonObject set(JsonObject r) throws CommandSyntaxException {
        Sandbox sb = sandbox(r);
        int flags = flags(r);
        int changed = 0;
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        for (JsonElement e : r.getAsJsonArray("blocks")) {
            JsonArray b = e.getAsJsonArray();
            pos.set(b.get(0).getAsInt(), b.get(1).getAsInt(), b.get(2).getAsInt());
            checkInside(sb, pos);
            // immutable(): a block event queued by an onPlace (a piston placed with updates)
            // keeps the position object; a shared MutableBlockPos would move it to the next block.
            if (sb.level.setBlock(pos.immutable(), parse(b.get(3).getAsString()), flags)) {
                changed++;
            }
        }
        JsonObject o = new JsonObject();
        o.addProperty("changed", changed);
        return o;
    }

    private JsonObject fill(JsonObject r) throws CommandSyntaxException {
        Sandbox sb = sandbox(r);
        int flags = flags(r);
        int[] a = ints(r.getAsJsonArray("from"));
        int[] b = ints(r.getAsJsonArray("to"));
        BlockState state = parse(r.get("state").getAsString());
        int changed = 0;
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        for (int y = Math.min(a[1], b[1]); y <= Math.max(a[1], b[1]); y++) {
            for (int z = Math.min(a[2], b[2]); z <= Math.max(a[2], b[2]); z++) {
                for (int x = Math.min(a[0], b[0]); x <= Math.max(a[0], b[0]); x++) {
                    pos.set(x, y, z);
                    checkInside(sb, pos);
                    if (sb.level.setBlock(pos.immutable(), state, flags)) {
                        changed++;
                    }
                }
            }
        }
        JsonObject o = new JsonObject();
        o.addProperty("changed", changed);
        return o;
    }

    /** Region read: palette + one u16 (little endian, base64) per block, x fastest, then z, then y. */
    /**
     * Blocks in the region whose state cannot survive where it stands (a floating lever, torch,
     * dust or repeater, a button on air...): raw placement keeps them, but a survival player
     * could not build them, so the judge rejects designs that contain any.
     */
    private JsonObject unsupported(JsonObject r) {
        Sandbox sb = sandbox(r);
        int[] a = ints(r.getAsJsonArray("from"));
        int[] b = ints(r.getAsJsonArray("to"));
        JsonArray out = new JsonArray();
        JsonArray random = new JsonArray();
        BlockPos.MutableBlockPos p = new BlockPos.MutableBlockPos();
        for (int x = Math.min(a[0], b[0]); x <= Math.max(a[0], b[0]); x++) {
            for (int y = Math.min(a[1], b[1]); y <= Math.max(a[1], b[1]); y++) {
                for (int z = Math.min(a[2], b[2]); z <= Math.max(a[2], b[2]); z++) {
                    p.set(x, y, z);
                    BlockState s = sb.level.getBlockState(p);
                    if (!s.isAir() && !s.canSurvive(sb.level, p)) {
                        out.add(pos(x, y, z));
                    }
                    // Blocks that change by themselves (random ticks): unwaxed copper oxidizes,
                    // leaves decay, crops grow... A door must not depend on luck over time.
                    if (!s.isAir() && s.isRandomlyTicking()) {
                        random.add(pos(x, y, z));
                    }
                }
            }
        }
        JsonObject o = new JsonObject();
        o.add("positions", out);
        o.add("random", random);
        return o;
    }

    /** The sandbox's level random state, read-only ({@link Sandbox#randomState}), and its randValue. */
    private JsonObject randomState(JsonObject r) {
        Sandbox sb = sandbox(r);
        JsonObject o = new JsonObject();
        o.addProperty("seed", sb.randomState());
        o.addProperty("rand_value", sb.randValue());
        return o;
    }

    /**
     * The randomly ticking sections of the chunks the level ticks ({@code
     * LevelChunkSection.isRandomlyTicking}: a randomly ticking block or fluid in it), [x, sectionY, z]
     * each, in the ticking chunks' order: redsim's {@code crops::TickingSections} is checked against
     * it (the compare's {@code MCTAS_EXACT_SECTIONS}).
     */
    private JsonObject tickingSections(JsonObject r) {
        Sandbox sb = sandbox(r);
        JsonArray out = new JsonArray();
        for (int[] c : sb.tickingChunks()) {
            net.minecraft.world.level.chunk.LevelChunk chunk = sb.level.getChunk(c[0], c[1]);
            net.minecraft.world.level.chunk.LevelChunkSection[] sections = chunk.getSections();
            for (int i = 0; i < sections.length; i++) {
                if (sections[i].isRandomlyTicking()) {
                    JsonArray a = new JsonArray();
                    a.add(c[0]);
                    a.add(chunk.getSectionYFromSectionIndex(i));
                    a.add(c[1]);
                    out.add(a);
                }
            }
        }
        JsonObject o = new JsonObject();
        o.add("sections", out);
        return o;
    }

    /**
     * The last natural spawn tick's mob counts per category ({@code SpawnState.getMobCategoryCounts}:
     * the global caps' numbers; empty before the first spawn tick): redsim's are checked against it
     * (the compare's {@code MCTAS_EXACT_SPAWNSTATE}).
     */
    private JsonObject spawnState(JsonObject r) {
        Sandbox sb = sandbox(r);
        JsonObject counts = new JsonObject();
        net.minecraft.world.level.NaturalSpawner.SpawnState st = sb.level.getChunkSource().getLastSpawnState();
        if (st != null) {
            for (it.unimi.dsi.fastutil.objects.Object2IntMap.Entry<net.minecraft.world.entity.MobCategory> e : st.getMobCategoryCounts().object2IntEntrySet()) {
                counts.addProperty(e.getKey().getName(), e.getIntValue());
            }
        }
        JsonObject o = new JsonObject();
        o.add("counts", counts);
        // and the live mobs by category and type, as createState filters them (a Mob not
        // persistent and needing no custom persistence, not MISC), with the persistent ones apart
        JsonObject byType = new JsonObject();
        for (net.minecraft.world.entity.Entity e : sb.level.getAllEntities()) {
            if (!(e instanceof net.minecraft.world.entity.Mob mob)) {
                continue;
            }
            net.minecraft.world.entity.MobCategory c = e.getType().getCategory();
            String k = c.getName() + " " + net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE.getKey(e.getType())
                    + (mob.isPersistenceRequired() || mob.requiresCustomPersistence() ? " (persistent)" : "")
                    + (e.isRemoved() ? " (removed)" : "");
            byType.addProperty(k, (byType.has(k) ? byType.get(k).getAsInt() : 0) + 1);
        }
        o.add("by_type", byType);
        return o;
    }

    /** The chunks the level ticks, in its order ({@link Sandbox#tickingChunks}). */
    private JsonObject tickingChunks(JsonObject r) {
        Sandbox sb = sandbox(r);
        JsonArray out = new JsonArray();
        for (int[] c : sb.tickingChunks()) {
            JsonArray a = new JsonArray();
            a.add(c[0]);
            a.add(c[1]);
            out.add(a);
        }
        JsonObject o = new JsonObject();
        o.add("chunks", out);
        return o;
    }

    private JsonObject get(JsonObject r) {
        Sandbox sb = sandbox(r);
        int[] a = ints(r.getAsJsonArray("from"));
        int[] b = ints(r.getAsJsonArray("to"));
        Region region = new Region(a, b);
        Palette palette = new Palette();
        JsonObject o = new JsonObject();
        o.add("from", pos(region.x0, region.y0, region.z0));
        o.add("size", pos(region.dx, region.dy, region.dz));
        o.addProperty("data", region.capture(sb, palette));
        o.add("palette", palette.toJson());
        o.add("flags", palette.flagsJson(sb));
        o.addProperty("time", sb.gameTime());
        if (r.has("moving") && r.get("moving").getAsBoolean()) {
            o.add("moving", moving(sb, region));
        }
        if (r.has("block_entities") && r.get("block_entities").getAsBoolean()) {
            o.addProperty("block_entities", region.captureBlockEntities(sb));
        }
        return o;
    }

    /**
     * {@code bedata}: merge SNBT into the block entity at {@code pos} exactly like
     * {@code data merge block} ({@code DataCommands.mergeData}: the old data merged with the tag,
     * written with {@code BlockDataAccessor.setData} = loadWithComponents + setChanged, only when it
     * changes). Fails when there is no block entity there. Returns {@code changed} and the block
     * entity's data afterwards ({@code saveWithoutMetadata}).
     */
    private JsonObject beData(JsonObject r) throws CommandSyntaxException {
        Sandbox sb = sandbox(r);
        int[] p = ints(r.getAsJsonArray("pos"));
        BlockPos pos = new BlockPos(p[0], p[1], p[2]);
        checkInside(sb, pos);
        BlockEntity be = sb.level.getBlockEntity(pos);
        if (be == null) {
            throw new IllegalArgumentException("no block entity at " + pos.toShortString() + " (" + serialize(sb.level.getBlockState(pos)) + ")");
        }
        CompoundTag tag = TagParser.parseCompoundFully(r.get("nbt").getAsString());
        BlockDataAccessor accessor = new BlockDataAccessor(be, pos);
        CompoundTag old = accessor.getData();
        CompoundTag merged = old.copy().merge(tag);
        boolean changed = !old.equals(merged);
        if (changed) {
            accessor.setData(merged);
        }
        JsonObject o = new JsonObject();
        o.addProperty("changed", changed);
        o.addProperty("nbt", sb.level.getBlockEntity(pos).saveWithoutMetadata(sb.level.registryAccess()).toString());
        return o;
    }

    private JsonObject use(JsonObject r) {
        Sandbox sb = sandbox(r);
        int[] p = ints(r.getAsJsonArray("pos"));
        Direction face = r.has("face") ? Direction.byName(r.get("face").getAsString()) : Direction.UP;
        if (face == null) {
            throw new IllegalArgumentException("bad face " + r.get("face"));
        }
        JsonObject o = new JsonObject();
        o.addProperty("result", sb.use(new BlockPos(p[0], p[1], p[2]), face));
        return o;
    }

    /** Right-click the item frame in a block cell (an empty hand turns its item one step). */
    private JsonObject useEntity(JsonObject r) {
        Sandbox sb = sandbox(r);
        int[] p = ints(r.getAsJsonArray("pos"));
        JsonObject o = new JsonObject();
        o.addProperty("result", sb.useEntity(new BlockPos(p[0], p[1], p[2])));
        return o;
    }

    /**
     * Advance sandboxes. {@code ticks} is the count (or the cap with {@code until_idle}); with
     * {@code record} the given region is captured after every tick.
     */
    private JsonObject step(JsonObject r) {
        List<Sandbox> targets = new ArrayList<>();
        if (r.has("sandboxes")) {
            for (JsonElement e : r.getAsJsonArray("sandboxes")) {
                targets.add(sandboxById(e.getAsInt()));
            }
        } else {
            targets.add(sandbox(r));
        }
        int ticks = r.has("ticks") ? r.get("ticks").getAsInt() : 1;
        boolean untilIdle = r.has("until_idle") && r.get("until_idle").getAsBoolean();
        Region region = null;
        // record.block_entities: also capture every block entity of the region after every tick
        // (Region#captureBlockEntities), deduplicated: be_palette (distinct dumps) + be_frames.
        boolean recordBlockEntities = false;
        if (r.has("record")) {
            JsonObject rec = r.getAsJsonObject("record");
            region = new Region(ints(rec.getAsJsonArray("from")), ints(rec.getAsJsonArray("to")));
            recordBlockEntities = rec.has("block_entities") && rec.get("block_entities").getAsBoolean();
        }
        // Optional signal probes: after every tick, for each position, the best signal its
        // neighbors give into it (what a piston or lamp there would see) and the strong (direct)
        // signal into it, as [best, direct] pairs flattened per tick.
        List<BlockPos> signals = null;
        if (r.has("signals")) {
            signals = new ArrayList<>();
            for (JsonElement e : r.getAsJsonArray("signals")) {
                int[] p = ints(e.getAsJsonArray());
                signals.add(new BlockPos(p[0], p[1], p[2]));
            }
        }
        JsonArray results = new JsonArray();
        // Optional entity watch (designs with entities): after every tick, does any entity with
        // collision (Sandbox#collides) overlap the walk box (the opening and its walkway), the
        // first tick an entity's box leaves the watch box, and the first tick an entity the
        // design did not declare exists (a drop, a broken cart).
        net.minecraft.world.phys.AABB watchBox = null;
        net.minecraft.world.phys.AABB walkBox = null;
        if (r.has("entity_watch")) {
            JsonObject ew = r.getAsJsonObject("entity_watch");
            watchBox = blockBox(ew.getAsJsonArray("from"), ew.getAsJsonArray("to"));
            walkBox = blockBox(ew.getAsJsonArray("walk_from"), ew.getAsJsonArray("walk_to"));
        }
        for (Sandbox sb : targets) {
            Palette palette = region == null ? null : new Palette();
            JsonArray frames = region == null ? null : new JsonArray();
            Map<String, Integer> beIds = recordBlockEntities ? new HashMap<>() : null;
            JsonArray bePalette = recordBlockEntities ? new JsonArray() : null;
            JsonArray beFrames = recordBlockEntities ? new JsonArray() : null;
            JsonArray signalFrames = signals == null ? null : new JsonArray();
            JsonArray blocked = watchBox == null ? null : new JsonArray();
            JsonObject escape = null;
            JsonObject foreign = null;
            int ticked = 0;
            boolean idle = false;
            while (ticked < ticks) {
                sb.tick(this.server);
                ticked++;
                if (frames != null) {
                    frames.add(region.capture(sb, palette));
                }
                if (beFrames != null) {
                    String dump = region.captureBlockEntities(sb);
                    Integer beId = beIds.get(dump);
                    if (beId == null) {
                        beId = beIds.size();
                        beIds.put(dump, beId);
                        bePalette.add(dump);
                    }
                    beFrames.add(beId);
                }
                if (signalFrames != null) {
                    JsonArray row = new JsonArray();
                    for (BlockPos p : signals) {
                        row.add(sb.level.getBestNeighborSignal(p));
                        row.add(sb.level.getDirectSignalTo(p));
                    }
                    signalFrames.add(row);
                }
                if (blocked != null) {
                    boolean any = false;
                    for (Entity e : sb.entities()) {
                        net.minecraft.world.phys.AABB bb = e.getBoundingBox();
                        if (Sandbox.collides(e) && bb.intersects(walkBox)) {
                            any = true;
                        }
                        boolean inside = bb.minX >= watchBox.minX && bb.minY >= watchBox.minY && bb.minZ >= watchBox.minZ
                                && bb.maxX <= watchBox.maxX && bb.maxY <= watchBox.maxY && bb.maxZ <= watchBox.maxZ;
                        if (!inside && escape == null) {
                            escape = entityEvent(ticked, e);
                        }
                        if (!e.entityTags().contains(Sandbox.DESIGN_TAG) && foreign == null) {
                            foreign = entityEvent(ticked, e);
                        }
                    }
                    blocked.add(any);
                }
                if (untilIdle && sb.idle()) {
                    idle = true;
                    break;
                }
            }
            JsonObject res = new JsonObject();
            res.addProperty("sandbox", sb.id);
            res.addProperty("ticked", ticked);
            res.addProperty("time", sb.gameTime());
            if (untilIdle) {
                res.addProperty("idle", idle);
            }
            if (frames != null) {
                res.add("from", pos(region.x0, region.y0, region.z0));
                res.add("size", pos(region.dx, region.dy, region.dz));
                res.add("palette", palette.toJson());
                res.add("flags", palette.flagsJson(sb));
                res.add("frames", frames);
            }
            if (beFrames != null) {
                res.add("be_palette", bePalette);
                res.add("be_frames", beFrames);
            }
            if (signalFrames != null) {
                res.add("signals", signalFrames);
            }
            if (blocked != null) {
                res.add("entity_blocked", blocked);
                if (escape != null) {
                    res.add("entity_escape", escape);
                }
                if (foreign != null) {
                    res.add("entity_foreign", foreign);
                }
            }
            results.add(res);
        }
        JsonObject o = new JsonObject();
        o.add("results", results);
        return o;
    }

    /** Pending block events in queue order: pos, block id, type (paramA) and data (paramB). */
    private JsonObject events(Sandbox sb) {
        JsonArray list = new JsonArray();
        for (net.minecraft.world.level.BlockEventData e : sb.blockEvents()) {
            JsonObject o = new JsonObject();
            JsonArray p = new JsonArray();
            p.add(e.pos().getX());
            p.add(e.pos().getY());
            p.add(e.pos().getZ());
            o.add("pos", p);
            o.addProperty("block", net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(e.block()).toString());
            o.addProperty("a", e.paramA());
            o.addProperty("b", e.paramB());
            list.add(o);
        }
        JsonObject o = new JsonObject();
        o.add("events", list);
        return o;
    }

    private JsonObject status(Sandbox sb) {
        JsonObject o = new JsonObject();
        o.addProperty("time", sb.gameTime());
        o.addProperty("block_ticks", sb.level.getBlockTicks().count());
        o.addProperty("fluid_ticks", sb.level.getFluidTicks().count());
        o.addProperty("block_events", sb.blockEventCount());
        o.addProperty("moving_pistons", sb.movingPistonCount());
        o.addProperty("entities", sb.entities().size());
        o.addProperty("idle", sb.idle());
        return o;
    }

    private JsonObject entities(JsonObject r) {
        Sandbox sb = sandbox(r);
        JsonArray list = new JsonArray();
        for (Entity e : sb.entities()) {
            list.add(entityJson(sb, e));
        }
        JsonObject o = new JsonObject();
        o.add("entities", list);
        return o;
    }

    /**
     * One entity: type, exact position, uuid, motion, yaw/pitch, whether the design declared it,
     * whether it is at rest, whether it collides for the walkway rule, its bounding box, and its key
     * state (Sandbox#keyState: saved data without position/motion, plus a minecart's rail).
     */
    private static JsonObject entityJson(Sandbox sb, Entity e) {
        JsonObject j = new JsonObject();
        j.addProperty("type", BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString());
        Vec3 p = e.position();
        JsonArray a = new JsonArray();
        a.add(p.x);
        a.add(p.y);
        a.add(p.z);
        j.add("pos", a);
        j.addProperty("uuid", e.getStringUUID());
        // the network id within the sandbox: Entity.getId() - Sandbox#entityIdBase (the bridge
        // numbers each sandbox's entities from 0 in the order they enter; some ticks read it:
        // ItemEntity's 1-in-4 rest moves)
        j.addProperty("net_id", e.getId() - sb.entityIdBase());
        Vec3 m = e.getDeltaMovement();
        JsonArray mv = new JsonArray();
        mv.add(m.x);
        mv.add(m.y);
        mv.add(m.z);
        j.add("motion", mv);
        j.addProperty("yaw", e.getYRot());
        j.addProperty("pitch", e.getXRot());
        j.addProperty("design", e.entityTags().contains(Sandbox.DESIGN_TAG));
        j.addProperty("still", sb.still(e));
        j.addProperty("collides", Sandbox.collides(e));
        j.addProperty("on_ground", e.onGround());
        net.minecraft.world.phys.AABB bb = e.getBoundingBox();
        // Overlaps a block's collision shape (stuck inside a block: a player cannot place it so).
        j.addProperty("embedded", !sb.level.noBlockCollision(e, bb.deflate(1.0E-7)));
        JsonArray box = new JsonArray();
        box.add(bb.minX);
        box.add(bb.minY);
        box.add(bb.minZ);
        box.add(bb.maxX);
        box.add(bb.maxY);
        box.add(bb.maxZ);
        j.add("box", box);
        j.addProperty("state", sb.keyState(e));
        Entity holder = e instanceof net.minecraft.world.entity.Leashable leashed ? leashed.getLeashHolder() : null;
        if (holder == null) {
            j.add("leash_holder", com.google.gson.JsonNull.INSTANCE);
        } else {
            j.addProperty("leash_holder", holder.getId() - sb.entityIdBase());
        }
        net.minecraft.world.entity.Leashable.LeashData leashData =
                e instanceof net.minecraft.world.entity.Leashable leashed ? leashed.getLeashData() : null;
        j.addProperty("angular_momentum", leashData == null ? 0.0 : leashData.angularMomentum);
        return j;
    }

    /**
     * Summon a design's entities ({@code entities: [{type, pos, nbt?}]}, see Sandbox#summon);
     * {@code mobs: true} (tickexact): mobs and passengers allowed, seeded (Sandbox#summonSeeded),
     * each spec may add {@code follow_bonus} and {@code finalize}. {@code net_ids}: each summoned
     * entity's number in the sandbox, tree order.
     */
    private JsonObject summon(JsonObject r) throws CommandSyntaxException {
        Sandbox sb = sandbox(r);
        boolean mobs = r.has("mobs") && r.get("mobs").getAsBoolean();
        JsonArray uuids = new JsonArray();
        JsonArray netIds = new JsonArray();
        for (Entity e : sb.summon(r.getAsJsonArray("entities"), mobs)) {
            uuids.add(e.getStringUUID());
            netIds.add(e.getId() - sb.entityIdBase());
        }
        JsonObject o = new JsonObject();
        o.add("uuids", uuids);
        o.add("net_ids", netIds);
        return o;
    }

    /**
     * (tickexact) Make a sandbox a world sandbox: the overworld's clock, weather and game rules,
     * which 26.3 keeps server-wide (so the worker must hold this sandbox alone). Fields, each
     * optional: {@code advance_time}, {@code advance_weather}, {@code spawn_mobs},
     * {@code spawn_monsters} (booleans: game rules), {@code rules} ({@code {name: value}}: any other
     * game rule, by its id), {@code day_time} (the overworld clock's total ticks now),
     * {@code difficulty} ("peaceful".."hard"), {@code weather} ({@code {clear, rain, thunder,
     * raining, thundering, rain_level, thunder_level}}: WeatherData and the level's rain and thunder
     * levels), {@code trace_randoms} (tracing randoms, for draw logs). From then on each tick of the
     * sandbox ticks the server's clock first (MinecraftServer.tickChildren's order).
     */
    private JsonObject world(JsonObject r) {
        Sandbox sb = sandbox(r);
        if (this.sandboxes.size() != 1) {
            throw new IllegalArgumentException("a world sandbox needs a worker of its own: 26.3 keeps the clock, the weather and the game rules server-wide ("
                    + this.sandboxes.size() + " sandboxes here)");
        }
        if (!sb.collectionsSeeded) {
            Exact.seedCollectionsShuffle(sb.seed);
            sb.collectionsSeeded = true;
        }
        net.minecraft.world.level.gamerules.GameRules rules = this.server.getGameRules();
        if (r.has("advance_time")) {
            rules.set(net.minecraft.world.level.gamerules.GameRules.ADVANCE_TIME, r.get("advance_time").getAsBoolean(), this.server);
        }
        if (r.has("advance_weather")) {
            rules.set(net.minecraft.world.level.gamerules.GameRules.ADVANCE_WEATHER, r.get("advance_weather").getAsBoolean(), this.server);
        }
        if (r.has("spawn_mobs")) {
            rules.set(net.minecraft.world.level.gamerules.GameRules.SPAWN_MOBS, r.get("spawn_mobs").getAsBoolean(), this.server);
        }
        if (r.has("spawn_monsters")) {
            rules.set(net.minecraft.world.level.gamerules.GameRules.SPAWN_MONSTERS, r.get("spawn_monsters").getAsBoolean(), this.server);
        }
        if (r.has("rules")) {
            for (Map.Entry<String, JsonElement> e : r.getAsJsonObject("rules").entrySet()) {
                setRule(rules, e.getKey(), e.getValue().getAsString());
            }
        }
        if (r.has("day_time")) {
            var clock = sb.level.registryAccess().get(net.minecraft.world.clock.WorldClocks.OVERWORLD).orElseThrow();
            this.server.clockManager().setTotalTicks(clock, r.get("day_time").getAsLong());
        }
        if (r.has("difficulty")) {
            net.minecraft.world.Difficulty d = net.minecraft.world.Difficulty.byName(r.get("difficulty").getAsString());
            if (d == null) {
                throw new IllegalArgumentException("unknown difficulty " + r.get("difficulty"));
            }
            sb.data.setDifficulty(d);
            this.server.setDifficulty(d, true);
        }
        if (r.has("weather")) {
            JsonObject w = r.getAsJsonObject("weather");
            net.minecraft.world.level.saveddata.WeatherData wd = this.server.getWeatherData();
            if (w.has("clear")) {
                wd.setClearWeatherTime(w.get("clear").getAsInt());
            }
            if (w.has("rain")) {
                wd.setRainTime(w.get("rain").getAsInt());
            }
            if (w.has("thunder")) {
                wd.setThunderTime(w.get("thunder").getAsInt());
            }
            if (w.has("raining")) {
                wd.setRaining(w.get("raining").getAsBoolean());
            }
            if (w.has("thundering")) {
                wd.setThundering(w.get("thundering").getAsBoolean());
            }
            if (w.has("rain_level")) {
                sb.level.setRainLevel(w.get("rain_level").getAsFloat());
            }
            if (w.has("thunder_level")) {
                sb.level.setThunderLevel(w.get("thunder_level").getAsFloat());
            }
        }
        if (r.has("trace_randoms")) {
            sb.setTraceRandoms(r.get("trace_randoms").getAsBoolean());
        }
        if (r.has("player")) {
            // {pos, mode ("creative"...), simulation, view, flying}
            JsonObject p = r.getAsJsonObject("player");
            JsonArray pp = p.getAsJsonArray("pos");
            net.minecraft.world.level.GameType mode = net.minecraft.world.level.GameType.byName(p.has("mode") ? p.get("mode").getAsString() : "creative");
            int sim = p.has("simulation") ? p.get("simulation").getAsInt() : 6;
            int view = p.has("view") ? p.get("view").getAsInt() : sim;
            net.minecraft.server.level.ServerPlayer pl = sb.addPlayer(this.server, pp.get(0).getAsDouble(), pp.get(1).getAsDouble(), pp.get(2).getAsDouble(), mode, sim, view);
            if (!p.has("flying") || p.get("flying").getAsBoolean()) {
                // the AFK player hangs where it is put (the real-game gate's bot sends no moves)
                pl.getAbilities().flying = pl.getAbilities().mayfly;
                pl.setNoGravity(true);
            }
        }
        sb.tickClock = true;
        JsonObject o = new JsonObject();
        o.addProperty("day_time", sb.level.getOverworldClockTime());
        JsonArray chunks = new JsonArray();
        for (int[] c : sb.tickingChunks()) {
            JsonArray a = new JsonArray();
            a.add(c[0]);
            a.add(c[1]);
            chunks.add(a);
        }
        o.add("ticking_chunks", chunks);
        JsonArray spawning = new JsonArray();
        for (int[] c : sb.spawningChunks()) {
            JsonArray a = new JsonArray();
            a.add(c[0]);
            a.add(c[1]);
            spawning.add(a);
        }
        o.add("spawning_chunks", spawning);
        // (mob-AI) the chunks collectSpawningChunks walks, in its order, before its distance
        // filter (DistanceManager.getSpawnCandidateChunks with a ticking chunk): redsim filters
        // them each tick by the players' positions, as the game does once its AFK player is pushed
        JsonArray candidates = new JsonArray();
        for (int[] c : sb.spawnCandidates()) {
            JsonArray a = new JsonArray();
            a.add(c[0]);
            a.add(c[1]);
            candidates.add(a);
        }
        o.add("spawn_candidates", candidates);
        o.addProperty("spawn_chunk_count", sb.level.getChunkSource().chunkMap.getDistanceManager().getNaturalSpawnChunkCount());
        // (tickexact) the respawn position NaturalSpawner keeps 24 blocks clear (the server's)
        net.minecraft.core.BlockPos respawn = sb.level.getRespawnData().pos();
        JsonArray rs = new JsonArray();
        rs.add(respawn.getX());
        rs.add(respawn.getY());
        rs.add(respawn.getZ());
        o.add("respawn", rs);
        // the chunks where entities may be spawned and tick (ServerLevel.canSpawnEntitiesInChunk:
        // the entity manager's ticking sections, within the world border), over the spawn
        // candidates' square around the player and the forced box
        JsonArray entityTicking = new JsonArray();
        int px = sb.afk == null ? sb.minChunkX : net.minecraft.util.Mth.floor(sb.afk.getX()) >> 4;
        int pz = sb.afk == null ? sb.minChunkZ : net.minecraft.util.Mth.floor(sb.afk.getZ()) >> 4;
        int x0 = Math.min(sb.minChunkX, px - 9), x1 = Math.max(sb.maxChunkX, px + 9);
        int z0 = Math.min(sb.minChunkZ, pz - 9), z1 = Math.max(sb.maxChunkZ, pz + 9);
        for (int cx = x0; cx <= x1; cx++) {
            for (int cz = z0; cz <= z1; cz++) {
                if (sb.level.canSpawnEntitiesInChunk(new net.minecraft.world.level.ChunkPos(cx, cz))) {
                    JsonArray a = new JsonArray();
                    a.add(cx);
                    a.add(cz);
                    entityTicking.add(a);
                }
            }
        }
        o.add("entity_ticking_chunks", entityTicking);
        // the chunks loaded at full status (Level.hasChunk: hasChunksAt, what the cat and patrol
        // spawners ask around their spot)
        JsonArray full = new JsonArray();
        for (int cx = x0; cx <= x1; cx++) {
            for (int cz = z0; cz <= z1; cz++) {
                if (sb.level.getChunkSource().getChunkNow(cx, cz) != null) {
                    JsonArray a = new JsonArray();
                    a.add(cx);
                    a.add(cz);
                    full.add(a);
                }
            }
        }
        o.add("full_chunks", full);
        return o;
    }

    /** The chunk lists the level ticks by, now (the world op's, again: before a compare starts). */
    /**
     * (tickexact) Each listed column's WORLD_SURFACE height as {@code getRandomPosWithin} reads it
     * ({@code chunk.getHeight(WORLD_SURFACE, x, z)}: the highest taken y), and the level's.
     */
    private JsonObject heights(JsonObject r) {
        Sandbox sb = sandbox(r);
        JsonArray out = new JsonArray();
        for (JsonElement e : r.getAsJsonArray("columns")) {
            int x = e.getAsJsonArray().get(0).getAsInt();
            int z = e.getAsJsonArray().get(1).getAsInt();
            net.minecraft.world.level.chunk.LevelChunk chunk = sb.level.getChunk(x >> 4, z >> 4);
            JsonArray c = new JsonArray();
            c.add(x);
            c.add(z);
            c.add(chunk.getHeight(net.minecraft.world.level.levelgen.Heightmap.Types.WORLD_SURFACE, x & 15, z & 15));
            out.add(c);
        }
        JsonObject o = new JsonObject();
        o.add("heights", out);
        return o;
    }

    /**
     * (mob-AI) The identity hash of every POI type's registry holder: {@code AcquirePoi}'s failed
     * POIs get their retry markers in a {@code HashSet<Pair<Holder<PoiType>, BlockPos>>}'s order,
     * and a Pair's hash holds its holder's identity hash (this JVM's own, fixed for its life).
     */
    private JsonObject identityHashes() {
        JsonObject o = new JsonObject();
        var reg = net.minecraft.core.registries.BuiltInRegistries.POINT_OF_INTEREST_TYPE;
        for (var e : reg.entrySet()) {
            o.addProperty(e.getKey().identifier().toString(), System.identityHashCode(reg.wrapAsHolder(e.getValue())));
        }
        JsonObject out = new JsonObject();
        out.add("poi_types", o);
        return out;
    }

    private JsonObject chunkLists(JsonObject r) {
        JsonObject w = new JsonObject();
        w.addProperty("sandbox", r.get("sandbox").getAsInt());
        return world(w);
    }

    /** Set the game rule named {@code name} (its registry id: "spawn_phantoms", ...) from text. */
    @SuppressWarnings({"unchecked", "rawtypes"})
    private void setRule(net.minecraft.world.level.gamerules.GameRules rules, String name, String value) {
        net.minecraft.world.level.gamerules.GameRule rule = BuiltInRegistries.GAME_RULE
                .getOptional(net.minecraft.resources.Identifier.withDefaultNamespace(name))
                .orElseThrow(() -> new IllegalArgumentException("unknown game rule " + name));
        Object current = rules.get(rule);
        Object v;
        if (current instanceof Boolean) {
            v = Boolean.parseBoolean(value);
        } else if (current instanceof Integer) {
            v = Integer.parseInt(value);
        } else {
            throw new IllegalArgumentException("game rule " + name + ": unsupported type " + current.getClass().getSimpleName());
        }
        rules.set(rule, v, this.server);
    }

    /**
     * (tickexact 2026-11-10, a bench's drive replayed: `act t tp eN x y z`) Entity {@code entity}
     * (its net id) moved to {@code pos} at rest: {@code Entity.teleportTo} (snapped there, its
     * passengers placed on it), then its motion zeroed.
     */
    private JsonObject teleport(JsonObject r) {
        Sandbox sb = sandbox(r);
        int n = r.get("entity").getAsInt();
        Entity e = sb.level.getEntity(sb.entityIdBase() + n);
        if (e == null) {
            throw new IllegalArgumentException("teleport: no entity #" + n);
        }
        JsonArray p = r.getAsJsonArray("pos");
        e.teleportTo(p.get(0).getAsDouble(), p.get(1).getAsDouble(), p.get(2).getAsDouble());
        e.setDeltaMovement(net.minecraft.world.phys.Vec3.ZERO);
        return new JsonObject();
    }

    /**
     * (tickexact 2026-11-10, a bench's drive replayed: `act t poi-take x y z`) The POI at
     * {@code pos} taken (its ticket: what a villager's claim of its bed does).
     */
    private JsonObject poiTake(JsonObject r) {
        Sandbox sb = sandbox(r);
        int[] p = ints(r.getAsJsonArray("pos"));
        BlockPos pos = new BlockPos(p[0], p[1], p[2]);
        sb.level.getPoiManager().take(t -> true, (t, q) -> q.equals(pos), pos, 1)
                .orElseThrow(() -> new IllegalArgumentException("poi_take: no free POI at " + pos.toShortString()));
        return new JsonObject();
    }

    /**
     * (tickexact 2026-11-11, ironspec's ticket accounting: a bench's `unhome eN` / `forget eN`) The
     * villager's HOME given up: {@code release} true as SetWalkTargetFromBlockMemory's release does
     * it ({@code Villager.releasePoi(HOME)}: PoiManager.release, the record's ticket back when one is
     * taken; then the memory erased), false the memory erased only (ValidateNearbyPoi near an
     * occupied bed). {@code free_before} / {@code free_after}: the HOME record's free tickets.
     */
    private JsonObject homeRelease(JsonObject r) {
        Sandbox sb = sandbox(r);
        int n = r.get("entity").getAsInt();
        Entity e = sb.level.getEntity(sb.entityIdBase() + n);
        if (!(e instanceof net.minecraft.world.entity.npc.villager.Villager v)) {
            throw new IllegalArgumentException("home_release: no villager #" + n);
        }
        boolean release = r.has("release") && r.get("release").getAsBoolean();
        net.minecraft.world.entity.ai.memory.MemoryModuleType<net.minecraft.core.GlobalPos> home = net.minecraft.world.entity.ai.memory.MemoryModuleType.HOME;
        java.util.Optional<net.minecraft.core.GlobalPos> had = v.getBrain().getMemory(home);
        java.util.function.ToIntFunction<BlockPos> free = pos -> sb.level.getPoiManager()
                .getInSquare(t -> true, pos, 0, net.minecraft.world.entity.ai.village.poi.PoiManager.Occupancy.ANY)
                .filter(rec -> rec.getPos().equals(pos))
                .findFirst()
                .map(net.minecraft.world.entity.ai.village.poi.PoiRecord::getFreeTickets)
                .orElse(-1);
        int before = had.map(g -> free.applyAsInt(g.pos())).orElse(-1);
        if (release) {
            v.releasePoi(home);
        }
        v.getBrain().eraseMemory(home);
        JsonObject o = new JsonObject();
        o.addProperty("free_before", before);
        o.addProperty("free_after", had.map(g -> free.applyAsInt(g.pos())).orElse(-1));
        return o;
    }

    /** (tickexact 2026-11-11) The free tickets of the POI record at {@code pos} ({@code free}: -1 when there is none). */
    private JsonObject poiFree(JsonObject r) {
        Sandbox sb = sandbox(r);
        int[] p = ints(r.getAsJsonArray("pos"));
        BlockPos pos = new BlockPos(p[0], p[1], p[2]);
        int free = sb.level.getPoiManager()
                .getInSquare(t -> true, pos, 0, net.minecraft.world.entity.ai.village.poi.PoiManager.Occupancy.ANY)
                .filter(rec -> rec.getPos().equals(pos))
                .findFirst()
                .map(net.minecraft.world.entity.ai.village.poi.PoiRecord::getFreeTickets)
                .orElse(-1);
        JsonObject o = new JsonObject();
        o.addProperty("free", free);
        return o;
    }

    /**
     * (tickexact 2026-11-11, ironspec's forced-phase mode) A villager's brain sensor start delay
     * set: the villager nearest {@code pos} (its block's centre; within 2 blocks) gets its {@code
     * sensor}-th sensor's {@code timeToTick} (the brain's sensor order: {@code Brain.<init>}'s
     * {@code randomlyDelayStart}, one {@code nextInt(scanRate)} each) set to {@code value}. Returns
     * the villager's id and the delay it had.
     */
    private JsonObject sensorDelay(JsonObject r) {
        Sandbox sb = sandbox(r);
        int[] p = ints(r.getAsJsonArray("pos"));
        int k = r.get("sensor").getAsInt();
        long v = r.get("value").getAsLong();
        Vec3 c = new Vec3(p[0] + 0.5, p[1] + 0.5, p[2] + 0.5);
        net.minecraft.world.entity.npc.villager.Villager best = null;
        double bd = 4.0;
        for (net.minecraft.world.entity.npc.villager.Villager e : sb.level.getEntitiesOfClass(
                net.minecraft.world.entity.npc.villager.Villager.class, new net.minecraft.world.phys.AABB(c.x - 2, c.y - 2, c.z - 2, c.x + 2, c.y + 2, c.z + 2))) {
            double d = e.position().distanceToSqr(c);
            if (d < bd) {
                bd = d;
                best = e;
            }
        }
        if (best == null) {
            throw new IllegalArgumentException("sensor_delay: no villager within 2 of " + p[0] + " " + p[1] + " " + p[2]);
        }
        long was = Exact.setSensorDelay(best, k, v);
        JsonObject o = new JsonObject();
        o.addProperty("entity", best.getId());
        o.addProperty("was", was);
        return o;
    }

    /**
     * (tickexact) Set brain memories of entity {@code entity} (its net id): {@code memories:
     * {"home": [x,y,z], "last_slept": 123, ...}} (a position memory takes [x,y,z] in this sandbox,
     * a time memory a number, a flag memory true; {@code ttl} per memory: {@code {"golem_detected_recently":
     * {value: true, ttl: 600}}}). {@code take_poi: true}: a position memory naming a POI also takes
     * its ticket, as {@code AcquirePoi} does when a villager claims it.
     */
    @SuppressWarnings({"unchecked", "rawtypes"})
    /**
     * The cells where an iron golem summoned from each of {@code starts} (a villager's block
     * position) can spawn now (mobviewer 2026-11-08, vjeux: "why is the spawning ui not respecting
     * the game mechanics?"): every column of {@code SpawnUtil.trySpawnMob(IRON_GOLEM, .., 10, 8, 6,
     * LEGACY_IRON_GOLEM, false)}'s 17 x 17, its cell found by the game's own
     * {@code SpawnUtil.moveToPossibleSpawnPosition} (by reflection), then checked like trySpawnMob
     * checks it: a golem made there ({@code EntityType.create(level, reason)}: no random drawn, the
     * level's entity counter put back after, never added) passes {@code checkSpawnRules} and
     * {@code IronGolem.checkSpawnObstruction} with the level's entities in place. Returns
     * {@code cells} (passing, each once, in villager then column order) and {@code refused} (the
     * cells the scan finds that fail the checks).
     */
    private JsonObject golemSpots(JsonObject r) throws ReflectiveOperationException {
        Sandbox sb = sandbox(r);
        net.minecraft.server.level.ServerLevel level = sb.level;
        java.lang.reflect.Method move = net.minecraft.util.SpawnUtil.class.getDeclaredMethod(
            "moveToPossibleSpawnPosition", net.minecraft.server.level.ServerLevel.class, int.class, BlockPos.MutableBlockPos.class, net.minecraft.util.SpawnUtil.Strategy.class);
        move.setAccessible(true);
        java.lang.reflect.Field counterField = net.minecraft.server.level.ServerLevel.class.getDeclaredField("ENTITY_COUNTER");
        counterField.setAccessible(true);
        java.util.concurrent.atomic.AtomicInteger counter = (java.util.concurrent.atomic.AtomicInteger) counterField.get(null);
        int saved = counter.get();
        JsonArray cells = new JsonArray();
        JsonArray refused = new JsonArray();
        java.util.Set<Long> seen = new java.util.HashSet<>();
        try {
            for (JsonElement s : r.getAsJsonArray("starts")) {
                int[] p = ints(s.getAsJsonArray());
                BlockPos start = new BlockPos(p[0], p[1], p[2]);
                for (int dz = -8; dz <= 8; dz++) {
                    for (int dx = -8; dx <= 8; dx++) {
                        BlockPos.MutableBlockPos search = start.mutable();
                        search.setWithOffset(start, dx, 6, dz);
                        if (!level.getWorldBorder().isWithinBounds(search)
                            || !(boolean) move.invoke(null, level, 6, search, net.minecraft.util.SpawnUtil.Strategy.LEGACY_IRON_GOLEM)) {
                            continue;
                        }
                        BlockPos cell = search.immutable();
                        if (!seen.add(cell.asLong())) {
                            continue;
                        }
                        net.minecraft.world.entity.animal.golem.IronGolem golem =
                            net.minecraft.world.entity.EntityTypes.IRON_GOLEM.create(level, net.minecraft.world.entity.EntitySpawnReason.MOB_SUMMONED);
                        boolean ok = false;
                        if (golem != null) {
                            golem.snapTo(cell.getX() + 0.5, cell.getY(), cell.getZ() + 0.5, 0.0F, 0.0F);
                            ok = golem.checkSpawnRules(level, net.minecraft.world.entity.EntitySpawnReason.MOB_SUMMONED) && golem.checkSpawnObstruction(level);
                        }
                        (ok ? cells : refused).add(pos(cell.getX(), cell.getY(), cell.getZ()));
                    }
                }
            }
        } finally {
            counter.set(saved);
        }
        JsonObject o = new JsonObject();
        o.add("cells", cells);
        o.add("refused", refused);
        return o;
    }

    private JsonObject memory(JsonObject r) {
        Sandbox sb = sandbox(r);
        int n = r.get("entity").getAsInt();
        Entity e = sb.level.getEntity(sb.entityIdBase() + n);
        if (!(e instanceof net.minecraft.world.entity.LivingEntity living)) {
            throw new IllegalArgumentException("memory: no living entity #" + n);
        }
        boolean takePoi = r.has("take_poi") && r.get("take_poi").getAsBoolean();
        net.minecraft.world.entity.ai.Brain brain = living.getBrain();
        for (Map.Entry<String, JsonElement> m : r.getAsJsonObject("memories").entrySet()) {
            net.minecraft.world.entity.ai.memory.MemoryModuleType type = BuiltInRegistries.MEMORY_MODULE_TYPE
                    .getOptional(net.minecraft.resources.Identifier.withDefaultNamespace(m.getKey()))
                    .orElseThrow(() -> new IllegalArgumentException("unknown memory " + m.getKey()));
            JsonElement v = m.getValue();
            Long ttl = null;
            if (v.isJsonObject()) {
                JsonObject vo = v.getAsJsonObject();
                ttl = vo.has("ttl") ? vo.get("ttl").getAsLong() : null;
                v = vo.get("value");
            }
            Object value;
            if (v.isJsonArray()) {
                int[] p = ints(v.getAsJsonArray());
                BlockPos pos = new BlockPos(p[0], p[1], p[2]);
                value = net.minecraft.core.GlobalPos.of(sb.level.dimension(), pos);
                if (takePoi) {
                    sb.level.getPoiManager().take(t -> true, (t, q) -> q.equals(pos), pos, 1)
                            .orElseThrow(() -> new IllegalArgumentException("memory " + m.getKey() + ": no free POI at " + pos.toShortString()));
                }
            } else if (v.getAsJsonPrimitive().isBoolean()) {
                value = v.getAsBoolean();
            } else {
                value = v.getAsLong();
            }
            if (ttl == null) {
                brain.setMemory(type, value);
            } else {
                brain.setMemoryWithExpiry(type, value, ttl);
            }
        }
        return new JsonObject();
    }

    /**
     * (tickexact) Step with a frame per tick for the exact compare ({@code ticks}; {@code box:
     * {from, to}}: report block and block-entity changes there; {@code draws}: each tick's draw log,
     * needs {@code trace_randoms}). A frame: {@code t} (game time), {@code day} (the overworld
     * clock), {@code rng}/{@code rv} (the level random's state, randValue), {@code weather},
     * {@code entities} (Exact#entityFrame, every entity), {@code entered}/{@code left} (entity
     * numbers), {@code blocks}/{@code bes} (changes in the box since the previous frame: [x, y, z,
     * state] / [x, y, z, dump]), {@code draws}.
     */
    private JsonObject trace(JsonObject r) {
        Sandbox sb = sandbox(r);
        int ticks = r.has("ticks") ? r.get("ticks").getAsInt() : 1;
        boolean draws = r.has("draws") && r.get("draws").getAsBoolean();
        Exact.Watch watch = sb.watch(r.has("box") ? r.getAsJsonObject("box") : null);
        // (tickexact 2026-11-10) its Nether's frame beside each frame ("nether_box": the Nether
        // sandbox's watched cells)
        Exact.Watch netherWatch = sb.nether != null && r.has("nether_box") ? sb.nether.watch(r.getAsJsonObject("nether_box")) : null;
        JsonArray frames = new JsonArray();
        for (int i = 0; i < ticks; i++) {
            Exact.DrawLog log = draws ? new Exact.DrawLog() : null;
            // a probed tick: the spawning chunks before the shuffle (as tickChunks collects them)
            List<int[]> pre = draws ? sb.spawningChunks() : null;
            Exact.drawLog = log;
            try {
                sb.tick(this.server);
            } finally {
                Exact.drawLog = null;
            }
            JsonObject f = watch.frame(sb, log);
            if (netherWatch != null) {
                f.add("nether", netherWatch.frame(sb.nether, null));
            }
            if (log != null && log.shuffleState >= 0) {
                // Util.shuffle(list, level.random) redone from the state its first draw saw
                net.minecraft.world.level.levelgen.LegacyRandomSource copy = new net.minecraft.world.level.levelgen.LegacyRandomSource(0L);
                Exact.setState(copy, log.shuffleState);
                List<int[]> shuffled = new ArrayList<>(pre);
                for (int k = shuffled.size(); k > 1; k--) {
                    java.util.Collections.swap(shuffled, k - 1, copy.nextInt(k));
                }
                JsonArray a = new JsonArray();
                for (int[] c : shuffled) {
                    JsonArray p = new JsonArray();
                    p.add(c[0]);
                    p.add(c[1]);
                    a.add(p);
                }
                f.add("spawning_shuffled", a);
            }
            frames.add(f);
        }
        JsonObject o = new JsonObject();
        o.add("frames", frames);
        return o;
    }

    /** Direct attachment fixture: normal canHaveALeashAttachedTo/setLeashedTo semantics. */
    private JsonObject leash(JsonObject r) {
        Sandbox sb = sandbox(r);
        Entity e = sb.level.getEntity(java.util.UUID.fromString(r.get("entity").getAsString()));
        if (!(e instanceof net.minecraft.world.entity.Leashable leashed)) {
            throw new IllegalArgumentException("leash: missing or non-leashable entity");
        }
        if (!r.has("holder") || r.get("holder").isJsonNull()) {
            leashed.removeLeash();
        } else {
            Entity holder = sb.level.getEntity(java.util.UUID.fromString(r.get("holder").getAsString()));
            if (holder == null || !leashed.canHaveALeashAttachedTo(holder)) {
                throw new IllegalArgumentException("leash: invalid holder, self-link or outside attachment range");
            }
            leashed.setLeashedTo(holder, false);
        }
        return new JsonObject();
    }

    /** Continuous box [from, to + 1] of an inclusive block range. */
    private static net.minecraft.world.phys.AABB blockBox(JsonArray from, JsonArray to) {
        int[] a = ints(from);
        int[] b = ints(to);
        return new net.minecraft.world.phys.AABB(Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.min(a[2], b[2]),
                Math.max(a[0], b[0]) + 1, Math.max(a[1], b[1]) + 1, Math.max(a[2], b[2]) + 1);
    }

    private static JsonObject entityEvent(int tick, Entity e) {
        JsonObject o = new JsonObject();
        o.addProperty("tick", tick);
        o.addProperty("type", BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString());
        o.addProperty("uuid", e.getStringUUID());
        Vec3 p = e.position();
        o.addProperty("pos", String.format(java.util.Locale.ROOT, "%.4f %.4f %.4f", p.x, p.y, p.z));
        return o;
    }

    /**
     * Signal probes, now: for each position, [best signal its neighbors give into it (what a
     * piston or lamp there would see), strong signal into it].
     */
    private JsonObject signals(JsonObject r) {
        Sandbox sb = sandbox(r);
        JsonArray out = new JsonArray();
        for (JsonElement e : r.getAsJsonArray("positions")) {
            int[] p = ints(e.getAsJsonArray());
            BlockPos pos = new BlockPos(p[0], p[1], p[2]);
            JsonArray v = new JsonArray();
            v.add(sb.level.getBestNeighborSignal(pos));
            v.add(sb.level.getDirectSignalTo(pos));
            out.add(v);
        }
        JsonObject o = new JsonObject();
        o.add("signals", out);
        return o;
    }

    /**
     * Manual updates. kind=neighbors: notify the 6 neighbors of each position (like a block
     * change there); kind=self: the block at each position re-checks its neighbors; kind=shape:
     * recompute the block's shape-dependent state (wire connections, fences...) without updates.
     */
    private JsonObject update(JsonObject r) {
        Sandbox sb = sandbox(r);
        String kind = r.has("kind") ? r.get("kind").getAsString() : "neighbors";
        int changed = 0;
        for (JsonElement e : r.getAsJsonArray("positions")) {
            int[] p = ints(e.getAsJsonArray());
            BlockPos pos = new BlockPos(p[0], p[1], p[2]);
            BlockState state = sb.level.getBlockState(pos);
            switch (kind) {
                case "neighbors" -> sb.level.updateNeighborsAt(pos, state.getBlock());
                case "self" -> sb.level.neighborChanged(pos, state.getBlock(), null);
                case "shape" -> {
                    BlockState updated = Block.updateFromNeighbourShapes(state, sb.level, pos);
                    if (updated != state) {
                        sb.level.setBlock(pos, updated, Sandbox.RAW_FLAGS);
                        changed++;
                    }
                }
                default -> throw new IllegalArgumentException("unknown update kind " + kind);
            }
        }
        JsonObject o = new JsonObject();
        o.addProperty("changed", changed);
        return o;
    }

    /**
     * Survival building: run player actions one by one, letting the world settle after each (a
     * player is slow): {@code place} (a real item placement: click a neighbouring block's face with
     * the look direction that gives the wanted orientation, then right-click until click-set
     * properties match; see Sandbox#placeLikePlayer), {@code use} (right-click), {@code break}
     * (break like a player, no drops). After each action: at least {@code min_ticks} ticks, then
     * until idle (at most {@code settle}). Stops at the first failed action unless
     * {@code stop_on_fail} is false.
     */
    private JsonObject build(JsonObject r) throws CommandSyntaxException {
        Sandbox sb = sandbox(r);
        int settle = r.has("settle") ? r.get("settle").getAsInt() : 200;
        int minTicks = r.has("min_ticks") ? r.get("min_ticks").getAsInt() : 4;
        boolean stopOnFail = !r.has("stop_on_fail") || r.get("stop_on_fail").getAsBoolean();
        JsonArray results = new JsonArray();
        int failedAt = -1;
        int i = 0;
        for (JsonElement e : r.getAsJsonArray("steps")) {
            JsonObject st = e.getAsJsonObject();
            String what = st.get("do").getAsString();
            int[] p = ints(st.getAsJsonArray("pos"));
            BlockPos pos = new BlockPos(p[0], p[1], p[2]);
            checkInside(sb, pos);
            JsonObject res;
            switch (what) {
                case "place" -> res = sb.placeLikePlayer(this.server, pos, parse(st.get("state").getAsString()), this::serialize, settle);
                case "use" -> {
                    res = new JsonObject();
                    res.addProperty("ok", true);
                    res.addProperty("result", sb.use(pos, Direction.UP));
                }
                case "break" -> {
                    sb.breakLikePlayer(pos);
                    res = new JsonObject();
                    res.addProperty("ok", true);
                }
                default -> throw new IllegalArgumentException("unknown build action '" + what + "'");
            }
            int ticked = 0;
            int min = st.has("min_ticks") ? st.get("min_ticks").getAsInt() : minTicks;
            while (ticked < min || (ticked < settle && !sb.idle())) {
                sb.tick(this.server);
                ticked++;
            }
            res.addProperty("ticks", ticked);
            res.addProperty("idle", sb.idle());
            results.add(res);
            if (!res.get("ok").getAsBoolean() && failedAt < 0) {
                failedAt = i;
                if (stopOnFail) {
                    break;
                }
            }
            i++;
        }
        JsonObject o = new JsonObject();
        o.add("results", results);
        o.addProperty("failed_at", failedAt);
        return o;
    }

    private JsonObject exec(JsonObject r) {
        Sandbox sb = sandbox(r);
        CommandSourceStack source = this.server.createCommandSourceStack()
                .withLevel(sb.level)
                .withPosition(Vec3.ZERO)
                .withSuppressedOutput();
        this.server.getCommands().performPrefixedCommand(source, r.get("command").getAsString());
        return new JsonObject();
    }

    /**
     * Emit a game event at block positions, at each cell's bottom center (where a player's feet
     * are) or, with {@code at} = {@code center}, at its center (where {@code Level.gameEvent} posts a
     * block's own events, e.g. a lever's BLOCK_ACTIVATE). {@code event}: {@code step} (the default),
     * {@code hit_ground}, {@code block_place}, {@code block_destroy}, {@code entity_place},
     * {@code block_activate}, {@code block_deactivate}, {@code block_open}, {@code block_close},
     * {@code note_block_play}. No source entity, no affected state (sculk sensors hear it the same
     * way).
     */
    private JsonObject gameEvent(JsonObject r) {
        Sandbox sb = sandbox(r);
        String name = r.has("event") ? r.get("event").getAsString() : "step";
        boolean center = r.has("at") && r.get("at").getAsString().equals("center");
        net.minecraft.core.Holder<net.minecraft.world.level.gameevent.GameEvent> ev = switch (name.startsWith("minecraft:") ? name.substring(10) : name) {
            case "step" -> net.minecraft.world.level.gameevent.GameEvent.STEP;
            case "hit_ground" -> net.minecraft.world.level.gameevent.GameEvent.HIT_GROUND;
            case "block_place" -> net.minecraft.world.level.gameevent.GameEvent.BLOCK_PLACE;
            case "block_destroy" -> net.minecraft.world.level.gameevent.GameEvent.BLOCK_DESTROY;
            case "entity_place" -> net.minecraft.world.level.gameevent.GameEvent.ENTITY_PLACE;
            case "block_activate" -> net.minecraft.world.level.gameevent.GameEvent.BLOCK_ACTIVATE;
            case "block_deactivate" -> net.minecraft.world.level.gameevent.GameEvent.BLOCK_DEACTIVATE;
            case "block_open" -> net.minecraft.world.level.gameevent.GameEvent.BLOCK_OPEN;
            case "block_close" -> net.minecraft.world.level.gameevent.GameEvent.BLOCK_CLOSE;
            case "note_block_play" -> net.minecraft.world.level.gameevent.GameEvent.NOTE_BLOCK_PLAY;
            default -> throw new IllegalArgumentException("unsupported game event " + name);
        };
        int n = 0;
        for (JsonElement e : r.getAsJsonArray("positions")) {
            int[] p = ints(e.getAsJsonArray());
            sb.level.gameEvent(ev, new Vec3(p[0] + 0.5, p[1] + (center ? 0.5 : 0.0), p[2] + 0.5), net.minecraft.world.level.gameevent.GameEvent.Context.of((Entity) null, (BlockState) null));
            n++;
        }
        JsonObject o = new JsonObject();
        o.addProperty("emitted", n);
        return o;
    }

    private JsonObject viewers() {
        JsonObject o = new JsonObject();
        JsonArray names = new JsonArray();
        for (net.minecraft.server.level.ServerPlayer p : this.server.getPlayerList().getPlayers()) {
            names.add(p.getGameProfile().name() + String.format(" pos=%.2f,%.2f,%.2f yaw=%.1f pitch=%.1f level=%s", p.getX(), p.getY(), p.getZ(), p.getYRot(), p.getXRot(), p.level().dimension().identifier()));
        }
        o.addProperty("count", names.size());
        o.add("names", names);
        return o;
    }

    /**
     * Every connected viewer's game mode ({@code mode}: creative, spectator, survival, adventure)
     * and, with {@code operator}, operator status (a client mod that pastes with commands, such as
     * Litematica, needs both: `mctas litematic verify-ingame`).
     */
    private JsonObject viewerMode(JsonObject r) {
        net.minecraft.world.level.GameType type = net.minecraft.world.level.GameType.byName(r.has("mode") ? r.get("mode").getAsString() : "spectator", net.minecraft.world.level.GameType.SPECTATOR);
        boolean op = r.has("operator") && r.get("operator").getAsBoolean();
        int n = 0;
        for (net.minecraft.server.level.ServerPlayer player : this.server.getPlayerList().getPlayers()) {
            if (op) {
                // explicit permissions: this server's default operator level is ALL (0)
                this.server.getPlayerList().op(player.nameAndId(), java.util.Optional.of(net.minecraft.server.permissions.LevelBasedPermissionSet.OWNER), java.util.Optional.empty());
            }
            player.setGameMode(type);
            if (type == net.minecraft.world.level.GameType.CREATIVE) {
                // flying: the player stays where the camera put it (no fall onto the builds)
                player.getAbilities().flying = true;
                player.onUpdateAbilities();
            }
            n++;
        }
        JsonObject o = new JsonObject();
        o.addProperty("changed", n);
        return o;
    }

    /** Put every connected viewer in spectator mode at a camera pose inside a sandbox. */
    private JsonObject camera(JsonObject r) {
        Sandbox sb = sandbox(r);
        JsonArray p = r.getAsJsonArray("pos");
        float yaw = r.has("yaw") ? r.get("yaw").getAsFloat() : 0F;
        float pitch = r.has("pitch") ? r.get("pitch").getAsFloat() : 0F;
        int n = 0;
        for (net.minecraft.server.level.ServerPlayer player : this.server.getPlayerList().getPlayers()) {
            player.setGameMode(net.minecraft.world.level.GameType.SPECTATOR);
            player.teleportTo(sb.level, p.get(0).getAsDouble(), p.get(1).getAsDouble(), p.get(2).getAsDouble(), java.util.Set.of(), yaw, pitch, true);
            n++;
        }
        JsonObject o = new JsonObject();
        o.addProperty("moved", n);
        return o;
    }

    /** Chunk radius the sandbox's level sends to viewers (the level is made with 2). */
    private JsonObject view(JsonObject r) {
        Sandbox sb = sandbox(r);
        sb.level.getChunkSource().setViewDistance(r.get("chunks").getAsInt());
        return new JsonObject();
    }

    /** Step a sandbox at a wall-clock pace (for viewers), with network work between ticks. */
    private JsonObject play(JsonObject r) throws InterruptedException {
        Sandbox sb = sandbox(r);
        int ticks = r.get("ticks").getAsInt();
        double tps = r.has("tps") ? r.get("tps").getAsDouble() : 20.0;
        long period = (long) (1_000_000_000L / tps);
        long next = System.nanoTime();
        for (int i = 0; i < ticks; i++) {
            sb.tick(this.server);
            this.server.viewerTick();
            next += period;
            long sleep = next - System.nanoTime();
            if (sleep > 0) {
                Thread.sleep(sleep / 1_000_000L, (int) (sleep % 1_000_000L));
            }
        }
        JsonObject o = new JsonObject();
        o.addProperty("time", sb.gameTime());
        return o;
    }

    /** Keep viewers served for a while without ticking any level. */
    private JsonObject hold(JsonObject r) throws InterruptedException {
        long until = System.nanoTime() + (long) (r.get("ms").getAsDouble() * 1_000_000L);
        while (System.nanoTime() < until) {
            this.server.viewerTick();
            Thread.sleep(25);
        }
        return new JsonObject();
    }

    // ---- helpers -----------------------------------------------------------------------------

    /**
     * Block states turned by {@code turns} quarter turns clockwise seen from above (the game's
     * own {@code BlockState.rotate(Rotation.CLOCKWISE_90)}: facings, axes, dust connections, rail
     * shapes, lever faces...). No sandbox needed.
     */
    private JsonObject rotate(JsonObject r) throws CommandSyntaxException {
        int turns = Math.floorMod(r.get("turns").getAsInt(), 4);
        // Optional mirror image first (x -> -x, the game's Mirror.FRONT_BACK), then the turns.
        boolean mirror = r.has("mirror") && r.get("mirror").getAsBoolean();
        JsonArray out = new JsonArray();
        for (JsonElement e : r.getAsJsonArray("states")) {
            BlockState s = parse(e.getAsString());
            if (mirror) {
                s = s.mirror(net.minecraft.world.level.block.Mirror.FRONT_BACK);
            }
            for (int i = 0; i < turns; i++) {
                s = s.rotate(net.minecraft.world.level.block.Rotation.CLOCKWISE_90);
            }
            out.add(serialize(s));
        }
        JsonObject o = new JsonObject();
        o.add("states", out);
        // Entities ({type, nbt?}): turned the way structure templates turn them
        // (StructureTemplate.placeEntities: yRot = rotate(rotation) + mirror(mirror) - yRot; a
        // hanging entity's facing turns too). Returns each entity's data with its new Rotation (and
        // facing), keeping every other key as given. Positions are turned by the caller.
        if (r.has("entities")) {
            net.minecraft.world.level.block.Rotation rot = switch (turns) {
                case 1 -> net.minecraft.world.level.block.Rotation.CLOCKWISE_90;
                case 2 -> net.minecraft.world.level.block.Rotation.CLOCKWISE_180;
                case 3 -> net.minecraft.world.level.block.Rotation.COUNTERCLOCKWISE_90;
                default -> net.minecraft.world.level.block.Rotation.NONE;
            };
            net.minecraft.world.level.block.Mirror mir = mirror ? net.minecraft.world.level.block.Mirror.FRONT_BACK : net.minecraft.world.level.block.Mirror.NONE;
            JsonArray ents = new JsonArray();
            for (JsonElement e : r.getAsJsonArray("entities")) {
                JsonObject spec = e.getAsJsonObject();
                net.minecraft.nbt.CompoundTag given = spec.has("nbt") && !spec.get("nbt").isJsonNull()
                        ? net.minecraft.nbt.TagParser.parseCompoundFully(spec.get("nbt").getAsString())
                        : new net.minecraft.nbt.CompoundTag();
                net.minecraft.nbt.CompoundTag tag = given.copy();
                tag.putString("id", spec.get("type").getAsString());
                Sandbox.attachAt(tag, spec.get("type").getAsString(), 0.5, 0.5, 0.5);
                net.minecraft.server.level.ServerLevel scratch = this.server.overworld();
                Entity entity = net.minecraft.world.entity.EntityType.loadEntityRecursive(tag, scratch,
                        new net.minecraft.world.entity.EntitySpawnRequest(net.minecraft.world.entity.EntitySpawnReason.COMMAND, false), en -> en);
                if (entity == null) {
                    throw new IllegalArgumentException("cannot create " + spec.get("type").getAsString());
                }
                float yRot = entity.rotate(rot);
                yRot += entity.mirror(mir) - entity.getYRot();
                entity.snapTo(entity.getX(), entity.getY(), entity.getZ(), yRot, entity.getXRot());
                net.minecraft.world.level.storage.TagValueOutput saved = net.minecraft.world.level.storage.TagValueOutput.createWithContext(
                        net.minecraft.util.ProblemReporter.DISCARDING, scratch.registryAccess());
                entity.saveWithoutId(saved);
                net.minecraft.nbt.CompoundTag full = saved.buildResult();
                net.minecraft.nbt.CompoundTag result = given.copy();
                if (given.contains("Rotation") || net.minecraft.util.Mth.wrapDegrees(entity.getYRot()) != 0.0F || entity.getXRot() != 0.0F) {
                    result.put("Rotation", full.get("Rotation"));
                }
                for (String k : new String[] {"Facing", "facing"}) {
                    if (full.contains(k)) {
                        result.put(k, full.get(k));
                    }
                }
                ents.add(result.isEmpty() ? null : result.toString());
            }
            o.add("entities", ents);
        }
        return o;
    }

    private JsonArray moving(Sandbox sb, Region region) {
        JsonArray list = new JsonArray();
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        for (int y = region.y0; y < region.y0 + region.dy; y++) {
            for (int z = region.z0; z < region.z0 + region.dz; z++) {
                for (int x = region.x0; x < region.x0 + region.dx; x++) {
                    pos.set(x, y, z);
                    BlockEntity be = sb.level.getBlockEntity(pos);
                    if (be instanceof PistonMovingBlockEntity m) {
                        JsonObject j = new JsonObject();
                        j.add("pos", pos(x, y, z));
                        j.addProperty("state", serialize(m.getMovedState()));
                        j.addProperty("extending", m.isExtending());
                        j.addProperty("source", m.isSourcePiston());
                        j.addProperty("direction", m.getDirection().getSerializedName());
                        j.addProperty("progress", m.getProgress(0.0F));
                        list.add(j);
                    }
                }
            }
        }
        return list;
    }

    private Sandbox sandbox(JsonObject r) {
        if (!r.has("sandbox")) {
            throw new IllegalArgumentException("missing 'sandbox'");
        }
        return sandboxById(r.get("sandbox").getAsInt());
    }

    private Sandbox sandboxById(int id) {
        Sandbox sb = this.sandboxes.get(id);
        if (sb == null) {
            throw new IllegalArgumentException("no sandbox " + id);
        }
        return sb;
    }

    private static void checkInside(Sandbox sb, BlockPos pos) {
        if (pos.getX() < sb.minX() || pos.getX() > sb.maxX() || pos.getZ() < sb.minZ() || pos.getZ() > sb.maxZ()
                || pos.getY() < sb.level.getMinY() || pos.getY() > sb.level.getMaxY()) {
            throw new IllegalArgumentException("position " + pos.toShortString() + " is outside sandbox " + sb.id);
        }
    }

    BlockState parse(String s) throws CommandSyntaxException {
        BlockState state = this.parsedStates.get(s);
        if (state == null) {
            state = BlockStateParser.parseForBlock(this.blockLookup, s, false).blockState();
            this.parsedStates.put(s, state);
        }
        return state;
    }

    String serialize(BlockState state) {
        return this.serializedStates.computeIfAbsent(state, BlockStateParser::serialize);
    }

    private static int[] ints(JsonArray a) {
        int[] out = new int[a.size()];
        for (int i = 0; i < out.length; i++) {
            out[i] = a.get(i).getAsInt();
        }
        return out;
    }

    private static JsonArray pos(int x, int y, int z) {
        JsonArray a = new JsonArray();
        a.add(x);
        a.add(y);
        a.add(z);
        return a;
    }

    private void write(JsonObject o) {
        try {
            this.out.write(this.gson.toJson(o).getBytes(StandardCharsets.UTF_8));
            this.out.write('\n');
        } catch (IOException e) {
            Runtime.getRuntime().halt(0);
        }
    }

    private void flush() {
        try {
            this.out.flush();
        } catch (IOException e) {
            Runtime.getRuntime().halt(0);
        }
    }

    /** Block states seen in one response, in first-seen order. */
    final class Palette {
        private final Map<BlockState, Integer> index = new IdentityHashMap<>();
        private final List<BlockState> states = new ArrayList<>();

        int id(BlockState state) {
            Integer i = this.index.get(state);
            if (i == null) {
                i = this.states.size();
                this.index.put(state, i);
                this.states.add(state);
            }
            return i;
        }

        JsonArray toJson() {
            JsonArray a = new JsonArray();
            for (BlockState s : this.states) {
                a.add(serialize(s));
            }
            return a;
        }

        /** Per palette entry: does it collide at all, and is its collision a full cube. */
        JsonArray flagsJson(Sandbox sb) {
            JsonArray a = new JsonArray();
            for (BlockState s : this.states) {
                JsonObject f = new JsonObject();
                f.addProperty("collides", !s.getCollisionShape(sb.level, BlockPos.ZERO).isEmpty());
                f.addProperty("full", s.isCollisionShapeFullBlock(sb.level, BlockPos.ZERO));
                a.add(f);
            }
            return a;
        }
    }

    static final class Region {
        final int x0, y0, z0, dx, dy, dz;

        Region(int[] a, int[] b) {
            this.x0 = Math.min(a[0], b[0]);
            this.y0 = Math.min(a[1], b[1]);
            this.z0 = Math.min(a[2], b[2]);
            this.dx = Math.abs(a[0] - b[0]) + 1;
            this.dy = Math.abs(a[1] - b[1]) + 1;
            this.dz = Math.abs(a[2] - b[2]) + 1;
            if ((long) this.dx * this.dy * this.dz > 16_000_000L) {
                throw new IllegalArgumentException("region too large");
            }
        }

        String capture(Sandbox sb, Palette palette) {
            byte[] bytes = new byte[this.dx * this.dy * this.dz * 2];
            BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
            int i = 0;
            for (int y = this.y0; y < this.y0 + this.dy; y++) {
                for (int z = this.z0; z < this.z0 + this.dz; z++) {
                    for (int x = this.x0; x < this.x0 + this.dx; x++) {
                        int id = palette.id(sb.level.getBlockState(pos.set(x, y, z)));
                        bytes[i++] = (byte) id;
                        bytes[i++] = (byte) (id >> 8);
                    }
                }
            }
            return Base64.getEncoder().encodeToString(bytes);
        }

        /**
         * Every block entity of the region in the block order (x fastest, then z, then y), one line
         * each: {@code x y z <type> <saveWithoutMetadata SNBT>} (keys sorted, so equal states give
         * equal text: container items, a hopper's TransferCooldown, a comparator's OutputSignal, a
         * moving piston's progress...); "" when there is none.
         */
        String captureBlockEntities(Sandbox sb) {
            StringBuilder out = new StringBuilder();
            BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
            for (int y = this.y0; y < this.y0 + this.dy; y++) {
                for (int z = this.z0; z < this.z0 + this.dz; z++) {
                    for (int x = this.x0; x < this.x0 + this.dx; x++) {
                        BlockEntity be = sb.level.getBlockEntity(pos.set(x, y, z));
                        if (be == null) {
                            continue;
                        }
                        if (out.length() > 0) {
                            out.append('\n');
                        }
                        out.append(x).append(' ').append(y).append(' ').append(z).append(' ')
                                .append(BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(be.getType())).append(' ')
                                .append(be.saveWithoutMetadata(sb.level.registryAccess()).toString());
                    }
                }
            }
            return out.toString();
        }
    }
}
