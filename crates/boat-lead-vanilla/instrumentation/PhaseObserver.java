package dev.vjeux.mctas.boatprobe;

import java.io.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;

/** Reflection keeps the observer independent of the official bundler's game classloader.
 * All accesses are getters/field reads. An error is recorded and aborts the evidence run. */
public final class PhaseObserver {
    private static PrintWriter out;
    private static long sequence;
    private static boolean failed;

    public static synchronized void observe(Object entity, String phase) {
        if (failed) return;
        try {
            Set<?> tags = (Set<?>) call(entity, "entityTags");
            if (!tags.contains("mctas_probe")) return;
            Object level = call(entity, "level");
            if ((boolean) call(level, "isClientSide")) return;
            if (out == null) {
                Path path = Path.of(System.getProperty("mctas.probe.trace"));
                Files.createDirectories(path.getParent());
                out = new PrintWriter(Files.newBufferedWriter(path));
            }
            Map<String, Object> row = new LinkedHashMap<>();
            row.put("seq", sequence++);
            row.put("tick", call(level, "getGameTime"));
            row.put("phase", phase);
            row.put("entity", state(entity));
            List<Object> passengerStates = new ArrayList<>();
            for (Object passenger : (List<?>) call(entity, "getPassengers")) passengerStates.add(state(passenger));
            row.put("passengers", passengerStates);
            Object holder = optional(entity, "getLeashHolder");
            if (holder != null) row.put("holder", state(holder));
            out.println(json(row));
            out.flush();
            if (out.checkError()) throw new IOException("observer write failed");
        } catch (Throwable error) {
            failed = true;
            String message = "BOAT_PROBE_ERROR " + error;
            System.err.println(message);
            error.printStackTrace();
            if (out != null) { out.println("{\"error\":" + json(message) + "}"); out.flush(); }
        }
    }

    private static Map<String, Object> state(Object e) throws Exception {
        Map<String, Object> s = new LinkedHashMap<>();
        s.put("id", call(e, "getId"));
        s.put("type", e.getClass().getSimpleName());
        s.put("tags", call(e, "entityTags"));
        s.put("tick_count", field(e, "tickCount"));
        s.put("position", vec(call(e, "position")));
        s.put("delta", vec(call(e, "getDeltaMovement")));
        s.put("known", vec(call(e, "getKnownMovement")));
        s.put("yaw", call(e, "getYRot"));
        s.put("width", call(e, "getBbWidth"));
        s.put("height", call(e, "getBbHeight"));
        s.put("authoritative", call(e, "isLocalInstanceAuthoritative"));
        s.put("controller", id(call(e, "getControllingPassenger")));
        s.put("vehicle", id(call(e, "getVehicle")));
        s.put("root", id(call(e, "getRootVehicle")));
        s.put("root_controller", id(call(call(e, "getRootVehicle"), "getControllingPassenger")));
        s.put("passengers", ((List<?>) call(e, "getPassengers")).stream().map(PhaseObserver::uncheckedId).toList());
        s.put("on_ground", call(e, "onGround"));
        s.put("vertical_collision", field(e, "verticalCollision"));
        s.put("horizontal_collision", field(e, "horizontalCollision"));
        s.put("below_collision", field(e, "verticalCollisionBelow"));
        s.put("support_block", field(e, "mainSupportingBlockPos").toString());
        s.put("no_gravity", call(e, "isNoGravity"));
        s.put("no_physics", field(e, "noPhysics"));
        s.put("no_ai", optional(e, "isNoAi"));
        s.put("in_water", call(e, "isInWater"));
        s.put("in_lava", call(e, "isInLava"));
        s.put("alive", call(e, "isAlive"));
        s.put("removed", call(e, "isRemoved"));
        s.put("touching_unloaded_chunk", call(e, "touchingUnloadedChunk"));
        s.put("gravity", call(e, "getGravity"));
        s.put("holder", id(optional(e, "getLeashHolder")));
        Object leash = optional(e, "getLeashData");
        s.put("angular_momentum", leash == null ? null : field(leash, "angularMomentum"));
        s.put("child_anchor", anchor(e, true));
        s.put("holder_anchor", anchor(e, false));
        Object box = call(e, "getBoundingBox");
        s.put("bbox", List.of(field(box, "minX"), field(box, "minY"), field(box, "minZ"),
                field(box, "maxX"), field(box, "maxY"), field(box, "maxZ")));
        return s;
    }

    private static Object uncheckedId(Object e) {
        try { return id(e); } catch (Exception ex) { throw new IllegalStateException(ex); }
    }
    private static Object id(Object e) throws Exception { return e == null ? null : call(e, "getId"); }
    private static List<Double> vec(Object v) throws Exception {
        return List.of(((Number) field(v, "x")).doubleValue(), ((Number) field(v, "y")).doubleValue(), ((Number) field(v, "z")).doubleValue());
    }
    private static List<Double> anchor(Object e, boolean child) throws Exception {
        Object position = call(e, "position");
        Class<?> v = position.getClass();
        float width = ((Number) call(e, "getBbWidth")).floatValue();
        float height = ((Number) call(e, "getBbHeight")).floatValue();
        float radians = ((Number) call(e, "getYRot")).floatValue() * (float) (Math.PI / 180.0);
        Object offset = v.getConstructor(double.class, double.class, double.class)
                .newInstance(0.0, height * 0.5, child ? width * 0.5 : 0.0);
        Object rotated = v.getMethod("yRot", float.class).invoke(offset, -radians);
        return vec(v.getMethod("add", v).invoke(position, rotated));
    }
    private static Object optional(Object o, String name) throws Exception {
        try { return call(o, name); } catch (NoSuchMethodException e) { return null; }
    }
    private static Object call(Object o, String name) throws Exception {
        Method m;
        try { m = o.getClass().getMethod(name); }
        catch (NoSuchMethodException missing) {
            m = null;
            for (Class<?> c = o.getClass(); c != null; c = c.getSuperclass()) {
                try { m = c.getDeclaredMethod(name); break; } catch (NoSuchMethodException ignored) {}
            }
            if (m == null) throw missing;
        }
        m.setAccessible(true);
        return m.invoke(o);
    }
    private static Object field(Object o, String name) throws Exception {
        for (Class<?> c = o.getClass(); c != null; c = c.getSuperclass()) {
            try { Field f = c.getDeclaredField(name); f.setAccessible(true); return f.get(o); }
            catch (NoSuchFieldException ignored) {}
        }
        throw new NoSuchFieldException(name);
    }
    private static String json(Object o) {
        if (o == null) return "null";
        if (o instanceof Number || o instanceof Boolean) return o.toString();
        if (o instanceof Map<?, ?> m) {
            List<String> parts = new ArrayList<>();
            m.forEach((k, v) -> parts.add(json(k.toString()) + ":" + json(v)));
            return "{" + String.join(",", parts) + "}";
        }
        if (o instanceof Collection<?> c) return "[" + String.join(",", c.stream().map(PhaseObserver::json).toList()) + "]";
        return "\"" + o.toString().replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n").replace("\r", "\\r") + "\"";
    }
}
