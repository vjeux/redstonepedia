package dev.vjeux.mctas.bridge;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.TreeSet;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.FireBlock;
import net.minecraft.world.level.block.LeavesBlock;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.pathfinder.PathComputationType;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.shapes.VoxelShape;

/**
 * The {@code blockinfo} op (the terrain lane, designs/briefs/terrain-and-spawning.md): the game's
 * own answers about block states, for redsim's natural-state table. Each state is placed alone
 * (raw) in the sandbox, its methods are called, and it is removed again.
 */
final class BlockInfo {
    private BlockInfo() {
    }

    private static final Direction[] DIRS = {Direction.DOWN, Direction.UP, Direction.NORTH, Direction.SOUTH, Direction.WEST, Direction.EAST};

    /** The {@code blockstates} op: every possible state of each block id ({@code "blocks"}). */
    /**
     * The {@code spawnrules} op (the terrain lane's check of the hostile checks' C1): for each
     * entity type ({@code "types"}) and cell ({@code "cells"}): {@code SpawnPlacements
     * .isSpawnPositionOk}, {@code noCollision} of the type's spawn box at the cell's center, and
     * how many of {@code "tries"} calls of {@code SpawnPlacements.checkSpawnRules(type, level,
     * NATURAL, pos, random)} pass, each with its own random ({@code RandomSource.create(i)}), at
     * the level's current light and time. Read-only.
     */
    @SuppressWarnings({"unchecked", "rawtypes"})
    static JsonObject spawnRules(Protocol p, Sandbox sb, JsonObject r) {
        int tries = r.has("tries") ? r.get("tries").getAsInt() : 200;
        JsonArray out = new JsonArray();
        for (var t : r.getAsJsonArray("types")) {
            Identifier id = Identifier.parse(t.getAsString());
            EntityType<?> type = BuiltInRegistries.ENTITY_TYPE.getOptional(id).orElseThrow(() -> new IllegalArgumentException("no entity type " + id));
            JsonArray cells = new JsonArray();
            for (var c : r.getAsJsonArray("cells")) {
                JsonArray a = c.getAsJsonArray();
                BlockPos pos = new BlockPos(a.get(0).getAsInt(), a.get(1).getAsInt(), a.get(2).getAsInt());
                boolean placement = net.minecraft.world.entity.SpawnPlacements.isSpawnPositionOk(type, sb.level, pos);
                boolean free = sb.level.noCollision(type.getSpawnAABB(pos.getX() + 0.5, pos.getY(), pos.getZ() + 0.5));
                int pass = 0;
                for (int i = 0; i < tries; i++) {
                    if (net.minecraft.world.entity.SpawnPlacements.checkSpawnRules((EntityType) type, sb.level, net.minecraft.world.entity.EntitySpawnReason.NATURAL, pos, net.minecraft.util.RandomSource.create(i))) {
                        pass++;
                    }
                }
                JsonArray row = new JsonArray();
                row.add(pos.getX());
                row.add(pos.getY());
                row.add(pos.getZ());
                row.add(placement);
                row.add(free);
                row.add(pass);
                cells.add(row);
            }
            JsonObject o = new JsonObject();
            o.addProperty("type", id.toString());
            o.add("cells", cells);
            out.add(o);
        }
        JsonObject res = new JsonObject();
        res.add("types", out);
        res.addProperty("difficulty", sb.level.getDifficulty().getSerializedName());
        res.addProperty("sky_darken", sb.level.getSkyDarken());
        return res;
    }

    static JsonObject states(Protocol p, JsonObject r) {
        JsonArray out = new JsonArray();
        for (var e : r.getAsJsonArray("blocks")) {
            Identifier id = Identifier.parse(e.getAsString());
            Block block = BuiltInRegistries.BLOCK.getOptional(id).orElseThrow(() -> new IllegalArgumentException("no block " + id));
            for (BlockState st : block.getStateDefinition().getPossibleStates()) {
                out.add(p.serialize(st));
            }
        }
        JsonObject res = new JsonObject();
        res.add("states", out);
        return res;
    }

    static JsonObject run(Protocol p, Sandbox sb, JsonObject r) throws CommandSyntaxException, ReflectiveOperationException {
        List<EntityType<?>> types = new ArrayList<>();
        for (var t : r.getAsJsonArray("types")) {
            Identifier id = Identifier.parse(t.getAsString());
            types.add(BuiltInRegistries.ENTITY_TYPE.getOptional(id).orElseThrow(() -> new IllegalArgumentException("no entity type " + id)));
        }
        Method burn = FireBlock.class.getDeclaredMethod("getBurnOdds", BlockState.class);
        Method ignite = FireBlock.class.getDeclaredMethod("getIgniteOdds", BlockState.class);
        burn.setAccessible(true);
        ignite.setAccessible(true);
        FireBlock fire = (FireBlock) Blocks.FIRE;
        BlockPos pos = new BlockPos(sb.minX() + 8, 100, sb.minZ() + 8);
        JsonArray out = new JsonArray();
        for (var e : r.getAsJsonArray("states")) {
            BlockState st = p.parse(e.getAsString());
            sb.level.setBlock(pos, st, Sandbox.RAW_FLAGS);
            JsonObject o = describe(p, sb, st, pos, types);
            o.addProperty("burn_odds", (int) burn.invoke(fire, st));
            o.addProperty("ignite_odds", (int) ignite.invoke(fire, st));
            // support: per direction, whether the neighbour there turning from dirt into air changes
            // the state (`updateShape`: a plant or a wall light popping off, a fence, wall, pane,
            // stair, bed or door reshaping); a state that cannot even stand among dirt counts every
            // direction
            BlockState dirt = Blocks.DIRT.defaultBlockState();
            BlockState air = Blocks.AIR.defaultBlockState();
            for (Direction d : DIRS) {
                sb.level.setBlock(pos.relative(d), dirt, Sandbox.RAW_FLAGS);
            }
            StringBuilder support = new StringBuilder();
            for (Direction d : DIRS) {
                BlockState baseline = st.updateShape(sb.level, sb.level, pos, d, pos.relative(d), dirt, sb.level.getRandom());
                sb.level.setBlock(pos.relative(d), air, Sandbox.RAW_FLAGS);
                BlockState after = st.updateShape(sb.level, sb.level, pos, d, pos.relative(d), air, sb.level.getRandom());
                sb.level.setBlock(pos.relative(d), dirt, Sandbox.RAW_FLAGS);
                support.append(baseline.isAir() || !after.equals(baseline) ? '1' : '0');
            }
            for (Direction d : DIRS) {
                sb.level.setBlock(pos.relative(d), air, Sandbox.RAW_FLAGS);
            }
            o.addProperty("support", support.toString());
            o.addProperty("ignited_by_lava", st.ignitedByLava());
            out.add(o);
            sb.level.setBlock(pos, Blocks.AIR.defaultBlockState(), Sandbox.RAW_FLAGS);
        }
        JsonObject res = new JsonObject();
        res.add("states", out);
        return res;
    }

    private static JsonObject describe(Protocol p, Sandbox sb, BlockState st, BlockPos pos, List<EntityType<?>> types) {
        JsonObject o = new JsonObject();
        o.addProperty("name", p.serialize(st));
        Block block = st.getBlock();
        JsonArray classes = new JsonArray();
        TreeSet<String> declared = new TreeSet<>();
        for (Class<?> c = block.getClass(); c != null && c != BlockBehaviour.class; c = c.getSuperclass()) {
            classes.add(c.getSimpleName());
            if (c != Block.class) {
                for (Method m : c.getDeclaredMethods()) {
                    declared.add(m.getName());
                }
            }
        }
        o.add("classes", classes);
        JsonArray decl = new JsonArray();
        declared.forEach(decl::add);
        o.add("declared", decl);
        JsonArray boxes = new JsonArray();
        for (AABB b : st.getCollisionShape(sb.level, pos).toAabbs()) {
            // a shape's boxes are in the block's own coordinates (0..1)
            JsonArray a = new JsonArray();
            a.add(b.minX);
            a.add(b.minY);
            a.add(b.minZ);
            a.add(b.maxX);
            a.add(b.maxY);
            a.add(b.maxZ);
            boxes.add(a);
        }
        o.add("collision", boxes);
        // the outline (selection) shape (`getShape`: ClipContext.Block.OUTLINE), at this cell; an
        // offset block's shapes carry this cell's offset, reported so a reader can move them
        JsonArray outline = new JsonArray();
        for (AABB b : st.getShape(sb.level, pos).toAabbs()) {
            JsonArray a = new JsonArray();
            a.add(b.minX);
            a.add(b.minY);
            a.add(b.minZ);
            a.add(b.maxX);
            a.add(b.maxY);
            a.add(b.maxZ);
            outline.add(a);
        }
        o.add("outline", outline);
        net.minecraft.world.phys.Vec3 off = st.getOffset(pos);
        JsonArray offset = new JsonArray();
        offset.add(off.x);
        offset.add(off.y);
        offset.add(off.z);
        o.add("offset", offset);
        float maxH = 0.0F;
        float maxV = 0.0F;
        if (st.hasOffsetFunction()) {
            for (Class<?> c = block.getClass(); c != null; c = c.getSuperclass()) {
                try {
                    Method mh = c.getDeclaredMethod("getMaxHorizontalOffset");
                    mh.setAccessible(true);
                    maxH = (Float) mh.invoke(block);
                    break;
                } catch (NoSuchMethodException e) {
                    // look further up
                } catch (ReflectiveOperationException e) {
                    break;
                }
            }
            for (Class<?> c = block.getClass(); c != null; c = c.getSuperclass()) {
                try {
                    Method mv = c.getDeclaredMethod("getMaxVerticalOffset");
                    mv.setAccessible(true);
                    maxV = (Float) mv.invoke(block);
                    break;
                } catch (NoSuchMethodException e) {
                    // look further up
                } catch (ReflectiveOperationException e) {
                    break;
                }
            }
        }
        o.addProperty("offset_max_h", st.hasOffsetFunction() ? maxH : 0.0F);
        // XYZ blocks move vertically too: their offset's y is not 0 somewhere (probed at two cells)
        boolean xyz = st.hasOffsetFunction() && (off.y != 0.0 || st.getOffset(pos.offset(5, 0, 3)).y != 0.0 || st.getOffset(pos.offset(1, 0, 9)).y != 0.0);
        o.addProperty("offset_max_v", xyz ? maxV : 0.0F);
        // whether the outline moves with the offset (getShape applies it): compared at a cell whose
        // offset differs
        boolean moves = false;
        if (st.hasOffsetFunction()) {
            BlockPos other = pos.offset(5, 0, 3);
            net.minecraft.world.phys.Vec3 off2 = st.getOffset(other);
            net.minecraft.world.phys.shapes.VoxelShape a = st.getShape(sb.level, pos);
            net.minecraft.world.phys.shapes.VoxelShape b2 = st.getShape(sb.level, other);
            if (!a.isEmpty() && !b2.isEmpty()) {
                AABB ab = a.bounds();
                AABB bb = b2.bounds();
                moves = Math.abs((bb.minX - ab.minX) - (off2.x - off.x)) < 1e-9 && Math.abs((bb.minZ - ab.minZ) - (off2.z - off.z)) < 1e-9 && (off2.x != off.x || off2.z != off.z);
            }
        }
        o.addProperty("outline_moves", moves);
        o.addProperty("can_occlude", st.canOcclude());
        o.addProperty("shape_occlusion", st.useShapeForLightOcclusion());
        if (st.canOcclude() && st.useShapeForLightOcclusion()) {
            JsonArray faces = new JsonArray();
            for (Direction d : DIRS) {
                faces.add(face(st.getFaceOcclusionShape(d), d));
            }
            o.add("faces", faces);
        }
        o.addProperty("dampening", st.getLightDampening());
        o.addProperty("emission", st.getLightEmission());
        o.addProperty("skylight_down", st.propagatesSkylightDown());
        o.addProperty("solid", st.isSolid());
        o.addProperty("solid_render", st.isSolidRender());
        o.addProperty("conductor", st.isRedstoneConductor(sb.level, pos));
        o.addProperty("full_collision", st.isCollisionShapeFullBlock(sb.level, pos));
        StringBuilder sturdy = new StringBuilder();
        for (net.minecraft.world.level.block.SupportType support : new net.minecraft.world.level.block.SupportType[] {
            net.minecraft.world.level.block.SupportType.FULL, net.minecraft.world.level.block.SupportType.CENTER, net.minecraft.world.level.block.SupportType.RIGID}) {
            for (Direction d : DIRS) {
                sturdy.append(st.isFaceSturdy(sb.level, pos, d, support) ? '1' : '0');
            }
        }
        o.addProperty("sturdy", sturdy.toString());
        o.addProperty("signal", st.isSignalSource());
        o.addProperty("air", st.isAir());
        o.addProperty("liquid", st.liquid());
        o.addProperty("replaceable", st.canBeReplaced());
        o.addProperty("suffocating", st.isSuffocating(sb.level, pos));
        o.addProperty("path_land", st.isPathfindable(PathComputationType.LAND));
        o.addProperty("path_water", st.isPathfindable(PathComputationType.WATER));
        o.addProperty("path_air", st.isPathfindable(PathComputationType.AIR));
        FluidState fs = st.getFluidState();
        o.addProperty("fluid", BuiltInRegistries.FLUID.getKey(fs.getType()).toString());
        o.addProperty("fluid_amount", fs.getAmount());
        o.addProperty("fluid_source", fs.isSource());
        JsonArray tags = new JsonArray();
        st.typeHolder().tags().map(t -> t.location().toString()).sorted().forEach(tags::add);
        o.add("tags", tags);
        o.addProperty("leaves_block", block instanceof LeavesBlock);
        o.addProperty("random_ticking", st.isRandomlyTicking());
        o.addProperty("friction", block.getFriction());
        o.addProperty("speed_factor", block.getSpeedFactor());
        o.addProperty("jump_factor", block.getJumpFactor());
        o.addProperty("push", st.getPistonPushReaction().name());
        o.addProperty("block_entity", st.hasBlockEntity());
        StringBuilder heights = new StringBuilder();
        for (Heightmap.Types h : new Heightmap.Types[] {Heightmap.Types.WORLD_SURFACE, Heightmap.Types.MOTION_BLOCKING, Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, Heightmap.Types.OCEAN_FLOOR}) {
            heights.append(h.isOpaque().test(st) ? '1' : '0');
        }
        o.addProperty("heightmaps", heights.toString());
        StringBuilder spawn = new StringBuilder();
        StringBuilder danger = new StringBuilder();
        for (EntityType<?> t : types) {
            spawn.append(st.isValidSpawn(sb.level, pos, t) ? '1' : '0');
            danger.append(t.isBlockDangerous(st) ? '1' : '0');
        }
        o.addProperty("valid_spawn", spawn.toString());
        o.addProperty("dangerous", danger.toString());
        o.addProperty("resistance", block.getExplosionResistance());
        return o;
    }

    /**
     * A face occlusion shape on a 16 x 16 grid: rows v of bits u, as daylight.rs reads them: X faces
     * (u = z, v = y), Y faces (u = x, v = z), Z faces (u = x, v = y). A cell is set when its centre is
     * inside one of the shape's boxes.
     */
    private static JsonArray face(VoxelShape shape, Direction d) {
        int[] rows = new int[16];
        for (AABB b : shape.toAabbs()) {
            for (int v = 0; v < 16; v++) {
                for (int u = 0; u < 16; u++) {
                    double cu = (u + 0.5) / 16.0;
                    double cv = (v + 0.5) / 16.0;
                    boolean in = switch (d.getAxis()) {
                        case X -> cu > b.minZ && cu < b.maxZ && cv > b.minY && cv < b.maxY;
                        case Y -> cu > b.minX && cu < b.maxX && cv > b.minZ && cv < b.maxZ;
                        case Z -> cu > b.minX && cu < b.maxX && cv > b.minY && cv < b.maxY;
                    };
                    if (in) {
                        rows[v] |= 1 << u;
                    }
                }
            }
        }
        JsonArray a = new JsonArray();
        for (int row : rows) {
            a.add(row);
        }
        return a;
    }
}
