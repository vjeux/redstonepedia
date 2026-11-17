package dev.vjeux.mctas.rng;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.authlib.GameProfile;
import com.mojang.authlib.services.MinecraftServicesDiscoveryService;
import com.mojang.serialization.Lifecycle;
import io.netty.channel.ChannelHandler;
import io.netty.channel.embedded.EmbeddedChannel;
import it.unimi.dsi.fastutil.objects.Object2IntMap;
import java.io.BufferedReader;
import java.io.File;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.net.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicLong;
import net.minecraft.SharedConstants;
import net.minecraft.commands.Commands;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.IdMap;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.Connection;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.Services;
import net.minecraft.server.WorldLoader;
import net.minecraft.server.WorldStem;
import net.minecraft.server.dedicated.DedicatedServer;
import net.minecraft.server.dedicated.DedicatedServerProperties;
import net.minecraft.server.dedicated.DedicatedServerSettings;
import net.minecraft.server.jsonrpc.JsonRpc;
import net.minecraft.server.jsonrpc.ManagementServer;
import net.minecraft.server.level.ClientInformation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.network.CommonListenerCookie;
import net.minecraft.server.network.ServerCommonPacketListenerImpl;
import net.minecraft.server.notifications.NotificationManager;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.util.RandomSource;
import net.minecraft.util.Util;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.world.RandomSequence;
import net.minecraft.world.RandomSequences;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.npc.villager.Villager;
import net.minecraft.world.entity.npc.villager.VillagerData;
import net.minecraft.world.entity.npc.villager.VillagerProfession;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.inventory.DataSlot;
import net.minecraft.world.inventory.EnchantmentMenu;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.enchantment.Enchantable;
import net.minecraft.world.item.enchantment.Enchantment;
import net.minecraft.world.item.enchantment.EnchantmentHelper;
import net.minecraft.world.item.enchantment.EnchantmentInstance;
import net.minecraft.world.item.enchantment.ItemEnchantments;
import net.minecraft.world.item.trading.MerchantOffer;
import net.minecraft.world.item.trading.MerchantOffers;
import net.minecraft.world.item.trading.TradeSet;
import net.minecraft.world.item.trading.VillagerTrade;
import net.minecraft.world.level.LevelSettings;
import net.minecraft.world.level.WorldDataConfiguration;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.EnchantingTableBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.dimension.LevelStem;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.WorldDimensions;
import net.minecraft.world.level.levelgen.WorldGenSettings;
import net.minecraft.world.level.levelgen.WorldOptions;
import net.minecraft.world.level.levelgen.Xoroshiro128PlusPlus;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.storage.LevelDataAndDimensions;
import net.minecraft.world.level.storage.LevelStorageSource;
import net.minecraft.world.level.storage.PrimaryLevelData;

/**
 * The rng crate's bridge into the real Minecraft 26.3 dedicated server (`mctas rng game ...`,
 * crates/rng/src/game.rs): the unmodified DedicatedServer started in this JVM the way
 * net.minecraft.server.Main starts it (a fresh superflat world), with mock players ticked like
 * network clients (an embedded connection in the server's connection list) and villagers left to
 * their own AI.
 *
 * <p>It only performs primitive game actions (open an enchanting table and read it, click an
 * enchant button, press Q, reroll a villager, place a block) and reads game state (a player's
 * random generator and enchanting seed, a villager's offers, a world random sequence); every
 * decision and every comparison lives in the Rust driver. Protocol: one JSON object per stdin
 * line, one reply per command on stdout prefixed with "@@BRIDGE ".
 */
public final class RngBridge {
   private static final Gson GSON = new Gson();
   private static PrintStream out;
   private static DedicatedServer server;
   private static final Map<String, ServerPlayer> players = new HashMap<>();
   // Table k has exactly k bookshelves around it.
   private static final BlockPos[] TABLES = new BlockPos[16];
   private static final int FLOOR_Y = -60; // first air block of the default superflat preset

   public static void main(final String[] args) throws Exception {
      out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), true, StandardCharsets.UTF_8);
      // The dedicated server starts a console thread reading System.in; keep our command stream for ourselves.
      java.io.InputStream commands = System.in;
      System.setIn(new java.io.ByteArrayInputStream(new byte[0]));
      startServer();
      long t0 = System.currentTimeMillis();
      while (!server.isReady() || server.getTickCount() < 20) {
         if (!server.getRunningThread().isAlive()) {
            // (a world that cannot load, a port in use: the reason is in the server's log)
            reply(obj("error", "the server stopped before it was ready (its log says why)"));
            System.exit(1);
         }
         if (System.currentTimeMillis() - t0 > 600_000) {
            throw new IllegalStateException("server did not become ready");
         }
         Thread.sleep(20);
      }
      server.submit(RngBridge::buildFixtures).get();
      reply(obj("ready", true, "tick", server.getTickCount(), "seed", server.overworld().getSeed()));
      BufferedReader in = new BufferedReader(new InputStreamReader(commands, StandardCharsets.UTF_8));
      String line;
      while ((line = in.readLine()) != null) {
         line = line.trim();
         if (line.isEmpty()) {
            continue;
         }
         JsonObject cmd = JsonParser.parseString(line).getAsJsonObject();
         String name = cmd.get("cmd").getAsString();
         if (name.equals("quit")) {
            reply(obj("ok", true));
            break;
         }
         JsonObject result;
         try {
            if (name.equals("tick")) {
               result = waitTicks(cmd.get("n").getAsInt());
            } else {
               result = server.submit(() -> {
                  keepAlive();
                  try {
                     return handle(name, cmd);
                  } catch (RuntimeException e) {
                     throw e;
                  } catch (Exception e) {
                     throw new RuntimeException(e);
                  }
               }).get();
            }
         } catch (Throwable t) {
            Throwable c = t.getCause() != null ? t.getCause() : t;
            java.io.StringWriter sw = new java.io.StringWriter();
            c.printStackTrace(new java.io.PrintWriter(sw));
            result = obj("error", c.toString(), "trace", sw.toString());
         }
         reply(result);
      }
      server.halt(false);
      System.exit(0);
   }

   private static void reply(final JsonObject o) {
      synchronized (out) {
         out.println("@@BRIDGE " + GSON.toJson(o));
         out.flush();
      }
   }

   // ---------------------------------------------------------------- server startup (mirrors net.minecraft.server.Main)

   private static void startServer() throws Exception {
      SharedConstants.tryDetectVersion();
      Bootstrap.bootStrap();
      Bootstrap.validate();
      Path settingsFile = Paths.get("server.properties");
      DedicatedServerSettings settings = new DedicatedServerSettings(settingsFile);
      settings.forceSave();
      File universePath = new File(".");
      Services services = Services.create(MinecraftServicesDiscoveryService.create(Proxy.NO_PROXY), universePath);
      NotificationManager notificationManager = new NotificationManager();
      ManagementServer jsonRpcServer = JsonRpc.create(settings, notificationManager);
      String levelName = settings.getProperties().levelName;
      LevelStorageSource levelStorageSource = LevelStorageSource.createDefault(universePath.toPath());
      LevelStorageSource.LevelStorageAccess access = levelStorageSource.validateAndCreateAccess(levelName);
      if (access.hasWorldData()) {
         throw new IllegalStateException("bridge expects a fresh world directory");
      }
      PackRepository packRepository = ServerPacksSource.createPackRepository(access);
      DedicatedServerProperties properties = settings.getProperties();
      WorldDataConfiguration dataConfig = new WorldDataConfiguration(properties.initialDataPackConfiguration, FeatureFlags.DEFAULT_FLAGS);
      WorldLoader.PackConfig packConfig = new WorldLoader.PackConfig(packRepository, dataConfig, false, true);
      WorldLoader.InitConfig initConfig = new WorldLoader.InitConfig(packConfig, Commands.CommandSelection.DEDICATED, properties.functionPermissions);
      WorldStem worldStem = Util.<WorldStem>blockUntilDone(
            executor -> WorldLoader.load(
               initConfig,
               context -> {
                  Registry<LevelStem> datapackDimensions = context.datapackDimensions().lookupOrThrow(Registries.LEVEL_STEM);
                  LevelSettings levelSettings = new LevelSettings(
                     properties.levelName,
                     properties.gameMode.get(),
                     new LevelSettings.DifficultySettings(properties.difficulty.get(), properties.hardcore, false),
                     false,
                     context.dataConfiguration()
                  );
                  WorldOptions worldOptions = properties.worldOptions;
                  WorldDimensions dimensions = properties.createDimensions(context.datapackWorldRegistries());
                  WorldDimensions.Complete finalDimensions = dimensions.bake(datapackDimensions);
                  Lifecycle lifecycle = finalDimensions.lifecycle().add(context.datapackWorldRegistries().allRegistriesLifecycle());
                  PrimaryLevelData primaryLevelData = new PrimaryLevelData(levelSettings, finalDimensions.specialWorldProperty(), lifecycle);
                  return new WorldLoader.DataLoadOutput<>(
                     new LevelDataAndDimensions.WorldDataAndGenSettings(primaryLevelData, new WorldGenSettings(worldOptions, dimensions)),
                     finalDimensions.dimensionsRegistryAccess()
                  );
               },
               WorldStem::new,
               Util.backgroundExecutor(),
               executor
            )
         )
         .get();
      access.saveDataTag(worldStem.worldDataAndGenSettings().data());
      server = MinecraftServer.spin(thread -> {
         DedicatedServer s = new DedicatedServer(
            thread, access, packRepository, worldStem, Optional.empty(), settings, DataFixers.getDataFixer(), services, jsonRpcServer, notificationManager
         );
         notificationManager.setServer(s);
         s.setPort(-1);
         s.setDemo(false);
         return s;
      });
   }

   private static JsonObject waitTicks(final int n) throws Exception {
      int target = server.getTickCount() + n;
      long last = 0;
      long lastProgress = System.currentTimeMillis();
      int lastTick = server.getTickCount();
      while (server.getTickCount() < target) {
         if (System.currentTimeMillis() - last > 2000) {
            server.submit(RngBridge::keepAlive).get();
            last = System.currentTimeMillis();
         }
         if (server.getTickCount() != lastTick) {
            lastTick = server.getTickCount();
            lastProgress = System.currentTimeMillis();
         } else if (System.currentTimeMillis() - lastProgress > 30_000) {
            throw new IllegalStateException("the server stopped ticking at tick " + lastTick);
         }
         Thread.sleep(2);
      }
      return obj("tick", server.getTickCount());
   }

   /** Mock players have no client answering keep-alives; refresh their timers so they are never kicked. */
   private static void keepAlive() {
      try {
         Field time = ServerCommonPacketListenerImpl.class.getDeclaredField("keepAliveTime");
         Field pending = ServerCommonPacketListenerImpl.class.getDeclaredField("keepAlivePending");
         time.setAccessible(true);
         pending.setAccessible(true);
         for (ServerPlayer p : players.values()) {
            time.setLong(p.connection, Util.getMillis());
            pending.setBoolean(p.connection, false);
         }
      } catch (ReflectiveOperationException e) {
         throw new RuntimeException(e);
      }
   }

   // ---------------------------------------------------------------- fixtures

   private static void buildFixtures() {
      ServerLevel level = server.overworld();
      for (int cx = -2; cx <= 12; cx++) {
         for (int cz = -2; cz <= 4; cz++) {
            level.setChunkForced(cx, cz, true);
         }
      }
      for (int k = 0; k < 16; k++) {
         BlockPos table = new BlockPos(16 + 8 * k, FLOOR_Y, 16);
         TABLES[k] = table;
         level.setBlockAndUpdate(table, Blocks.ENCHANTING_TABLE.defaultBlockState());
         for (int i = 0; i < EnchantingTableBlock.BOOKSHELF_OFFSETS.size(); i++) {
            BlockPos at = table.offset(EnchantingTableBlock.BOOKSHELF_OFFSETS.get(i));
            level.setBlockAndUpdate(at, i < k ? Blocks.BOOKSHELF.defaultBlockState() : Blocks.AIR.defaultBlockState());
         }
      }
   }

   // ---------------------------------------------------------------- command dispatch

   private static JsonObject handle(final String name, final JsonObject cmd) throws Exception {
      switch (name) {
         case "dump":
            return dump();
         case "offers":
            return offers(cmd);
         case "player_new":
            return playerNew(cmd);
         case "player_state":
            return playerState(player(cmd));
         case "player_set_xp_seed":
            return playerSetXpSeed(cmd);
         case "observe":
            return observe(cmd);
         case "enchant":
            return enchant(cmd);
         case "drop":
            return drop(cmd);
         case "set_block":
            return setBlock(cmd);
         case "villager_spawn":
            return villagerSpawn(cmd);
         case "villager_set_profession":
            return villagerSetProfession(cmd);
         case "villager_offers":
            return villagerOffers(cmd);
         case "villager_reroll": {
            // Atomically: lose the profession (offers cleared), take it again, open the trade screen.
            JsonObject none = cmd.deepCopy();
            none.addProperty("profession", "minecraft:none");
            villagerSetProfession(none);
            villagerSetProfession(cmd);
            return villagerOffers(cmd);
         }
         case "villager_remove":
            villager(cmd).discard();
            return obj("ok", true);
         case "command": {
            server.getCommands().performPrefixedCommand(server.createCommandSourceStack(), cmd.get("command").getAsString());
            return obj("ok", true, "tick", server.getTickCount());
         }
         case "villager_info":
            return villagerInfo(cmd);
         case "seq_state":
            return seqState(cmd);
         case "trade_sets":
            return tradeSets();
         default:
            throw new IllegalArgumentException("unknown command " + name);
      }
   }

   // ---------------------------------------------------------------- data dump

   private static String id(final Holder<?> h) {
      return h.unwrapKey().orElseThrow().identifier().toString();
   }

   private static JsonObject dump() {
      RegistryAccess access = server.registryAccess();
      Registry<Enchantment> reg = access.lookupOrThrow(Registries.ENCHANTMENT);
      IdMap<Holder<Enchantment>> ids = reg.asHolderIdMap();
      List<Holder.Reference<Enchantment>> all = reg.listElements().toList();
      JsonArray enchants = new JsonArray();
      for (Holder.Reference<Enchantment> h : all) {
         Enchantment e = h.value();
         JsonObject o = new JsonObject();
         o.addProperty("id", id(h));
         o.addProperty("rid", ids.getId(h));
         o.addProperty("weight", e.getWeight());
         o.addProperty("min_level", e.getMinLevel());
         o.addProperty("max_level", e.getMaxLevel());
         o.addProperty("anvil_cost", e.getAnvilCost());
         JsonArray minCost = new JsonArray();
         JsonArray maxCost = new JsonArray();
         for (int lvl = 1; lvl <= e.getMaxLevel(); lvl++) {
            minCost.add(e.getMinCost(lvl));
            maxCost.add(e.getMaxCost(lvl));
         }
         o.add("min_cost", minCost);
         o.add("max_cost", maxCost);
         JsonArray incompatible = new JsonArray();
         for (Holder.Reference<Enchantment> other : all) {
            if (!Enchantment.areCompatible(h, other)) {
               incompatible.add(id(other));
            }
         }
         o.add("incompatible", incompatible);
         enchants.add(o);
      }
      JsonArray items = new JsonArray();
      for (Item item : BuiltInRegistries.ITEM) {
         ItemStack stack = new ItemStack(item);
         Enchantable enchantable = stack.get(DataComponents.ENCHANTABLE);
         if (enchantable == null) {
            continue;
         }
         JsonObject o = new JsonObject();
         o.addProperty("id", BuiltInRegistries.ITEM.getKey(item).toString());
         o.addProperty("enchantability", enchantable.value());
         o.addProperty("is_enchantable", stack.isEnchantable());
         JsonArray primary = new JsonArray();
         JsonArray supported = new JsonArray();
         for (Holder.Reference<Enchantment> h : all) {
            if (h.value().isPrimaryItem(stack)) {
               primary.add(id(h));
            }
            if (h.value().isSupportedItem(stack)) {
               supported.add(id(h));
            }
         }
         o.add("primary", primary);
         o.add("supported", supported);
         items.add(o);
      }
      JsonObject r = new JsonObject();
      r.addProperty("version", SharedConstants.getCurrentVersion().name());
      r.add("enchantments", enchants);
      r.add("items", items);
      JsonObject tags = new JsonObject();
      reg.getTags().forEach(named -> {
         JsonArray a = new JsonArray();
         named.stream().forEach(h -> a.add(id(h)));
         tags.add(named.key().location().toString(), a);
      });
      r.add("enchantment_tags", tags);
      JsonObject maxStack = new JsonObject();
      for (Item item : BuiltInRegistries.ITEM) {
         maxStack.addProperty(BuiltInRegistries.ITEM.getKey(item).toString(), item.getDefaultMaxStackSize());
      }
      r.add("max_stack", maxStack);
      r.add("trade_sets", tradeSets().get("trade_sets"));
      return r;
   }

   private static JsonObject tradeSets() {
      Registry<TradeSet> sets = server.registryAccess().lookupOrThrow(Registries.TRADE_SET);
      JsonArray a = new JsonArray();
      for (Holder.Reference<TradeSet> h : sets.listElements().toList()) {
         TradeSet t = h.value();
         JsonObject o = new JsonObject();
         o.addProperty("id", id(h));
         o.addProperty("allow_duplicates", t.allowDuplicates());
         o.addProperty("random_sequence", t.randomSequence().map(Identifier::toString).orElse(null));
         JsonArray trades = new JsonArray();
         for (Holder<VillagerTrade> vt : t.trades()) {
            trades.add(id(vt));
         }
         o.add("trades", trades);
         a.add(o);
      }
      return obj("trade_sets", a);
   }

   // ---------------------------------------------------------------- enchanting table (real EnchantmentMenu)

   private static Item item(final String id) {
      return BuiltInRegistries.ITEM.getValue(Identifier.parse(id));
   }

   private static JsonArray enchList(final List<EnchantmentInstance> list) {
      JsonArray a = new JsonArray();
      for (EnchantmentInstance e : list) {
         JsonArray p = new JsonArray();
         p.add(id(e.enchantment()));
         p.add(e.level());
         a.add(p);
      }
      return a;
   }

   private static Method getEnchantmentList;
   private static Field menuSeed;

   @SuppressWarnings("unchecked")
   private static List<EnchantmentInstance> fullList(final EnchantmentMenu menu, final ItemStack stack, final int slot, final int cost) throws Exception {
      if (getEnchantmentList == null) {
         getEnchantmentList = EnchantmentMenu.class.getDeclaredMethod("getEnchantmentList", RegistryAccess.class, ItemStack.class, int.class, int.class);
         getEnchantmentList.setAccessible(true);
      }
      return (List<EnchantmentInstance>)getEnchantmentList.invoke(menu, server.registryAccess(), stack, slot, cost);
   }

   private static JsonObject readMenu(final EnchantmentMenu menu, final ItemStack stack, final boolean withLists) throws Exception {
      IdMap<Holder<Enchantment>> ids = server.registryAccess().lookupOrThrow(Registries.ENCHANTMENT).asHolderIdMap();
      JsonArray costs = new JsonArray();
      JsonArray clues = new JsonArray();
      JsonArray lists = new JsonArray();
      int[] c = menu.costs.clone();
      int[] clue = menu.enchantClue.clone();
      int[] lvl = menu.levelClue.clone();
      for (int i = 0; i < 3; i++) {
         costs.add(c[i]);
         if (clue[i] >= 0) {
            JsonArray p = new JsonArray();
            p.add(id(ids.byId(clue[i])));
            p.add(lvl[i]);
            clues.add(p);
         } else {
            clues.add(JsonNull.INSTANCE);
         }
      }
      JsonObject o = obj("costs", costs, "clues", clues);
      if (withLists) {
         for (int i = 0; i < 3; i++) {
            lists.add(c[i] > 0 ? enchList(fullList(menu, stack, i, c[i])) : new JsonArray());
         }
         o.add("lists", lists);
      }
      return o;
   }

   /** Bulk vectors: for every (seed, item, shelves) the real menu's costs, clues and full lists. */
   private static JsonObject offers(final JsonObject cmd) throws Exception {
      ServerPlayer p = ensureBulkPlayer();
      if (menuSeed == null) {
         menuSeed = EnchantmentMenu.class.getDeclaredField("enchantmentSeed");
         menuSeed.setAccessible(true);
      }
      EnchantmentMenu[] menus = new EnchantmentMenu[16];
      for (int k = 0; k < 16; k++) {
         menus[k] = new EnchantmentMenu(1000 + k, p.getInventory(), net.minecraft.world.inventory.ContainerLevelAccess.create(server.overworld(), TABLES[k]));
      }
      JsonArray rows = new JsonArray();
      for (JsonElement s : cmd.getAsJsonArray("seeds")) {
         int seed = s.getAsInt();
         for (JsonElement it : cmd.getAsJsonArray("items")) {
            Item item = item(it.getAsString());
            for (JsonElement sh : cmd.getAsJsonArray("shelves")) {
               int k = sh.getAsInt();
               EnchantmentMenu menu = menus[k];
               ((DataSlot)menuSeed.get(menu)).set(seed);
               ItemStack stack = new ItemStack(item);
               menu.getSlot(0).set(ItemStack.EMPTY);
               menu.getSlot(0).set(stack);
               JsonObject row = readMenu(menu, stack, true);
               row.addProperty("seed", seed);
               row.addProperty("item", it.getAsString());
               row.addProperty("shelves", k);
               rows.add(row);
               menu.getSlot(0).set(ItemStack.EMPTY);
            }
         }
      }
      return obj("rows", rows);
   }

   private static ServerPlayer ensureBulkPlayer() {
      ServerPlayer p = players.get("bulk");
      if (p == null) {
         JsonObject c = new JsonObject();
         c.addProperty("name", "bulk");
         playerNew(c);
         p = players.get("bulk");
      }
      return p;
   }

   // ---------------------------------------------------------------- mock player (ticked like a real connection)

   private static ServerPlayer player(final JsonObject cmd) {
      String name = cmd.has("name") ? cmd.get("name").getAsString() : "p";
      ServerPlayer p = players.get(name);
      if (p == null) {
         throw new IllegalArgumentException("no player " + name);
      }
      return p;
   }

   private static JsonObject playerNew(final JsonObject cmd) {
      String name = cmd.has("name") ? cmd.get("name").getAsString() : "p";
      ServerLevel level = server.overworld();
      CommonListenerCookie cookie = CommonListenerCookie.createInitial(new GameProfile(UUID.randomUUID(), name), false);
      ServerPlayer p = new ServerPlayer(server, level, cookie.gameProfile(), ClientInformation.createDefault());
      Connection connection = new Connection(PacketFlow.SERVERBOUND);
      new EmbeddedChannel(new ChannelHandler[]{connection});
      server.getPlayerList().placeNewPlayer(connection, p, cookie);
      // Tick the connection like a real network client so the player entity ticks every game tick.
      server.getConnection().getConnections().add(connection);
      p.teleportTo(level, 16.5, FLOOR_Y, 24.5, java.util.Set.of(), 0.0F, 0.0F, true);
      p.setGameMode(net.minecraft.world.level.GameType.SURVIVAL);
      players.put(name, p);
      return playerState(p);
   }

   private static long rngState(final RandomSource random) throws Exception {
      Field f = LegacyRandomSource.class.getDeclaredField("seed");
      f.setAccessible(true);
      return ((AtomicLong)f.get(random)).get();
   }

   private static JsonObject playerState(final ServerPlayer p) {
      try {
         return obj(
            "rng_state", rngState(p.getRandom()),
            "xp_seed", p.getEnchantmentSeed(),
            "level", p.experienceLevel,
            "x", p.getX(), "y", p.getY(), "z", p.getZ(),
            "tick", server.getTickCount(),
            "entity_ticks", p.tickCount,
            "alive", p.isAlive()
         );
      } catch (Exception e) {
         throw new RuntimeException(e);
      }
   }

   private static JsonObject playerSetXpSeed(final JsonObject cmd) throws Exception {
      ServerPlayer p = player(cmd);
      Field f = net.minecraft.world.entity.player.Player.class.getDeclaredField("enchantmentSeed");
      f.setAccessible(true);
      f.setInt(p, cmd.get("xp_seed").getAsInt());
      return playerState(p);
   }

   private static EnchantmentMenu openTable(final ServerPlayer p, final int shelves) {
      ServerLevel level = server.overworld();
      BlockPos pos = TABLES[shelves];
      BlockState state = level.getBlockState(pos);
      if (p.containerMenu instanceof EnchantmentMenu old) {
         old.getSlot(0).set(ItemStack.EMPTY);
         old.getSlot(1).set(ItemStack.EMPTY);
      }
      p.openMenu(state.getMenuProvider(level, pos));
      if (!(p.containerMenu instanceof EnchantmentMenu menu)) {
         throw new IllegalStateException("table did not open: " + p.containerMenu);
      }
      return menu;
   }

   private static void closeTable(final ServerPlayer p, final EnchantmentMenu menu) {
      menu.getSlot(0).set(ItemStack.EMPTY);
      menu.getSlot(1).set(ItemStack.EMPTY);
      p.closeContainer();
   }

   /** What a player sees: open the table that has `shelves` bookshelves, put the item in, read the three slots. */
   private static JsonObject observe(final JsonObject cmd) throws Exception {
      ServerPlayer p = player(cmd);
      long before = rngState(p.getRandom());
      EnchantmentMenu menu = openTable(p, cmd.get("shelves").getAsInt());
      ItemStack stack = new ItemStack(item(cmd.get("item").getAsString()));
      menu.getSlot(0).set(stack);
      JsonObject o = readMenu(menu, stack, true);
      closeTable(p, menu);
      o.addProperty("rng_untouched", before == rngState(p.getRandom()));
      o.add("player", playerState(p));
      return o;
   }

   /** Click an enchant button in a real menu (the same clickMenuButton the network handler calls). */
   private static JsonObject enchant(final JsonObject cmd) throws Exception {
      ServerPlayer p = player(cmd);
      long before = rngState(p.getRandom());
      int slot = cmd.get("slot").getAsInt();
      if (p.experienceLevel < 40) {
         p.giveExperienceLevels(100);
      }
      EnchantmentMenu menu = openTable(p, cmd.get("shelves").getAsInt());
      ItemStack stack = new ItemStack(item(cmd.get("item").getAsString()));
      menu.getSlot(1).set(new ItemStack(Items.LAPIS_LAZULI, 64));
      menu.getSlot(0).set(stack);
      JsonObject seen = readMenu(menu, stack, true);
      boolean ok = menu.clickMenuButton(p, slot);
      ItemStack result = menu.getSlot(0).getItem();
      JsonArray got = new JsonArray();
      ItemEnchantments enchs = EnchantmentHelper.getEnchantmentsForCrafting(result);
      for (Object2IntMap.Entry<Holder<Enchantment>> e : enchs.entrySet()) {
         JsonArray pair = new JsonArray();
         pair.add(id(e.getKey()));
         pair.add(e.getIntValue());
         got.add(pair);
      }
      closeTable(p, menu);
      long after = rngState(p.getRandom());
      JsonObject o = obj("ok", ok, "seen", seen, "result_item", BuiltInRegistries.ITEM.getKey(result.getItem()).toString(), "result", got);
      o.addProperty("rng_before", before);
      o.addProperty("rng_after", after);
      o.add("player", playerState(p));
      return o;
   }

   /** Press Q `count` times with a cobblestone stack in the hand: the DROP_ITEM action handler calls player.drop(false). */
   private static JsonObject drop(final JsonObject cmd) throws Exception {
      ServerPlayer p = player(cmd);
      int count = cmd.get("count").getAsInt();
      long before = rngState(p.getRandom());
      int selected = p.getInventory().getSelectedSlot();
      for (int i = 0; i < count; i++) {
         ItemStack hand = p.getInventory().getItem(selected);
         if (hand.isEmpty()) {
            p.getInventory().setItem(selected, new ItemStack(Items.COBBLESTONE, 64));
         }
         p.drop(false);
      }
      // Remove the dropped items so they never touch the player again (no pickup sounds, no lag).
      server.overworld().getEntities(EntityTypes.ITEM, e -> true).forEach(e -> e.discard());
      long after = rngState(p.getRandom());
      JsonObject o = obj("rng_before", before, "rng_after", after);
      o.add("player", playerState(p));
      return o;
   }

   private static JsonObject setBlock(final JsonObject cmd) {
      BlockPos pos = new BlockPos(cmd.get("x").getAsInt(), cmd.get("y").getAsInt(), cmd.get("z").getAsInt());
      Block block = BuiltInRegistries.BLOCK.getValue(Identifier.parse(cmd.get("block").getAsString()));
      server.overworld().setBlockAndUpdate(pos, block.defaultBlockState());
      return obj("ok", true);
   }

   // ---------------------------------------------------------------- villagers and random sequences

   private static Villager villager(final JsonObject cmd) {
      UUID uuid = UUID.fromString(cmd.get("uuid").getAsString());
      if (server.overworld().getEntity(uuid) instanceof Villager v) {
         return v;
      }
      throw new IllegalArgumentException("no villager " + uuid);
   }

   private static JsonObject villagerSpawn(final JsonObject cmd) {
      ServerLevel level = server.overworld();
      Villager v = EntityTypes.VILLAGER.create(level, EntitySpawnReason.COMMAND);
      v.snapTo(cmd.get("x").getAsDouble(), cmd.get("y").getAsDouble(), cmd.get("z").getAsDouble(), 0.0F, 0.0F);
      v.setPersistenceRequired();
      level.addFreshEntity(v);
      return obj("uuid", v.getUUID().toString());
   }

   private static JsonObject villagerSetProfession(final JsonObject cmd) {
      Villager v = villager(cmd);
      ResourceKey<VillagerProfession> key = ResourceKey.create(Registries.VILLAGER_PROFESSION, Identifier.parse(cmd.get("profession").getAsString()));
      VillagerData data = v.getVillagerData().withProfession(server.registryAccess(), key);
      if (cmd.has("level")) {
         data = data.withLevel(cmd.get("level").getAsInt());
      }
      v.setVillagerData(data);
      return villagerInfo(cmd);
   }

   private static JsonObject itemJson(final ItemStack stack) {
      JsonObject o = new JsonObject();
      o.addProperty("id", BuiltInRegistries.ITEM.getKey(stack.getItem()).toString());
      o.addProperty("count", stack.getCount());
      JsonArray enchs = new JsonArray();
      for (Object2IntMap.Entry<Holder<Enchantment>> e : EnchantmentHelper.getEnchantmentsForCrafting(stack).entrySet()) {
         JsonArray pair = new JsonArray();
         pair.add(id(e.getKey()));
         pair.add(e.getIntValue());
         enchs.add(pair);
      }
      o.add("enchantments", enchs);
      return o;
   }

   private static JsonObject villagerOffers(final JsonObject cmd) {
      Villager v = villager(cmd);
      MerchantOffers offers = v.getOffers();
      JsonArray a = new JsonArray();
      for (MerchantOffer offer : offers) {
         JsonObject o = new JsonObject();
         o.add("cost_a", itemJson(offer.getBaseCostA()));
         o.add("cost_b", offer.getItemCostB().isPresent() ? itemJson(offer.getCostB()) : JsonNull.INSTANCE);
         o.add("result", itemJson(offer.getResult()));
         o.addProperty("max_uses", offer.getMaxUses());
         o.addProperty("xp", offer.getXp());
         a.add(o);
      }
      JsonObject r = villagerInfo(cmd);
      r.add("offers", a);
      return r;
   }

   private static JsonObject villagerInfo(final JsonObject cmd) {
      Villager v = villager(cmd);
      VillagerData d = v.getVillagerData();
      boolean generated;
      try {
         Field f = net.minecraft.world.entity.npc.villager.AbstractVillager.class.getDeclaredField("offers");
         f.setAccessible(true);
         generated = f.get(v) != null;
      } catch (ReflectiveOperationException e) {
         throw new RuntimeException(e);
      }
      return obj(
         "uuid", v.getUUID().toString(),
         "profession", id(d.profession()),
         "level", d.level(),
         "xp", v.getVillagerXp(),
         "offers_generated", generated,
         "x", v.getX(), "y", v.getY(), "z", v.getZ()
      );
   }

   /** Ground truth for a world random sequence: the Xoroshiro128++ state, or null if never created. */
   @SuppressWarnings("unchecked")
   private static JsonObject seqState(final JsonObject cmd) throws Exception {
      Identifier key = Identifier.parse(cmd.get("key").getAsString());
      Field seqs = RandomSequences.class.getDeclaredField("sequences");
      seqs.setAccessible(true);
      Map<Identifier, RandomSequence> map = (Map<Identifier, RandomSequence>)seqs.get(server.getRandomSequences());
      RandomSequence seq = map.get(key);
      if (seq == null) {
         return obj("exists", false);
      }
      Field source = RandomSequence.class.getDeclaredField("source");
      source.setAccessible(true);
      XoroshiroRandomSource x = (XoroshiroRandomSource)source.get(seq);
      Field gen = XoroshiroRandomSource.class.getDeclaredField("randomNumberGenerator");
      gen.setAccessible(true);
      Xoroshiro128PlusPlus g = (Xoroshiro128PlusPlus)gen.get(x);
      Field lo = Xoroshiro128PlusPlus.class.getDeclaredField("seedLo");
      Field hi = Xoroshiro128PlusPlus.class.getDeclaredField("seedHi");
      lo.setAccessible(true);
      hi.setAccessible(true);
      return obj("exists", true, "lo", lo.getLong(g), "hi", hi.getLong(g));
   }

   // ---------------------------------------------------------------- json helpers

   private static JsonObject obj(final Object... kv) {
      JsonObject o = new JsonObject();
      for (int i = 0; i < kv.length; i += 2) {
         String k = (String)kv[i];
         Object v = kv[i + 1];
         if (v == null) {
            o.add(k, JsonNull.INSTANCE);
         } else if (v instanceof JsonElement e) {
            o.add(k, e);
         } else if (v instanceof Boolean b) {
            o.addProperty(k, b);
         } else if (v instanceof Number n) {
            o.addProperty(k, n);
         } else {
            o.addProperty(k, v.toString());
         }
      }
      return o;
   }
}
