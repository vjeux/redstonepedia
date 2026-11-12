package dev.vjeux.mctas.agent;

import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassHierarchyResolver;
import java.lang.classfile.ClassModel;
import java.lang.classfile.ClassTransform;
import java.lang.classfile.CodeBuilder;
import java.lang.classfile.CodeElement;
import java.lang.classfile.CodeTransform;
import java.lang.classfile.MethodModel;
import java.lang.classfile.MethodTransform;
import java.lang.classfile.instruction.ReturnInstruction;
import java.lang.constant.ClassDesc;
import java.lang.constant.ConstantDescs;
import java.lang.constant.MethodTypeDesc;
import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.lang.reflect.Method;
import java.security.ProtectionDomain;
import java.util.HashMap;
import java.util.Map;
import java.util.function.BiFunction;

/**
 * (tickexact 2026-11-11, ironspec's chunk-loader study) The load-window agent (`-javaagent` on the
 * official server jar): from the server's first tick, for each watched chunk of the overworld, one
 * line per tick on stderr: whether its scheduled block ticks run
 * ({@code ServerLevel.isPositionTickingWithEntitiesLoaded}), whether its block events and block
 * entities run ({@code shouldTickBlocksAt}), whether its entities are loaded and whether they tick
 * ({@code isPositionEntityTicking}), and how many scheduled block ticks and entity ticks it ran
 * that tick ({@code ServerLevel.tickBlock}, {@code ServerLevel.tickNonPassenger}, counted at their
 * entry). Arguments: {@code watch=X0,Z0,X1,Z1} (chunk coordinates) and {@code ticks=N} (a line for
 * each of the level's first N ticks; after them, a line only for a tick where a watched chunk's
 * state changed, or where its scheduled ticks or block events run while its entities don't tick).
 *
 * <p>The injected code reaches the hooks through JDK types only (a {@code BiFunction} each, kept
 * in the system properties), as the golem agent does: the server's classes live in the bundler's
 * class loader, which need not see the agent's.
 */
public final class LoadAgent {
    private static final String END = "mctas.load.end";
    private static final String BLOCK = "mctas.load.blocktick";
    private static final String ENTITY = "mctas.load.entitytick";
    private static final String LEVEL = "net/minecraft/server/level/ServerLevel";

    private static int x0, z0, x1, z1, ticks = 400;
    private static int levelTicks = 0;
    private static final Map<Long, int[]> counts = new HashMap<>();
    /** Each watched chunk's flags at the end of the last tick (st 1, bt 2, el 4, et 8). */
    private static final Map<Long, Integer> previous = new HashMap<>();

    private LoadAgent() {}

    public static void premain(String args, Instrumentation inst) {
        x0 = z0 = x1 = z1 = 0;
        if (args != null) {
            for (String a : args.split(";")) {
                String[] kv = a.split("=", 2);
                if (kv.length == 2 && kv[0].equals("watch")) {
                    String[] p = kv[1].split(",");
                    x0 = Integer.parseInt(p[0].trim());
                    z0 = Integer.parseInt(p[1].trim());
                    x1 = Integer.parseInt(p[2].trim());
                    z1 = Integer.parseInt(p[3].trim());
                } else if (kv.length == 2 && kv[0].equals("ticks")) {
                    ticks = Integer.parseInt(kv[1].trim());
                }
            }
        }
        System.getProperties().put(END, (BiFunction<Object, Object, Object>) LoadAgent::end);
        System.getProperties().put(BLOCK, (BiFunction<Object, Object, Object>) LoadAgent::block);
        System.getProperties().put(ENTITY, (BiFunction<Object, Object, Object>) LoadAgent::entity);
        inst.addTransformer(new ClassFileTransformer() {
            @Override
            public byte[] transform(ClassLoader loader, String name, Class<?> redefined, ProtectionDomain domain, byte[] bytes) {
                if (!LEVEL.equals(name)) {
                    return null;
                }
                try {
                    byte[] out = instrument(loader, bytes);
                    System.err.println("mctas load agent: ServerLevel instrumented (watching chunks " + x0 + "," + z0 + " to " + x1 + "," + z1 + ", " + ticks + " ticks)");
                    return out;
                } catch (Throwable t) {
                    System.err.println("mctas load agent: could not instrument ServerLevel: " + t);
                    t.printStackTrace();
                    return null;
                }
            }
        });
    }

    private static boolean overworld(Object level) throws ReflectiveOperationException {
        return String.valueOf(level.getClass().getMethod("dimension").invoke(level)).contains("minecraft:overworld");
    }

    private static boolean watched(int cx, int cz) {
        return cx >= x0 && cx <= x1 && cz >= z0 && cz <= z1;
    }

    private static long key(int cx, int cz) {
        return ((long) cx & 0xFFFFFFFFL) | (((long) cz & 0xFFFFFFFFL) << 32);
    }

    /** A scheduled block tick ran at {@code pos} (a BlockPos): counted for its chunk. */
    static Object block(Object level, Object pos) {
        try {
            if (!overworld(level)) {
                return null;
            }
            int x = (int) pos.getClass().getMethod("getX").invoke(pos);
            int z = (int) pos.getClass().getMethod("getZ").invoke(pos);
            if (watched(x >> 4, z >> 4)) {
                counts.computeIfAbsent(key(x >> 4, z >> 4), k -> new int[2])[0]++;
            }
        } catch (ReflectiveOperationException e) {
            System.err.println("mctas load agent: block tick: " + e);
        }
        return null;
    }

    /** An entity ticked: counted for its chunk. */
    static Object entity(Object level, Object entity) {
        try {
            if (!overworld(level)) {
                return null;
            }
            Object cp = entity.getClass().getMethod("chunkPosition").invoke(entity);
            int cx = (int) cp.getClass().getMethod("x").invoke(cp);
            int cz = (int) cp.getClass().getMethod("z").invoke(cp);
            if (watched(cx, cz)) {
                counts.computeIfAbsent(key(cx, cz), k -> new int[2])[1]++;
            }
        } catch (ReflectiveOperationException e) {
            System.err.println("mctas load agent: entity tick: " + e);
        }
        return null;
    }

    /** The overworld's tick ended: one line per watched chunk. */
    static Object end(Object level, Object unused) {
        try {
            if (!overworld(level)) {
                return null;
            }
            Class<?> c = level.getClass();
            Method sched = c.getMethod("isPositionTickingWithEntitiesLoaded", long.class);
            Method blocks = c.getMethod("shouldTickBlocksAt", long.class);
            Method loaded = c.getMethod("areEntitiesLoaded", long.class);
            Method ticking = c.getMethod("isPositionEntityTicking", Class.forName("net.minecraft.core.BlockPos", false, c.getClassLoader()));
            java.lang.reflect.Constructor<?> bp = Class.forName("net.minecraft.core.BlockPos", false, c.getClassLoader()).getConstructor(int.class, int.class, int.class);
            long gt = (long) c.getMethod("getGameTime").invoke(level);
            StringBuilder sb = new StringBuilder("mctas-load tick ").append(levelTicks).append(" gt ").append(gt);
            // every tick for the first `ticks`, then only a tick where a chunk's state changed, or
            // where its redstone or block events run while its entities don't tick
            boolean print = levelTicks < ticks;
            for (int cx = x0; cx <= x1; cx++) {
                for (int cz = z0; cz <= z1; cz++) {
                    long k = key(cx, cz);
                    int[] n = counts.getOrDefault(k, new int[2]);
                    Object pos = bp.newInstance(cx * 16 + 8, 64, cz * 16 + 8);
                    boolean st = (boolean) sched.invoke(level, k);
                    boolean bt = (boolean) blocks.invoke(level, k);
                    boolean el = (boolean) loaded.invoke(level, k);
                    boolean et = (boolean) ticking.invoke(level, pos);
                    int code = (st ? 1 : 0) | (bt ? 2 : 0) | (el ? 4 : 0) | (et ? 8 : 0);
                    Integer before = previous.put(k, code);
                    if (before == null || before != code || ((st || bt) && !et) || (!et && n[0] > 0)) {
                        print = true;
                    }
                    sb.append(" | ").append(cx).append(',').append(cz)
                        .append(" st ").append(st ? 1 : 0)
                        .append(" bt ").append(bt ? 1 : 0)
                        .append(" el ").append(el ? 1 : 0)
                        .append(" et ").append(et ? 1 : 0)
                        .append(" sched ").append(n[0])
                        .append(" ents ").append(n[1]);
                }
            }
            if (print) {
                System.err.println(sb);
            }
        } catch (ReflectiveOperationException e) {
            System.err.println("mctas load agent: end of tick: " + e);
        }
        counts.clear();
        levelTicks++;
        return null;
    }

    /** The hook call: [] -> [] (the hook's result dropped), with {@code this} and argument 1, or null. */
    private static void hook(CodeBuilder b, String property, boolean withArg) {
        ClassDesc properties = ClassDesc.of("java.util.Properties");
        ClassDesc biFunction = ClassDesc.of("java.util.function.BiFunction");
        b.invokestatic(ClassDesc.of("java.lang.System"), "getProperties", MethodTypeDesc.of(properties));
        b.ldc(property);
        b.invokevirtual(properties, "get", MethodTypeDesc.of(ConstantDescs.CD_Object, ConstantDescs.CD_Object));
        b.checkcast(biFunction);
        b.aload(0);
        if (withArg) {
            b.aload(1);
        } else {
            b.aconst_null();
        }
        b.invokeinterface(biFunction, "apply", MethodTypeDesc.of(ConstantDescs.CD_Object, ConstantDescs.CD_Object, ConstantDescs.CD_Object));
        b.pop();
    }

    /** Calls a hook before a method's first instruction. */
    private static final class AtEntry implements CodeTransform {
        private final String property;
        private boolean done;

        AtEntry(String property) {
            this.property = property;
        }

        @Override
        public void accept(CodeBuilder b, CodeElement e) {
            if (!this.done) {
                this.done = true;
                hook(b, this.property, true);
            }
            b.with(e);
        }
    }

    static byte[] instrument(ClassLoader loader, byte[] bytes) {
        ClassHierarchyResolver resolver = ClassHierarchyResolver.defaultResolver();
        if (loader != null) {
            resolver = ClassHierarchyResolver.ofClassLoading(loader).orElse(resolver);
        }
        ClassFile cf = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(resolver));
        ClassModel model = cf.parse(bytes);
        int[] found = {0, 0, 0};
        // one class transform that picks each method's code transform (chained
        // transformingMethodBodies don't reach the methods an earlier one passed through: tried)
        byte[] out = cf.transformClass(model, (cb, ce) -> {
            if (ce instanceof MethodModel m) {
                String name = m.methodName().stringValue();
                String type = m.methodType().stringValue();
                CodeTransform code = null;
                if (name.equals("tickBlock") && type.equals("(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/Block;)V")) {
                    found[0]++;
                    code = new AtEntry(BLOCK);
                } else if (name.equals("tickNonPassenger") && type.equals("(Lnet/minecraft/world/entity/Entity;)V")) {
                    found[1]++;
                    code = new AtEntry(ENTITY);
                } else if (name.equals("tick") && type.equals("(Ljava/util/function/BooleanSupplier;)V")) {
                    code = (b, e) -> {
                        if (e instanceof ReturnInstruction) {
                            found[2]++;
                            hook(b, END, false);
                        }
                        b.with(e);
                    };
                }
                if (code != null) {
                    cb.transformMethod(m, MethodTransform.transformingCode(code));
                    return;
                }
            }
            cb.with(ce);
        });
        if (found[0] != 1 || found[1] != 1 || found[2] == 0) {
            throw new IllegalStateException("ServerLevel: tickBlock " + found[0] + ", tickNonPassenger " + found[1] + ", tick's returns " + found[2] + " (1, 1 and at least 1 expected)");
        }
        return out;
    }
}
