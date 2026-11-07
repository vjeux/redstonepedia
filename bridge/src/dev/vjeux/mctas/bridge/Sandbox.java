package dev.vjeux.mctas.bridge;

import com.mojang.authlib.GameProfile;
import com.mojang.serialization.Lifecycle;
import java.io.IOException;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;
import java.util.concurrent.locks.LockSupport;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ChunkMap;
import net.minecraft.server.level.ChunkTaskDispatcher;
import net.minecraft.server.level.ServerChunkCache;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ThreadedLevelLightEngine;
import net.minecraft.util.Util;
import net.minecraft.util.thread.AbstractConsecutiveExecutor;
import net.minecraft.util.profiling.Profiler;
import net.minecraft.util.profiling.ProfilerFiller;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.RandomSequences;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.item.FallingBlockEntity;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.ButtonBlock;
import net.minecraft.world.level.block.LeverBlock;
import net.minecraft.world.level.block.entity.TickingBlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.entity.PersistentEntitySectionManager;
import net.minecraft.world.level.entity.LevelCallback;
import net.minecraft.util.RandomSource;
import net.minecraft.util.Mth;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.storage.PrimaryLevelData;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.Vec3;

/**
 * One isolated simulation: its own {@link ServerLevel} with a void generator, a fixed set of
 * force-loaded chunks, its own game time (so scheduled ticks fire on its own clock) and its own
 * seeded random. Nothing outside {@link #tick()} ever advances it.
 */
final class Sandbox {
    /** setBlock flags for "raw" placement: no neighbor updates, no shape updates, no onPlace, no drops. */
    static final int RAW_FLAGS = Block.UPDATE_CLIENTS
            | Block.UPDATE_KNOWN_SHAPE
            | Block.UPDATE_SUPPRESS_DROPS
            | Block.UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS
            | Block.UPDATE_SKIP_ON_PLACE;

    private static final Field BLOCK_EVENTS = field(ServerLevel.class, "blockEvents");
    private static final Field BLOCK_ENTITY_TICKERS = field(Level.class, "blockEntityTickers");
    private static final Field PENDING_BLOCK_ENTITY_TICKERS = field(Level.class, "pendingBlockEntityTickers");
    private static final Field ENTITY_MANAGER = field(ServerLevel.class, "entityManager");
    /** {@code Level.randValue}: the LCG behind {@code getBlockRandomPos} (random ticks, precipitation). */
    private static final Field RAND_VALUE = field(Level.class, "randValue");

    /**
     * The sandbox's {@code Level.randValue}, from its seed (clk-misc 2026-10-21, exact random ticks):
     * vanilla seeds it with {@code RandomSource.createThreadLocalInstance().nextInt()}, so the cells
     * random ticks pick differed on every run of the same seeded sandbox; set at create and reset to
     * {@code (int) (seed ^ 0x72616E64)} ("rand"), which redsim's mirror starts from.
     */
    static int randValueOf(long seed) {
        return (int) (seed ^ 0x72616E64L);
    }

    private static void seedRandValue(ServerLevel level, long seed) {
        try {
            RAND_VALUE.setInt(level, randValueOf(seed));
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }

    /** {@code Level.randValue} now, read without advancing it. */
    int randValue() {
        try {
            return RAND_VALUE.getInt(this.level);
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }

    /**
     * The salt of the sandbox's loot random sequences (clk-misc 2026-10-21, melon loot): vanilla keeps
     * ONE random sequence per loot table for the whole server ({@code MinecraftServer.getRandomSequence},
     * seeded from the world seed and the table's id), so a sandbox's melon drops (3-7 slices: the
     * table's {@code set_count} draws from it) followed whatever loot drew before in the same JVM. At
     * create and reset the sequences are cleared and their seed defaults set to this salt, without the
     * world seed: each table's sequence then starts from {@code (long) salt} and its id, which redsim's
     * mirror ({@code redsim::stems::LootSequence}) starts from.
     */
    static int lootSaltOf(long seed) {
        return (int) (seed ^ 0x6C6F6F74L);
    }

    private static void seedLootSequences(ServerLevel level, long seed) {
        RandomSequences sequences = level.getServer().getRandomSequences();
        sequences.clear();
        sequences.setSeedDefaults(lootSaltOf(seed), false, true);
    }

    /**
     * The chunks {@code ServerChunkCache.tickChunks} hands to {@code ServerLevel.tickChunk}, in its
     * order ({@code ChunkMap.forEachBlockTickingChunk}: the entity-ticking chunks of the simulation
     * tracker's hash map, which redsim's mirror recomputes): [x, z] each.
     */
    List<int[]> tickingChunks() {
        List<int[]> out = new ArrayList<>();
        this.level.getChunkSource().chunkMap.forEachBlockTickingChunk(chunk -> out.add(new int[] {chunk.getPos().x(), chunk.getPos().z()}));
        return out;
    }

    final int id;
    final ServerLevel level;
    final PrimaryLevelData data;
    final int minChunkX;
    final int minChunkZ;
    final int maxChunkX;
    final int maxChunkZ;
    final long seed;
    private Player mockPlayer;

    private Sandbox(int id, ServerLevel level, PrimaryLevelData data, int minChunkX, int minChunkZ, int maxChunkX, int maxChunkZ, long seed) {
        this.id = id;
        this.level = level;
        this.data = data;
        this.minChunkX = minChunkX;
        this.minChunkZ = minChunkZ;
        this.maxChunkX = maxChunkX;
        this.maxChunkZ = maxChunkZ;
        this.seed = seed;
    }

    static Sandbox create(SimServer server, int id, int minChunkX, int minChunkZ, int maxChunkX, int maxChunkZ, long seed) {
        ResourceKey<Level> key = ResourceKey.create(Registries.DIMENSION, Identifier.fromNamespaceAndPath("mctas", "sandbox_" + id));
        PrimaryLevelData data = new PrimaryLevelData(server.levelSettings, PrimaryLevelData.SpecialWorldProperty.FLAT, Lifecycle.stable());
        data.setInitialized(true);
        ServerLevel level = new ServerLevel(
                server,
                Util.backgroundExecutor(),
                server.storage(),
                data,
                key,
                server.voidStem,
                false,
                BiomeManager.obfuscateSeed(seed),
                List.of(),
                true);
        level.noSave = true;
        server.registerLevel(level);
        // the forced tickets first, then the loads (clk-misc 2026-10-21, exact random ticks): the
        // simulation tracker queues every ticket and runs them together (the forced chunks in this
        // order, then their neighbours), so the order the level ticks its chunks in (a fastutil
        // hash map's iteration) is a function of the box alone, which redsim recomputes
        // (crops::tick_order); setChunkForced chunk by chunk let the chunk loads decide when each
        // ticket ran
        for (int cx = minChunkX; cx <= maxChunkX; cx++) {
            for (int cz = minChunkZ; cz <= maxChunkZ; cz++) {
                level.getChunkSource().updateChunkForced(new ChunkPos(cx, cz), true);
            }
        }
        for (int cx = minChunkX; cx <= maxChunkX; cx++) {
            for (int cz = minChunkZ; cz <= maxChunkZ; cz++) {
                level.getChunk(cx, cz);
            }
        }
        // Block entities (moving pistons) and scheduled block ticks only run in chunks whose
        // entity sections are loaded. That load is asynchronous (region-file IO, even for an
        // empty world), so pump it here; otherwise the first ticks of a fresh sandbox would
        // silently skip block entities.
        PersistentEntitySectionManager<?> entityManager = (PersistentEntitySectionManager<?>) get(ENTITY_MANAGER, level);
        long deadline = System.nanoTime() + 30_000_000_000L;
        while (!entitiesLoaded(level, minChunkX, minChunkZ, maxChunkX, maxChunkZ)) {
            server.pumpTasks();
            level.getChunkSource().pollTask();
            entityManager.tick();
            if (System.nanoTime() > deadline) {
                throw new IllegalStateException("entity sections of sandbox " + id + " did not load");
            }
            LockSupport.parkNanos(20_000L);
        }
        level.getRandom().setSeed(seed);
        seedRandValue(level, seed);
        seedLootSequences(level, seed);
        Sandbox sb = new Sandbox(id, level, data, minChunkX, minChunkZ, maxChunkX, maxChunkZ, seed);
        sb.installDropSeeder(entityManager);
        return sb;
    }

    // ---- shulker study (clk-rt): reproducible drops ---------------------------------------------

    private static final Field ENTITY_CALLBACKS = field(PersistentEntitySectionManager.class, "callbacks");

    /** True while {@link #tick} runs the level: an entity that enters now was spawned by the game. */
    private boolean ticking;
    /** Item entities the game dropped through {@code Block.popResource} in this sandbox so far. */
    private long dropIndex;
    /** Item entities this sandbox's dispensers and droppers shot into the world so far. */
    private long dispenseIndex;
    /** Entities that entered this sandbox so far (see {@link #entityIdBase}). */
    private int entityCount;

    /**
     * The id of this sandbox's first entity: every entity that enters the sandbox (summoned,
     * spawned by the game, falling blocks, primed TNT, drops) gets {@code base + n}, n counting
     * from 0 in the order they enter (reset with the sandbox), in place of vanilla's server-wide
     * counter. The base is a multiple of 4 ({@code (tickCount + getId()) % 4} paces an item's
     * moves at rest), per sandbox (ids stay unique per level), so ids are deterministic and grow
     * with insertion order (the entity tick list's order) as redsim's do from 0; the protocol
     * reports n as {@code net_id}.
     */
    int entityIdBase() {
        return 0x40000000 + ((this.id & 0x3FF) << 20);
    }

    @SuppressWarnings("unchecked")
    private void installDropSeeder(PersistentEntitySectionManager<?> manager) {
        LevelCallback<Entity> vanilla = (LevelCallback<Entity>) get(ENTITY_CALLBACKS, manager);
        try {
            ENTITY_CALLBACKS.set(manager, new DropSeeder(vanilla));
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }

    /**
     * Determinism for entities: every entity that enters the sandbox gets its id from the
     * sandbox ({@link #entityIdBase}); and drops (the shulker study's loaders break boxes with
     * pistons). Vanilla's
     * {@code new ItemEntity(level, x, y, z, stack)} draws its yaw and its drift from the entity's
     * own random, which {@code RandomSource.create()} seeds from {@code System.nanoTime()}: the
     * same build in the same seeded sandbox drifts differently on every run. An item entity the
     * game spawns during a tick through {@code Block.popResource(Level, BlockPos, ItemStack)} (a
     * block broken with drops) gets its random reseeded to {@code seed ^ (0x6d63746173 + n)} (n:
     * the n-th such drop of this sandbox, from 0, reset with the sandbox) and ALL its draws redone
     * from it in vanilla's order: the position's offsets in its cell (popResource's three; the
     * level random still made its own three draws, so its sequence is vanilla's), the yaw, the x
     * and z drift (the constructor's). The same distribution, reproducible, and a function of the
     * seed, the drop's number and its cell only (redsim mirrors it without the level random,
     * whose draws depend on the sandbox's chunk count). {@code popResourceFromFace} and
     * dispensers set the motion from the level random afterwards and are left alone, as are
     * summoned design entities (their own path) and every other entity.
     */
    private final class DropSeeder implements LevelCallback<Entity> {
        private final LevelCallback<Entity> vanilla;

        DropSeeder(LevelCallback<Entity> vanilla) {
            this.vanilla = vanilla;
        }

        @Override
        public void onCreated(Entity e) {
            // deterministic ids for every entity, before it is tracked (the level's id lookup)
            e.setId(entityIdBase() + entityCount++);
            if (ticking && e instanceof ItemEntity item && fromPopResource()) {
                // popResource put it at the cell's center + [-0.25, 0.25) on each axis (y minus
                // half the item's height); recover the cell, then draw everything from the
                // drop's own random: offsets x, y, z (popResource's order), yaw, drift x and z
                // (the constructor's)
                double half = item.getBbHeight() / 2.0;
                BlockPos cell = BlockPos.containing(item.getX(), item.getY() + half, item.getZ());
                RandomSource r = item.getRandom();
                r.setSeed(seed ^ (0x6d63746173L + dropIndex++));
                double x = cell.getX() + 0.5 + Mth.nextDouble(r, -0.25, 0.25);
                double y = cell.getY() + 0.5 + Mth.nextDouble(r, -0.25, 0.25) - half;
                double z = cell.getZ() + 0.5 + Mth.nextDouble(r, -0.25, 0.25);
                item.setPos(x, y, z);
                item.setYRot(r.nextFloat() * 360.0F);
                item.setDeltaMovement(r.nextDouble() * 0.2 - 0.1, 0.2, r.nextDouble() * 0.2 - 0.1);
            } else if (ticking && e instanceof ItemEntity item && fromDispenser()) {
                // (clk-misc 2026-10-21, a bubble-column elevator's intake) an item a dispenser or
                // dropper shoots into the world: spawnItem already set its motion from the level
                // random; its own random (yaw, drift, later moveTowardsClosestSpace) was unseeded.
                // Reseeded to seed ^ (0x64697370 + n) (n: the n-th dispensed item of the sandbox,
                // its own count, reset with the sandbox) and the constructor's draws redone (yaw;
                // the drift's two, whose values spawnItem overwrote), which redsim mirrors
                // (World::spawn_dispensed)
                RandomSource r = item.getRandom();
                r.setSeed(seed ^ (0x64697370L + dispenseIndex++));
                item.setYRot(r.nextFloat() * 360.0F);
                r.nextDouble();
                r.nextDouble();
            }
            this.vanilla.onCreated(e);
        }

        @Override
        public void onDestroyed(Entity e) {
            this.vanilla.onDestroyed(e);
        }

        @Override
        public void onTickingStart(Entity e) {
            this.vanilla.onTickingStart(e);
        }

        @Override
        public void onTickingEnd(Entity e) {
            this.vanilla.onTickingEnd(e);
        }

        @Override
        public void onTrackingStart(Entity e) {
            this.vanilla.onTrackingStart(e);
        }

        @Override
        public void onTrackingEnd(Entity e) {
            this.vanilla.onTrackingEnd(e);
        }

        @Override
        public void onSectionChange(Entity e) {
            this.vanilla.onSectionChange(e);
        }
    }

    /** The entity being added comes from a dispenser's {@code DefaultDispenseItemBehavior.spawnItem}. */
    private static boolean fromDispenser() {
        return StackWalker.getInstance().walk(frames -> {
            for (StackWalker.StackFrame f : (Iterable<StackWalker.StackFrame>) frames::iterator) {
                if (f.getClassName().equals("net.minecraft.core.dispenser.DefaultDispenseItemBehavior") && f.getMethodName().equals("spawnItem")) {
                    return true;
                }
            }
            return false;
        });
    }

    /** The entity being added comes from {@code Block.popResource}, not {@code popResourceFromFace}. */
    private static boolean fromPopResource() {
        return StackWalker.getInstance().walk(frames -> {
            boolean pop = false;
            for (StackWalker.StackFrame f : (Iterable<StackWalker.StackFrame>) frames::iterator) {
                if (f.getClassName().equals("net.minecraft.world.level.block.Block")) {
                    if (f.getMethodName().equals("popResourceFromFace")) {
                        return false;
                    }
                    if (f.getMethodName().equals("popResource")) {
                        pop = true;
                    }
                }
            }
            return pop;
        });
    }

    // ---- end shulker study ----------------------------------------------------------------------

    private static boolean entitiesLoaded(ServerLevel level, int minChunkX, int minChunkZ, int maxChunkX, int maxChunkZ) {
        for (int cx = minChunkX; cx <= maxChunkX; cx++) {
            for (int cz = minChunkZ; cz <= maxChunkZ; cz++) {
                if (!level.areEntitiesLoaded(ChunkPos.pack(cx, cz))) {
                    return false;
                }
            }
        }
        return true;
    }

    int minX() {
        return this.minChunkX << 4;
    }

    int minZ() {
        return this.minChunkZ << 4;
    }

    int maxX() {
        return (this.maxChunkX << 4) + 15;
    }

    int maxZ() {
        return (this.maxChunkZ << 4) + 15;
    }

    BoundingBox bounds() {
        return new BoundingBox(minX(), this.level.getMinY(), minZ(), maxX(), this.level.getMaxY(), maxZ());
    }

    /**
     * One game tick of this sandbox, then the work vanilla does between two ticks: queued server
     * tasks and this level's chunk-system tasks (a real server finishes those during the 50 ms
     * gap, so they are always done before the next tick).
     */
    void tick(SimServer server) {
        // a real server's light engine has finished the updates queued before this tick
        // (between-tick edits: placements, commands): drainLight
        drainLight();
        // Where each design entity stood before this tick ({@link #still}).
        this.prevEntityPos.clear();
        for (Entity e : designEntities()) {
            this.prevEntityPos.put(e.getUUID(), e.position());
        }
        this.ticking = true;
        try {
            this.level.tick(() -> true);
        } finally {
            this.ticking = false;
        }
        // A level sends its block changes to players at the START of its next tick, while block
        // events (piston moves) go out at once. A real client never notices (the next tick is
        // 50 ms away), but a client watching a sandbox stepped slowly (tick-by-tick videos) gets a
        // piston move, finishes its own animation, then receives the late "this cell is a moving
        // block" (without the moving block's data) and draws nothing there for a whole step. Send
        // this tick's changes now. Packets only: no game state changes, and nothing at all when no
        // player is watching (the judge's runs).
        if (!this.level.players().isEmpty()) {
            try {
                BROADCAST_CHANGED_CHUNKS.invoke(this.level.getChunkSource(), Profiler.get());
            } catch (ReflectiveOperationException e) {
                throw new IllegalStateException(e);
            }
        }
        server.pumpTasks();
        while (this.level.getChunkSource().pollTask()) {
            // drain
        }
        drainLight();
    }

    // ---- the light engine (clk-misc 2026-10-22) ----

    private static final Field LIGHT_TASKS = field(ThreadedLevelLightEngine.class, "lightTasks");
    private static final Field LIGHT_SCHEDULED = field(ThreadedLevelLightEngine.class, "scheduled");
    private static final Field LIGHT_EXECUTOR = field(ThreadedLevelLightEngine.class, "consecutiveExecutor");
    private static final Field LIGHT_DISPATCHER = field(ChunkMap.class, "lightTaskDispatcher");
    private static final Field DISPATCHER_EXECUTOR = field(ChunkTaskDispatcher.class, "dispatcher");
    private static final Field EXECUTOR_STATUS = field(AbstractConsecutiveExecutor.class, "status");

    /** A consecutive executor with a task queued, scheduled on its pool or running. */
    private static boolean busy(Object executor) {
        AbstractConsecutiveExecutor<?> e = (AbstractConsecutiveExecutor<?>) executor;
        return e.size() > 0 || ((AtomicReference<?>) get(EXECUTOR_STATUS, e)).get().toString().equals("RUNNING");
    }

    /**
     * Nothing left on the light engine's way: its task dispatcher (where a block change's
     * checkBlock goes first, on a background pool) neither runs nor holds a task, the "light"
     * executor neither runs nor holds one, no update is scheduled, the engine's task list is empty
     * and it has no light work left.
     */
    private boolean lightIdle(ThreadedLevelLightEngine engine) {
        ChunkTaskDispatcher dispatcher = (ChunkTaskDispatcher) get(LIGHT_DISPATCHER, this.level.getChunkSource().chunkMap);
        if (busy(get(DISPATCHER_EXECUTOR, dispatcher)) || dispatcher.hasWork() || busy(get(LIGHT_EXECUTOR, engine))) {
            return false;
        }
        if (((AtomicBoolean) get(LIGHT_SCHEDULED, engine)).get() || !((List<?>) get(LIGHT_TASKS, engine)).isEmpty()) {
            return false;
        }
        return !engine.hasLightWork();
    }

    /**
     * Finish every queued light update now (clk-misc 2026-10-22). The level's light engine
     * ({@code ThreadedLevelLightEngine}) works off the main thread: a block change's checkBlock goes
     * through its task dispatcher onto the "light" executor's task list, and an update runs only
     * when the main thread's pollTask calls {@code tryScheduleUpdate} and finds that list or the
     * engine's pending work non-empty. A real server's 50 ms gap between ticks lets the update
     * finish; back-to-back sandbox ticks did not wait for it, so a random tick could read stale
     * light (bamboo grows only with light 9 or more above it: a flying machine shading a bamboo
     * field gave 4105, 4127 or 4174 growths from the same build in the same seed). Called before
     * and after every tick: the light is then exactly the light of the blocks as the previous tick
     * (and any edit since) left them, which redsim's light model computes.
     */
    void drainLight() {
        ThreadedLevelLightEngine engine = this.level.getChunkSource().getLightEngine();
        long deadline = System.nanoTime() + 60_000_000_000L;
        int quiet = 0;
        int spins = 0;
        while (quiet < 2) {
            engine.tryScheduleUpdate();
            if (lightIdle(engine)) {
                quiet++;
                continue;
            }
            quiet = 0;
            if (System.nanoTime() > deadline) {
                throw new IllegalStateException("the light engine did not finish its updates in 60 s");
            }
            if (++spins < 2000) {
                Thread.onSpinWait();
            } else {
                LockSupport.parkNanos(20_000L);
            }
        }
    }

    /** {@code ServerChunkCache.broadcastChangedChunks(ProfilerFiller)}: see {@link #tick}. */
    private static final Method BROADCAST_CHANGED_CHUNKS = broadcastMethod();

    private static Method broadcastMethod() {
        try {
            Method m = ServerChunkCache.class.getDeclaredMethod("broadcastChangedChunks", ProfilerFiller.class);
            m.setAccessible(true);
            return m;
        } catch (NoSuchMethodException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    long gameTime() {
        return this.level.getGameTime();
    }

    /**
     * The level random's internal state ({@code LegacyRandomSource.seed}: the 48-bit LCG state
     * after setSeed's scramble), read without advancing it: what redsim's level-random mirror is
     * checked against, tick by tick (trace-export's "rng"). Read-only (clk-misc, 2026-10-20).
     */
    long randomState() {
        Object random = this.level.getRandom();
        try {
            Field f = random.getClass().getDeclaredField("seed");
            f.setAccessible(true);
            return ((java.util.concurrent.atomic.AtomicLong) f.get(random)).get();
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException("the level random " + random.getClass().getName() + " has no seed field", e);
        }
    }

    int blockEventCount() {
        return ((Collection<?>) get(BLOCK_EVENTS, this.level)).size();
    }

    /** The pending block events, in queue order (the bedrock study reads which cells are armed). */
    java.util.List<net.minecraft.world.level.BlockEventData> blockEvents() {
        java.util.List<net.minecraft.world.level.BlockEventData> out = new java.util.ArrayList<>();
        for (Object o : (Collection<?>) get(BLOCK_EVENTS, this.level)) {
            out.add((net.minecraft.world.level.BlockEventData) o);
        }
        return out;
    }

    @SuppressWarnings("unchecked")
    int movingPistonCount() {
        int count = 0;
        for (Field f : new Field[] {BLOCK_ENTITY_TICKERS, PENDING_BLOCK_ENTITY_TICKERS}) {
            for (TickingBlockEntity ticker : (List<TickingBlockEntity>) get(f, this.level)) {
                if (!ticker.isRemoved() && ticker.getType().equals("minecraft:piston")) {
                    count++;
                }
            }
        }
        return count;
    }

    /**
     * True when nothing is pending: no scheduled block/fluid ticks, no block events, no moving
     * pistons, no falling blocks still in the air (sand/gravel/concrete powder falling is a
     * contraption still moving; it ends by landing as a block or breaking into an item), and every
     * entity of the design (summoned by {@link #summon}) at rest ({@link #still}). A design without
     * entities has none, so this is the plain block condition for it.
     */
    boolean idle() {
        return this.level.getBlockTicks().count() == steadySensorTicks()
                && this.level.getFluidTicks().count() == 0
                && blockEventCount() == 0
                && movingPistonCount() == 0
                && fallingBlockCount() == 0
                && designEntitiesStill();
    }

    private static final Method PLATE_SIGNAL_STRENGTH = method(net.minecraft.world.level.block.BasePressurePlateBlock.class, "getSignalStrength", Level.class, BlockPos.class);
    private static final Method PLATE_SIGNAL_FOR_STATE = method(net.minecraft.world.level.block.BasePressurePlateBlock.class, "getSignalForState", BlockState.class);

    /**
     * Scheduled ticks that are steady re-checks of a sensor held by a design entity at rest: a
     * powered detector rail with a minecart on it re-checks every 20 gt and only reschedules while
     * the cart stays (DetectorRailBlock.tick -> checkPressed: shouldBePressed == wasPressed, then
     * scheduleTick), and a pressed pressure plate does the same while its entities stay
     * (BasePressurePlateBlock.tick -> checkPressed with an unchanged signal). Nothing changes until
     * the entity moves, so these do not keep a sandbox busy. Only sensors pressed by the design's
     * own entities count: a sandbox without design entities has none (the plain idle rule).
     */
    int steadySensorTicks() {
        List<Entity> ents = designEntities();
        if (ents.isEmpty()) {
            return 0;
        }
        java.util.Set<BlockPos> steady = new java.util.HashSet<>();
        for (Entity e : ents) {
            net.minecraft.world.phys.AABB bb = e.getBoundingBox();
            BlockPos lo = BlockPos.containing(bb.minX, bb.minY - 1.0, bb.minZ);
            BlockPos hi = BlockPos.containing(bb.maxX, bb.maxY, bb.maxZ);
            for (BlockPos p : BlockPos.betweenClosed(lo, hi)) {
                BlockState s = this.level.getBlockState(p);
                Block b = s.getBlock();
                boolean held = false;
                if (b instanceof net.minecraft.world.level.block.DetectorRailBlock && s.getValue(net.minecraft.world.level.block.DetectorRailBlock.POWERED)) {
                    net.minecraft.world.phys.AABB search = new net.minecraft.world.phys.AABB(p.getX() + 0.2, p.getY(), p.getZ() + 0.2, p.getX() + 0.8, p.getY() + 0.8, p.getZ() + 0.8);
                    held = !this.level.getEntitiesOfClass(net.minecraft.world.entity.vehicle.minecart.AbstractMinecart.class, search, x -> x.entityTags().contains(DESIGN_TAG)).isEmpty();
                } else if (b instanceof net.minecraft.world.level.block.BasePressurePlateBlock) {
                    try {
                        int now = (int) PLATE_SIGNAL_FOR_STATE.invoke(b, s);
                        int would = (int) PLATE_SIGNAL_STRENGTH.invoke(b, this.level, p);
                        held = now > 0 && now == would;
                    } catch (ReflectiveOperationException ex) {
                        throw new IllegalStateException(ex);
                    }
                }
                if (held && this.level.getBlockTicks().hasScheduledTick(p, b)) {
                    steady.add(p.immutable());
                }
            }
        }
        return steady.size();
    }

    // ---- design entities (minecarts, armor stands, boats... declared by the design) --------------

    /** Scoreboard tag carried by every entity a design declares (the judge tells them from drops). */
    static final String DESIGN_TAG = "mctas_design";

    /** Position of every design entity before the last tick (see {@link #still}). */
    private final java.util.Map<UUID, Vec3> prevEntityPos = new java.util.HashMap<>();

    List<Entity> designEntities() {
        List<Entity> out = new ArrayList<>();
        for (Entity e : entities()) {
            if (e.entityTags().contains(DESIGN_TAG)) {
                out.add(e);
            }
        }
        return out;
    }

    /**
     * At rest: the last tick did not move it, and it has no motion left. Horizontal motion must be
     * exactly zero; vertical motion too, unless the entity stands on the ground: a living entity
     * (armor stand) gets gravity after its move every tick (LivingEntity.travelInAir: the floor
     * stops the move, then yd -= gravity), so it always carries a small downward motion while it
     * rests on a block. A minecart on a rail ends every tick with motion (0, 0, 0)
     * (OldMinecartBehavior.moveAlongTrack: applyNaturalSlowdown zeroes y) once it has stopped; a
     * rolling cart only slows by 0.96 per tick and never reaches zero by itself, so it stops for
     * good only on an unpowered powered rail (haltTrack: speed below 0.03 is set to zero) or
     * against a block (Entity.move zeroes the colliding component).
     */
    boolean still(Entity e) {
        Vec3 prev = this.prevEntityPos.get(e.getUUID());
        Vec3 p = e.position();
        Vec3 m = e.getDeltaMovement();
        return prev != null && prev.x == p.x && prev.y == p.y && prev.z == p.z && m.x == 0.0 && m.z == 0.0 && (m.y == 0.0 || e.onGround());
    }

    boolean designEntitiesStill() {
        for (Entity e : designEntities()) {
            if (!still(e)) {
                return false;
            }
        }
        return true;
    }

    /**
     * Summon a design's entities, deterministically: the entity's own random (Entity.random,
     * seeded from the clock by RandomSource.create) is reseeded and its UUID fixed by its index in
     * the design, so every run is the same. Mobs are refused (their AI is random). Each carries
     * {@link #DESIGN_TAG}.
     */
    /**
     * A block-attached entity (an item frame, a painting) loads its cell from {@code block_pos},
     * which must lie within 16 blocks of its {@code Pos} (`BlockAttachedEntity.readAdditionalSaveData`),
     * and its facing is read right after it: give both from the summon position, else the cell is
     * missing when the facing recomputes its box. Other entities are untouched.
     */
    static void attachAt(net.minecraft.nbt.CompoundTag tag, String type, double x, double y, double z) {
        if (!type.endsWith("item_frame") && !type.endsWith("painting")) {
            return;
        }
        net.minecraft.nbt.ListTag pos = new net.minecraft.nbt.ListTag();
        pos.add(net.minecraft.nbt.DoubleTag.valueOf(x));
        pos.add(net.minecraft.nbt.DoubleTag.valueOf(y));
        pos.add(net.minecraft.nbt.DoubleTag.valueOf(z));
        tag.put("Pos", pos);
        tag.putIntArray("block_pos", new int[] {(int) Math.floor(x), (int) Math.floor(y), (int) Math.floor(z)});
    }

    List<Entity> summon(com.google.gson.JsonArray specs) throws com.mojang.brigadier.exceptions.CommandSyntaxException {
        List<Entity> out = new ArrayList<>();
        // Numbered after the design entities already there (a drive script can summon more); a
        // number whose UUID one of them still holds is skipped (after a design entity left, e.g.
        // an item a hopper took from the sorter judge's water stream, the count reaches the number
        // of one still there).
        java.util.Set<UUID> taken = new java.util.HashSet<>();
        for (Entity d : designEntities()) {
            taken.add(d.getUUID());
        }
        int i = taken.size();
        for (com.google.gson.JsonElement el : specs) {
            com.google.gson.JsonObject s = el.getAsJsonObject();
            String type = s.get("type").getAsString();
            com.google.gson.JsonArray p = s.getAsJsonArray("pos");
            double x = p.get(0).getAsDouble();
            double y = p.get(1).getAsDouble();
            double z = p.get(2).getAsDouble();
            net.minecraft.nbt.CompoundTag tag = s.has("nbt") && !s.get("nbt").isJsonNull()
                    ? net.minecraft.nbt.TagParser.parseCompoundFully(s.get("nbt").getAsString())
                    : new net.minecraft.nbt.CompoundTag();
            tag.putString("id", type);
            attachAt(tag, type, x, y, z);
            Entity e = net.minecraft.world.entity.EntityType.loadEntityRecursive(tag, this.level,
                    new net.minecraft.world.entity.EntitySpawnRequest(net.minecraft.world.entity.EntitySpawnReason.COMMAND, false), en -> {
                        en.snapTo(x, y, z, en.getYRot(), en.getXRot());
                        return en;
                    });
            if (e == null) {
                throw new IllegalArgumentException("cannot summon " + type);
            }
            if (e instanceof net.minecraft.world.entity.Mob) {
                throw new IllegalArgumentException(type + " is a mob: its AI is random, mobs are not allowed");
            }
            if (!e.getPassengers().isEmpty()) {
                throw new IllegalArgumentException(type + ": passengers are not allowed");
            }
            UUID uuid = UUID.nameUUIDFromBytes(("mctas-entity-" + i).getBytes(java.nio.charset.StandardCharsets.UTF_8));
            while (taken.contains(uuid)) {
                i++;
                uuid = UUID.nameUUIDFromBytes(("mctas-entity-" + i).getBytes(java.nio.charset.StandardCharsets.UTF_8));
            }
            taken.add(uuid);
            e.setUUID(uuid);
            e.getRandom().setSeed(0x6d63746173L + i);
            e.addTag(DESIGN_TAG);
            if (!this.level.tryAddFreshEntityWithPassengers(e)) {
                throw new IllegalArgumentException("could not add " + type + " (duplicate UUID?)");
            }
            // a player putting the item in a frame updates the comparators reading it
            // (`ItemFrame.setItem` -> `updateNeighbourForOutputSignal`); a summon does not
            if (e instanceof net.minecraft.world.entity.decoration.ItemFrame frame) {
                this.level.updateNeighbourForOutputSignal(frame.getPos(), net.minecraft.world.level.block.Blocks.AIR);
            }
            out.add(e);
            i++;
        }
        return out;
    }

    /**
     * Collision for the walkway rule: an entity a player bumps into or shoves (boats, shulkers:
     * canBeCollidedWith; minecarts: isPushable). Armor stands, item frames and paintings neither
     * block nor get pushed, like dust or a torch.
     */
    static boolean collides(Entity e) {
        return e.canBeCollidedWith(null) || e.isPushable();
    }

    /**
     * The entity's state apart from its position and motion, for "back exactly as it was": its saved
     * data (contents, fuel, pose, hopper enabled, fuse, item frame item...) with the volatile
     * fields removed, keys sorted, plus the rail a minecart is on. A minecart's yaw is left out: the
     * cart sets it from its direction of travel and flips it (OldMinecartBehavior.tick), so it says
     * which way it last rolled, nothing more.
     */
    String keyState(Entity e) {
        net.minecraft.world.level.storage.TagValueOutput out = net.minecraft.world.level.storage.TagValueOutput.createWithContext(
                net.minecraft.util.ProblemReporter.DISCARDING, this.level.registryAccess());
        e.saveWithoutId(out);
        net.minecraft.nbt.CompoundTag t = out.buildResult();
        for (String k : new String[] {"Pos", "Motion", "UUID", "fall_distance", "FallDistance", "OnGround", "Fire", "Air", "PortalCooldown",
                "invulnerable_time", "TicksFrozen", "Tags", "HurtTime", "HurtByTimestamp", "DeathTime"}) {
            t.remove(k);
        }
        if (e instanceof net.minecraft.world.entity.vehicle.minecart.AbstractMinecart m) {
            t.remove("Rotation");
            t.remove("FlippedRotation");
            t.remove("HasTicked");
            BlockPos rail = m.getCurrentBlockPosOrRailBelow();
            boolean on = net.minecraft.world.level.block.BaseRailBlock.isRail(this.level.getBlockState(rail));
            t.putString("mctas_rail", on ? rail.getX() + "," + rail.getY() + "," + rail.getZ() : "none");
        }
        return new net.minecraft.nbt.SnbtPrinterTagVisitor().visit(t).replaceAll("\\s*\\n\\s*", " ").trim();
    }

    int fallingBlockCount() {
        int count = 0;
        for (Entity e : this.level.getAllEntities()) {
            if (e instanceof FallingBlockEntity && !e.isRemoved()) {
                count++;
            }
        }
        return count;
    }

    List<Entity> entities() {
        List<Entity> out = new ArrayList<>();
        for (Entity e : this.level.getAllEntities()) {
            if (e != null && !e.isRemoved()) {
                out.add(e);
            }
        }
        return out;
    }

    /** Back to an empty void: no blocks, entities, pending ticks or block events; time 0; reseeded. */
    /**
     * Redstone torch burnout history is a static map keyed by level (RedstoneTorchBlock
     * .RECENT_TOGGLES). clear() resets this level's game time, so entries stamped later than the
     * new time would never expire and would fake burnouts in the next run: forget them.
     */
    private void forgetTorchToggles() {
        try {
            java.lang.reflect.Field f = net.minecraft.world.level.block.RedstoneTorchBlock.class.getDeclaredField("RECENT_TOGGLES");
            f.setAccessible(true);
            ((java.util.Map<?, ?>) f.get(null)).remove(this.level);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }

    void clear() {
        forgetTorchToggles();
        for (Entity e : entities()) {
            e.discard();
        }
        this.prevEntityPos.clear();
        BoundingBox bb = bounds();
        this.level.clearBlockEvents(bb);
        this.level.getBlockTicks().clearArea(bb);
        this.level.getFluidTicks().clearArea(bb);
        BlockState air = Blocks.AIR.defaultBlockState();
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        // Include the ring of neighbor chunks: a contraption can push blocks one chunk out.
        for (int cx = this.minChunkX - 1; cx <= this.maxChunkX + 1; cx++) {
            for (int cz = this.minChunkZ - 1; cz <= this.maxChunkZ + 1; cz++) {
                LevelChunk chunk = this.level.getChunkSource().getChunkNow(cx, cz);
                if (chunk == null) {
                    continue;
                }
                LevelChunkSection[] sections = chunk.getSections();
                for (int si = 0; si < sections.length; si++) {
                    LevelChunkSection section = sections[si];
                    if (section.hasOnlyAir()) {
                        continue;
                    }
                    int baseY = chunk.getSectionYFromSectionIndex(si) << 4;
                    for (int y = 0; y < 16; y++) {
                        for (int z = 0; z < 16; z++) {
                            for (int x = 0; x < 16; x++) {
                                if (!section.getBlockState(x, y, z).isAir()) {
                                    pos.set((cx << 4) + x, baseY + y, (cz << 4) + z);
                                    this.level.setBlock(pos, air, RAW_FLAGS);
                                }
                            }
                        }
                    }
                }
            }
        }
        this.data.setGameTime(0L);
        this.level.getRandom().setSeed(this.seed);
        seedRandValue(this.level, this.seed);
        seedLootSequences(this.level, this.seed);
        this.dropIndex = 0;
        this.dispenseIndex = 0;
        this.entityCount = 0;
    }

    /**
     * Save, unload and load again every chunk of the sandbox, without ticking the world: a player
     * walking away and back, or a server restart (coordinator 2026-10-20: a clock must survive a
     * chunk reload). What the chunk data keeps (blocks, block entities, scheduled ticks, entities)
     * comes back; what it does not (pending block events, the order of ticks due together) is lost,
     * as in the game. The sandbox saves only while this runs.
     */
    void reload(SimServer server) {
        this.level.noSave = false;
        try {
            for (int cx = this.minChunkX; cx <= this.maxChunkX; cx++) {
                for (int cz = this.minChunkZ; cz <= this.maxChunkZ; cz++) {
                    this.level.setChunkForced(cx, cz, false);
                }
            }
            PersistentEntitySectionManager<?> entityManager = (PersistentEntitySectionManager<?>) get(ENTITY_MANAGER, this.level);
            long deadline = System.nanoTime() + 60_000_000_000L;
            while (anyLoaded()) {
                server.pumpTasks();
                this.level.getChunkSource().tick(() -> true, false);
                this.level.getChunkSource().pollTask();
                entityManager.tick();
                if (System.nanoTime() > deadline) {
                    throw new IllegalStateException("chunks of sandbox " + this.id + " did not unload");
                }
                LockSupport.parkNanos(20_000L);
            }
            // every ticket queued before any runs (see create): the chunk tick order comes back the same
            for (int cx = this.minChunkX; cx <= this.maxChunkX; cx++) {
                for (int cz = this.minChunkZ; cz <= this.maxChunkZ; cz++) {
                    this.level.getChunkSource().updateChunkForced(new ChunkPos(cx, cz), true);
                }
            }
            deadline = System.nanoTime() + 60_000_000_000L;
            while (!entitiesLoaded(this.level, this.minChunkX, this.minChunkZ, this.maxChunkX, this.maxChunkZ) || !allChunks()) {
                server.pumpTasks();
                this.level.getChunkSource().tick(() -> true, false);
                this.level.getChunkSource().pollTask();
                entityManager.tick();
                if (System.nanoTime() > deadline) {
                    throw new IllegalStateException("chunks of sandbox " + this.id + " did not load again");
                }
                LockSupport.parkNanos(20_000L);
            }
        } finally {
            this.level.noSave = true;
        }
    }

    private boolean anyLoaded() {
        for (int cx = this.minChunkX; cx <= this.maxChunkX; cx++) {
            for (int cz = this.minChunkZ; cz <= this.maxChunkZ; cz++) {
                if (this.level.getChunkSource().hasChunk(cx, cz) || this.level.areEntitiesLoaded(ChunkPos.pack(cx, cz))) {
                    return true;
                }
            }
        }
        return false;
    }

    private boolean allChunks() {
        for (int cx = this.minChunkX; cx <= this.maxChunkX; cx++) {
            for (int cz = this.minChunkZ; cz <= this.maxChunkZ; cz++) {
                if (!this.level.getChunkSource().hasChunk(cx, cz)) {
                    return false;
                }
            }
        }
        return true;
    }

    void drop(SimServer server) {
        clear();
        for (int cx = this.minChunkX; cx <= this.maxChunkX; cx++) {
            for (int cz = this.minChunkZ; cz <= this.maxChunkZ; cz++) {
                this.level.setChunkForced(cx, cz, false);
            }
        }
        server.unregisterLevel(this.level);
        try {
            this.level.close();
        } catch (IOException e) {
            throw new IllegalStateException(e);
        }
    }

    /** Interact with a block the way a player's right click would (levers and buttons directly). */
    String use(BlockPos pos, Direction face) {
        BlockState state = this.level.getBlockState(pos);
        Block block = state.getBlock();
        if (block instanceof LeverBlock lever) {
            lever.pull(state, this.level, pos, null);
            return "lever";
        }
        if (block instanceof ButtonBlock button) {
            button.press(state, this.level, pos, null);
            return "button";
        }
        BlockHitResult hit = new BlockHitResult(Vec3.atCenterOf(pos), face, pos, false);
        InteractionResult result = state.useWithoutItem(this.level, mockPlayer(), hit);
        return result.toString();
    }

    /**
     * Right-click the item frame in this block cell the way a player with an empty hand does: a
     * frame holding an item turns it one step (`ItemFrame.interact` -> `setRotation(r + 1)`, which
     * updates the comparators reading it; `/data` would not). Exactly one frame must be there.
     */
    String useEntity(BlockPos pos) {
        List<Entity> found = this.level.getEntities((Entity) null, new net.minecraft.world.phys.AABB(pos),
                e -> e instanceof net.minecraft.world.entity.decoration.ItemFrame);
        if (found.size() != 1) {
            List<Entity> near = this.level.getEntities((Entity) null, new net.minecraft.world.phys.AABB(pos).inflate(64),
                    e -> e instanceof net.minecraft.world.entity.decoration.ItemFrame || e instanceof ItemEntity);
            StringBuilder where = new StringBuilder();
            for (Entity n : near) {
                where.append(" ").append(n.getType().toShortString()).append("@").append(n.blockPosition().toShortString())
                        .append(" facing ").append(n.getDirection());
            }
            throw new IllegalArgumentException("use_entity: " + found.size() + " item frames in " + pos.toShortString() + ", expected 1 (frames and items within 64:" + where + ")");
        }
        Entity e = found.get(0);
        InteractionResult result = e.interact(mockPlayer(), net.minecraft.world.InteractionHand.MAIN_HAND, e.position());
        return result.toString();
    }

    // ---- survival building (op "build") -------------------------------------------------------

    private static final java.lang.reflect.Method GET_PLACEMENT_STATE = method(
            net.minecraft.world.item.BlockItem.class, "getPlacementState", net.minecraft.world.item.context.BlockPlaceContext.class);
    private static final float[] PITCHES = {0F, 90F, -90F, 30F, -30F, 60F, -60F};
    /** Cardinal look directions first, then 20 degrees off each (a face seen at an angle). */
    private static final float[] YAWS = {0F, 90F, 180F, 270F, 20F, 340F, 70F, 110F, 160F, 200F, 250F, 290F};

    /**
     * Properties the world computes (power, connections, piston extension...): a player placing the
     * block cannot pick them, they follow from the neighbours.
     */
    static boolean dynamicProperty(BlockState state, String name) {
        Block b = state.getBlock();
        return switch (name) {
            case "power", "extended", "lit", "locked", "triggered", "enabled", "sculk_sensor_phase", "instrument", "short",
                    "attached", "disarmed", "in_wall", "has_book", "occupied", "unstable", "bottom", "distance" -> true;
            case "powered" -> !(b instanceof LeverBlock);
            case "north", "south", "east", "west", "up" -> b instanceof net.minecraft.world.level.block.RedstoneWireBlock
                    || b instanceof net.minecraft.world.level.block.CrossCollisionBlock
                    || b instanceof net.minecraft.world.level.block.WallBlock;
            default -> false;
        };
    }

    /** Properties a player sets by right-clicking the placed block (repeater delay, lever...). */
    static boolean usableProperty(BlockState state, String name) {
        Block b = state.getBlock();
        return (name.equals("delay") && b instanceof net.minecraft.world.level.block.RepeaterBlock)
                || (name.equals("mode") && b instanceof net.minecraft.world.level.block.ComparatorBlock)
                || (name.equals("powered") && b instanceof LeverBlock)
                || (name.equals("note") && b instanceof net.minecraft.world.level.block.NoteBlock)
                || (name.equals("inverted") && b instanceof net.minecraft.world.level.block.DaylightDetectorBlock)
                || name.equals("open");
    }

    /**
     * Can a player looking along (yaw, pitch) click `face` (the face of the support block toward
     * `pos`) at `hit`? The view ray must travel into the face, and it reaches it through the
     * target cell: the cell it comes from (where the ray enters `pos`, walking back from the hit)
     * must be open, or the face is out of sight.
     */
    private boolean visible(BlockPos pos, Direction face, Vec3 hit, float yaw, float pitch) {
        double yr = Math.toRadians(yaw);
        double pr = Math.toRadians(pitch);
        double[] v = {-Math.sin(yr) * Math.cos(pr), -Math.sin(pr), Math.cos(yr) * Math.cos(pr)};
        if (v[0] * face.getStepX() + v[1] * face.getStepY() + v[2] * face.getStepZ() >= -1e-6) {
            return false;
        }
        double[] h = {hit.x, hit.y, hit.z};
        int[] lo = {pos.getX(), pos.getY(), pos.getZ()};
        double best = Double.MAX_VALUE;
        int axis = -1;
        int sign = 0;
        for (int a = 0; a < 3; a++) {
            double d = -v[a];
            if (Math.abs(d) < 1e-9) {
                continue;
            }
            double t = ((d > 0 ? lo[a] + 1 : lo[a]) - h[a]) / d;
            if (t > 1e-9 && t < best) {
                best = t;
                axis = a;
                sign = d > 0 ? 1 : -1;
            }
        }
        if (axis < 0) {
            return false;
        }
        BlockPos entry = pos.offset(axis == 0 ? sign : 0, axis == 1 ? sign : 0, axis == 2 ? sign : 0);
        BlockState s = this.level.getBlockState(entry);
        return s.isAir() || s.getCollisionShape(this.level, entry).isEmpty();
    }

    /** Blocks whose right-click does something (a player sneaks to place against them). */
    static boolean interactive(BlockState s) {
        Block b = s.getBlock();
        return b instanceof LeverBlock || b instanceof ButtonBlock || b instanceof net.minecraft.world.level.block.DiodeBlock
                || b instanceof net.minecraft.world.level.block.NoteBlock || b instanceof net.minecraft.world.level.block.DoorBlock
                || b instanceof net.minecraft.world.level.block.TrapDoorBlock || b instanceof net.minecraft.world.level.block.FenceGateBlock
                || b instanceof net.minecraft.world.level.block.DaylightDetectorBlock || b instanceof net.minecraft.world.level.block.RedstoneWireBlock
                || b instanceof net.minecraft.world.level.block.BaseEntityBlock && !(b instanceof net.minecraft.world.level.block.piston.MovingPistonBlock)
                        && !(b instanceof net.minecraft.world.level.block.SculkSensorBlock);
    }

    /** How many of `target`'s player-chosen properties (facing, face, half...) `got` has; -1 if another block. */
    private static int placementScore(BlockState got, BlockState target) {
        if (got == null || got.getBlock() != target.getBlock()) {
            return -1;
        }
        int score = 0;
        for (net.minecraft.world.level.block.state.properties.Property<?> prop : target.getProperties()) {
            String name = prop.getName();
            if (!dynamicProperty(target, name) && !usableProperty(target, name) && got.getValue(prop).equals(target.getValue(prop))) {
                score++;
            }
        }
        return score;
    }

    private static int placementWanted(BlockState target) {
        int n = 0;
        for (net.minecraft.world.level.block.state.properties.Property<?> prop : target.getProperties()) {
            if (!dynamicProperty(target, prop.getName()) && !usableProperty(target, prop.getName())) {
                n++;
            }
        }
        return n;
    }

    /** What the item would place for this click (null when the click cannot place at `pos`). */
    private BlockState trialPlacement(net.minecraft.world.item.BlockItem item, net.minecraft.world.item.ItemStack stack, BlockPos pos, BlockHitResult hit) {
        net.minecraft.world.item.context.BlockPlaceContext ctx = new net.minecraft.world.item.context.BlockPlaceContext(
                mockPlayer(), net.minecraft.world.InteractionHand.MAIN_HAND, stack, hit);
        if (!ctx.canPlace() || !ctx.getClickedPos().equals(pos)) {
            return null;
        }
        net.minecraft.world.item.context.BlockPlaceContext updated = item.updatePlacementContext(ctx);
        if (updated == null || !updated.getClickedPos().equals(pos)) {
            return null;
        }
        try {
            return (BlockState) GET_PLACEMENT_STATE.invoke(item, updated);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }

    /** One way to click: the hit, the look direction and sneaking. */
    private record Click(BlockHitResult hit, float yaw, float pitch, boolean sneak, boolean supported) {
        String describe() {
            String compass = Direction.fromYRot(yaw).getSerializedName();
            float off = net.minecraft.util.Mth.wrapDegrees(yaw - Direction.fromYRot(yaw).toYRot());
            if (Math.abs(off) > 1F) {
                compass += " (" + (int) Math.abs(off) + " degrees toward " + Direction.fromYRot(Direction.fromYRot(yaw).toYRot() + Math.signum(off) * 90F).getSerializedName() + ")";
            }
            String look = pitch >= 89F ? "looking down" : pitch <= -89F ? "looking up" : "facing " + compass
                    + (pitch > 0F ? " looking down " + (int) pitch : pitch < 0F ? " looking up " + (int) -pitch : "");
            BlockPos s = hit.getBlockPos();
            if (!supported) {
                return "put a temporary dirt block at " + s.getX() + " " + s.getY() + " " + s.getZ() + ", click its "
                        + hit.getDirection().getSerializedName() + " face, " + look + ", then break the dirt";
            }
            return "click the " + hit.getDirection().getSerializedName() + " face of " + s.getX() + " " + s.getY() + " " + s.getZ() + ", " + look
                    + (sneak ? ", sneaking" : "");
        }
    }

    private void aim(Click c) {
        Player p = mockPlayer();
        p.setShiftKeyDown(c.sneak());
        p.setYRot(c.yaw());
        p.setYHeadRot(c.yaw());
        p.setXRot(c.pitch());
    }

    /**
     * Place `target`'s block at `pos` like a survival player: try every click on a neighbouring
     * block's face (look direction, sneak; mid-air only when no click fits), keep the one whose
     * placement state has `target`'s player-chosen properties, run the real BlockItem.place
     * (getStateForPlacement, setBlock flags 11, setPlacedBy, the place game event), then right-click
     * the block until its click-set properties (repeater delay, lever...) match, settling between
     * clicks. "ok" is false when the block cannot be placed that way here.
     */
    com.google.gson.JsonObject placeLikePlayer(SimServer server, BlockPos pos, BlockState target, java.util.function.Function<BlockState, String> ser, int settle) {
        com.google.gson.JsonObject o = new com.google.gson.JsonObject();
        net.minecraft.world.item.Item it = target.getBlock().asItem();
        if (!(it instanceof net.minecraft.world.item.BlockItem item)) {
            o.addProperty("ok", false);
            o.addProperty("error", "no item places " + ser.apply(target));
            return o;
        }
        net.minecraft.world.item.ItemStack stack = new net.minecraft.world.item.ItemStack(item);
        Player player = mockPlayer();
        player.setItemInHand(net.minecraft.world.InteractionHand.MAIN_HAND, stack);
        boolean halves = target.getProperties().stream().anyMatch(p -> p.getName().equals("half") || p.getName().equals("type"));
        double[] hitYs = halves ? new double[] {0.25, 0.75} : new double[] {0.5};
        int wanted = placementWanted(target);
        int bestScore = -1;
        boolean scaffolded = false;
        Click best = null;
        search:
        for (int pass = 0; pass < 1; pass++) {
            for (Direction face : Direction.values()) {
                BlockPos support = pos.relative(face.getOpposite());
                BlockState supportState = this.level.getBlockState(support);
                if (supportState.isAir()) {
                    continue;
                }
                // Right-clicking a lever, repeater, dust... would use it instead of placing: sneak.
                boolean sneak = interactive(supportState);
                for (double hy : hitYs) {
                    Vec3 loc = Vec3.atCenterOf(support).add(face.getStepX() * 0.5,
                            face.getAxis() == Direction.Axis.Y ? face.getStepY() * 0.5 : hy - 0.5, face.getStepZ() * 0.5);
                    BlockHitResult hit = new BlockHitResult(loc, face, support, false);
                    for (float pitch : PITCHES) {
                        for (float yaw : YAWS) {
                            if (!visible(pos, face, loc, yaw, pitch)) {
                                continue;
                            }
                            Click c = new Click(hit, yaw, pitch, sneak, true);
                            aim(c);
                            int s = placementScore(trialPlacement(item, stack, pos, hit), target);
                            if (s > bestScore) {
                                bestScore = s;
                                best = c;
                                if (s == wanted) {
                                    break search;
                                }
                            }
                        }
                    }
                }
            }
        }
        // A temporary block only helps blocks that stand on their own: a repeater, dust, lever or
        // torch would pop off when the temporary block under it is broken.
        boolean standsAlone = target.canSurvive(this.level, pos);
        if (bestScore < wanted && this.level.getBlockState(pos).isAir() && standsAlone) {
            // No neighbouring block to click that gives this orientation (in the void the
            // sandbox is): put a temporary block next to it like a player would, click that,
            // break it afterwards. The temporary block's placement and removal update the
            // world like any other block.
            java.util.List<BlockPos> around = new java.util.ArrayList<>();
            for (BlockPos q : new BlockPos[] {pos.below(), pos.north(), pos.south(), pos.west(), pos.east(), pos.above()}) {
                if (this.level.getBlockState(q).isAir() && q.getY() > this.level.getMinY()) {
                    around.add(q);
                }
            }
            for (BlockPos q : around) {
                // Try the clicks against a dirt block pasted silently, so a dead end leaves no
                // trace; only the chosen one is built for real.
                BlockState dirt = Blocks.DIRT.defaultBlockState();
                this.level.setBlock(q, dirt, RAW_FLAGS);
                Direction face = Direction.getApproximateNearest((double) (pos.getX() - q.getX()), (double) (pos.getY() - q.getY()), (double) (pos.getZ() - q.getZ()));
                Click sClick = null;
                scaffoldSearch:
                for (double hy : hitYs) {
                    Vec3 loc = Vec3.atCenterOf(q).add(face.getStepX() * 0.5,
                            face.getAxis() == Direction.Axis.Y ? face.getStepY() * 0.5 : hy - 0.5, face.getStepZ() * 0.5);
                    BlockHitResult hit = new BlockHitResult(loc, face, q, false);
                    for (float pitch : PITCHES) {
                        for (float yaw : YAWS) {
                            if (!visible(pos, face, loc, yaw, pitch)) {
                                continue;
                            }
                            Click c = new Click(hit, yaw, pitch, false, false);
                            aim(c);
                            if (placementScore(trialPlacement(item, stack, pos, hit), target) == wanted) {
                                sClick = c;
                                break scaffoldSearch;
                            }
                        }
                    }
                }
                this.level.setBlock(q, Blocks.AIR.defaultBlockState(), RAW_FLAGS);
                if (sClick == null) {
                    continue;
                }
                this.level.setBlock(q, dirt, 11);
                this.level.gameEvent(net.minecraft.world.level.gameevent.GameEvent.BLOCK_PLACE, q,
                        net.minecraft.world.level.gameevent.GameEvent.Context.of(player, dirt));
                int dirtTicks = 0;
                for (int t = 0; t < settle && (t < 4 || !idle()); t++) {
                    tick(server);
                    dirtTicks++;
                }
                bestScore = wanted;
                best = sClick;
                aim(best);
                item.place(new net.minecraft.world.item.context.BlockPlaceContext(player, net.minecraft.world.InteractionHand.MAIN_HAND, stack, best.hit()));
                int placedTicks = 0;
                for (int t = 0; t < settle && (t < 4 || !idle()); t++) {
                    tick(server);
                    placedTicks++;
                }
                breakLikePlayer(q);
                o.addProperty("scaffold", q.getX() + " " + q.getY() + " " + q.getZ());
                // Ticks after the dirt went in and after the block went in (for exact replays).
                com.google.gson.JsonArray scaffoldTicks = new com.google.gson.JsonArray();
                scaffoldTicks.add(dirtTicks);
                scaffoldTicks.add(placedTicks);
                o.add("scaffold_ticks", scaffoldTicks);
                scaffolded = true;
                break;
            }
        }
        if (best == null || bestScore < wanted) {
            // A player would not place it wrong: the step fails, nothing is placed.
            player.setShiftKeyDown(false);
            player.setItemInHand(net.minecraft.world.InteractionHand.MAIN_HAND, net.minecraft.world.item.ItemStack.EMPTY);
            o.addProperty("ok", false);
            o.addProperty("error", !this.level.getBlockState(pos).isAir() ? "cannot place at " + pos.toShortString() + ": occupied by " + ser.apply(this.level.getBlockState(pos))
                    : !standsAlone ? "cannot place " + ser.apply(target) + " yet: nothing for it to stand on"
                    : "cannot place " + ser.apply(target) + " now: no visible face gives that orientation");
            return o;
        }
        if (!scaffolded) {
            aim(best);
            item.place(new net.minecraft.world.item.context.BlockPlaceContext(player, net.minecraft.world.InteractionHand.MAIN_HAND, stack, best.hit()));
        }
        player.setShiftKeyDown(false);
        player.setItemInHand(net.minecraft.world.InteractionHand.MAIN_HAND, net.minecraft.world.item.ItemStack.EMPTY);
        int clicks = 0;
        com.google.gson.JsonArray clickTicks = new com.google.gson.JsonArray();
        for (net.minecraft.world.level.block.state.properties.Property<?> prop : target.getProperties()) {
            if (!usableProperty(target, prop.getName())) {
                continue;
            }
            for (int i = 0; i < 25; i++) {
                BlockState now = this.level.getBlockState(pos);
                if (now.getBlock() != target.getBlock() || now.getValue(prop).equals(target.getValue(prop))) {
                    break;
                }
                // let the previous placement or click settle, then click again
                int waited = 0;
                for (int t = 0; t < settle && !idle(); t++) {
                    tick(server);
                    waited++;
                }
                use(pos, Direction.UP);
                clicks++;
                clickTicks.add(waited);
            }
        }
        // The exact click (look direction, hit point, sneaking) and the ticks before each
        // right-click, so another world (vanilla-build) can replay the very same inputs.
        o.addProperty("yaw", best.yaw());
        o.addProperty("pitch", best.pitch());
        o.addProperty("sneak", best.sneak());
        BlockPos hitPos = best.hit().getBlockPos();
        com.google.gson.JsonArray hitPosJson = new com.google.gson.JsonArray();
        hitPosJson.add(hitPos.getX());
        hitPosJson.add(hitPos.getY());
        hitPosJson.add(hitPos.getZ());
        o.add("hit_pos", hitPosJson);
        o.addProperty("hit_face", best.hit().getDirection().getSerializedName());
        Vec3 hitLoc = best.hit().getLocation();
        com.google.gson.JsonArray hitLocJson = new com.google.gson.JsonArray();
        hitLocJson.add(hitLoc.x);
        hitLocJson.add(hitLoc.y);
        hitLocJson.add(hitLoc.z);
        o.add("hit_loc", hitLocJson);
        o.add("click_ticks", clickTicks);
        BlockState placed = this.level.getBlockState(pos);
        boolean ok = bestScore == wanted && placed.getBlock() == target.getBlock();
        for (net.minecraft.world.level.block.state.properties.Property<?> prop : target.getProperties()) {
            if (usableProperty(target, prop.getName()) && placed.getBlock() == target.getBlock() && !placed.getValue(prop).equals(target.getValue(prop))) {
                ok = false;
            }
        }
        o.addProperty("ok", ok);
        o.addProperty("placed", ser.apply(placed));
        o.addProperty("supported", best.supported());
        o.addProperty("click", best.describe());
        o.addProperty("clicks", clicks);
        if (!ok) {
            o.addProperty("error", "placed " + ser.apply(placed) + ", wanted " + ser.apply(target));
        }
        return o;
    }

    /** Break a block like a survival player (no drops: the player picks them up). */
    void breakLikePlayer(BlockPos pos) {
        BlockState state = this.level.getBlockState(pos);
        if (state.isAir()) {
            return;
        }
        BlockState adjusted = state.getBlock().playerWillDestroy(this.level, pos, state, mockPlayer());
        if (this.level.removeBlock(pos, false)) {
            state.getBlock().destroy(this.level, pos, adjusted);
        }
    }

    private static java.lang.reflect.Method method(Class<?> owner, String name, Class<?>... params) {
        try {
            java.lang.reflect.Method m = owner.getDeclaredMethod(name, params);
            m.setAccessible(true);
            return m;
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException("Missing method " + owner.getName() + "." + name, e);
        }
    }

    private Player mockPlayer() {
        if (this.mockPlayer == null) {
            this.mockPlayer = new Player(this.level, new GameProfile(UUID.nameUUIDFromBytes(("mctas" + this.id).getBytes()), "mctas")) {
                @Override
                public GameType gameMode() {
                    return GameType.CREATIVE;
                }

                @Override
                public boolean isClientAuthoritative() {
                    return false;
                }
            };
        }
        return this.mockPlayer;
    }

    private static Field field(Class<?> owner, String name) {
        try {
            Field f = owner.getDeclaredField(name);
            f.setAccessible(true);
            return f;
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException("Missing field " + owner.getName() + "." + name, e);
        }
    }

    private static Object get(Field f, Object target) {
        try {
            return f.get(target);
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }
}
