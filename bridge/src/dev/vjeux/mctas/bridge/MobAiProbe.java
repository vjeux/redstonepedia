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
     * {@code path_nodes} (mob-AI 2026-11-10): a live mob of the sandbox (by its entity number)
     * searches again as its navigation's {@code createPath(Set.of(target), 0, false, reach)} would
     * (the same region, maximum path length and node budget; the navigation's current path is
     * left alone), and every node its evaluator made is returned (position, type, cost malus,
     * {@code g}, walked distance, closed, in the open set, came from) with the path: the game's
     * search to set beside redsim's (MCTAS_PATH_DUMP).
     */
    @SuppressWarnings("unchecked")
    static JsonObject pathNodes(Sandbox sb, JsonObject r) throws ReflectiveOperationException {
        Entity e = sb.level.getEntity(sb.entityIdBase() + r.get("entity").getAsInt());
        if (!(e instanceof Mob mob)) {
            throw new IllegalArgumentException("path_nodes: no mob #" + r.get("entity").getAsInt());
        }
        PathNavigation nav = mob.getNavigation();
        JsonArray t = r.getAsJsonArray("target");
        BlockPos target = new BlockPos(t.get(0).getAsInt(), t.get(1).getAsInt(), t.get(2).getAsInt());
        int reach = r.get("reach").getAsInt();
        java.lang.reflect.Method gm = PathNavigation.class.getDeclaredMethod("getMaxPathLength");
        gm.setAccessible(true);
        float maxPathLength = (Float) gm.invoke(nav);
        java.lang.reflect.Field pf = PathNavigation.class.getDeclaredField("pathFinder");
        pf.setAccessible(true);
        net.minecraft.world.level.pathfinder.PathFinder finder = (net.minecraft.world.level.pathfinder.PathFinder) pf.get(nav);
        java.lang.reflect.Field mv = PathNavigation.class.getDeclaredField("maxVisitedNodesMultiplier");
        mv.setAccessible(true);
        float multiplier = mv.getFloat(nav);
        BlockPos fromPos = mob.blockPosition();
        int radius = (int) (maxPathLength + 8);
        net.minecraft.world.level.PathNavigationRegion region = new net.minecraft.world.level.PathNavigationRegion(sb.level, fromPos.offset(-radius, -radius, -radius), fromPos.offset(radius, radius, radius));
        Path path = finder.findPath(region, mob, java.util.Set.of(target), maxPathLength, reach, multiplier);
        java.lang.reflect.Field ne = PathNavigation.class.getDeclaredField("nodeEvaluator");
        ne.setAccessible(true);
        Object eval = ne.get(nav);
        java.lang.reflect.Field nf = net.minecraft.world.level.pathfinder.NodeEvaluator.class.getDeclaredField("nodes");
        nf.setAccessible(true);
        java.util.Map<Integer, Node> nodes = (java.util.Map<Integer, Node>) nf.get(eval);
        java.lang.reflect.Field heap = Node.class.getDeclaredField("heapIdx");
        heap.setAccessible(true);
        JsonArray out = new JsonArray();
        for (Node n : nodes.values()) {
            String from = n.cameFrom == null ? "-" : n.cameFrom.x + "," + n.cameFrom.y + "," + n.cameFrom.z;
            out.add(n.x + "," + n.y + "," + n.z + " " + n.type.name() + " malus " + n.costMalus + " g " + n.g + " walked " + n.walkedDistance + " closed " + n.closed + " open " + (heap.getInt(n) >= 0) + " from " + from);
        }
        JsonObject o = new JsonObject();
        o.add("nodes", out);
        o.addProperty("max_path_length", Float.toString(maxPathLength));
        if (path != null) {
            StringBuilder s = new StringBuilder();
            for (int i = 0; i < path.getNodeCount(); i++) {
                Node n = path.getNode(i);
                s.append(" ").append(n.x).append(",").append(n.y).append(",").append(n.z);
            }
            o.addProperty("path", s.toString().trim() + (path.canReach() ? " reached" : " partial"));
        }
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
