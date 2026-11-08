# Iron farm, middle: the optimizer's input (designs/briefs/farm-optimizer.md, "The loop").
# Argued in designs/briefs/farm-iron.md, Stages: Middle (revision 9). Owner: the stage-spec session.
#
# The player: a base, iron gear, the Nether (quartz, redstone, magma, slime); villagers from a
# village or a small breeder. No End. They build it in an evening, near the base.

farm: iron
stage: middle

# 1. Net iron per hour: the expected iron collected per hour over one game day from the running
#    farm, the weather cycle and natural spawning on Hard included; a break (a villager killed or
#    turned, the zombie lost, a part burnt) stops it for the rest of that day. Player: "a steady
#    supply: hoppers for the other farms, rails, anvils, a beacon's blocks".
objective: max iron_per_hour
# 2. The build in minutes, day 1's formula (2 s a block placed, 1 s dug, 120 s a lava trip,
#    180 s a villager moved). Player: "an evening, not a week".
objective: min build_minutes

constraint: materials <= nether
# vjeux 10:48: "middle one should have more than one module"
constraint: modules in 2..4
# the modules share at least one part (the zombie, the calm, the drop, the kill or the
# collection); copies at a no-coupling pitch are the bar, printed beside the front
constraint: shared_parts >= 1
# an evening's build; anything bigger is the late farm's job
constraint: build_minutes <= 120

space: placement void

merge: 10%

# What the card shows (vjeux 11:17: "adapt what is shown based on the objective"): the stage's
# default rows in designs/circuits/mobfarm/cards.tsv (key `mob-farm iron_ingot middle`), and each
# presented farm's own rows there. front.md lists those fields first for each member.
