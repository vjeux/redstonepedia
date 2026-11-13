package dev.vjeux.mctas.agent;

import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassHierarchyResolver;
import java.lang.classfile.ClassModel;
import java.lang.classfile.ClassTransform;
import java.lang.classfile.CodeBuilder;
import java.lang.classfile.instruction.InvokeInstruction;
import java.lang.constant.ClassDesc;
import java.lang.constant.ConstantDescs;
import java.lang.constant.MethodTypeDesc;
import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.security.ProtectionDomain;
import java.util.Optional;
import java.util.function.BiConsumer;
import java.util.function.BiFunction;

/**
 * (tickexact 2026-11-12, ironspec's seated-rider clear scene) A Java agent for the official server
 * jar: every change of a watched brain memory ({@code Brain.eraseMemory} and both
 * {@code Brain.setMemoryInternal}) and every activity switch ({@code Brain.setActiveActivity})
 * prints to stderr the memory, its new value and the behaviour that made the change (the first
 * stack frame in net.minecraft.world.entity.ai.behavior), so a harness stepping one tick at a time
 * can log a villager's WALK_TARGET and its source, its CANT_REACH_WALK_TARGET_SINCE stamp (not in
 * the saved data) and each erasure's cause. The agent's argument is the watched memory ids
 * ({@code minecraft:walk_target,minecraft:home,...}). The injected code reaches the hooks through
 * a JDK type only (a {@code BiConsumer} kept in the system properties).
 */
public final class BrainAgent {
    private static final String ERASE = "mctas.brain.erase";
    private static final String SET = "mctas.brain.set";
    private static final String ACTIVITY = "mctas.brain.activity";
    private static final String BRAIN = "net/minecraft/world/entity/ai/Brain";
    private static volatile String[] memories = new String[0];
    private static volatile boolean panicOn = false;
    private static volatile boolean pathOn = false;
    /** (tickexact 2026-11-13, cc767b78 04:2x: the navigation's state for a carried rider) {@code +nav}. */
    private static volatile boolean navOn = false;
    private static final String NAV_HOOK = "mctas.brain.nav";
    private static final String NAV_CLASS = "net/minecraft/world/entity/ai/navigation/PathNavigation";
    private static final String SINK_CLASS = "net/minecraft/world/entity/ai/behavior/MoveToTargetSink";
    /** The mob whose brain is ticking, the object itself (for the PATH set's rider state). */
    private static final ThreadLocal<Object> CURRENT_BODY = new ThreadLocal<>();

    private static Object field(Object o, Class<?> from, String name) throws ReflectiveOperationException {
        for (Class<?> c = from; c != null; c = c.getSuperclass()) {
            try {
                java.lang.reflect.Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(o);
            } catch (NoSuchFieldException e) {
                // the superclass
            }
        }
        throw new NoSuchFieldException(name);
    }

    private static Object call(Object o, String name) throws ReflectiveOperationException {
        for (Class<?> c = o.getClass(); c != null; c = c.getSuperclass()) {
            try {
                java.lang.reflect.Method m = c.getDeclaredMethod(name);
                m.setAccessible(true);
                return m.invoke(o);
            } catch (NoSuchMethodException e) {
                // the superclass
            }
        }
        throw new NoSuchMethodException(name);
    }

    /** A rider's state: position, box x range, isInWater, isPassenger, getSpeed. */
    private static String riderState(Object mob) throws ReflectiveOperationException {
        Object pos = call(mob, "position");
        Object bb = call(mob, "getBoundingBox");
        Object minX = field(bb, bb.getClass(), "minX");
        Object maxX = field(bb, bb.getClass(), "maxX");
        Object minY = field(bb, bb.getClass(), "minY");
        return "pos " + pos + " box x " + minX + ".." + maxX + " y " + minY + " inWater " + call(mob, "isInWater") + " passenger " + call(mob, "isPassenger") + " speed " + call(mob, "getSpeed");
    }

    /** (+nav) PathNavigation.doStuckDetection (every followed tick), timeoutPath and stop, and
     * MoveToTargetSink.stop with its cause, for villagers only. */
    static void onNav(Object what, Object args) {
        Object[] a = (Object[]) args;
        try {
            Object self = a[0];
            if (what.equals("sink-stop")) {
                Object body = a[1];
                if (!body.getClass().getSimpleName().contains("Villager")) {
                    return;
                }
                Object level = call(body, "level");
                long now = (Long) level.getClass().getMethod("getGameTime").invoke(level);
                long end = (Long) field(self, self.getClass(), "endTimestamp");
                Object nav = call(body, "getNavigation");
                System.err.println("mctas brain agent: nav sink-stop at " + now + " endTimestamp " + end + " timedOut " + (now > end) + " navDone " + nav.getClass().getMethod("isDone").invoke(nav) + " stuck " + nav.getClass().getMethod("isStuck").invoke(nav) + " " + riderState(body) + " @" + body.getClass().getMethod("getStringUUID").invoke(body));
                return;
            }
            Class<?> navClass = self.getClass();
            while (navClass != null && !navClass.getSimpleName().equals("PathNavigation")) {
                navClass = navClass.getSuperclass();
            }
            Object mob = field(self, navClass, "mob");
            if (!mob.getClass().getSimpleName().contains("Villager")) {
                return;
            }
            String uuid = (String) mob.getClass().getMethod("getStringUUID").invoke(mob);
            if (what.equals("stuck-check")) {
                Object path = field(self, navClass, "path");
                String p = "none";
                if (path != null) {
                    p = "next " + path.getClass().getMethod("getNextNodeIndex").invoke(path) + " of " + path.getClass().getMethod("getNodeCount").invoke(path) + " done " + path.getClass().getMethod("isDone").invoke(path) + " id " + System.identityHashCode(path);
                }
                System.err.println("mctas brain agent: nav tick " + field(self, navClass, "tick") + " path " + p + " timeoutTimer " + field(self, navClass, "timeoutTimer") + " timeoutLimit " + field(self, navClass, "timeoutLimit") + " cachedNode " + field(self, navClass, "timeoutCachedNode") + " lastStuckCheck " + field(self, navClass, "lastStuckCheck") + " stuck " + field(self, navClass, "isStuck") + " " + riderState(mob) + " @" + uuid);
            } else {
                // timeoutPath or stop: the caller frames
                StringBuilder by = new StringBuilder();
                int n = 0;
                for (StackTraceElement e : new Throwable().getStackTrace()) {
                    String c = e.getClassName();
                    if (c.startsWith("net.minecraft.") && n < 3) {
                        by.append(by.length() == 0 ? "" : " < ").append(c.substring(c.lastIndexOf('.') + 1)).append('.').append(e.getMethodName()).append(':').append(e.getLineNumber());
                        n++;
                    }
                }
                System.err.println("mctas brain agent: nav " + what + " by " + by + " @" + uuid);
            }
        } catch (Throwable t) {
            System.err.println("mctas brain agent: nav " + what + " (unread): " + t);
        }
    }

    /** The entry hooks: PathNavigation.doStuckDetection(Vec3) / timeoutPath() / stop(), and
     * MoveToTargetSink.stop(ServerLevel, Mob, long). */
    static byte[] navHook(ClassLoader loader, byte[] bytes, String name) {
        ClassHierarchyResolver resolver = ClassHierarchyResolver.defaultResolver();
        if (loader != null) {
            resolver = ClassHierarchyResolver.ofClassLoading(loader).orElse(resolver);
        }
        ClassFile cf = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(resolver));
        ClassModel model = cf.parse(bytes);
        boolean sink = SINK_CLASS.equals(name);
        int[] hooked = {0};
        byte[] out = cf.transformClass(model, (cb, ce) -> {
            if (ce instanceof java.lang.classfile.MethodModel m && m.code().isPresent() && !m.flags().has(java.lang.reflect.AccessFlag.BRIDGE) && !m.flags().has(java.lang.reflect.AccessFlag.SYNTHETIC)) {
                String n = m.methodName().stringValue();
                int params = m.methodTypeSymbol().parameterCount();
                String what = null;
                if (sink && n.equals("stop") && params == 3) {
                    what = "sink-stop";
                } else if (!sink && n.equals("doStuckDetection") && params == 1) {
                    what = "stuck-check";
                } else if (!sink && n.equals("timeoutPath") && params == 0) {
                    what = "timeoutPath";
                } else if (!sink && n.equals("stop") && params == 0) {
                    what = "stop";
                }
                if (what == null) {
                    cb.with(ce);
                    return;
                }
                hooked[0]++;
                final String w = what;
                cb.transformMethod(m, java.lang.classfile.MethodTransform.transformingCode(new java.lang.classfile.CodeTransform() {
                    boolean first = true;

                    @Override
                    public void accept(CodeBuilder b, java.lang.classfile.CodeElement e) {
                        if (first) {
                            first = false;
                            ClassDesc properties = ClassDesc.of("java.util.Properties");
                            ClassDesc biConsumer = ClassDesc.of("java.util.function.BiConsumer");
                            b.invokestatic(ClassDesc.of("java.lang.System"), "getProperties", MethodTypeDesc.of(properties));
                            b.ldc(NAV_HOOK);
                            b.invokevirtual(properties, "get", MethodTypeDesc.of(ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                            b.checkcast(biConsumer);
                            b.ldc(w);
                            // new Object[] { this, (sink-stop: the body, local 2) }
                            b.iconst_2();
                            b.anewarray(ConstantDescs.CD_Object);
                            b.dup();
                            b.iconst_0();
                            b.aload(0);
                            b.aastore();
                            if (w.equals("sink-stop")) {
                                b.dup();
                                b.iconst_1();
                                b.aload(2);
                                b.aastore();
                            }
                            b.invokeinterface(biConsumer, "accept", MethodTypeDesc.of(ConstantDescs.CD_void, ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                        }
                        b.with(e);
                    }
                }));
            } else {
                cb.with(ce);
            }
        });
        System.err.println("mctas brain agent: " + name + " reports " + (sink ? "stop" : "doStuckDetection, timeoutPath, stop") + " (" + hooked[0] + " methods)");
        return out;
    }

    private BrainAgent() {}

    public static void premain(String args, Instrumentation inst) {
        if (args != null && !args.isEmpty()) {
            memories = args.split(",");
            panicOn = java.util.Arrays.asList(memories).contains("+panic");
            pathOn = java.util.Arrays.asList(memories).contains("+path");
            navOn = java.util.Arrays.asList(memories).contains("+nav");
        }
        System.getProperties().put(NAV_HOOK, (BiConsumer<Object, Object>) BrainAgent::onNav);
        System.getProperties().put(ERASE, (BiConsumer<Object, Object>) BrainAgent::onErase);
        System.getProperties().put(SET, (BiConsumer<Object, Object>) BrainAgent::onSet);
        System.getProperties().put(ACTIVITY, (BiConsumer<Object, Object>) BrainAgent::onActivity);
        System.getProperties().put(MARK, (BiConsumer<Object, Object>) BrainAgent::onMark);
        System.getProperties().put(OWNER, (BiConsumer<Object, Object>) BrainAgent::onOwner);
        System.getProperties().put(SPOT, (BiFunction<Object, Object, Object>) BrainAgent::onSpot);
        System.getProperties().put(PANIC, (BiConsumer<Object, Object>) BrainAgent::onPanic);
        System.getProperties().put(PATH_HOOK, (BiConsumer<Object, Object>) BrainAgent::onPath);
        inst.addTransformer(new ClassFileTransformer() {
            @Override
            public byte[] transform(ClassLoader loader, String name, Class<?> redefined, ProtectionDomain domain, byte[] bytes) {
                try {
                    if (BRAIN.equals(name)) {
                        byte[] out = instrument(loader, bytes);
                        System.err.println("mctas brain agent: Brain reports " + String.join(" ", memories) + " and activity switches");
                        return out;
                    }
                    if (LAND.equals(name) || DEFAULT.equals(name)) {
                        return mark(loader, bytes, name);
                    }
                    if (VILLAGER.equals(name)) {
                        return spot(loader, bytes);
                    }
                    if (PANIC_TRIGGER.equals(name)) {
                        return panic(loader, bytes);
                    }
                    if (PATH_CLASS.equals(name) && pathOn) {
                        return pathHook(loader, bytes);
                    }
                    if (navOn && (NAV_CLASS.equals(name) || SINK_CLASS.equals(name))) {
                        return navHook(loader, bytes, name);
                    }
                    return null;
                } catch (Throwable t) {
                    System.err.println("mctas brain agent: could not instrument " + name + ": " + t);
                    t.printStackTrace();
                    return null;
                }
            }
        });
    }

    private static final String MARK = "mctas.brain.mark";
    private static final String OWNER = "mctas.brain.owner";
    private static final String SPOT = "mctas.brain.spot";
    private static final String PANIC = "mctas.brain.panic";
    private static final String PATH_HOOK = "mctas.brain.path";
    private static final String VILLAGER = "net/minecraft/world/entity/npc/villager/Villager";
    private static final String PANIC_TRIGGER = "net/minecraft/world/entity/ai/behavior/VillagerPanicTrigger";
    private static final String PATH_CLASS = "net/minecraft/world/level/pathfinder/Path";

    /** (ironspec 22:11 PT: A6's restart trap, the mechanism) a Path's node index moving: `advance()`
     * and `setNextNodeIndex(int)`, entered, printed as {@code path <id> advance from <i>} or
     * {@code path <id> set index <i> (was <j>)} when the agent's argument list holds {@code +path};
     * the id is the Path's identity hash, as the PATH memory's set line prints it. */
    static void onPath(Object path, Object arg) {
        if (!pathOn) {
            return;
        }
        try {
            int was = (Integer) path.getClass().getMethod("getNextNodeIndex").invoke(path);
            int id = System.identityHashCode(path);
            if (arg == null) {
                System.err.println("mctas brain agent: path " + id + " advance from " + was);
            } else {
                System.err.println("mctas brain agent: path " + id + " set index " + arg + " (was " + was + ")");
            }
        } catch (ReflectiveOperationException e) {
            System.err.println("mctas brain agent: path (unread): " + e);
        }
    }

    /** The hook at the entry of Path's `advance()` and `setNextNodeIndex(int)`. */
    static byte[] pathHook(ClassLoader loader, byte[] bytes) {
        ClassHierarchyResolver resolver = ClassHierarchyResolver.defaultResolver();
        if (loader != null) {
            resolver = ClassHierarchyResolver.ofClassLoading(loader).orElse(resolver);
        }
        ClassFile cf = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(resolver));
        ClassModel model = cf.parse(bytes);
        int[] hooked = {0};
        byte[] out = cf.transformClass(model, (cb, ce) -> {
            if (ce instanceof java.lang.classfile.MethodModel m && m.code().isPresent()
                    && ((m.methodName().equalsString("advance") && m.methodTypeSymbol().parameterCount() == 0)
                        || (m.methodName().equalsString("setNextNodeIndex") && m.methodTypeSymbol().parameterCount() == 1))) {
                boolean set = m.methodName().equalsString("setNextNodeIndex");
                hooked[0]++;
                cb.transformMethod(m, java.lang.classfile.MethodTransform.transformingCode(new java.lang.classfile.CodeTransform() {
                    boolean first = true;

                    @Override
                    public void accept(CodeBuilder b, java.lang.classfile.CodeElement e) {
                        if (first) {
                            first = false;
                            ClassDesc properties = ClassDesc.of("java.util.Properties");
                            ClassDesc biConsumer = ClassDesc.of("java.util.function.BiConsumer");
                            b.invokestatic(ClassDesc.of("java.lang.System"), "getProperties", MethodTypeDesc.of(properties));
                            b.ldc(PATH_HOOK);
                            b.invokevirtual(properties, "get", MethodTypeDesc.of(ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                            b.checkcast(biConsumer);
                            b.aload(0);
                            if (set) {
                                b.iload(1);
                                b.invokestatic(ClassDesc.of("java.lang.Integer"), "valueOf", MethodTypeDesc.of(ClassDesc.of("java.lang.Integer"), ConstantDescs.CD_int));
                            } else {
                                b.aconst_null();
                            }
                            b.invokeinterface(biConsumer, "accept", MethodTypeDesc.of(ConstantDescs.CD_void, ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                        }
                        b.with(e);
                    }
                }));
            } else {
                cb.with(ce);
            }
        });
        System.err.println("mctas brain agent: Path reports advance and setNextNodeIndex (" + hooked[0] + " methods)");
        return out;
    }

    /** (ironspec 19:51 PT: the 60-gt behaviour's skip) VillagerPanicTrigger's own `start` and `tick`
     * (level, villager, timestamp), entered: printed as {@code panic <start|tick> by <UUID> at <t>}
     * when the agent's argument list holds {@code +panic} (it would print every tick otherwise). */
    static void onPanic(Object what, Object args) {
        if (!panicOn) {
            return;
        }
        Object[] a = (Object[]) args;
        try {
            String uuid = (String) a[0].getClass().getMethod("getStringUUID").invoke(a[0]);
            System.err.println("mctas brain agent: panic " + what + " by " + uuid + " at " + a[1]);
        } catch (ReflectiveOperationException e) {
            System.err.println("mctas brain agent: panic " + what + " (unattributed): " + e);
        }
    }

    /** The hook at the entry of VillagerPanicTrigger's `start` and `tick`. */
    static byte[] panic(ClassLoader loader, byte[] bytes) {
        ClassHierarchyResolver resolver = ClassHierarchyResolver.defaultResolver();
        if (loader != null) {
            resolver = ClassHierarchyResolver.ofClassLoading(loader).orElse(resolver);
        }
        ClassFile cf = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(resolver));
        ClassModel model = cf.parse(bytes);
        int[] hooked = {0};
        byte[] out = cf.transformClass(model, (cb, ce) -> {
            if (ce instanceof java.lang.classfile.MethodModel m && m.code().isPresent()
                    && (m.methodName().equalsString("start") || m.methodName().equalsString("tick"))
                    && m.methodTypeSymbol().parameterCount() == 3
                    && m.methodTypeSymbol().parameterType(2).equals(ConstantDescs.CD_long)
                    && !m.flags().has(java.lang.reflect.AccessFlag.BRIDGE)
                    && !m.flags().has(java.lang.reflect.AccessFlag.SYNTHETIC)) {
                String what = m.methodName().stringValue();
                hooked[0]++;
                cb.transformMethod(m, java.lang.classfile.MethodTransform.transformingCode(new java.lang.classfile.CodeTransform() {
                    boolean first = true;

                    @Override
                    public void accept(CodeBuilder b, java.lang.classfile.CodeElement e) {
                        if (first) {
                            first = false;
                            ClassDesc properties = ClassDesc.of("java.util.Properties");
                            ClassDesc biConsumer = ClassDesc.of("java.util.function.BiConsumer");
                            b.invokestatic(ClassDesc.of("java.lang.System"), "getProperties", MethodTypeDesc.of(properties));
                            b.ldc(PANIC);
                            b.invokevirtual(properties, "get", MethodTypeDesc.of(ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                            b.checkcast(biConsumer);
                            b.ldc(what);
                            // new Object[] { villager (local 2), Long.valueOf(timestamp (locals 3-4)) }
                            b.iconst_2();
                            b.anewarray(ConstantDescs.CD_Object);
                            b.dup();
                            b.iconst_0();
                            b.aload(2);
                            b.aastore();
                            b.dup();
                            b.iconst_1();
                            b.lload(3);
                            b.invokestatic(ClassDesc.of("java.lang.Long"), "valueOf", MethodTypeDesc.of(ClassDesc.of("java.lang.Long"), ConstantDescs.CD_long));
                            b.aastore();
                            b.invokeinterface(biConsumer, "accept", MethodTypeDesc.of(ConstantDescs.CD_void, ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                        }
                        b.with(e);
                    }
                }));
            } else {
                cb.with(ce);
            }
        });
        System.err.println("mctas brain agent: VillagerPanicTrigger reports start and tick (" + hooked[0] + " methods)");
        return out;
    }

    /** (ironspec 19:33 PT: spot misses classified directly) `Villager.spawnGolemIfNeeded`'s
     * `SpawnUtil.trySpawnMob` result: an empty Optional (no spot found in its tries) is printed as
     * {@code no spot by <the caller's UUID> at <game time>}; the Optional is returned as is. */
    static Object onSpot(Object spawned, Object villager) {
        if (spawned instanceof Optional<?> o && o.isEmpty()) {
            try {
                String uuid = (String) villager.getClass().getMethod("getStringUUID").invoke(villager);
                Object level = villager.getClass().getMethod("level").invoke(villager);
                long time = (Long) level.getClass().getMethod("getGameTime").invoke(level);
                System.err.println("mctas brain agent: no spot by " + uuid + " at " + time);
            } catch (ReflectiveOperationException e) {
                System.err.println("mctas brain agent: no spot (unattributed): " + e);
            }
        }
        return spawned;
    }

    /** The hook after spawnGolemIfNeeded's one trySpawnMob call: [Optional] -> [Optional]. */
    static byte[] spot(ClassLoader loader, byte[] bytes) {
        ClassHierarchyResolver resolver = ClassHierarchyResolver.defaultResolver();
        if (loader != null) {
            resolver = ClassHierarchyResolver.ofClassLoading(loader).orElse(resolver);
        }
        ClassFile cf = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(resolver));
        ClassModel model = cf.parse(bytes);
        ClassDesc properties = ClassDesc.of("java.util.Properties");
        ClassDesc biFunction = ClassDesc.of("java.util.function.BiFunction");
        int[] calls = {0};
        byte[] out = cf.transformClass(model, ClassTransform.transformingMethodBodies(
            m -> m.methodName().equalsString("spawnGolemIfNeeded"),
            (b, e) -> {
                b.with(e);
                if (e instanceof InvokeInstruction i
                        && i.owner().asInternalName().equals("net/minecraft/util/SpawnUtil")
                        && i.name().equalsString("trySpawnMob")) {
                    calls[0]++;
                    b.invokestatic(ClassDesc.of("java.lang.System"), "getProperties", MethodTypeDesc.of(properties));
                    b.ldc(SPOT);
                    b.invokevirtual(properties, "get", MethodTypeDesc.of(ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                    b.checkcast(biFunction);
                    b.swap();
                    b.aload(0);
                    b.invokeinterface(biFunction, "apply", MethodTypeDesc.of(ConstantDescs.CD_Object, ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                    b.checkcast(ClassDesc.of("java.util.Optional"));
                }
            }));
        if (calls[0] != 1) {
            throw new IllegalStateException("spawnGolemIfNeeded calls SpawnUtil.trySpawnMob " + calls[0] + " times (1 expected)");
        }
        System.err.println("mctas brain agent: Villager.spawnGolemIfNeeded reports trySpawnMob's empty returns");
        return out;
    }
    private static final String LAND = "net/minecraft/world/entity/ai/util/LandRandomPos";
    private static final String DEFAULT = "net/minecraft/world/entity/ai/util/DefaultRandomPos";
    /** The mob whose brain is ticking (each line names it: several villagers in one world). */
    private static final ThreadLocal<String> CURRENT = new ThreadLocal<>();

    static void onOwner(Object brain, Object body) {
        CURRENT_BODY.set(body);
        try {
            CURRENT.set((String) body.getClass().getMethod("getStringUUID").invoke(body));
        } catch (ReflectiveOperationException e) {
            CURRENT.set("?");
        }
    }

    /** " @<uuid>" of the mob whose brain is ticking, or " @none" outside any brain tick (a summon,
     * a command, a brain rebuilt on load: the thread-local would still name the last mob ticked). */
    private static String owner() {
        String u = CURRENT.get();
        if (u == null) {
            return " @none";
        }
        boolean inTick = StackWalker.getInstance().walk(s -> s.anyMatch(f -> f.getMethodName().equals("tick") && f.getClassName().equals("net.minecraft.world.entity.ai.Brain")));
        return inTick ? " @" + u : " @none";
    }

    /** A random-position draw (its method), printed when VillageBoundRandomStroll made it. */
    static void onMark(Object method, Object unused) {
        for (StackTraceElement e : new Throwable().getStackTrace()) {
            if (e.getClassName().endsWith(".VillageBoundRandomStroll")) {
                System.err.println("mctas brain agent: random pos " + method + " by VillageBoundRandomStroll" + owner());
                return;
            }
        }
    }

    /** Every public static method of LandRandomPos / DefaultRandomPos reports its name at entry. */
    static byte[] mark(ClassLoader loader, byte[] bytes, String name) {
        ClassHierarchyResolver resolver = ClassHierarchyResolver.defaultResolver();
        if (loader != null) {
            resolver = ClassHierarchyResolver.ofClassLoading(loader).orElse(resolver);
        }
        ClassFile cf = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(resolver));
        ClassModel model = cf.parse(bytes);
        String simple = name.substring(name.lastIndexOf('/') + 1);
        return cf.transformClass(model, (cb, ce) -> {
            if (ce instanceof java.lang.classfile.MethodModel m && m.code().isPresent()
                    && m.flags().has(java.lang.reflect.AccessFlag.STATIC) && m.flags().has(java.lang.reflect.AccessFlag.PUBLIC)) {
                String label = simple + "." + m.methodName().stringValue() + "/" + m.methodTypeSymbol().parameterCount();
                cb.transformMethod(m, java.lang.classfile.MethodTransform.transformingCode(new java.lang.classfile.CodeTransform() {
                    boolean first = true;

                    @Override
                    public void accept(CodeBuilder b, java.lang.classfile.CodeElement e) {
                        if (first) {
                            first = false;
                            ClassDesc properties = ClassDesc.of("java.util.Properties");
                            ClassDesc biConsumer = ClassDesc.of("java.util.function.BiConsumer");
                            b.invokestatic(ClassDesc.of("java.lang.System"), "getProperties", MethodTypeDesc.of(properties));
                            b.ldc(MARK);
                            b.invokevirtual(properties, "get", MethodTypeDesc.of(ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                            b.checkcast(biConsumer);
                            b.ldc(label);
                            b.aconst_null();
                            b.invokeinterface(biConsumer, "accept", MethodTypeDesc.of(ConstantDescs.CD_void, ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                        }
                        b.with(e);
                    }
                }));
            } else {
                cb.with(ce);
            }
        });
    }

    private static boolean watched(Object type) {
        String t = String.valueOf(type);
        for (String m : memories) {
            if (m.equals(t)) {
                return true;
            }
        }
        return false;
    }

    /** The first behaviour frame on the stack (the change's source), past the declarative wrappers. */
    private static String source() {
        for (StackTraceElement e : new Throwable().getStackTrace()) {
            String c = e.getClassName();
            if (c.contains(".behavior.declarative.")) {
                continue;
            }
            String s = c.substring(c.lastIndexOf('.') + 1);
            if (s.equals("Behavior") || s.equals("OneShot") || s.equals("GateBehavior") || s.equals("RunOne") || s.equals("ShufflingList")) {
                continue;
            }
            if (c.startsWith("net.minecraft.world.entity.ai.behavior.") || c.startsWith("net.minecraft.world.entity.npc.")) {
                return s + "." + e.getMethodName() + ":" + e.getLineNumber();
            }
        }
        return "elsewhere";
    }

    private static String describe(Object v) {
        if (v == null) {
            return "none";
        }
        try {
            if (v.getClass().getSimpleName().equals("WalkTarget")) {
                Object tracker = v.getClass().getMethod("getTarget").invoke(v);
                Object pos = tracker.getClass().getMethod("currentPosition").invoke(tracker);
                Object close = v.getClass().getMethod("getCloseEnoughDist").invoke(v);
                return "walk target " + pos + " close enough " + close;
            }
            if (v.getClass().getSimpleName().equals("Path")) {
                Object target = v.getClass().getMethod("getTarget").invoke(v);
                Object nodes = v.getClass().getMethod("getNodeCount").invoke(v);
                Object reach = v.getClass().getMethod("canReach").invoke(v);
                String start = "";
                if (((Integer) nodes) > 0) {
                    Object first = v.getClass().getMethod("getNode", int.class).invoke(v, 0);
                    start = " start (" + first.getClass().getField("x").get(first) + ", " + first.getClass().getField("y").get(first) + ", " + first.getClass().getField("z").get(first) + ")";
                }
                String end = "";
                Object node = v.getClass().getMethod("getEndNode").invoke(v);
                if (node != null) {
                    end = " end (" + node.getClass().getField("x").get(node) + ", " + node.getClass().getField("y").get(node) + ", " + node.getClass().getField("z").get(node) + ")";
                }
                // (+path) the identity id, the next index and every node, for the advance lines
                String all = "";
                if (pathOn) {
                    StringBuilder sb = new StringBuilder(" id " + System.identityHashCode(v) + " next " + v.getClass().getMethod("getNextNodeIndex").invoke(v) + " nodes [");
                    for (int i = 0; i < (Integer) nodes; i++) {
                        Object n = v.getClass().getMethod("getNode", int.class).invoke(v, i);
                        sb.append(i == 0 ? "" : " ").append("(").append(n.getClass().getField("x").get(n)).append(",").append(n.getClass().getField("y").get(n)).append(",").append(n.getClass().getField("z").get(n)).append(")");
                    }
                    all = sb.append("]").toString();
                }
                return "path to " + target + " nodes " + nodes + " reach " + reach + start + end + all;
            }
        } catch (ReflectiveOperationException e) {
            return String.valueOf(v);
        }
        return String.valueOf(v);
    }

    static void onErase(Object brain, Object type) {
        if (!watched(type)) {
            return;
        }
        // every erase call is printed, with whether the memory was there (an erase of an absent
        // stamp still marks a reached or completely pathed walk target)
        String had = "";
        try {
            boolean present = (Boolean) brain.getClass().getMethod("hasMemoryValue", type.getClass()).invoke(brain, type);
            had = present ? " (had it)" : " (absent)";
        } catch (ReflectiveOperationException e) {
            had = " (?)";
        }
        System.err.println("mctas brain agent: erase " + type + had + " by " + source() + owner());
    }

    static void onSet(Object type, Object value) {
        if (!watched(type)) {
            return;
        }
        // (+nav) a PATH set carries the rider's own state: position, box, isInWater, getSpeed
        String rider = "";
        if (navOn && value != null && value.getClass().getSimpleName().equals("Path") && CURRENT_BODY.get() != null && !owner().equals(" @none")) {
            try {
                rider = " | rider " + riderState(CURRENT_BODY.get());
            } catch (ReflectiveOperationException e) {
                rider = " | rider (unread: " + e + ")";
            }
        }
        System.err.println("mctas brain agent: " + (value == null ? "erase " + type : "set " + type + " = " + describe(value)) + " by " + source() + rider + owner());
    }

    static void onActivity(Object brain, Object activity) {
        try {
            boolean active = (Boolean) brain.getClass().getMethod("isActive", activity.getClass()).invoke(brain, activity);
            if (active) {
                return;
            }
        } catch (ReflectiveOperationException e) {
            // print it anyway
        }
        System.err.println("mctas brain agent: activity " + activity + " by " + source() + sink(brain) + owner());
    }

    /** MoveToTargetSink's state (its status, remainingCooldown), read from the brain's behaviours. */
    private static String sink(Object brain) {
        try {
            java.lang.reflect.Field f = brain.getClass().getDeclaredField("availableBehaviorsByPriority");
            f.setAccessible(true);
            for (Object byActivity : ((java.util.Map<?, ?>) f.get(brain)).values()) {
                for (Object set : ((java.util.Map<?, ?>) byActivity).values()) {
                    for (Object b : (java.util.Set<?>) set) {
                        if (b.getClass().getSimpleName().equals("MoveToTargetSink")) {
                            java.lang.reflect.Field cd = b.getClass().getDeclaredField("remainingCooldown");
                            cd.setAccessible(true);
                            Object status = b.getClass().getMethod("getStatus").invoke(b);
                            return " (MoveToTargetSink " + status + ", remainingCooldown " + cd.get(b) + ")";
                        }
                    }
                }
            }
        } catch (Throwable t) {
            return " (MoveToTargetSink unread: " + t + ")";
        }
        return " (no MoveToTargetSink)";
    }

    private static void hook(CodeBuilder b, String key, int a, int c) {
        ClassDesc properties = ClassDesc.of("java.util.Properties");
        ClassDesc biConsumer = ClassDesc.of("java.util.function.BiConsumer");
        b.invokestatic(ClassDesc.of("java.lang.System"), "getProperties", MethodTypeDesc.of(properties));
        b.ldc(key);
        b.invokevirtual(properties, "get", MethodTypeDesc.of(ConstantDescs.CD_Object, ConstantDescs.CD_Object));
        b.checkcast(biConsumer);
        b.aload(a);
        b.aload(c);
        b.invokeinterface(biConsumer, "accept", MethodTypeDesc.of(ConstantDescs.CD_void, ConstantDescs.CD_Object, ConstantDescs.CD_Object));
    }

    static byte[] instrument(ClassLoader loader, byte[] bytes) {
        ClassHierarchyResolver resolver = ClassHierarchyResolver.defaultResolver();
        if (loader != null) {
            resolver = ClassHierarchyResolver.ofClassLoading(loader).orElse(resolver);
        }
        ClassFile cf = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(resolver));
        ClassModel model = cf.parse(bytes);
        int[] done = {0};
        byte[] out = cf.transformClass(model, (cb, ce) -> {
            if (ce instanceof java.lang.classfile.MethodModel m && m.code().isPresent()) {
                String n = m.methodName().stringValue();
                String d = m.methodType().stringValue();
                String key;
                int a;
                int c;
                if (n.equals("eraseMemory") && d.startsWith("(Lnet/minecraft/world/entity/ai/memory/MemoryModuleType;)")) {
                    key = ERASE;
                    a = 0;
                    c = 1;
                } else if (n.equals("setMemoryInternal") && d.startsWith("(Lnet/minecraft/world/entity/ai/memory/MemoryModuleType;Ljava/lang/Object;")) {
                    key = SET;
                    a = 1;
                    c = 2;
                } else if (n.equals("setActiveActivity")) {
                    key = ACTIVITY;
                    a = 0;
                    c = 1;
                } else if (n.equals("tick") && d.startsWith("(Lnet/minecraft/server/level/ServerLevel;Lnet/minecraft/world/entity/LivingEntity;)")) {
                    key = OWNER;
                    a = 0;
                    c = 2;
                } else {
                    cb.with(ce);
                    return;
                }
                done[0]++;
                final String k = key;
                final int fa = a;
                final int fc = c;
                cb.transformMethod(m, java.lang.classfile.MethodTransform.transformingCode(new java.lang.classfile.CodeTransform() {
                    boolean first = true;

                    @Override
                    public void accept(CodeBuilder b, java.lang.classfile.CodeElement e) {
                        if (first) {
                            first = false;
                            hook(b, k, fa, fc);
                        }
                        b.with(e);
                    }
                }));
            } else {
                cb.with(ce);
            }
        });
        if (done[0] < 5) {
            throw new IllegalStateException("Brain: " + done[0] + " of 5 methods found (eraseMemory, two setMemoryInternal, setActiveActivity, tick)");
        }
        return out;
    }
}
