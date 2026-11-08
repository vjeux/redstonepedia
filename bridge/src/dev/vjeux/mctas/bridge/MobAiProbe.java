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
}
