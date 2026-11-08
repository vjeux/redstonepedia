# Iron farm, late: the optimizer's input (designs/briefs/farm-optimizer.md, "The loop").
# Argued in designs/briefs/farm-iron.md, Stages: Late (revision 9). Owner: the stage-spec session.
#
# The player: everything, End included (shulker boxes, crafters); villagers from a breeder. They
# AFK at one spot for hours (overnight) and want the most iron from a compact build (vjeux 04:26:
# "stack multiple modules next to each other and increase efficiency while keeping it small").
#
# Hard limits are only real facts about the player or the game (iteration 4, vjeux 12:18). The
# number of modules is the player's choice: the three objectives trade it, and every member
# prints its curve over N (iron per hour per block and the volume per module at N = 1, 2, 4, 8,
# 16).

farm: iron
stage: late

# 1. Net iron per hour PER MODULE over 8 h AFK, at the farm's own N (brief: Stages, Common, "A
#    farm and its N": builds that differ only in how many copies of one stacked unit they hold are
#    one farm, compared at the smallest N whose iron per hour per build minute is within 10% of its
#    best on the curve; the card prints the curve and the player picks N): from the running farm,
#    the weather cycle and natural spawning on Hard included; a break stops it for the rest of the
#    8 h; iron the storage cannot hold is lost. Player: "what one night AFK brings".
objective: max iron_per_hour_per_module
# 2. The build in minutes PER MODULE: placing, digging and getting every item at LATE prices
#    (designs/farms/prices.tsv: farms and super smelters for every material, a lava trip 30 s, a
#    villager delivered by a breeder 60 s). Player: "the effort". Iron per hour per block is
#    printed on every member; the best iron per hour per BUILD MINUTE is always on this front,
#    and the per-block best is listed beside the front when it falls off it.
objective: min build_minutes_per_module
# 3. The bounding box's volume per module. Player: "the room it takes beside the other farms
#    around the AFK spot"; it falls as modules share parts (vjeux 04:26: "keeping it small").
objective: min volume_per_module

# real: entities tick only within the simulation distance of a player (DistanceManager, a
# dedicated server's default 10), so every module runs from the one spot the player AFKs at
constraint: afk_spots <= 1
# real (no line needed: the grammar never builds past it): the world's build height, y -64..319

# Search boundary, not a player limit: N up to 16 modules; past the sharing's period each added
# module repeats the last one's numbers, so a larger N is the same farm with more copies of its
# stacked unit (the curve is printed). Copies at a no-coupling pitch (no shared part) are the bar,
# printed beside the front: a member that does not beat it on volume per module or iron per hour
# per build minute is reported as no gain from stacking. Placement: built in the open (the void:
# every wall placed) or dug into flat ground (the plains sample) where the grammar has it;
# neither is forbidden.

merge: 10%

# What the card shows (vjeux 11:17: "adapt what is shown based on the objective"): the stage's
# default rows in designs/circuits/mobfarm/cards.tsv (key `mob-farm iron_ingot fastest`: the
# site's name for the late stage), and each presented farm's own rows there. front.md lists those
# fields first for each member.
