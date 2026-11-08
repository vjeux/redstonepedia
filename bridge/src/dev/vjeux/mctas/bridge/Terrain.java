package dev.vjeux.mctas.bridge;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.SectionPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ThreadedLevelLightEngine;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.DataLayer;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.levelgen.Heightmap;

/**
 * (tickexact 2026-11-08) A terrain sample ({@code .mcterrain}, the terrain lane's format: the official
 * server's region files read back) put into a world sandbox the way the game loads a saved chunk,
 * so the game and redsim ({@code redsim::terrain::build_world}) start from the same bytes:
 *
 * <ul>
 *   <li>each section's block states written into the chunk's section (no updates, no light
 *       checks: a loaded chunk's blocks), cells outside the sample's box air;
 *   <li>its biomes ({@code fillBiomesFromNoise} from the sample's 4x4x4 cells; outside them, the
 *       sandbox's own);
 *   <li>its saved sky and block light ({@code queueSectionData}, as {@code SerializableChunkData.read}
 *       does), every section's status and the chunk's sky light sources refreshed, light enabled;
 *   <li>the heightmaps primed from the blocks, a default block entity for every block that has one
 *       (the format keeps no block entity data yet), the POIs found in the blocks
 *       ({@code PoiManager.checkConsistencyWithBlocks}, what a chunk without POI data gets);
 *   <li>each chunk's inhabited time when given;
 *   <li>its entities ({@code ENTS}: their saved data), each seeded before its data loads like a
 *       design mob ({@link Sandbox#loadSaved}), and the POI tickets of each one's HOME, JOB_SITE,
 *       POTENTIAL_JOB_SITE and MEETING_POINT taken (a save keeps them taken).
 * </ul>
 */
final class Terrain {
    private Terrain() {}

    static JsonObject load(Protocol p, Sandbox sb, JsonObject r) throws Exception {
        // the biome cells: origin and size in quart coordinates, palette, ids (u16 LE, x fastest,
        // then z, then y)
        BiomeCells biomes = r.has("biomes") ? new BiomeCells(sb, r.getAsJsonObject("biomes")) : null;
        Map<Long, LevelChunk> chunks = new LinkedHashMap<>();
        ThreadedLevelLightEngine light = sb.level.getChunkSource().getLightEngine();
        int sections = 0;
        for (JsonElement el : r.getAsJsonArray("sections")) {
            JsonObject s = el.getAsJsonObject();
            int cx = s.get("x").getAsInt();
            int sy = s.get("y").getAsInt();
            int cz = s.get("z").getAsInt();
            LevelChunk chunk = sb.level.getChunkSource().getChunkNow(cx, cz);
            if (chunk == null) {
                throw new IllegalArgumentException("terrain: chunk " + cx + " " + cz + " is not loaded in the sandbox");
            }
            chunks.put(ChunkPos.pack(cx, cz), chunk);
            LevelChunkSection section = chunk.getSection(chunk.getSectionIndexFromSectionY(sy));
            JsonArray pal = s.getAsJsonArray("palette");
            BlockState[] states = new BlockState[pal.size()];
            for (int i = 0; i < states.length; i++) {
                states[i] = p.parse(pal.get(i).getAsString());
            }
            byte[] cells = Base64.getDecoder().decode(s.get("cells").getAsString());
            int i = 0;
            for (int y = 0; y < 16; y++) {
                for (int z = 0; z < 16; z++) {
                    for (int x = 0; x < 16; x++) {
                        int id = (cells[i * 2] & 0xFF) | ((cells[i * 2 + 1] & 0xFF) << 8);
                        section.setBlockState(x, y, z, states[id], false);
                        i++;
                    }
                }
            }
            SectionPos spos = SectionPos.of(cx, sy, cz);
            if (s.has("sky") && !s.get("sky").isJsonNull()) {
                light.queueSectionData(LightLayer.SKY, spos, new DataLayer(Base64.getDecoder().decode(s.get("sky").getAsString())));
            }
            if (s.has("block") && !s.get("block").isJsonNull()) {
                light.queueSectionData(LightLayer.BLOCK, spos, new DataLayer(Base64.getDecoder().decode(s.get("block").getAsString())));
            }
            sections++;
        }
        Map<Long, Long> inhabited = new java.util.HashMap<>();
        if (r.has("inhabited")) {
            for (JsonElement el : r.getAsJsonArray("inhabited")) {
                JsonArray a = el.getAsJsonArray();
                inhabited.put(ChunkPos.pack(a.get(0).getAsInt(), a.get(1).getAsInt()), a.get(2).getAsLong());
            }
        }
        int pois = 0;
        for (LevelChunk chunk : chunks.values()) {
            ChunkPos cp = chunk.getPos();
            for (int si = 0; si < chunk.getSectionsCount(); si++) {
                LevelChunkSection section = chunk.getSection(si);
                int sy = chunk.getSectionYFromSectionIndex(si);
                if (biomes != null) {
                    section.fillBiomesFromNoise(biomes.resolver(section), cp.x() << 2, sy << 2, cp.z() << 2);
                }
                light.updateSectionStatus(SectionPos.of(cp, sy), section.hasOnlyAir());
                SectionPos spos = SectionPos.of(cp, sy);
                sb.level.getPoiManager().checkConsistencyWithBlocks(spos, section);
                // block entities: a loaded chunk has one for every block that needs one
                if (!section.hasOnlyAir() && section.maybeHas(BlockState::hasBlockEntity)) {
                    for (int y = 0; y < 16; y++) {
                        for (int z = 0; z < 16; z++) {
                            for (int x = 0; x < 16; x++) {
                                BlockState st = section.getBlockState(x, y, z);
                                if (st.hasBlockEntity() && st.getBlock() instanceof EntityBlock eb) {
                                    BlockPos pos = new BlockPos(cp.getMinBlockX() + x, (sy << 4) + y, cp.getMinBlockZ() + z);
                                    BlockEntity be = eb.newBlockEntity(pos, st);
                                    if (be != null) {
                                        chunk.setBlockEntity(be);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Heightmap.primeHeightmaps(chunk, java.util.EnumSet.allOf(Heightmap.Types.class));
            chunk.initializeLightSources();
            light.setLightEnabled(cp, true);
            light.retainData(cp, false);
            Long t = inhabited.get(cp.pack());
            if (t != null) {
                chunk.setInhabitedTime(t);
            }
            chunk.markUnsaved();
        }
        sb.drainLight();
        List<Entity> loaded = r.has("entities") ? sb.loadSaved(r.getAsJsonArray("entities")) : List.of();
        for (Entity e : loaded) {
            if (e instanceof net.minecraft.world.entity.LivingEntity living) {
                pois += takeTickets(sb, living);
            }
        }
        JsonObject o = new JsonObject();
        o.addProperty("sections", sections);
        o.addProperty("chunks", chunks.size());
        o.addProperty("pois_taken", pois);
        JsonArray ids = new JsonArray();
        for (Entity e : loaded) {
            ids.add(e.getId() - sb.entityIdBase());
        }
        o.add("net_ids", ids);
        return o;
    }

    /** The POI tickets a loaded villager held (its HOME, JOB_SITE, POTENTIAL_JOB_SITE, MEETING_POINT). */
    @SuppressWarnings({"unchecked", "rawtypes"})
    private static int takeTickets(Sandbox sb, net.minecraft.world.entity.LivingEntity e) {
        int n = 0;
        net.minecraft.world.entity.ai.Brain brain = e.getBrain();
        for (net.minecraft.world.entity.ai.memory.MemoryModuleType<net.minecraft.core.GlobalPos> t : List.of(
                net.minecraft.world.entity.ai.memory.MemoryModuleType.HOME,
                net.minecraft.world.entity.ai.memory.MemoryModuleType.JOB_SITE,
                net.minecraft.world.entity.ai.memory.MemoryModuleType.POTENTIAL_JOB_SITE,
                net.minecraft.world.entity.ai.memory.MemoryModuleType.MEETING_POINT)) {
            if (!brain.checkMemory(t, net.minecraft.world.entity.ai.memory.MemoryStatus.REGISTERED)) {
                continue;
            }
            java.util.Optional<net.minecraft.core.GlobalPos> g = brain.getMemory(t);
            if (g.isEmpty()) {
                continue;
            }
            BlockPos pos = g.get().pos();
            if (sb.level.getPoiManager().take(x -> true, (x, q) -> q.equals(pos), pos, 1).isPresent()) {
                n++;
            }
        }
        return n;
    }

    /** The sample's biome cells, as a resolver for a section's {@code fillBiomesFromNoise}. */
    private static final class BiomeCells {
        final int[] origin;
        final int[] size;
        final List<Holder<Biome>> palette = new ArrayList<>();
        final byte[] cells;

        BiomeCells(Sandbox sb, JsonObject b) {
            JsonArray o = b.getAsJsonArray("origin");
            JsonArray s = b.getAsJsonArray("size");
            this.origin = new int[] {o.get(0).getAsInt(), o.get(1).getAsInt(), o.get(2).getAsInt()};
            this.size = new int[] {s.get(0).getAsInt(), s.get(1).getAsInt(), s.get(2).getAsInt()};
            var lookup = sb.level.registryAccess().lookupOrThrow(Registries.BIOME);
            for (JsonElement e : b.getAsJsonArray("palette")) {
                this.palette.add(lookup.getOrThrow(ResourceKey.create(Registries.BIOME, Identifier.parse(e.getAsString()))));
            }
            this.cells = Base64.getDecoder().decode(b.get("cells").getAsString());
        }

        /** The cell's biome, or the section's own outside the sample's cells. */
        net.minecraft.world.level.biome.BiomeResolver resolver(LevelChunkSection section) {
            var own = section.getBiomes();
            return (qx, qy, qz) -> {
                int lx = qx - this.origin[0];
                int ly = qy - this.origin[1];
                int lz = qz - this.origin[2];
                if (lx < 0 || ly < 0 || lz < 0 || lx >= this.size[0] || ly >= this.size[1] || lz >= this.size[2]) {
                    return own.get(qx & 3, qy & 3, qz & 3);
                }
                int i = (ly * this.size[2] + lz) * this.size[0] + lx;
                int id = (this.cells[i * 2] & 0xFF) | ((this.cells[i * 2 + 1] & 0xFF) << 8);
                return this.palette.get(id);
            };
        }
    }

    /**
     * The light the level reads in a box (sky, block: one byte each per cell, x fastest, then z,
     * then y), for the import's check against the saved light.
     */
    static JsonObject lightBox(Sandbox sb, int[] from, int[] to) {
        int dx = to[0] - from[0] + 1;
        int dy = to[1] - from[1] + 1;
        int dz = to[2] - from[2] + 1;
        byte[] sky = new byte[dx * dy * dz];
        byte[] block = new byte[dx * dy * dz];
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        int i = 0;
        for (int y = from[1]; y <= to[1]; y++) {
            for (int z = from[2]; z <= to[2]; z++) {
                for (int x = from[0]; x <= to[0]; x++) {
                    pos.set(x, y, z);
                    sky[i] = (byte) sb.level.getBrightness(LightLayer.SKY, pos);
                    block[i] = (byte) sb.level.getBrightness(LightLayer.BLOCK, pos);
                    i++;
                }
            }
        }
        JsonObject o = new JsonObject();
        o.addProperty("sky", Base64.getEncoder().encodeToString(sky));
        o.addProperty("block", Base64.getEncoder().encodeToString(block));
        return o;
    }
}
