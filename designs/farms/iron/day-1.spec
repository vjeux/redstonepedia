# Iron farm, day 1: the optimizer's input (designs/briefs/farm-optimizer.md, "The loop").
# Argued in designs/briefs/farm-iron.md, Stages: Day 1 (revision 9). Owner: the stage-spec session.
#
# The player: a fresh survival world. Wood and stone tools, a furnace, a bucket and a few iron
# from a cave, a village nearby (its villagers and beds). No Nether (no quartz), no slime.

farm: iron
stage: day-1

# 1. The minutes from the first block to owning a full iron kit (24 iron: the armour), at the
#    cheapest of the 12 nearest valid spots at village-edge: 2 s a block placed (gathering
#    included), 1 s a block dug, 120 s a lava trip, 180 s a villager moved, then
#    (build iron + 24) / net iron per hour. Player: "iron armour as soon as possible".
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
# default rows, staged in designs/farms/iron/cards.tsv (key `mob-farm iron_ingot day-1`) until the
# site reads designs/circuits/mobfarm/cards.tsv, and each presented farm's own rows there.
# front.md lists those fields first for each member.
