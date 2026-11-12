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
import java.util.function.Consumer;

/**
 * (tickexact 2026-11-12, the rail-T probe) A Java agent for the official server jar: every call of
 * {@code BaseRailBlock.updateDir(Level, BlockPos, BlockState, boolean)} (a rail re-settling its
 * shape from its neighbours) at a watched position prints its call stack to stderr, so the path
 * that re-settles a rail is named. The agent's argument is the watched positions,
 * {@code x,y,z[;x,y,z...]}. As in the golem agent, the injected code reaches the hook through a JDK
 * type only (a {@code Consumer} kept in the system properties).
 */
public final class RailAgent {
    private static final String HOOK = "mctas.rail.hook";
    private static final String BASE_RAIL = "net/minecraft/world/level/block/BaseRailBlock";
    private static volatile String[] watched = new String[0];

    private RailAgent() {}

    public static void premain(String args, Instrumentation inst) {
        if (args != null && !args.isEmpty()) {
            String[] parts = args.split(";");
            String[] w = new String[parts.length];
            for (int i = 0; i < parts.length; i++) {
                String[] c = parts[i].split(",");
                // BlockPos.toString(): "BlockPos{x=1, y=2, z=3}"
                w[i] = "BlockPos{x=" + c[0].trim() + ", y=" + c[1].trim() + ", z=" + c[2].trim() + "}";
            }
            watched = w;
        }
        System.getProperties().put(HOOK, (Consumer<Object>) RailAgent::onUpdateDir);
        inst.addTransformer(new ClassFileTransformer() {
            @Override
            public byte[] transform(ClassLoader loader, String name, Class<?> redefined, ProtectionDomain domain, byte[] bytes) {
                if (!BASE_RAIL.equals(name)) {
                    return null;
                }
                try {
                    byte[] out = instrument(loader, bytes);
                    System.err.println("mctas rail agent: BaseRailBlock.updateDir reports its calls at " + String.join(" ", watched));
                    return out;
                } catch (Throwable t) {
                    System.err.println("mctas rail agent: could not instrument BaseRailBlock: " + t);
                    t.printStackTrace();
                    return null;
                }
            }
        });
    }

    /** At a watched position: print the call stack (the frames inside net.minecraft). */
    static void onUpdateDir(Object pos) {
        String p = String.valueOf(pos);
        for (String w : watched) {
            if (w.equals(p)) {
                StringBuilder sb = new StringBuilder("mctas rail agent: updateDir at " + p + " from:");
                for (StackTraceElement e : new Throwable().getStackTrace()) {
                    String c = e.getClassName();
                    if (c.startsWith("net.minecraft")) {
                        sb.append("\n  mctas rail agent:   ").append(c).append('.').append(e.getMethodName()).append(':').append(e.getLineNumber());
                    }
                }
                System.err.println(sb);
                return;
            }
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
        ClassDesc consumer = ClassDesc.of("java.util.function.Consumer");
        boolean[] done = {false};
        byte[] out = cf.transformClass(model, ClassTransform.transformingMethodBodies(
            m -> m.methodName().equalsString("updateDir"),
            (b, e) -> {
                if (!done[0]) {
                    done[0] = true;
                    // [] -> [Consumer] -> [Consumer, pos] -> []
                    b.invokestatic(ClassDesc.of("java.lang.System"), "getProperties", MethodTypeDesc.of(properties));
                    b.ldc(HOOK);
                    b.invokevirtual(properties, "get", MethodTypeDesc.of(ConstantDescs.CD_Object, ConstantDescs.CD_Object));
                    b.checkcast(consumer);
                    b.aload(2);
                    b.invokeinterface(consumer, "accept", MethodTypeDesc.of(ConstantDescs.CD_void, ConstantDescs.CD_Object));
                }
                b.with(e);
            }));
        if (!done[0]) {
            throw new IllegalStateException("BaseRailBlock has no updateDir");
        }
        return out;
    }
}
