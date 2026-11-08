# Iron farm, day 1: the optimizer's input (designs/briefs/farm-optimizer.md, "The loop").
# Argued in designs/briefs/farm-iron.md, Stages: Day 1 (revision 9). Owner: the stage-spec session.
#
# The player: a fresh survival world. Wood and stone tools, a furnace, a village nearby (its
# villagers and beds). No Nether (no quartz), no slime. Every iron the build needs is mined first.

farm: iron
stage: day-1

# 1. T, MEASURED: the minutes from the first block until the farm's chest holds 24 iron (a full
#    iron kit: the armour). The player starts at dawn (day time 0) of day 1 with stone tools and
#    spends build_minutes: placing (1 s a block), digging (1 s a block; each dug block is also a
#    free filler block) and GETTING every item at day-1 prices (designs/farms/prices.tsv: glass
#    7 s, an iron ingot 30 s, the bucket and other tools once, a lava trip 120 s, a villager moved
#    180 s, the zombie 180 s), at the cheapest of the 12 nearest valid spots at village-edge. The
#    farm stands complete at day time 1200 x build_minutes (mod 24000); the villagers arrive with
#    the LAST_SLEPT of their last night in the village, the zombie in its boat; then the judge's
#    terrain run (weather cycle, natural spawning on Hard) until 24 iron are in the chest (the
#    build's own iron was mined, so the chest owes only the kit); the mean over its seeds. The run
#    performs every hand step the card prints (the first-night hide: the zombie hidden at the
#    first REST start for at least 20 gt): the player is there. Iteration 2 (the parent, from
#    D1U's first terrain day): measured, not build + (bill + 24) / rate. Iteration 3 (vjeux 12:01,
#    "this amount of glass is def not accessible day 1"): the items' acquisition priced.
#    Player: "iron armour as soon as possible".
objective: min t_kit_minutes
# 2. Net iron per hour once it runs: the judge's rate in the terrain (weather cycle, natural
#    spawning on Hard) less the iron it destroys (lightning on drops, fire). Player: "it keeps
#    running for days after the kit: tools, a shield, buckets, hoppers, rails, an anvil".
objective: max iron_per_hour

constraint: materials <= day-1
# what the village-edge sample's village has (8 villagers asleep in their beds)
constraint: villagers <= 8
constraint: modules in 1..2

# real terrain (vjeux 04:22): village-edge.mcterrain, natural rock as walls where it helps
space: placement dug

# none: no redstone dust (the bed plate drives the hide); dust: the player has dived for
# redstone ore (iron pickaxe counted, 3 iron): torches, repeaters and dust, the hide's pulse the
# smallest day-1 catalog part (no quartz, no slime). One front per value.
fork: redstone

# members within 10% on every objective are one farm (brief revision 9: the merge rule)
merge: 10%

# What the card shows (vjeux 11:17: "adapt what is shown based on the objective"): the stage's
# default rows in designs/circuits/mobfarm/cards.tsv (key `mob-farm iron_ingot day-1`), and each
# presented farm's own rows there. front.md lists those fields first for each member.
