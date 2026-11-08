package dev.vjeux.mctas.bridge;

import com.google.gson.JsonArray;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import java.lang.reflect.Method;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.ai.navigation.PathNavigation;
import net.minecraft.world.level.pathfinder.Node;
import net.minecraft.world.level.pathfinder.Path;

/**
 * The mob-AI lane's probes (designs/briefs/mob-ai.md): the game's own answers for redsim's mob AI,
 * with no randomness involved. {@code path}: a mob of a type is created (never added to the level:
 * nothing ticks, nothing else sees it) at a position, standing or not, and its navigation's
 * {@code createPath(target, reach)} is returned node by node.
 */
final class MobAiProbe {
    private MobAiProbe() {
    }

    /**
     * {@code math}: the game JVM's own answers for the math the mob AI uses (no sandbox): for each
     * input (a double's raw bits, as a decimal string), {@code Math.sin}, {@code Math.cos},
     * {@code Math.sqrt}, {@code Mth.sin}/{@code Mth.cos} of it as a float, and {@code Mth.atan2(x, y)}
     * against the next input; results as raw bits (decimal strings).
     */
    static JsonObject math(JsonObject r) {
        JsonArray in = r.getAsJsonArray("x");
        JsonArray out = new JsonArray();
        for (int k = 0; k < in.size(); k++) {
            double x = Double.longBitsToDouble(Long.parseLong(in.get(k).getAsString()));
            double y = Double.longBitsToDouble(Long.parseLong(in.get((k + 1) % in.size()).getAsString()));
            JsonArray row = new JsonArray();
            row.add(Long.toString(Double.doubleToRawLongBits(Math.sin(x))));
            row.add(Long.toString(Double.doubleToRawLongBits(Math.cos(x))));
            row.add(Long.toString(Double.doubleToRawLongBits(Math.sqrt(Math.abs(x)))));
            row.add(Integer.toString(Float.floatToRawIntBits(net.minecraft.util.Mth.sin((float) x))));
            row.add(Integer.toString(Float.floatToRawIntBits(net.minecraft.util.Mth.cos((float) x))));
            row.add(Long.toString(Double.doubleToRawLongBits(net.minecraft.util.Mth.atan2(x, y))));
            out.add(row);
        }
        JsonObject res = new JsonObject();
        res.add("out", out);
        return res;
    }

    static JsonObject path(Sandbox sb, JsonObject r) throws ReflectiveOperationException {
        EntityType<?> type = BuiltInRegistries.ENTITY_TYPE.getValue(Identifier.parse(r.get("mob").getAsString()));
        Entity e = type.create(sb.level, EntitySpawnReason.COMMAND);
        if (!(e instanceof Mob mob)) {
            throw new IllegalArgumentException("path: " + r.get("mob").getAsString() + " is not a mob");
        }
        JsonArray p = r.getAsJsonArray("pos");
        mob.setPos(p.get(0).getAsDouble(), p.get(1).getAsDouble(), p.get(2).getAsDouble());
        mob.setOnGround(r.has("on_ground") && r.get("on_ground").getAsBoolean());
        // the fluid trackers (isInWater, isInFloatableFluid) as a tick's baseTick leaves them
        Method fluid = Entity.class.getDeclaredMethod("updateFluidInteraction");
        fluid.setAccessible(true);
        fluid.invoke(mob);
        PathNavigation nav = mob.getNavigation();
        if (r.has("can_open_doors")) {
            nav.setCanOpenDoors(r.get("can_open_doors").getAsBoolean());
        }
        if (r.has("can_float")) {
            nav.setCanFloat(r.get("can_float").getAsBoolean());
        }
        JsonArray t = r.getAsJsonArray("target");
        BlockPos target = new BlockPos(t.get(0).getAsInt(), t.get(1).getAsInt(), t.get(2).getAsInt());
        Path path = nav.createPath(target, r.get("reach").getAsInt());
        JsonObject o = new JsonObject();
        if (path == null) {
            o.add("nodes", JsonNull.INSTANCE);
            return o;
        }
        JsonArray nodes = new JsonArray();
        for (int i = 0; i < path.getNodeCount(); i++) {
            Node n = path.getNode(i);
            JsonArray a = new JsonArray();
            a.add(n.x);
            a.add(n.y);
            a.add(n.z);
            a.add(n.type.name());
            nodes.add(a);
        }
        o.add("nodes", nodes);
        o.addProperty("reached", path.canReach());
        JsonArray tp = new JsonArray();
        tp.add(path.getTarget().getX());
        tp.add(path.getTarget().getY());
        tp.add(path.getTarget().getZ());
        o.add("target", tp);
        o.addProperty("in_water", mob.isInWater());
        o.addProperty("min_y", sb.level.getMinY());
        return o;
    }

    /**
     * {@code mobstate}: every mob of the sandbox's level (debugging the mob AI between ticks): its
     * position and rotations as raw bits, its move control's wanted position and operation, its
     * look control's wanted position, and its path (nodes, next index).
     */
    static JsonObject mobState(Sandbox sb) throws ReflectiveOperationException {
        java.lang.reflect.Field op = net.minecraft.world.entity.ai.control.MoveControl.class.getDeclaredField("operation");
        op.setAccessible(true);
        JsonArray out = new JsonArray();
        for (Entity e : sb.level.getAllEntities()) {
            if (!(e instanceof Mob mob)) {
                continue;
            }
            JsonObject m = new JsonObject();
            m.addProperty("type", BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString());
            m.addProperty("x", Double.toString(e.getX()));
            m.addProperty("y", Double.toString(e.getY()));
            m.addProperty("z", Double.toString(e.getZ()));
            m.addProperty("yaw", Float.toString(e.getYRot()));
            var mc = mob.getMoveControl();
            m.addProperty("move", mc.getWantedX() + " " + mc.getWantedY() + " " + mc.getWantedZ() + " " + op.get(mc) + " " + mc.getSpeedModifier());
            var lc = mob.getLookControl();
            m.addProperty("look", lc.getWantedX() + " " + lc.getWantedY() + " " + lc.getWantedZ() + " " + lc.isLookingAtTarget());
            Path path = mob.getNavigation().getPath();
            if (path != null) {
                StringBuilder sbuf = new StringBuilder();
                sbuf.append(path.getNextNodeIndex()).append(":");
                for (int i = 0; i < path.getNodeCount(); i++) {
                    Node n = path.getNode(i);
                    sbuf.append(" ").append(n.x).append(",").append(n.y).append(",").append(n.z);
                }
                m.addProperty("path", sbuf.toString());
            }
            out.add(m);
        }
        JsonObject res = new JsonObject();
        res.add("mobs", out);
        return res;
    }
}
