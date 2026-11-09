package dev.vjeux.mctas.agent;

import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassHierarchyResolver;
import java.lang.classfile.ClassModel;
import java.lang.classfile.ClassTransform;
import java.lang.classfile.instruction.InvokeInstruction;
import java.lang.constant.ClassDesc;
import java.lang.constant.ConstantDescs;
import java.lang.constant.MethodTypeDesc;
import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.security.ProtectionDomain;
import java.util.Optional;
import java.util.function.BiFunction;

/**
 * The mob-farm gate's Java agent (`-javaagent` on the official server jar): every iron golem a
 * villager summons gets the tag {@code mctas.by.<the villager's UUID>}, so the gate attributes a
 * golem by its summoner (one of the farm's villagers or the village's) instead of by where it
 * stands.
 *
 * <p>{@code Villager.spawnGolemIfNeeded} calls {@code SpawnUtil.trySpawnMob(IRON_GOLEM, ...)},
 * which returns the golem it added as an {@code Optional}; right after that call the agent hands
 * the Optional and the villager ({@code this}) to {@link #tag}, which tags the golem and returns
 * the same Optional (the method's stack is unchanged). The injected code reaches the hook through
 * a JDK type only (a {@code BiFunction} kept in the system properties): the server's classes live
 * in the bundler's class loader, which need not see the agent's.
 */
public final class GolemAgent {
    private static final String HOOK = "mctas.golem.hook";
    private static final String VILLAGER = "net/minecraft/world/entity/npc/villager/Villager";

    private GolemAgent() {}

    public static void premain(String args, Instrumentation inst) {
        System.getProperties().put(HOOK, (BiFunction<Object, Object, Object>) GolemAgent::tag);
        inst.addTransformer(new ClassFileTransformer() {
            @Override
            public byte[] transform(ClassLoader loader, String name, Class<?> redefined, ProtectionDomain domain, byte[] bytes) {
                if (!VILLAGER.equals(name)) {
                    return null;
                }
                try {
                    byte[] out = instrument(loader, bytes);
                    System.err.println("mctas golem agent: Villager.spawnGolemIfNeeded tags its golems");
                    return out;
                } catch (Throwable t) {
                    System.err.println("mctas golem agent: could not instrument Villager: " + t);
                    t.printStackTrace();
                    return null;
                }
            }
        });
    }

    /** Tag the golem {@code spawned} holds (if any) with its summoner's UUID; return it as is. */
    static Object tag(Object spawned, Object villager) {
        if (spawned instanceof Optional<?> o && o.isPresent()) {
            try {
                Object golem = o.get();
                String uuid = (String) villager.getClass().getMethod("getStringUUID").invoke(villager);
                golem.getClass().getMethod("addTag", String.class).invoke(golem, "mctas.by." + uuid);
            } catch (ReflectiveOperationException e) {
                System.err.println("mctas golem agent: could not tag a golem: " + e);
            }
        }
        return spawned;
    }

    static byte[] instrument(ClassLoader loader, byte[] bytes) {
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
                    // [Optional] -> [Optional, BiFunction] -> [BiFunction, Optional, this] -> [Optional]
                    b.invokestatic(ClassDesc.of("java.lang.System"), "getProperties", MethodTypeDesc.of(properties));
                    b.ldc(HOOK);
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
        return out;
    }
}
