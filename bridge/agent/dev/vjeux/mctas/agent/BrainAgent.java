package dev.vjeux.mctas.agent;

import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassHierarchyResolver;
import java.lang.classfile.ClassModel;
import java.lang.classfile.ClassTransform;
import java.lang.classfile.CodeBuilder;
import java.lang.constant.ClassDesc;
import java.lang.constant.ConstantDescs;
import java.lang.constant.MethodTypeDesc;
import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.security.ProtectionDomain;
import java.util.function.BiConsumer;

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

    private BrainAgent() {}

    public static void premain(String args, Instrumentation inst) {
        if (args != null && !args.isEmpty()) {
            memories = args.split(",");
        }
        System.getProperties().put(ERASE, (BiConsumer<Object, Object>) BrainAgent::onErase);
        System.getProperties().put(SET, (BiConsumer<Object, Object>) BrainAgent::onSet);
        System.getProperties().put(ACTIVITY, (BiConsumer<Object, Object>) BrainAgent::onActivity);
        System.getProperties().put(MARK, (BiConsumer<Object, Object>) BrainAgent::onMark);
        System.getProperties().put(OWNER, (BiConsumer<Object, Object>) BrainAgent::onOwner);
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
    private static final String LAND = "net/minecraft/world/entity/ai/util/LandRandomPos";
    private static final String DEFAULT = "net/minecraft/world/entity/ai/util/DefaultRandomPos";
    /** The mob whose brain is ticking (each line names it: several villagers in one world). */
    private static final ThreadLocal<String> CURRENT = new ThreadLocal<>();

    static void onOwner(Object brain, Object body) {
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
                return "path to " + target + " nodes " + nodes + " reach " + reach + start + end;
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
        System.err.println("mctas brain agent: " + (value == null ? "erase " + type : "set " + type + " = " + describe(value)) + " by " + source() + owner());
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
