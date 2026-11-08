package dev.vjeux.mctas.bridge;

import com.mojang.authlib.GameProfile;
import com.mojang.authlib.services.ServicesKeySet;
import com.mojang.serialization.Lifecycle;
import java.lang.reflect.Field;
import java.net.Proxy;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;
import java.util.function.BooleanSupplier;
import net.minecraft.CrashReport;
import net.minecraft.ReportType;
import net.minecraft.SharedConstants;
import net.minecraft.SystemReport;
import net.minecraft.commands.Commands;
import net.minecraft.core.Holder;
import net.minecraft.core.MappedRegistry;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.Registries;
import net.minecraft.gizmos.GizmoCollector;
import net.minecraft.gizmos.Gizmos;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.Services;
import net.minecraft.server.TickTask;
import net.minecraft.server.WorldLoader;
import net.minecraft.server.WorldStem;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.progress.LoggingLevelLoadListener;
import net.minecraft.server.notifications.EmptyNotificationService;
import net.minecraft.server.notifications.NotificationManager;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.permissions.LevelBasedPermissionSet;
import net.minecraft.server.permissions.PermissionSet;
import net.minecraft.server.players.NameAndId;
import net.minecraft.server.players.PlayerList;
import net.minecraft.server.players.ProfileResolver;
import net.minecraft.server.players.UserNameToIdResolver;
import net.minecraft.util.Util;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.util.debugchart.LocalSampleLogger;
import net.minecraft.util.debugchart.SampleLogger;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.LevelSettings;
import net.minecraft.world.level.WorldDataConfiguration;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.Biomes;
import net.minecraft.world.level.dimension.BuiltinDimensionTypes;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.dimension.LevelStem;
import net.minecraft.world.level.gamerules.GameRules;
import net.minecraft.world.level.levelgen.FlatLevelSource;
import net.minecraft.world.level.levelgen.WorldDimensions;
import net.minecraft.world.level.levelgen.WorldGenSettings;
import net.minecraft.world.level.levelgen.WorldOptions;
import net.minecraft.world.level.levelgen.flat.FlatLevelGeneratorSettings;
import net.minecraft.world.level.storage.LevelDataAndDimensions;
import net.minecraft.world.level.storage.LevelStorageSource;
import net.minecraft.world.level.storage.PrimaryLevelData;

/**
 * A headless Minecraft server modeled on vanilla's {@code GameTestServer}.
 *
 * <p>Differences from a dedicated server: the overworld is an empty void that never ticks, there is
 * no network listener, no player, no autosave. Work happens in {@link Sandbox}es: each one is its
 * own {@link ServerLevel} (own block-tick queue, block-event queue, game time and random), so
 * hundreds of independent contraptions can live in one JVM and be stepped one tick at a time.
 * The server thread only advances a sandbox when the controller asks for it ({@code step}).
 */
public final class SimServer extends MinecraftServer {
    private static final Services NO_SERVICES =
            new Services(null, ServicesKeySet.EMPTY, null, new MockUserNameToIdResolver(), new MockProfileResolver());

    /** Vanilla data pack and vanilla feature flags only: no experiments (redstone experiments are off). */
    static final WorldDataConfiguration DATA_CONFIG = WorldDataConfiguration.DEFAULT;

    private final LocalSampleLogger sampleLogger = new LocalSampleLogger(4);
    private final Protocol protocol;
    private final Map<ResourceKey<Level>, ServerLevel> levelsMap;
    final LevelStem voidStem;
    final LevelSettings levelSettings;

    /** Port for real game clients (viewers, for video recording); -1 = no network at all. */
    static int listenPort = -1;

    public static void launch(Path universe, Protocol protocol, int listen) throws Exception {
        listenPort = listen;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Bootstrap.validate();
        LevelStorageSource.LevelStorageAccess access = LevelStorageSource.createDefault(universe).createAccess("world");
        PackRepository packRepository = ServerPacksSource.createPackRepository(access);
        MinecraftServer.spin(thread -> create(thread, access, packRepository, protocol));
    }

    private static SimServer create(
            Thread serverThread,
            LevelStorageSource.LevelStorageAccess access,
            PackRepository packRepository,
            Protocol protocol) {
        packRepository.reload();
        LevelSettings settings =
                new LevelSettings("mctas", GameType.CREATIVE, LevelSettings.DifficultySettings.DEFAULT, true, DATA_CONFIG);
        WorldLoader.PackConfig packConfig = new WorldLoader.PackConfig(packRepository, DATA_CONFIG, false, true);
        WorldLoader.InitConfig initConfig =
                new WorldLoader.InitConfig(packConfig, Commands.CommandSelection.DEDICATED, LevelBasedPermissionSet.OWNER);
        try {
            LevelStem[] stemOut = new LevelStem[1];
            WorldStem worldStem = Util.<WorldStem>blockUntilDone(executor -> WorldLoader.load(
                            initConfig,
                            context -> {
                                Holder<Biome> theVoid = context.datapackWorldRegistries()
                                        .lookupOrThrow(Registries.BIOME)
                                        .getOrThrow(Biomes.THE_VOID);
                                Holder<DimensionType> overworldType = context.datapackWorldRegistries()
                                        .lookupOrThrow(Registries.DIMENSION_TYPE)
                                        .getOrThrow(BuiltinDimensionTypes.OVERWORLD);
                                FlatLevelGeneratorSettings flat =
                                        new FlatLevelGeneratorSettings(Optional.empty(), theVoid, List.of());
                                flat.updateLayers();
                                LevelStem stem = new LevelStem(overworldType, new FlatLevelSource(flat));
                                stemOut[0] = stem;
                                WorldDimensions dimensions = new WorldDimensions(Map.of(LevelStem.OVERWORLD, stem));
                                Registry<LevelStem> noDatapackDimensions =
                                        new MappedRegistry<>(Registries.LEVEL_STEM, Lifecycle.stable()).freeze();
                                WorldDimensions.Complete complete = dimensions.bake(noDatapackDimensions);
                                PrimaryLevelData levelData = new PrimaryLevelData(
                                        settings, complete.specialWorldProperty(), complete.lifecycle());
                                // Skip the spawn search: it would load ~121 chunks of void.
                                levelData.setInitialized(true);
                                return new WorldLoader.DataLoadOutput<>(
                                        new LevelDataAndDimensions.WorldDataAndGenSettings(
                                                levelData, new WorldGenSettings(new WorldOptions(0L, false, false), dimensions)),
                                        complete.dimensionsRegistryAccess());
                            },
                            WorldStem::new,
                            Util.backgroundExecutor(),
                            executor))
                    .get();
            return new SimServer(serverThread, access, packRepository, worldStem, protocol, stemOut[0], settings);
        } catch (Exception e) {
            throw new IllegalStateException("Failed to load the vanilla data pack", e);
        }
    }

    private static GameRules gameRules() {
        GameRules rules = new GameRules(DATA_CONFIG.enabledFeatures());
        // A void world has nothing to spawn on, but turn it off explicitly; weather and the
        // day cycle are global server state that sandboxes must not drift.
        rules.set(GameRules.SPAWN_MOBS, false, null);
        rules.set(GameRules.SPAWN_MONSTERS, false, null);
        rules.set(GameRules.ADVANCE_WEATHER, false, null);
        rules.set(GameRules.ADVANCE_TIME, false, null);
        return rules;
    }

    @SuppressWarnings("unchecked")
    private SimServer(
            Thread serverThread,
            LevelStorageSource.LevelStorageAccess access,
            PackRepository packRepository,
            WorldStem worldStem,
            Protocol protocol,
            LevelStem voidStem,
            LevelSettings levelSettings) {
        super(
                serverThread,
                access,
                packRepository,
                worldStem,
                Optional.of(gameRules()),
                Proxy.NO_PROXY,
                DataFixers.getDataFixer(),
                NO_SERVICES,
                LoggingLevelLoadListener.forDedicatedServer(),
                false,
                new NotificationManager());
        this.protocol = protocol;
        this.voidStem = voidStem;
        this.levelSettings = levelSettings;
        try {
            Field field = MinecraftServer.class.getDeclaredField("levels");
            field.setAccessible(true);
            this.levelsMap = (Map<ResourceKey<Level>, ServerLevel>) field.get(this);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    LevelStorageSource.LevelStorageAccess storage() {
        return this.storageSource;
    }

    private final Map<String, LevelStem> stems = new java.util.HashMap<>();

    /**
     * (tickexact) A void stem whose chunks carry {@code biome} (a world sandbox standing in a biome:
     * the real-game gate's void superflat is plains, where it rains), cached per biome; the
     * overworld's dimension type like the void stem's.
     */
    LevelStem stemFor(String biome) {
        if (biome == null || biome.equals("minecraft:the_void")) {
            return this.voidStem;
        }
        return this.stems.computeIfAbsent(biome, id -> {
            Holder<Biome> b = this.registryAccess().lookupOrThrow(Registries.BIOME)
                    .getOrThrow(ResourceKey.create(Registries.BIOME, net.minecraft.resources.Identifier.parse(id)));
            FlatLevelGeneratorSettings flat = new FlatLevelGeneratorSettings(Optional.empty(), b, List.of());
            flat.updateLayers();
            return new LevelStem(this.voidStem.type(), new FlatLevelSource(flat));
        });
    }

    boolean listening() {
        return listenPort > 0;
    }

    /**
     * Network work a real server does every tick, for connected viewers: packets, keep-alives,
     * player bookkeeping and chunk sending. Levels are NOT ticked here.
     */
    void viewerTick() {
        if (!listening()) {
            return;
        }
        java.util.List<net.minecraft.server.level.ServerPlayer> players = this.getPlayerList().getPlayers();
        players.forEach(p -> p.connection.suspendFlushing());
        this.tickConnection();
        this.getPlayerList().tick();
        for (net.minecraft.server.level.ServerPlayer p : this.getPlayerList().getPlayers()) {
            p.connection.chunkSender.sendNextChunks(p);
            p.connection.resumeFlushing();
        }
    }

    
    public boolean enforceSecureProfile() {
        return false;
    }

    /** Run queued server-thread tasks (chunk futures and the like) without ticking anything. */
    void pumpTasks() {
        this.runAllTasks();
    }

    void registerLevel(ServerLevel level) {
        this.levelsMap.put(level.dimension(), level);
    }

    void unregisterLevel(ServerLevel level) {
        this.levelsMap.remove(level.dimension());
    }

    @Override
    protected boolean initServer() throws java.io.IOException {
        this.setPlayerList(new PlayerList(this, this.registries(), this.playerDataStorage, new EmptyNotificationService()) {});
        Gizmos.withCollector(GizmoCollector.NOOP);
        this.loadLevel();
        if (listenPort > 0) {
            this.setUsesAuthentication(false);
            this.getConnection().startTcpServerListener(java.net.InetAddress.getLoopbackAddress(), listenPort);
        }
        this.protocol.start(this);
        return true;
    }

    /**
     * Replaces vanilla's server tick: the overworld never ticks. Each loop iteration blocks until
     * the controller sends requests and then executes all queued requests on the server thread.
     */
    @Override
    protected void tickServer(final BooleanSupplier haveTime) {
        this.protocol.serve();
    }

    @Override
    protected void waitUntilNextTick() {
        this.runAllTasks();
    }

    /**
     * Vanilla only runs queued server tasks while it "has time" before the next 50 ms tick. This
     * server has no wall clock: tasks (chunk futures, entity-section loads, ...) always run.
     */
    @Override
    protected boolean shouldRun(final TickTask task) {
        return true;
    }

    @Override
    protected SampleLogger getTickTimeLogger() {
        return this.sampleLogger;
    }

    @Override
    public boolean isTickTimeLoggingEnabled() {
        return false;
    }

    @Override
    public SystemReport fillServerSystemReport(final SystemReport systemReport) {
        systemReport.setDetail("Type", "mctas simulation server");
        return systemReport;
    }

    @Override
    protected void onServerCrash(final CrashReport report) {
        super.onServerCrash(report);
        System.err.println(report.getFriendlyReport(ReportType.CRASH));
        Runtime.getRuntime().halt(1);
    }

    @Override
    protected void onServerExit() {
        super.onServerExit();
        Runtime.getRuntime().halt(0);
    }

    @Override
    public boolean isHardcore() {
        return false;
    }

    @Override
    public LevelBasedPermissionSet operatorUserPermissions() {
        return LevelBasedPermissionSet.ALL;
    }

    @Override
    public PermissionSet getFunctionCompilationPermissions() {
        return LevelBasedPermissionSet.OWNER;
    }

    @Override
    public boolean shouldRconBroadcast() {
        return false;
    }

    @Override
    public boolean isDedicatedServer() {
        return false;
    }

    @Override
    public int getRateLimitPacketsPerSecond() {
        return 0;
    }

    @Override
    public int getCommandSpamThresholdSeconds() {
        return 0;
    }

    @Override
    public int getChatSpamThresholdSeconds() {
        return 0;
    }

    @Override
    public boolean useNativeTransport() {
        return false;
    }

    @Override
    public boolean isPublished() {
        return false;
    }

    @Override
    public boolean shouldInformAdmins() {
        return false;
    }

    @Override
    public boolean isSingleplayerOwner(final NameAndId nameAndId) {
        return false;
    }

    @Override
    public int getMaxPlayers() {
        return 1;
    }

    private static final class MockProfileResolver implements ProfileResolver {
        @Override
        public Optional<GameProfile> fetchByName(final String name) {
            return Optional.empty();
        }

        @Override
        public Optional<GameProfile> fetchById(final UUID id) {
            return Optional.empty();
        }
    }

    private static final class MockUserNameToIdResolver implements UserNameToIdResolver {
        private final Set<NameAndId> savedIds = new HashSet<>();

        @Override
        public void add(final NameAndId nameAndId) {
            this.savedIds.add(nameAndId);
        }

        @Override
        public Optional<NameAndId> get(final String name) {
            return this.savedIds.stream()
                    .filter(e -> e.name().equals(name))
                    .findFirst()
                    .or(() -> Optional.of(NameAndId.createOffline(name)));
        }

        @Override
        public Optional<NameAndId> get(final UUID id) {
            return this.savedIds.stream().filter(e -> e.id().equals(id)).findFirst();
        }

        @Override
        public void resolveOfflineUsers(final boolean value) {}

        @Override
        public void save() {}
    }
}
