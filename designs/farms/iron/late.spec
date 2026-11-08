# Iron farm, late: the optimizer's input (designs/briefs/farm-optimizer.md, "The loop").
# Argued in designs/briefs/farm-iron.md, Stages: Late (revision 9). Owner: the stage-spec session.
#
# The player: everything, End included (shulker boxes, crafters); villagers from a breeder. They
# AFK at one spot for hours (overnight) and want the most iron from a compact build (vjeux 04:26:
# "stack multiple modules next to each other and increase efficiency while keeping it small").

farm: iron
stage: late

# Every number is the whole build at N = 8 modules; the curve at N = 1, 2, 4 (and 16 when it
# fits) is printed with each member.
# 1. Net iron per hour over 8 h AFK: from the running farm, the weather cycle and natural
#    spawning on Hard included; a break stops it for the rest of the 8 h; iron the storage
#    cannot hold is lost. Player: "what one night AFK brings".
objective: max iron_per_hour
# 2. The build in minutes: placing, digging and getting every item at LATE prices
#    (designs/farms/prices.tsv: farms and super smelters for every material, a lava trip 30 s, a
#    villager delivered by a breeder 60 s). Player: "the effort". Iron per hour per block is
#    printed on every member; the best iron per hour per BUILD MINUTE is always on this front,
#    and the per-block best is listed beside the front when it falls off it.
objective: min build_minutes
# 3. The bounding box's volume per module. Player: "the room it takes beside the other farms
#    around the AFK spot".
objective: min volume_per_module

constraint: modules in 8..8
# stacking must pay (vjeux 04:26): iron per hour per block at N = 8 at least 5% above the module
# alone, and the volume per module at N = 8 at least 5% below it; copies at a no-coupling pitch
# are the bar, printed beside the front
constraint: per_block_gain >= 1.05
constraint: volume_gain >= 1.05
# every module ticks from one AFK spot (simulation distance 10, DistanceManager)
constraint: afk_spots <= 1
# the collection holds the night: 8 h of the whole farm's output (iron packed into blocks by the
# catalog's crafter, or chests; poppies composted or destroyed). A part-level limit (vjeux 11:23:
# parts searched on their own): a collection that fills sooner only loses iron over the night.
constraint: storage_hours >= 8

space: placement void

merge: 10%

# What the card shows (vjeux 11:17: "adapt what is shown based on the objective"): the stage's
# default rows in designs/circuits/mobfarm/cards.tsv (key `mob-farm iron_ingot fastest`: the
# site's name for the late stage), and each presented farm's own rows there. front.md lists those
# fields first for each member.
