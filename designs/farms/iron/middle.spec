# Iron farm, middle: the optimizer's input (designs/briefs/farm-optimizer.md, "The loop").
# Argued in designs/briefs/farm-iron.md, Stages: Middle (revision 9). Owner: the stage-spec session.
#
# The player: a base, iron gear, the Nether (quartz, redstone, magma, slime); villagers from a
# village or a small breeder. No End.
#
# Hard limits are only real facts about the player or the game (iteration 4, vjeux 12:18: "You
# just invented the two hours build limit. This is not a real restriction"). How big a build the
# player takes on is theirs to choose: build minutes is an objective, so the front shows what
# each extra hour of building buys.

farm: iron
stage: middle

# 1. Net iron per hour PER MODULE, at the farm's own N (brief: Stages, Common, "A farm and its
#    N": builds that differ only in how many copies of one stacked unit they hold are one farm,
#    compared at the smallest N >= 2 (the stage's limit) whose iron per hour per build minute is
#    within 10% of its best on the curve; the card prints the curve and the player picks N): the
#    expected iron collected per hour over one game day from the running farm, the weather cycle
#    and natural spawning on Hard included; a break (a villager killed or turned, the zombie
#    lost, a part burnt) stops it for the rest of that day. Player: "a steady supply: hoppers for
#    the other farms, rails, anvils, a beacon's blocks; each module costs its villagers and its
#    zombie".
objective: max iron_per_hour_per_module
# 2. The build in minutes PER MODULE: placing, digging and getting every item at MIDDLE prices
#    (designs/farms/prices.tsv: iron tools, 8 furnaces, the Nether, a day-1 farm's iron; a lava
#    trip 60 s plus 3 s a source; villagers moved by minecart or boat at 120 s each, or bred at
#    the farm, whichever is cheaper). Player: "the time it takes to build".
objective: min build_minutes_per_module

# real: the middle player has the Nether but not the End (no shulker box, no end rod)
constraint: materials <= nether
# vjeux 10:48, the owner's word: "middle one should have more than one module"; no maximum
constraint: modules >= 2
# real: entities tick only within the simulation distance of a player (DistanceManager), so
# every module runs from the one spot the player stands at
constraint: afk_spots <= 1

# Search boundary, not a player limit: N up to 16 modules; past the sharing's period each added
# module repeats the last one's numbers, so a larger N is the same farm with more copies of its
# stacked unit (the curve is printed). Placement: built in the open (the void: every wall placed)
# or dug into flat ground (the plains sample) where the grammar has it; neither is forbidden.
# Copies at a no-coupling pitch (no shared part) are the bar, printed beside the front.

merge: 10%

# What the card shows (vjeux 11:17: "adapt what is shown based on the objective"): the stage's
# default rows in designs/circuits/mobfarm/cards.tsv (key `mob-farm iron_ingot middle`), and each
# presented farm's own rows there. front.md lists those fields first for each member.
