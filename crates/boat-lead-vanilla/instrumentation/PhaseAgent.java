package dev.vjeux.mctas.boatprobe;

import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.security.ProtectionDomain;
import org.objectweb.asm.*;

/** Read-only phase taps: inserts only observer calls, retaining every original instruction. */
public final class PhaseAgent {
    public static void premain(String args, Instrumentation instrumentation) {
        instrumentation.addTransformer(new Taps());
    }

    private static final class Taps implements ClassFileTransformer {
        @Override
        public byte[] transform(ClassLoader loader, String name, Class<?> redef,
                ProtectionDomain domain, byte[] bytes) {
            if (!name.equals("net/minecraft/world/entity/Entity")
                    && !name.equals("net/minecraft/world/entity/Leashable")
                    && !name.equals("net/minecraft/world/entity/vehicle/boat/AbstractBoat")) return null;
            ClassReader reader = new ClassReader(bytes);
            ClassWriter writer = new ClassWriter(reader, ClassWriter.COMPUTE_MAXS);
            reader.accept(new ClassVisitor(Opcodes.ASM9, writer) {
                @Override
                public MethodVisitor visitMethod(int access, String method, String descriptor,
                        String signature, String[] exceptions) {
                    MethodVisitor base = super.visitMethod(access, method, descriptor, signature, exceptions);
                    String phase = phase(name, method, descriptor);
                    if (phase == null) return base;
                    int local = method.equals("tickLeash") ? 1 : 0;
                    return new MethodVisitor(Opcodes.ASM9, base) {
                        private void observe(String where) {
                            super.visitVarInsn(Opcodes.ALOAD, local);
                            super.visitLdcInsn(where);
                            super.visitMethodInsn(Opcodes.INVOKESTATIC,
                                    "dev/vjeux/mctas/boatprobe/PhaseObserver", "observe",
                                    "(Ljava/lang/Object;Ljava/lang/String;)V", false);
                        }
                        @Override public void visitCode() {
                            super.visitCode();
                            observe(phase + "_pre");
                        }
                        @Override public void visitInsn(int opcode) {
                            if (opcode == Opcodes.RETURN) observe(phase + "_post");
                            super.visitInsn(opcode);
                        }
                        @Override public void visitMethodInsn(int opcode, String owner,
                                String called, String desc, boolean itf) {
                            super.visitMethodInsn(opcode, owner, called, desc, itf);
                            if (phase.equals("ride") && called.equals("tick") && desc.equals("()V"))
                                observe("ride_after_tick");
                        }
                    };
                }
            }, 0);
            System.err.println("BOAT_PROBE_INSTRUMENTED " + name);
            return writer.toByteArray();
        }
    }

    private static String phase(String owner, String method, String desc) {
        if (owner.endsWith("/Entity") && method.equals("rideTick") && desc.equals("()V")) return "ride";
        if (owner.endsWith("/Leashable") && method.equals("tickLeash")
                && desc.equals("(Lnet/minecraft/server/level/ServerLevel;Lnet/minecraft/world/entity/Entity;)V")) return "leash";
        if (owner.endsWith("/AbstractBoat") && desc.equals("()V")) {
            if (method.equals("tick")) return "boat";
            if (method.equals("floatBoat")) return "float";
        }
        return null;
    }
}
