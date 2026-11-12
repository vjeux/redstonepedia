package dev.vjeux.mctas.agent;

import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassHierarchyResolver;
import java.lang.classfile.ClassModel;
import java.lang.classfile.ClassTransform;
import java.lang.constant.ClassDesc;
import java.lang.constant.ConstantDescs;
import java.lang.constant.MethodTypeDesc;
import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.security.ProtectionDomain;
import java.util.function.BiConsumer;

/**
 * (tickexact 2026-11-12, ironspec's pad-golem bridge) A Java agent for the official server jar:
 * every {@code Mob.setTarget(LivingEntity)} call by a mob whose type id is one of the agent's
 * argument ({@code minecraft:pillager,minecraft:iron_golem,...}) prints the game time, the mob and
 * its new target to stderr, so a harness can read a mob's target (it isn't in the saved data). As
 * in the golem agent, the injected code reaches the hook through a JDK type only (a
 * {@code BiConsumer} kept in the system properties).
 */
public final class TargetAgent {
    private static final String HOOK = "mctas.target.hook";
    private static final String MOB = "net/minecraft/world/entity/Mob";
    private static volatile String[] types = new String[0];

    private TargetAgent() {}

    public static void premain(String args, Instrumentation inst) {
        if (args != null && !args.isEmpty()) {
            types = args.split(",");
        }
        System.getProperties().put(HOOK, (BiConsumer<Object, Object>) TargetAgent::onSetTarget);
        inst.addTransformer(new ClassFileTransformer() {
            @Override
            public byte[] transform(ClassLoader loader, String name, Class<?> redefined, ProtectionDomain domain, byte[] bytes) {
                if (!MOB.equals(name)) {
                    return null;
                }
                try {
                    byte[] out = instrument(loader, bytes);
                    System.err.println("mctas target agent: Mob.setTarget reports the targets of " + String.join(" ", types));
                    return out;
                } catch (Throwable t) {
                    System.err.println("mctas target agent: could not instrument Mob: " + t);
                    t.printStackTrace();
                    return null;
                }
            }
        });
    }

    private static String describe(Object e) throws ReflectiveOperationException {
        if (e == null) {
            return "none";
        }
        Object type = e.getClass().getMethod("getType").invoke(e);
        Object key = type.getClass().getMethod("builtInRegistryHolder").invoke(type);
        Object rk = key.getClass().getMethod("key").invoke(key);
        Object id = rk.getClass().getMethod("identifier").invoke(rk);
        String uuid = (String) e.getClass().getMethod("getStringUUID").invoke(e);
        return id + " " + uuid;
    }

    /** A watched mob's new target: game time, the mob, the target (or none). */
    static void onSetTarget(Object mob, Object target) {
        try {
            String m = describe(mob);
            boolean watched = false;
            for (String t : types) {
                if (m.startsWith(t + " ")) {
                    watched = true;
                }
            }
            if (!watched) {
                return;
            }
            Object current = mob.getClass().getMethod("getTarget").invoke(mob);
            if (current == target) {
                return;
            }
            Object level = mob.getClass().getMethod("level").invoke(mob);
            long time = (Long) level.getClass().getMethod("getGameTime").invoke(level);
            System.err.println("mctas target agent: t=" + time + " " + m + " -> " + describe(target));
        } catch (Throwable t) {
            System.err.println("mctas target agent: could not describe a target: " + t);
        }
    }

    static byte[] instrument(ClassLoader loader, byte[] bytes) {
        ClassHierarchyResolver resolver = ClassHierarchyResolver.defaultResolver();
        if (loader != null) {
            resolver = ClassHierarchyResolver.ofClassLoading(loader).orElse(resolver);
        }
        ClassFile cf = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(resolver));
        ClassModel model = cf.parse(bytes);
        ClassDesc properties = ClassDesc.of("java.util.Properties");
        ClassDesc biConsumer = ClassDesc.of("java.util.function.BiConsumer");
        boolean[] done = {false};
        byte[] out = cf.transformClass(model, ClassTransform.transformingMethodBodies(
            m -> m.methodName().equalsString("setTarget") && m.methodTypeSymbol().parameterCount() == 1,
            (b, e) -> {
                if (!done[0]) {
                    done[0] = true;
                    // [] -> [BiConsumer] -> [BiConsumer, this, target] -> []
                    b.invokestatic(ClassDesc.of("java.lang.System"), "getProperties", MethodTypeDesc.of(properties));
                    b.ldc(HOOK);
                    b.invokevirtual(properties, "get", MethodTypeDesc.of(ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                    b.checkcast(biConsumer);
                    b.aload(0);
                    b.aload(1);
                    b.invokeinterface(biConsumer, "accept", MethodTypeDesc.of(ConstantDescs.CD_void, ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                }
                b.with(e);
            }));
        if (!done[0]) {
            throw new IllegalStateException("Mob has no setTarget(LivingEntity)");
        }
        return out;
    }
}
