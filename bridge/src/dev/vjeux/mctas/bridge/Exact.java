package dev.vjeux.mctas.bridge;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicLong;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.util.Mth;
import net.minecraft.util.RandomSource;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LightningBolt;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.ai.Brain;
import net.minecraft.world.entity.ai.behavior.BehaviorControl;
import net.minecraft.world.entity.ai.behavior.BlockPosTracker;
import net.minecraft.world.entity.ai.behavior.EntityTracker;
import net.minecraft.world.entity.ai.behavior.GateBehavior;
import net.minecraft.world.entity.ai.behavior.PositionTracker;
import net.minecraft.world.entity.ai.behavior.ShufflingList;
import net.minecraft.world.entity.ai.goal.WrappedGoal;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.ai.memory.NearestVisibleLivingEntities;
import net.minecraft.world.entity.ai.memory.WalkTarget;
import net.minecraft.world.entity.ai.sensing.Sensor;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.schedule.Activity;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.RandomSupport;
import net.minecraft.world.level.pathfinder.Path;
import net.minecraft.world.phys.Vec3;

/**
 * Tick-exact mob worlds (track tickexact, 2026-11-08; docs/mechanics.md "The bridge's seeds"):
 * every random a mob, spawn, weather or fire path draws from is seeded from the sandbox seed, so
 * the same build in the same seed gives the same run, and redsim mirrors each stream draw for draw.
 *
 * <p>Vanilla seeds {@code Entity.random} and every {@code ShufflingList.random} (the shuffles of
 * {@code GateBehavior}/{@code RunOne}) with {@code RandomSupport.generateUniqueSeed()} (a static
 * uniquifier ^ {@code System.nanoTime()}). The bridge reseeds them when the entity enters its
 * sandbox ({@link #seedNew}), before it is tracked or ticked, and redoes the constructor's draws in
 * the game's order ({@link #replayConstructor}) so every field they set has the distribution of the
 * game's own draws. The level random, {@code randValue} and the loot sequences are seeded by
 * {@link Sandbox} (since 2026-10-21); this class adds:
 *
 * <ul>
 *   <li>entity n's own random: {@link #entitySeed} (n = the n-th entity to enter the sandbox, its
 *       {@code net_id});
 *   <li>each brain's shuffling lists: {@link #shuffleSeed}, keyed by the behavior's slot in the
 *       brain;
 *   <li>a tracing random ({@link TracingRandom}) that logs each draw's call site, for the exact
 *       compare's first-difference report;
 *   <li>the per-tick trace frame of every entity ({@link #entityFrame}).
 * </ul>
 */
final class Exact {
    private Exact() {}

    static final long ENTITY_SALT = 0x656e74697479L; // "entity"
    static final long GOLDEN = 0x9E3779B97F4A7C15L;

    /**
     * The seed of entity n's own random in a sandbox seeded {@code seed}: SplitMix64's step
     * ({@code RandomSupport.mixStafford13} of the seed plus (n + 1) golden-ratio increments), so
     * consecutive entities get independent LCG streams (a plain {@code seed ^ (salt + n)} keeps
     * Java's LCG outputs correlated for consecutive n).
     */
    static long entitySeed(long seed, long n) {
        return RandomSupport.mixStafford13((seed ^ ENTITY_SALT) + (n + 1) * GOLDEN);
    }

    /**
     * The seed of the shuffling list at {@code key} of a brain whose entity's random was seeded
     * {@code entitySeed}: key = {@code "<generation>/<activity>/<priority>/<index>"} (the behavior's
     * index in that activity's set at that priority, insertion order), then {@code "/<k>"} per
     * nested gate (its index in the parent list before any shuffle), and {@code "#<j>"} for the j-th
     * list found inside one behavior that is not a gate (a {@code TriggerGate} closure).
     */
    static long shuffleSeed(long entitySeed, String key) {
        return RandomSupport.mixStafford13(entitySeed + (long) key.hashCode() * GOLDEN);
    }

    /** The seed of raid {@code id}'s own random ({@code Raid.random}) in a sandbox seeded {@code seed}. */
    static long raidSeed(long seed, int id) {
        return RandomSupport.mixStafford13((seed ^ 0x72616964L) + (long) id * GOLDEN);
    }

    /**
     * The seed of a world sandbox's {@code WanderingTraderSpawner.random} (its own
     * {@code RandomSource.create()}; the trader spawner is the overworld's fifth custom spawner).
     */
    static long traderSeed(long seed) {
        return RandomSupport.mixStafford13((seed ^ 0x747261646572L) + GOLDEN);
    }

    // ---- reflection ------------------------------------------------------------------------------

    private static final Field LEGACY_SEED = field(LegacyRandomSource.class, "seed");
    private static final Field ENTITY_RANDOM = field(Entity.class, "random");
    private static final Field BRAIN_SENSORS = field(Brain.class, "sensors");
    private static final Field BRAIN_BEHAVIORS = field(Brain.class, "availableBehaviorsByPriority");
    private static final Field BRAIN_MEMORIES = field(Brain.class, "memories");
    private static final Field GATE_BEHAVIORS = field(GateBehavior.class, "behaviors");
    private static final Field SHUFFLE_RANDOM = field(ShufflingList.class, "random");
    private static final Field SHUFFLE_ENTRIES = field(ShufflingList.class, "entries");
    private static final Field SENSOR_SCAN_RATE = field(Sensor.class, "scanRate");
    private static final Field SENSOR_TIME_TO_TICK = field(Sensor.class, "timeToTick");
    private static final Field BOLT_FLASHES = field(LightningBolt.class, "flashes");
    private static final Field GOAL_SELECTOR = field(Mob.class, "goalSelector");
    private static final Field TARGET_SELECTOR = field(Mob.class, "targetSelector");
    private static final Field VISIBLE_NEARBY = field(NearestVisibleLivingEntities.class, "nearbyEntities");

    static Field field(Class<?> owner, String name) {
        try {
            Field f = owner.getDeclaredField(name);
            f.setAccessible(true);
            return f;
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException("Missing field " + owner.getName() + "." + name, e);
        }
    }

    static Object get(Field f, Object target) {
        try {
            return f.get(target);
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }

    /** A {@code LegacyRandomSource}'s internal 48-bit state, read without advancing it. */
    static long state(RandomSource r) {
        if (!(r instanceof LegacyRandomSource)) {
            throw new IllegalStateException("not a LegacyRandomSource: " + r.getClass().getName());
        }
        return ((AtomicLong) get(LEGACY_SEED, r)).get();
    }

    /** Set a {@code LegacyRandomSource}'s internal state (the value {@link #state} reads). */
    static void setState(RandomSource r, long state) {
        ((AtomicLong) get(LEGACY_SEED, r)).set(state);
    }

    // ---- what the bridge remembers about each seeded entity --------------------------------------

    /** One entity's seeding: its own random's seed and the brain whose lists were last seeded. */
    static final class Seeded {
        final long n;
        final long seed;
        int generation;
        Object brain;

        Seeded(long n, long seed) {
            this.n = n;
            this.seed = seed;
        }
    }

    // ---- the constructor's draws -----------------------------------------------------------------

    /**
     * Seed a new entity's own random and redo its constructor's draws ({@link #replayConstructor}).
     * {@code applyYaw}: the LivingEntity constructor's random yaw stays (a summoned design mob,
     * before its data loads); otherwise it is drawn and thrown away (every spawn path of the game
     * snaps the mob to a yaw of its own afterwards). {@code applyUuid}: the drawn UUID becomes the
     * entity's (the caller fixes the level's UUID set when the entity is already in it).
     */
    static Seeded seed(Entity e, long sandboxSeed, long n, boolean applyYaw, boolean applyUuid, ItemPath itemPath) {
        return seed(e, sandboxSeed, n, applyYaw, applyUuid, itemPath, null);
    }

    /**
     * {@code ctorYaw}: the yaw the entity's constructor drew (recorded when it was constructed:
     * {@link Sandbox#onConstructed}), or null (then found by stepping the random back).
     */
    static Seeded seed(Entity e, long sandboxSeed, long n, boolean applyYaw, boolean applyUuid, ItemPath itemPath, Float ctorYaw) {
        long s = entitySeed(sandboxSeed, n);
        RandomSource r = e.getRandom();
        // a LivingEntity's head (and yaw, when no spawn path snapped it) still holding the value its
        // constructor drew from the unseeded random, replaced by the seeded draw
        boolean headIsCtor = false;
        boolean yawIsCtor = false;
        // (tickexact 2026-11-10) a cube mob's body too: setUpSplitCube's setSize runs
        // AbstractCubeMob.onSyncedDataUpdated(ID_SIZE), yBodyRot = yHeadRot (the constructor's
        // unseeded yaw) before it enters; left alone, a split slime's body was the JVM's chance
        boolean bodyIsCtor = false;
        if (e instanceof LivingEntity living && !applyYaw) {
            if (ctorYaw != null) {
                headIsCtor = living.yHeadRot == ctorYaw;
                yawIsCtor = living.getYRot() == ctorYaw;
                bodyIsCtor = living.yBodyRot == ctorYaw;
            } else if (r instanceof net.minecraft.world.level.levelgen.LegacyRandomSource) {
                // the constructor's draws are the last ones the random made: step it back
                long cur = state(r);
                for (int k = 0; k < 128 && !headIsCtor; k++) {
                    float v = (int) (cur >>> 24) * 5.9604645E-8F * (float) (Math.PI * 2);
                    headIsCtor = v == living.yHeadRot;
                    yawIsCtor |= v == living.getYRot();
                    bodyIsCtor |= v == living.yBodyRot;
                    cur = ((cur - 0xBL) * LCG_INVERSE) & ((1L << 48) - 1);
                }
                yawIsCtor &= headIsCtor;
                bodyIsCtor &= headIsCtor;
            }
        }
        r.setSeed(s);
        float yaw = replayConstructor(e, r, applyYaw, applyUuid, itemPath);
        if (e instanceof LivingEntity living) {
            if (headIsCtor) {
                living.yHeadRot = yaw;
            }
            if (yawIsCtor) {
                living.setYRot(yaw);
                living.yRotO = yaw;
            }
            if (bodyIsCtor) {
                living.yBodyRot = yaw;
            }
        }
        Seeded info = new Seeded(n, s);
        seedBrain(e, info);
        return info;
    }

    /** The inverse of {@code LegacyRandomSource}'s multiplier mod 2^48 (stepping its LCG back). */
    static final long LCG_INVERSE = java.math.BigInteger.valueOf(0x5DEECE66DL).modInverse(java.math.BigInteger.ONE.shiftLeft(48)).longValue();

    /**
     * Seed {@code Collections.shuffle(List)}'s static random ({@code java.util.Collections.r}, a
     * {@code new Random()} the JDK makes on first use): {@code InsideBrownianWalk} (a villager
     * resting where it cannot sleep), {@code LongJumpToRandomPos} and the breeze's long jump
     * shuffle with it. One per JVM (a world sandbox has a worker of its own):
     * {@code new Random(s ^ "shuffle")}, the same LCG as the level random's.
     */
    static void seedCollectionsShuffle(long sandboxSeed) {
        try {
            java.lang.reflect.Field r = java.util.Collections.class.getDeclaredField("r");
            r.setAccessible(true);
            r.set(null, new java.util.Random(collectionsSeed(sandboxSeed)));
        } catch (ReflectiveOperationException | RuntimeException e) {
            throw new IllegalStateException("cannot seed Collections.shuffle (the worker needs --add-opens java.base/java.util): " + e);
        }
    }

    static long collectionsSeed(long sandboxSeed) {
        return sandboxSeed ^ 0x73687566666c65L;
    }

    private static final java.lang.reflect.Field SECTION_STORAGE = field(net.minecraft.world.level.chunk.storage.SectionStorage.class, "storage");
    private static final java.lang.reflect.Field POI_BY_TYPE = field(net.minecraft.world.entity.ai.village.poi.PoiSection.class, "byType");

    /** POI types in their registry order (a deterministic order for {@code PoiSection.byType}). */
    static final java.util.Comparator<net.minecraft.core.Holder<net.minecraft.world.entity.ai.village.poi.PoiType>> POI_TYPE_ORDER =
            java.util.Comparator.comparingInt(h -> net.minecraft.core.registries.BuiltInRegistries.POINT_OF_INTEREST_TYPE.getId(h.value()));

    /**
     * Give every POI section's by-type map a deterministic order (the types' registry order):
     * vanilla's is a {@code HashMap} keyed by registry holders, whose hash is the JVM's identity
     * hash, so a search over several POI types ({@code getRecords} with a tag: job sites, village
     * POIs) met equally near records in a different order on every run. Within a type the records
     * stay vanilla's (a {@code HashSet} of records hashed by their position). Run before each tick
     * (sections made since get theirs).
     */
    @SuppressWarnings("unchecked")
    static void fixPoiOrder(net.minecraft.world.entity.ai.village.poi.PoiManager poi) {
        try {
            var storage = (it.unimi.dsi.fastutil.longs.Long2ObjectMap<java.util.Optional<?>>) SECTION_STORAGE.get(poi);
            for (java.util.Optional<?> o : storage.values()) {
                if (o.isEmpty()) {
                    continue;
                }
                Object section = o.get();
                Object byType = POI_BY_TYPE.get(section);
                if (byType instanceof java.util.TreeMap) {
                    continue;
                }
                var sorted = new java.util.TreeMap<net.minecraft.core.Holder<net.minecraft.world.entity.ai.village.poi.PoiType>, java.util.Set<net.minecraft.world.entity.ai.village.poi.PoiRecord>>(POI_TYPE_ORDER);
                sorted.putAll((Map<net.minecraft.core.Holder<net.minecraft.world.entity.ai.village.poi.PoiType>, java.util.Set<net.minecraft.world.entity.ai.village.poi.PoiRecord>>) byType);
                POI_BY_TYPE.set(section, sorted);
            }
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }

    /**
     * The seed of the k-th entity constructed in a sandbox ({@code EntityType.create}: spawn
     * attempts that fail included), given to its random right after its constructor
     * ({@link Sandbox#onConstructed}): what it draws before it enters the level (a zombie's
     * {@code finalizeSpawn} -> {@code handleAttributes}, a salmon's variant) comes from it; at
     * entering it is reseeded as its entity number ({@link #entitySeed}).
     */
    static long constructSeed(long sandboxSeed, long k) {
        return RandomSupport.mixStafford13((sandboxSeed ^ 0x636f6e737472L) + (k + 1) * GOLDEN);
    }

    private static boolean factoriesHooked;

    /**
     * Wrap every entity type's factory (once per JVM; {@code EntityType.create} is the only caller)
     * so a sandbox sees each entity right after its constructor ({@link Sandbox#onConstructed}).
     */
    @SuppressWarnings({"unchecked", "rawtypes"})
    static synchronized void hookFactories() {
        if (factoriesHooked) {
            return;
        }
        factoriesHooked = true;
        try {
            java.lang.reflect.Field f = net.minecraft.world.entity.EntityType.class.getDeclaredField("factory");
            f.setAccessible(true);
            for (net.minecraft.world.entity.EntityType<?> t : net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE) {
                net.minecraft.world.entity.EntityType.EntityFactory orig = (net.minecraft.world.entity.EntityType.EntityFactory) f.get(t);
                net.minecraft.world.entity.EntityType.EntityFactory wrapped = (type, level) -> {
                    Entity e = orig.create(type, level);
                    if (e != null) {
                        Sandbox.onConstructed(e, level);
                    }
                    return e;
                };
                f.set(t, wrapped);
            }
        } catch (ReflectiveOperationException ex) {
            throw new IllegalStateException(ex);
        }
    }

    /** How an item entity was made (which constructor draws happened, which values stayed). */
    enum ItemPath {
        /** Not an item. */
        NONE,
        /** {@code new ItemEntity(level, x, y, z, stack)} whose drift stayed ({@code Entity.spawnAtLocation}). */
        DRIFT_KEPT,
        /** {@code new ItemEntity(level, x, y, z, stack)} whose motion the caller replaced ({@code BehaviorUtils.throwItem}). */
        DRIFT_REPLACED,
        /** Any other constructor: no drift draws. */
        NO_DRIFT
    }

    /**
     * The draws a constructor made from the entity's own random, redone in order from {@code r}
     * (just reseeded), each written back where the constructor wrote it:
     *
     * <ol>
     *   <li>{@code Entity}: the UUID ({@code Mth.createInsecureUUID}: 2 {@code nextLong});
     *   <li>{@code ItemEntity}: {@code bobOffs} (a field initializer, client-only: drawn), the yaw
     *       ({@code nextFloat() * 360}), the 5-argument constructor's drift (2 {@code nextDouble});
     *   <li>{@code LightningBolt}: {@code seed} ({@code nextLong}), {@code flashes}
     *       ({@code nextInt(3) + 1});
     *   <li>{@code LivingEntity}: the yaw ({@code nextFloat() * 2 pi}, see {@link #seed}), then each
     *       sensor's start delay ({@code Brain.<init>} -> {@code randomlyDelayStart}: one
     *       {@code nextInt(scanRate)} per sensor, in the brain's sensor order);
     *   <li>leaf classes: {@code Chicken.eggTime} ({@code nextInt(6000) + 6000}), {@code Rabbit}'s
     *       {@code idleAnimationTimeout} ({@code nextInt(40) + 180}), {@code Squid.tentacleSpeed}
     *       ({@code nextFloat}), {@code Guardian}'s tail animation ({@code nextFloat}).
     * </ol>
     */
    static float replayConstructor(Entity e, RandomSource r, boolean applyYaw, boolean applyUuid, ItemPath itemPath) {
        UUID uuid = Mth.createInsecureUUID(r);
        if (applyUuid) {
            e.setUUID(uuid);
        }
        if (e instanceof ItemEntity item) {
            r.nextFloat(); // bobOffs (client-only)
            item.setYRot(r.nextFloat() * 360.0F);
            if (itemPath == ItemPath.DRIFT_KEPT) {
                item.setDeltaMovement(r.nextDouble() * 0.2 - 0.1, 0.2, r.nextDouble() * 0.2 - 0.1);
            } else if (itemPath == ItemPath.DRIFT_REPLACED) {
                r.nextDouble();
                r.nextDouble();
            }
            return Float.NaN;
        }
        if (e instanceof LightningBolt bolt) {
            bolt.seed = r.nextLong();
            try {
                BOLT_FLASHES.setInt(bolt, r.nextInt(3) + 1);
            } catch (IllegalAccessException ex) {
                throw new IllegalStateException(ex);
            }
            return Float.NaN;
        }
        if (!(e instanceof LivingEntity living)) {
            return Float.NaN;
        }
        float yaw = r.nextFloat() * (float) (Math.PI * 2);
        if (applyYaw) {
            living.setYRot(yaw);
            living.yHeadRot = yaw;
        }
        if (e instanceof net.minecraft.world.entity.monster.cubemob.AbstractCubeMob cube) {
            // CubeMobMoveControl's constructor keeps the constructor's yaw (as degrees) as the
            // direction it turns to until a goal sets one
            Object mc = cube.getMoveControl();
            for (Class<?> k = mc.getClass(); k != null; k = k.getSuperclass()) {
                if (!k.getName().equals("net.minecraft.world.entity.monster.cubemob.AbstractCubeMob$CubeMobMoveControl")) {
                    continue;
                }
                try {
                    java.lang.reflect.Field f = k.getDeclaredField("yRot");
                    f.setAccessible(true);
                    f.setFloat(mc, 180.0F * yaw / (float) Math.PI);
                } catch (ReflectiveOperationException ex) {
                    throw new IllegalStateException(ex);
                }
                break;
            }
        }
        redoSensorDelays(living, r);
        if (e instanceof net.minecraft.world.entity.animal.chicken.Chicken chicken) {
            chicken.eggTime = r.nextInt(6000) + 6000;
        } else if (e instanceof net.minecraft.world.entity.animal.rabbit.Rabbit rabbit) {
            setInt(rabbit, net.minecraft.world.entity.animal.rabbit.Rabbit.class, "idleAnimationTimeout", r.nextInt(40) + 180);
        } else if (e instanceof net.minecraft.world.entity.animal.squid.Squid squid) {
            setFloat(squid, net.minecraft.world.entity.animal.squid.Squid.class, "tentacleSpeed", 1.0F / (r.nextFloat() + 1.0F) * 0.2F);
        } else if (e instanceof net.minecraft.world.entity.monster.Guardian guardian) {
            setFloat(guardian, net.minecraft.world.entity.monster.Guardian.class, "clientSideTailAnimation", r.nextFloat());
        } else if (e instanceof net.minecraft.world.entity.animal.golem.CopperGolem golem) {
            golem.getBrain().setMemory(MemoryModuleType.TRANSPORT_ITEMS_COOLDOWN_TICKS, r.nextInt(60, 100));
        } else if (e instanceof net.minecraft.world.entity.animal.fish.AbstractSchoolingFish fish) {
            // (mob-AI 2026-11-10) Mob's constructor runs registerGoals: FollowFlockLeaderGoal's
            // constructor draws its first nextStartTick, reducedTickDelay(200 + nextInt(200) % 20)
            int start = Mth.positiveCeilDiv(200 + r.nextInt(200) % 20, 2);
            net.minecraft.world.entity.ai.goal.GoalSelector goals = (net.minecraft.world.entity.ai.goal.GoalSelector) get(GOAL_SELECTOR, fish);
            for (WrappedGoal wrapped : goals.getAvailableGoals()) {
                if (wrapped.getGoal() instanceof net.minecraft.world.entity.ai.goal.FollowFlockLeaderGoal follow) {
                    setInt(follow, net.minecraft.world.entity.ai.goal.FollowFlockLeaderGoal.class, "nextStartTick", start);
                }
            }
        }
        return yaw;
    }

    /** Each sensor's start delay again, from {@code r}, in the brain's sensor order. */
    @SuppressWarnings("unchecked")
    static void redoSensorDelays(LivingEntity living, RandomSource r) {
        Map<Object, Sensor<?>> sensors = (Map<Object, Sensor<?>>) get(BRAIN_SENSORS, living.getBrain());
        for (Sensor<?> s : sensors.values()) {
            s.randomlyDelayStart(r);
        }
    }

    /**
     * (tickexact 2026-11-11, ironspec's forced-phase mode) Set {@code living}'s k-th brain sensor's
     * start delay ({@code timeToTick}, in the brain's sensor order); returns the delay it had.
     */
    @SuppressWarnings("unchecked")
    static long setSensorDelay(LivingEntity living, int k, long value) {
        Map<Object, Sensor<?>> sensors = (Map<Object, Sensor<?>>) get(BRAIN_SENSORS, living.getBrain());
        int i = 0;
        for (Sensor<?> s : sensors.values()) {
            if (i++ == k) {
                long was = (long) get(SENSOR_TIME_TO_TICK, s);
                try {
                    SENSOR_TIME_TO_TICK.setLong(s, value);
                } catch (IllegalAccessException ex) {
                    throw new IllegalStateException(ex);
                }
                return was;
            }
        }
        throw new IllegalArgumentException("sensor_delay: no sensor " + k + " (the brain has " + sensors.size() + ")");
    }

    private static void setInt(Object o, Class<?> owner, String name, int v) {
        try {
            field(owner, name).setInt(o, v);
        } catch (IllegalAccessException ex) {
            throw new IllegalStateException(ex);
        }
    }

    private static void setFloat(Object o, Class<?> owner, String name, float v) {
        try {
            field(owner, name).setFloat(o, v);
        } catch (IllegalAccessException ex) {
            throw new IllegalStateException(ex);
        }
    }

    // ---- the brain's shuffling lists -------------------------------------------------------------

    /**
     * Seed every shuffling list of the entity's live brain (generation {@code info.generation}),
     * once per brain: a brain rebuilt since ({@code Villager.refreshBrain}) counts one generation
     * more and is seeded at the next {@link #refreshBrains} (before its first tick).
     */
    static void seedBrain(Entity e, Seeded info) {
        if (!(e instanceof LivingEntity living)) {
            return;
        }
        Brain<?> brain = living.getBrain();
        if (info.brain == brain) {
            return;
        }
        if (info.brain != null) {
            info.generation++;
        }
        info.brain = brain;
        for (Map.Entry<String, ShufflingList<?>> k : shufflingLists(brain, info.generation).entrySet()) {
            ((RandomSource) get(SHUFFLE_RANDOM, k.getValue())).setSeed(shuffleSeed(info.seed, k.getKey()));
        }
    }

    /** Every seeded entity whose brain was rebuilt since: its new lists seeded (generation + 1). */
    static void refreshBrains(Iterable<Entity> entities, Map<Entity, Seeded> seeded) {
        for (Entity e : entities) {
            Seeded info = seeded.get(e);
            if (info != null && e instanceof LivingEntity living && living.getBrain() != info.brain) {
                seedBrain(e, info);
            }
        }
    }

    /** The brain's shuffling lists by key (see {@link #shuffleSeed}), in a stable order. */
    @SuppressWarnings("unchecked")
    static Map<String, ShufflingList<?>> shufflingLists(Brain<?> brain, int generation) {
        Map<String, ShufflingList<?>> out = new java.util.LinkedHashMap<>();
        Map<Integer, Map<Activity, Set<BehaviorControl<?>>>> byPriority =
                (Map<Integer, Map<Activity, Set<BehaviorControl<?>>>>) get(BRAIN_BEHAVIORS, brain);
        IdentityHashMap<Object, Boolean> seen = new IdentityHashMap<>();
        for (Map.Entry<Integer, Map<Activity, Set<BehaviorControl<?>>>> p : byPriority.entrySet()) {
            List<Activity> activities = new ArrayList<>(p.getValue().keySet());
            activities.sort(Comparator.comparing(Activity::getName));
            for (Activity a : activities) {
                int index = 0;
                for (BehaviorControl<?> b : p.getValue().get(a)) {
                    collect(b, generation + "/" + a.getName() + "/" + p.getKey() + "/" + index, out, seen);
                    index++;
                }
            }
        }
        return out;
    }

    /** One behavior's lists: a gate's own list, its entries' (nested), or lists in a closure. */
    private static void collect(Object b, String key, Map<String, ShufflingList<?>> out, IdentityHashMap<Object, Boolean> seen) {
        if (b == null || seen.put(b, Boolean.TRUE) != null) {
            return;
        }
        if (b instanceof GateBehavior<?> gate) {
            ShufflingList<?> list = (ShufflingList<?>) get(GATE_BEHAVIORS, gate);
            if (seen.put(list, Boolean.TRUE) == null) {
                out.put(key, list);
            }
            List<?> entries = (List<?>) get(SHUFFLE_ENTRIES, list);
            for (int k = 0; k < entries.size(); k++) {
                Object data = ((ShufflingList.WeightedEntry<?>) entries.get(k)).getData();
                collect(data, key + "/" + k, out, seen);
            }
            return;
        }
        // a TriggerGate's list is captured by the closure its OneShot runs: look through the
        // behavior's own fields (closures, anonymous classes, the behavior builder's parts)
        List<ShufflingList<?>> found = new ArrayList<>();
        closureLists(b, found, seen, 0);
        for (int j = 0; j < found.size(); j++) {
            out.put(key + "#" + j, found.get(j));
        }
    }

    /** Shuffling lists reachable from a behavior through closures and behavior-builder parts. */
    private static void closureLists(Object o, List<ShufflingList<?>> found, IdentityHashMap<Object, Boolean> seen, int depth) {
        if (depth > 8) {
            return;
        }
        Field[] fields = allFields(o.getClass());
        for (Field f : fields) {
            if (Modifier.isStatic(f.getModifiers()) || f.getType().isPrimitive()) {
                continue;
            }
            Object v;
            try {
                f.setAccessible(true);
                v = f.get(o);
            } catch (RuntimeException | IllegalAccessException ex) {
                continue;
            }
            if (v == null || seen.containsKey(v)) {
                continue;
            }
            if (v instanceof ShufflingList<?> list) {
                seen.put(v, Boolean.TRUE);
                found.add(list);
                for (Object entry : (List<?>) get(SHUFFLE_ENTRIES, list)) {
                    Object data = ((ShufflingList.WeightedEntry<?>) entry).getData();
                    if (data != null && !seen.containsKey(data)) {
                        seen.put(data, Boolean.TRUE);
                        closureLists(data, found, seen, depth + 1);
                    }
                }
            } else if (followable(v)) {
                seen.put(v, Boolean.TRUE);
                closureLists(v, found, seen, depth + 1);
            }
        }
    }

    /** Objects a behavior is built from: closures, anonymous and behavior classes. */
    private static boolean followable(Object v) {
        Class<?> c = v.getClass();
        if (c.isHidden() || c.isSynthetic() || c.isAnonymousClass()) {
            return true;
        }
        String n = c.getName();
        return n.startsWith("net.minecraft.world.entity.ai.behavior.");
    }

    private static Field[] allFields(Class<?> c) {
        List<Field> out = new ArrayList<>();
        for (Class<?> k = c; k != null && k != Object.class; k = k.getSuperclass()) {
            Field[] fs = k.getDeclaredFields();
            Arrays.sort(fs, Comparator.comparing(Field::getName));
            out.addAll(0, Arrays.asList(fs));
        }
        return out.toArray(new Field[0]);
    }

    // ---- the tracing random ----------------------------------------------------------------------

    /** The draws of one tick, in order: each call site with its count (consecutive draws merged). */
    static final class DrawLog {
        final List<String> sites = new ArrayList<>();
        final List<String> streams = new ArrayList<>();
        final List<Integer> counts = new ArrayList<>();
        /** Each aggregated row's raw {@code next(bits)} outputs, in order. */
        final List<List<Integer>> values = new ArrayList<>();
        /** The level random's state before the spawning chunks' shuffle ({@code Util.shuffle} in tickChunks), or -1. */
        long shuffleState = -1;

        void record(String stream, String site, int value) {
            int last = sites.size() - 1;
            if (last >= 0 && sites.get(last).equals(site) && streams.get(last).equals(stream)) {
                counts.set(last, counts.get(last) + 1);
                values.get(last).add(value);
            } else {
                sites.add(site);
                streams.add(stream);
                counts.add(1);
                List<Integer> v = new ArrayList<>();
                v.add(value);
                values.add(v);
            }
        }

        JsonArray toJson() {
            JsonArray a = new JsonArray();
            for (int i = 0; i < sites.size(); i++) {
                JsonArray row = new JsonArray();
                row.add(streams.get(i));
                row.add(sites.get(i));
                row.add(counts.get(i));
                JsonArray v = new JsonArray();
                for (int x : values.get(i)) {
                    v.add(x);
                }
                row.add(v);
                a.add(row);
            }
            return a;
        }
    }

    /** Where draws are logged while a traced step runs (null: not logging). */
    static DrawLog drawLog;

    /**
     * A {@code LegacyRandomSource} that logs the call site of every {@code next(bits)} while
     * {@link #drawLog} is set: the same LCG, so swapping it in changes nothing the game sees.
     */
    static final class TracingRandom extends LegacyRandomSource {
        final String stream;

        TracingRandom(String stream, long state) {
            super(0L);
            this.stream = stream;
            setState(this, state);
        }

        @Override
        public int next(final int bits) {
            DrawLog log = drawLog;
            if (log == null) {
                return super.next(bits);
            }
            String site = callSite();
            if (log.shuffleState < 0 && "level".equals(this.stream) && site.startsWith("ServerChunkCache.tickChunks")) {
                log.shuffleState = state(this);
            }
            int v = super.next(bits);
            log.record(this.stream, site, v);
            return v;
        }
    }

    private static final StackWalker WALKER = StackWalker.getInstance();

    /** The first two frames outside the random classes: "Class.method:line < Class.method:line". */
    static String callSite() {
        return WALKER.walk(frames -> {
            StringBuilder sb = new StringBuilder();
            int n = 0;
            for (StackWalker.StackFrame f : (Iterable<StackWalker.StackFrame>) frames::iterator) {
                String c = f.getClassName();
                if (c.startsWith("dev.vjeux.mctas.bridge.Exact") || c.endsWith("RandomSource") || c.endsWith("BitRandomSource")
                        || c.equals("net.minecraft.util.Mth") || c.endsWith("MarsagliaPolarGaussian") || c.startsWith("net.minecraft.util.random.")
                        || c.startsWith("net.minecraft.util.valueproviders.") || c.equals("net.minecraft.util.Util")) {
                    continue;
                }
                if (n > 0) {
                    sb.append(" < ");
                }
                sb.append(c.substring(c.lastIndexOf('.') + 1)).append('.').append(f.getMethodName()).append(':').append(f.getLineNumber());
                if (++n == 2) {
                    break;
                }
            }
            return sb.toString();
        });
    }

    /** Swap the entity's own random for a tracing one with the same state (stream "e<n>"). */
    static void traceEntity(Entity e, long n) {
        RandomSource r = e.getRandom();
        if (r instanceof TracingRandom) {
            return;
        }
        try {
            ENTITY_RANDOM.set(e, new TracingRandom("e" + n, state(r)));
        } catch (IllegalAccessException ex) {
            throw new IllegalStateException(ex);
        }
    }

    // ---- trace frames ----------------------------------------------------------------------------

    /**
     * What a traced step reports each tick ({@code trace} op): every entity's frame, the entities
     * that entered and left, and the block and block-entity changes in a box since the last frame
     * (the box as it was when the watch started is the baseline: the caller reads it with
     * {@code get}).
     */
    static final class Watch {
        private final int[] from;
        private final int[] to;
        private final net.minecraft.world.level.block.state.BlockState[] states;
        private final Map<Long, String> blockEntities = new java.util.HashMap<>();
        private final Map<Long, String> containerTexts = new java.util.HashMap<>();
        private java.util.Set<Integer> present = new java.util.HashSet<>();

        Watch(Sandbox sb, int[] from, int[] to) {
            this.from = from;
            this.to = to;
            if (from == null) {
                this.states = new net.minecraft.world.level.block.state.BlockState[0];
            } else {
                long size = (long) (to[0] - from[0] + 1) * (to[1] - from[1] + 1) * (to[2] - from[2] + 1);
                if (size <= 0 || size > 4_000_000L) {
                    throw new IllegalArgumentException("trace box of " + size + " cells");
                }
                this.states = new net.minecraft.world.level.block.state.BlockState[(int) size];
                scan(sb, null, null, null);
            }
            for (Entity e : sb.entities()) {
                if (e instanceof net.minecraft.server.level.ServerPlayer) {
                    continue;
                }
                this.present.add(e.getId() - sb.entityIdBase());
            }
        }

        boolean sameBox(int[] f, int[] t) {
            return Arrays.equals(f, this.from) && Arrays.equals(t, this.to);
        }

        /** Compare the box with what it held (record changes when the arrays are given). */
        private void scan(Sandbox sb, JsonArray blocks, JsonArray bes, JsonArray containers) {
            // (its chunks unloaded: a read would load them again; they hold what they held)
            if (this.from == null || sb.away) {
                return;
            }
            BlockPos.MutableBlockPos p = new BlockPos.MutableBlockPos();
            int i = 0;
            for (int y = this.from[1]; y <= this.to[1]; y++) {
                for (int z = this.from[2]; z <= this.to[2]; z++) {
                    for (int x = this.from[0]; x <= this.to[0]; x++) {
                        net.minecraft.world.level.block.state.BlockState s = sb.level.getBlockState(p.set(x, y, z));
                        if (s != this.states[i]) {
                            if (blocks != null) {
                                JsonArray c = new JsonArray();
                                c.add(x);
                                c.add(y);
                                c.add(z);
                                c.add(net.minecraft.commands.arguments.blocks.BlockStateParser.serialize(s));
                                blocks.add(c);
                            }
                            this.states[i] = s;
                        }
                        i++;
                        if (s.hasBlockEntity() || this.blockEntities.containsKey(p.asLong()) || this.containerTexts.containsKey(p.asLong())) {
                            net.minecraft.world.level.block.entity.BlockEntity be = sb.level.getBlockEntity(p);
                            String dump = be == null ? "" : BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(be.getType()).getPath() + " "
                                    + be.saveWithoutMetadata(sb.level.registryAccess());
                            String was = dump.isEmpty() ? this.blockEntities.remove(p.asLong()) : this.blockEntities.put(p.asLong(), dump);
                            if (bes != null && !dump.equals(was == null ? "" : was)) {
                                JsonArray c = new JsonArray();
                                c.add(x);
                                c.add(y);
                                c.add(z);
                                c.add(dump);
                                bes.add(c);
                            }
                            String text = containerText(be);
                            String wasText = text == null ? this.containerTexts.remove(p.asLong()) : this.containerTexts.put(p.asLong(), text);
                            if (containers != null && (text != null || wasText != null) && !java.util.Objects.equals(text == null ? "" : text, wasText)) {
                                JsonArray c = new JsonArray();
                                c.add(x);
                                c.add(y);
                                c.add(z);
                                c.add(text == null ? "" : text);
                                containers.add(c);
                            }
                        }
                    }
                }
            }
        }

        /** A container's items as redsim's compare prints them (`exact::container_text`). */
        static String containerText(net.minecraft.world.level.block.entity.BlockEntity be) {
            if (!(be instanceof net.minecraft.world.Container c)) {
                return null;
            }
            StringBuilder s = new StringBuilder();
            for (int k = 0; k < c.getContainerSize(); k++) {
                net.minecraft.world.item.ItemStack st = c.getItem(k);
                if (st.isEmpty()) {
                    continue;
                }
                s.append(s.length() > 0 ? " " : "").append(k).append(':').append(BuiltInRegistries.ITEM.getKey(st.getItem()).getPath()).append(':').append(st.getCount());
            }
            if (be instanceof net.minecraft.world.level.block.entity.CrafterBlockEntity cr) {
                int mask = 0;
                for (int k = 0; k < 9; k++) {
                    if (cr.isSlotDisabled(k)) {
                        mask |= 1 << k;
                    }
                }
                if (mask != 0) {
                    s.append(s.length() > 0 ? " " : "").append("disabled:").append(mask);
                }
            }
            return s.toString();
        }

        JsonObject frame(Sandbox sb, DrawLog log) {
            JsonObject f = new JsonObject();
            f.addProperty("t", sb.level.getGameTime());
            f.addProperty("day", sb.level.getOverworldClockTime());
            f.addProperty("rng", state(sb.level.getRandom()));
            f.addProperty("rv", sb.randValue());
            net.minecraft.world.level.saveddata.WeatherData wd = sb.level.getServer().getWeatherData();
            f.addProperty("weather", "clear " + wd.getClearWeatherTime() + " rain " + wd.getRainTime() + " thunder " + wd.getThunderTime()
                    + (wd.isRaining() ? " raining" : "") + (wd.isThundering() ? " thundering" : "")
                    + " levels " + sb.level.getRainLevel(1.0F) + " " + sb.level.getThunderLevel(1.0F));
            List<Entity> all = sb.entities();
            all.sort(Comparator.comparingInt(Entity::getId));
            JsonArray ents = new JsonArray();
            java.util.Set<Integer> now = new java.util.HashSet<>();
            JsonArray entered = new JsonArray();
            for (Entity e : all) {
                if (e instanceof net.minecraft.server.level.ServerPlayer p) {
                    // the AFK player: the scenario's input, not compared (its position shows it stays)
                    f.add("player", vec(p.position()));
                    continue;
                }
                int n = e.getId() - sb.entityIdBase();
                now.add(n);
                if (!this.present.contains(n)) {
                    entered.add(n);
                }
                ents.add(entityFrame(sb, e));
            }
            JsonArray left = new JsonArray();
            List<Integer> gone = new ArrayList<>();
            for (int n : this.present) {
                if (!now.contains(n)) {
                    gone.add(n);
                }
            }
            gone.sort(null);
            gone.forEach(left::add);
            this.present = now;
            f.add("entities", ents);
            f.add("entered", entered);
            f.add("left", left);
            JsonArray blocks = new JsonArray();
            JsonArray bes = new JsonArray();
            JsonArray containers = new JsonArray();
            scan(sb, blocks, bes, containers);
            f.add("blocks", blocks);
            f.add("bes", bes);
            f.add("containers", containers);
            if (log != null) {
                f.add("draws", log.toJson());
            }
            return f;
        }
    }

    static JsonArray vec(Vec3 v) {
        JsonArray a = new JsonArray();
        a.add(v.x);
        a.add(v.y);
        a.add(v.z);
        return a;
    }

    /** An entity's number in its sandbox ("#n"), or its type for one outside it. */
    static String ref(Sandbox sb, Entity e) {
        if (e == null) {
            return "none";
        }
        return "#" + (e.getId() - sb.entityIdBase());
    }

    /** A memory's value, canonical (redsim prints its modeled memories the same way). */
    static String memoryValue(Sandbox sb, Object v) {
        if (v == null) {
            return "none";
        }
        if (v instanceof Entity e) {
            return ref(sb, e);
        }
        if (v instanceof GlobalPos g) {
            return pos(g.pos());
        }
        if (v instanceof BlockPos p) {
            return pos(p);
        }
        if (v instanceof WalkTarget w) {
            return "walk " + tracker(sb, w.getTarget()) + " " + w.getSpeedModifier() + " " + w.getCloseEnoughDist();
        }
        if (v instanceof PositionTracker t) {
            return tracker(sb, t);
        }
        if (v instanceof NearestVisibleLivingEntities n) {
            // the nearby list itself: findAll would run (and cache) the lazy line-of-sight test,
            // which changes what the game sees later
            StringBuilder s = new StringBuilder("[");
            for (Object x : (List<?>) get(VISIBLE_NEARBY, n)) {
                s.append(s.length() > 1 ? " " : "").append(ref(sb, (Entity) x));
            }
            return s.append("]").toString();
        }
        if (v instanceof net.minecraft.world.damagesource.DamageSource d) {
            return "damage " + d.getMsgId() + (d.getEntity() == null ? "" : " " + ref(sb, d.getEntity()));
        }
        if (v instanceof Path p) {
            return path(p);
        }
        if (v instanceof java.util.Set<?> set) {
            // a set (DOORS_TO_CLOSE: GlobalPos): its elements' texts sorted (a HashSet's order is
            // not the model's)
            return "[" + String.join(" ", set.stream().map(x -> memoryValue(sb, x)).sorted().toList()) + "]";
        }
        if (v instanceof List<?> l) {
            StringBuilder s = new StringBuilder("[");
            for (Object x : l) {
                s.append(s.length() > 1 ? " " : "").append(memoryValue(sb, x));
            }
            return s.append("]").toString();
        }
        if (v instanceof Boolean || v instanceof Number) {
            return v.toString();
        }
        if (v instanceof net.minecraft.util.Unit) {
            return "unit";
        }
        return v.getClass().getSimpleName() + ":" + v;
    }

    static String pos(BlockPos p) {
        return p.getX() + " " + p.getY() + " " + p.getZ();
    }

    static String tracker(Sandbox sb, PositionTracker t) {
        if (t instanceof EntityTracker et) {
            return "entity " + ref(sb, et.getEntity());
        }
        if (t instanceof BlockPosTracker bt) {
            return "block " + pos(bt.currentBlockPosition());
        }
        return t.getClass().getSimpleName();
    }

    static String path(Path p) {
        BlockPos t = p.getTarget();
        return "path " + pos(t) + " " + p.getNextNodeIndex() + "/" + p.getNodeCount() + (p.canReach() ? " reach" : " partial");
    }

    /**
     * One entity's state after a tick: what the exact compare checks. Every field is exact (doubles
     * and floats as Java prints them, which round-trip).
     */
    @SuppressWarnings("unchecked")
    static JsonObject entityFrame(Sandbox sb, Entity e) {
        JsonObject j = new JsonObject();
        j.addProperty("n", e.getId() - sb.entityIdBase());
        j.addProperty("type", BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).getPath());
        j.add("pos", vec(e.position()));
        j.add("motion", vec(e.getDeltaMovement()));
        // floats as Java prints them (they round-trip as floats; a JSON number would be read as a
        // double first)
        j.addProperty("yaw", Float.toString(e.getYRot()));
        j.addProperty("pitch", Float.toString(e.getXRot()));
        j.addProperty("on_ground", e.onGround());
        j.addProperty("fire", e.getRemainingFireTicks());
        j.addProperty("air", e.getAirSupply());
        j.addProperty("pose", e.getPose().name().toLowerCase(java.util.Locale.ROOT));
        j.addProperty("vehicle", e.getVehicle() == null ? "none" : ref(sb, e.getVehicle()));
        RandomSource r = e.getRandom();
        if (r instanceof LegacyRandomSource) {
            j.addProperty("random", state(r));
        }
        if (e instanceof ItemEntity item) {
            j.addProperty("item", BuiltInRegistries.ITEM.getKey(item.getItem().getItem()).getPath() + " " + item.getItem().getCount());
            j.addProperty("age", item.getAge());
        }
        if (e instanceof LivingEntity living) {
            j.addProperty("head", Float.toString(living.yHeadRot));
            j.addProperty("body", Float.toString(living.yBodyRot));
            j.addProperty("health", Float.toString(living.getHealth()));
            j.addProperty("hurt_time", living.hurtTime);
            j.addProperty("death_time", living.deathTime);
            j.addProperty("no_action", living.getNoActionTime());
            Brain<?> brain = living.getBrain();
            Map<MemoryModuleType<?>, ?> memories = (Map<MemoryModuleType<?>, ?>) get(BRAIN_MEMORIES, brain);
            if (!memories.isEmpty()) {
                JsonObject mem = new JsonObject();
                List<String> names = new ArrayList<>();
                Map<String, MemoryModuleType<?>> byName = new java.util.HashMap<>();
                for (MemoryModuleType<?> t : memories.keySet()) {
                    String name = BuiltInRegistries.MEMORY_MODULE_TYPE.getKey(t).getPath();
                    names.add(name);
                    byName.put(name, t);
                }
                names.sort(null);
                for (String name : names) {
                    MemoryModuleType<Object> t = (MemoryModuleType<Object>) byName.get(name);
                    java.util.Optional<?> value = brain.getMemoryInternal(t);
                    if (value == null || value.isEmpty()) {
                        continue;
                    }
                    long ttl = brain.getTimeUntilExpiry(t);
                    mem.addProperty(name, memoryValue(sb, value.get()) + (ttl == Long.MAX_VALUE ? "" : " ttl " + ttl));
                }
                j.add("memories", mem);
                List<String> active = new ArrayList<>();
                for (Activity a : brain.getActiveActivities()) {
                    active.add(a.getName());
                }
                active.sort(null);
                j.addProperty("activities", String.join(" ", active));
                List<String> running = new ArrayList<>();
                for (BehaviorControl<?> b : brain.getRunningBehaviors()) {
                    running.add(b.debugString());
                }
                running.sort(null);
                j.addProperty("behaviors", String.join(" ", running));
            }
        }
        if (e instanceof Mob mob) {
            List<String> goals = new ArrayList<>();
            for (WrappedGoal g : ((net.minecraft.world.entity.ai.goal.GoalSelector) get(GOAL_SELECTOR, mob)).getAvailableGoals()) {
                if (g.isRunning()) {
                    goals.add(g.getPriority() + ":" + g.getGoal().getClass().getSimpleName());
                }
            }
            for (WrappedGoal g : ((net.minecraft.world.entity.ai.goal.GoalSelector) get(TARGET_SELECTOR, mob)).getAvailableGoals()) {
                if (g.isRunning()) {
                    goals.add("t" + g.getPriority() + ":" + g.getGoal().getClass().getSimpleName());
                }
            }
            goals.sort(null);
            j.addProperty("goals", String.join(" ", goals));
            Path p = mob.getNavigation().getPath();
            j.addProperty("path", p == null ? "none" : path(p));
            j.addProperty("target", mob.getTarget() == null ? "none" : ref(sb, mob.getTarget()));
        }
        return j;
    }
}
