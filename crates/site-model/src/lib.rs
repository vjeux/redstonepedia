//! REDSTONEPEDIA's data model and its query engine.
//!
//! `mctas site build` (crates/site) exports every build of the repository (the archived 3x3
//! piston doors, the clocks, and the circuit studies' designs) as one [`Site`] in
//! `site/data/site.json`; the browser UI (web/site: Rust compiled to WebAssembly) loads it and
//! filters it with [`Query`]. The query engine lives here, in plain Rust, so it is unit-tested
//! natively (`cargo test -p site-model`) and the browser only draws what it returns.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub mod html;

/// Bumped when a field changes meaning (the UI refuses a newer major).
pub const DATA_VERSION: u32 = 1;

/// Everything the site shows.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Site {
    pub version: u32,
    /// `YYYY-MM-DDTHH:MMZ`.
    pub generated: String,
    /// The repository commit the site was built from.
    pub commit: String,
    pub builds: Vec<Build>,
    /// Extendable clock families, one card each (families differing by one swapped block share
    /// a card, as in designs/clocks/CATALOG.md).
    #[serde(default)]
    pub cards: Vec<Card>,
    /// Every member n of every extendable family, generated at build time with the family's own
    /// `extend generator` (the clock time search expands families to these).
    #[serde(default)]
    pub members: Vec<Member>,
}

/// One build: an archived design with its properties.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Build {
    /// `doors/<stem>`, `clocks/<stem>`, `circuits/<kind>/<stem>`: unique, also the page path
    /// without `.html`.
    pub id: String,
    /// `door`, `clock` or `circuit`.
    pub kind: String,
    /// The main facet: a door's category (solid, gravity, sculk, entity), a clock's family (the
    /// block kinds that change state in a period: `observer+redstone_lamp`), a circuit's kind.
    pub group: String,
    pub name: String,
    /// Path of its page from the site root (`doors/fc2-40b.html`).
    pub page: String,
    /// Path of its design file from the site root (`designs/doors/fc2-40b.txt`).
    pub design: String,
    /// Blocks placed (a piston head is part of its piston; entities count one each).
    pub blocks: u32,
    /// Bounding-box volume (doors: the community's volume over a whole cycle; clocks: the box).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<u32>,
    /// Width x height x depth (`7x9x1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dims: Option<String>,
    /// Block kinds used (ids without `minecraft:`), sorted.
    #[serde(default)]
    pub kinds: Vec<String>,
    /// How it works (one paragraph).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub caption: String,
    /// Filterable yes/no properties ([`FLAGS`]).
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door: Option<Door>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<ClockInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circuit: Option<Circuit>,
    /// A real-game recording for the viewer's replay (`designs/clocks/traces/<stem>.trace`:
    /// entities and fluids redsim does not model), path from the site root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<String>,
    /// The thumbnail's still frame, found at build time by the viewer's own search
    /// (`mcview::tools::still_attr`: `<tick>[ out=x,y,z][ box=...]`, webrender's contract): the
    /// browser runs one pass to that tick instead of searching (a long clock deep in the Clocks
    /// tab: 3.6 s -> 0.1 s on a desktop). None: the viewer searches, as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub still: Option<String>,
    /// An interim picture (absolute URL: a clock's real-game picture, a door's board GIF) shown
    /// until the 3D viewer renders the build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// When it was archived (`YYYY-MM-DD HH:MMZ`; `""` unknown): the "newest first" sort.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub archived: String,
}

/// `2026-10-19T15:45Z` / `2026-10-19 15:45Z` -> `2026-10-19 15:45Z`.
pub fn normalize_time(s: &str) -> String {
    s.trim().replacen('T', " ", 1)
}

/// A 3x3 piston door's facts (designs/piston-door-3x3/archive and its caches).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Door {
    /// Worst-cycle ticks after the lever flip until open / closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close: Option<u32>,
    /// `safe`, `breaks at 90`, ... (`""`: unknown).
    #[serde(default)]
    pub rotation: String,
    /// The community regulation's seamless tier (`SUPER`, `FULL`, `SEMI`, `QUART`; `""` none).
    #[serde(default)]
    pub seamless: String,
    /// Frame cells holding the door block when open (`12/12`).
    #[serde(default)]
    pub frame: String,
    /// The boards it is on, and the ones it leads.
    #[serde(default)]
    pub boards: Vec<String>,
    #[serde(default)]
    pub leads: Vec<String>,
    /// Blocks of its row in the solid record progression (records.tsv), if it took a record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<String>,
    /// `door buildable`'s verdict (a survival build order), if a script exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buildable: Option<String>,
    /// The agent track it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    /// `YYYY-MM-DD HH:MMZ` it was archived.
    #[serde(default)]
    pub archived: String,
    #[serde(default)]
    pub entities: u32,
    /// Every cached property (static `door properties` + dynamic checks), for the properties
    /// table: (key, value).
    #[serde(default)]
    pub props: Vec<(String, String)>,
    /// The fill variants of one mechanism (`note: door fill of <design>`): the best of them (by
    /// size, then blocks) stands for the others, hidden with the variants by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<String>,
    /// The design it is a fill variant of (repository path).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fill_of: String,
    /// The one door card (vjeux 21:47Z): how many board records its page's table holds (0 on
    /// every other door).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub records: usize,
    /// The opening's side: 2 for a 2x2 door (designs/piston-door-2x2, the judge's `door-size:
    /// 2`); 0 for the 3x3 doors (the default, left out of the JSON). Read it with [`Door::side`].
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub size: u32,
}

impl Door {
    /// The opening's side: 3 (the 3x3 doors) or 2.
    pub fn side(&self) -> u32 {
        if self.size == 0 { 3 } else { self.size }
    }
}

/// A door build's opening side (3 for anything but a 2x2 door).
pub fn door_side(b: &Build) -> u32 {
    b.door.as_ref().map(Door::side).unwrap_or(3)
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// A clock's facts (designs/clocks/archive: verdicts-v1.tsv, extend.tsv, aliases.tsv).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ClockInfo {
    /// Period in game ticks (20 gt = 1 s): the whole state's cycle.
    pub period: u32,
    /// The output's own period when it pulses several times, evenly, per cycle (clockinfra's
    /// gaps: N equal gaps g): what a builder means by the clock's time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulse_every: Option<u32>,
    /// What a chunk reload mid-run does to it (clockinfra's `reload` column: `ok`, `glitch then
    /// resumes`, `stops`, `changes period to P gt`, `fails: why`, each maybe `in K of N builds`;
    /// empty: not checked).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reload: String,
    /// Whether a builder can take the output into a circuit (the judge's `usable` column,
    /// `clock::usable`: `yes`, or `no: <why>`; empty: not checked): a plain warning when not,
    /// never a reason to hide.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub usable: String,
    /// The one clock card (vjeux 21:47Z: one card per kind): the periods its page's table and
    /// families cover and how many were tested, `lo-hi:tested` (`4-312816:172`); empty on every
    /// other clock.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind_range: String,
    pub warmup: u32,
    /// Output on-ticks per period, rising edges per period, pulse widths.
    pub on: u32,
    pub edges: u32,
    #[serde(default)]
    pub pulses: String,
    /// `robust`, or how it depends on position / rotation.
    #[serde(default)]
    pub robustness: String,
    /// How it starts (`self-starting`, `kick: last 1,0,0`...).
    #[serde(default)]
    pub start: String,
    /// Its mirror image is a different type.
    #[serde(default, skip_serializing_if = "is_false")]
    pub chiral: bool,
    /// Other archived names of the same type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// Build id of the smaller clock of its period whose block kinds it only adds to (a variant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Build ids of its variants.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<String>,
    /// A mechanism's card (crates/site/src/mechanism.rs: the clocks of its period and mechanism
    /// fold under it, `variants`): the blocks they swap in (`note_block`), its "also works with".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub swaps: Vec<String>,
    /// The extendable family it is the base design or a member of: card id, family id, n.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<i64>,
    /// A beat clock (sub-clocks side by side, marked `# composite`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub composite: bool,
    /// An archived design that is a member of an extendable family other than its base design
    /// (hidden with the variants by default: the family's card stands for it, as in the catalog).
    #[serde(default, skip_serializing_if = "is_false")]
    pub instance: bool,
    /// Its extendable family, for a base design and its longer versions alike (vjeux 01:24Z:
    /// longer versions are clocks of their own; critique 41f1ec62a: the hopper clock's page never
    /// said one design makes any period from 1 s to 4 min 15 s).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extend: Option<Extend>,
    /// What a redstone lamp on its output does, simulated with one there (critique a417977c3: a
    /// lamp goes dark only after more than 4 gt without power): `stays lit` (never dark while the
    /// output pulses), `relit` (stays lit though the output is off longer: relit within the tick)
    /// or `misses pulses` (lights fewer times than the output rises); empty when it follows every
    /// pulse or was not simulated. `lamp_gap`: the output's shortest off-gap, in gt.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub lamp: String,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub lamp_gap: u32,
}

/// A clock's extendable family, in words and numbers.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Extend {
    /// The tested base design's build id.
    pub base: String,
    /// The name of its kind of clock (`dropper ring`), may be empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// How one step extends it (`one more stone in the first hopper (same blocks; ...)`).
    pub rule: String,
    /// The n of the base design.
    pub base_n: i64,
    /// What one step adds: ticks, blocks.
    pub dt: i64,
    pub db: i64,
    /// The slowest of the family on the site, in gt.
    pub hi: u32,
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

impl ClockInfo {
    /// How often its output pulses: the output's own period, else the whole cycle.
    pub fn every(&self) -> u32 {
        self.pulse_every.unwrap_or(self.period)
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// A circuit's facts (designs/circuits: the contraption studies, docs/circuits.md): its kind, the
/// value of its study's axis (pulse length, delay, reach...) and the other metrics, as the
/// judge's checker reports them.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Circuit {
    /// The study (`pulse`, `tff`, `delay`, `gates`, `memory`, `pext`, `fly`).
    pub study: String,
    /// Where it sorts among circuits: its study's place (pulse shapers first), the basics of a
    /// study last (the gates with their own levers, the sand elevators).
    #[serde(default)]
    pub order: u32,
    /// Its place within its kind (0 = the first shown design of the kind): the Circuits tab opens
    /// on the first of every kind, then the second... (critique C40: it opened on six cards of
    /// one kind).
    #[serde(default)]
    pub rank: u32,
    /// The variation a machine is the best design for (a shulker loader's badge where it is the
    /// fastest, then smallest): its card leads with it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub lead: String,
    /// Why this design is the one on the site (vjeux 21:47Z: one per kind; critique: "cards don't
    /// say why they're the best"): `smallest of 14 tested`, `tiles side by side, 1 block wide`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub claim: String,
    /// The other kinds this same build is archived under (`Signal shortener` for a 2-block
    /// rising-edge detector): its card says it is also one of those.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also: Vec<String>,
    /// What a machine carries or pulls, in words (a flying machine of the payload family with n
    /// extra blocks: `carries 9 extra blocks`; of the train family: `pulls 2 trailers`); how an
    /// iron farm works, from its design's `how:` line (`golems burn in lava pits`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub feature: String,
    /// A group card's sizes when its members differ (`3 to 15 blocks`): its title says them.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub span_blocks: String,
    /// The kind as its `circuit:` line writes it (`rising-edge`, `gate xor`, `counter 4`).
    #[serde(default)]
    pub kind: String,
    /// The value of the study's axis (`L 4 gt`, `xor`, `N 3`), and its number when numeric.
    #[serde(default)]
    pub axis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<i64>,
    /// Every metric in report order: (name, value).
    #[serde(default)]
    pub metrics: Vec<(String, String)>,
    /// Where the metrics come from: the judge's verdict line, else the redsim screen's.
    #[serde(default)]
    pub verdict: String,
    /// Inputs and outputs (port names), the markers of the 3D view.
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    /// The mechanism family (the block kinds that change state), the judge's robustness.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub family: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub robustness: String,
    /// Build id of the smaller design of its kind and axis value whose block kinds it only adds
    /// to (a variant, as `mctas circuit catalog` folds them).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Build id of the shown design this one is an equivalent layout of (same kind, value, block
    /// count and block kinds: "also N other layouts" on that design's page).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_of: Option<String>,
    /// A machine's travel direction in words (`horizontal`, `up`, `down`: its `travel:` header),
    /// part of a flying machine's place on its study's axis.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub direction: String,
    /// Other archived designs that are the SAME build under another kind (the same blocks and
    /// ports: the copper bulb T flip-flop is both `tff-bulb` and `counter2-bulb`), by build id.
    /// Pages only.
    #[serde(skip)]
    pub same_build: Vec<String>,
    /// The tunable group it belongs to (one mechanism with other repeater delays, comparator modes
    /// or a longer line: the catalog's `group` column), and on the group's shown design (the
    /// smallest) the group's span (`L 6-40 gt, 16 values`); the other members are hidden by
    /// default and come back through the value search (`L 12`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tunable: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub span: String,
    /// The group's title and note (`group title:` / `group note:` headers). Pages only.
    #[serde(skip)]
    pub group_title: String,
    #[serde(skip)]
    pub group_note: String,
    /// What the group's members differ in (`the input device`, `the repeater delays`...), and
    /// this member's own value of it (`stone button`, `delays 4+4+2`), computed from the members'
    /// designs. Pages only.
    #[serde(skip)]
    pub group_differs: String,
    #[serde(skip)]
    pub variant_label: String,
    /// The extendable family card it is a base design of (`families/<card>.html`); a card with
    /// several families shows its first base only (`card_member` on the others: hidden like a
    /// tunable group's members).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub card: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub card_member: bool,
    /// The kind's design that sits right on the ground, when this pick stands higher (its build
    /// id; vjeux 11:49Z: the 22-block clocked TNT duper beside the 16-block engine standing 4 up).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground: Option<String>,
    /// A design made to be copied side by side: its `tile: dx dy dz` header (a crop farm's copies,
    /// w86's study 17), empty when it has none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tile: String,
    /// A crop farm's crop cells (its `port crop` lines: one plant each), 0 for anything else.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub crops: u32,
    /// The block a crop farm's crop is planted on (`dirt`, `mud`: the block under its first `port
    /// crop` cell), empty for anything else.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub soil: String,
    /// A crop farm whose crop cell holds water in the design (kelp: planted in a water source).
    #[serde(default, skip_serializing_if = "is_false")]
    pub crop_wet: bool,
    /// The kind's best design without an on/off switch, when this pick has one (its build id; the
    /// coordinator 14:08Z: every TNT duper's pick has a lever, the smaller one without stays).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_switch: Option<String>,
    /// A piston extender's best design of this size that is safe to flip back any time, when this
    /// pick is not (its build id; the coordinator 2026-10-21, clk-misc's `flips` metric).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_safe: Option<String>,
    /// A piston bridge's design of the same gap with the other walkway (slime / stone), linked
    /// both ways (its build id; pext 2026-10-21).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt_walkway: Option<String>,
    /// An auto-smelter's design of the same number of furnaces with the other furnace type
    /// (blast furnaces / furnaces), linked both ways: the blast version is the pick's variant
    /// (its build id; the coordinator 2026-10-22, sorter's super smelters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blast_twin: Option<String>,
    /// A crop farm's full-size pick and its crop's best small module (1 to 4 plants), linked both
    /// ways: the module is the pick's variant (its build id; the coordinator via w86 2026-10-22).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub farm_twin: Option<String>,
    /// The flying machines in the design (clusters of slime or honey blocks with the pistons and
    /// observers touching them: `circuits::flying_machines`), 0 for most.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub flying_machines: u32,
    /// A crop farm's pick with a flying machine and its crop's best full-size farm without one,
    /// linked both ways (the coordinator 2026-10-22: the sweeper and the piston lattice).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_twin: Option<String>,
    /// An automatic door's pick that closes on someone standing in the doorway and its kind's
    /// best door that stays open for them, linked both ways (the coordinator 2026-10-22: the
    /// obvious door and lock-v12, its "stays open for someone in the doorway" variant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doorway_twin: Option<String>,
    /// A crop farm's pick and its crop's farm of the same kind on about half its field (the
    /// reviewer via the coordinator 2026-10-22: bamboo-sweeper-10x32 beside the 10x64, "half the
    /// field: ~1,350 items smelted an hour · 1,468 blocks"), linked both ways.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub half_twin: Option<String>,
    /// A big crop farm's judge's random tick speed (`circuit::farm::game_rts`), the page view's
    /// `data-farm-rts` (webrender 2026-10-22: without it the view shows a big farm as built).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub farm_rts: Option<u32>,
    /// A whole minecart line (`cart-line`, sorter's crate::cartsys designs in a study's `system/`
    /// folder): the stations its design names, by build id (sorter 2026-10-22: line-b is ld-chest,
    /// 128 blocks of track and ul-gate).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<String>,
    /// A station of a published minecart line: that line's build id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_of: Option<String>,
    /// A line's station kept beside its kind's pick (ul-gate, twice as fast as the smallest
    /// unloader), linked both ways: on the station, the pick's id; on the pick, the station's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station_twin: Option<String>,
    /// A display's lit still (`designs/stills/<study>/<name>.txt` from the site root): the design
    /// settled at a digit, block for block, which its card and its page's view show (the
    /// coordinator 2026-10-22: the 0-9 display's still at tick 0 showed 13 dark lamps; no
    /// simulation on load). Its downloads stay the design as built.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub view_src: String,
    /// The lit still's text (written next to the designs by `generate`). Not in the data.
    #[serde(skip)]
    pub view_text: String,
    /// A card's own still, when it differs from the page view's (`designs/stills/<study>/<name>.txt`
    /// from the site root): a crop farm's field grown (the coordinator 2026-10-22: "so it reads as
    /// a farm"), while its page view loads the design as built, so a played view replays the
    /// judge's run from its planting. Cards never simulate.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub card_src: String,
    /// The card's still's text (written by `generate`). Not in the data.
    #[serde(skip)]
    pub card_text: String,
    /// When the judge archived it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub archived: String,
    /// How each input is driven (parallel to `inputs`): `redstone block`, `lever`, `button`, or
    /// the custom on-state. Pages only (not exported).
    #[serde(skip)]
    pub devices: Vec<String>,
    /// Each input's button as the 3D view labels it (parallel to `inputs`; webrender's
    /// `mcview::circuit::input_label`: "A", "CLK", a lock's "Dial 1"): a page names the buttons a
    /// visitor sees (critique 058332315: "D0 … K2" where the view says "Dial 1 … Code 3"). Pages
    /// only (not exported).
    #[serde(skip)]
    pub input_labels: Vec<String>,
    /// The judge's stimulus as the design format's `stimulus:` text (per input its edges), for
    /// the viewer's demo (`data-stimulus`), and its labeled phases (`from-to label; ...`). Pages
    /// only.
    #[serde(skip)]
    pub stimulus: String,
    /// A strength input's values over the judge's test (`<frame> <input> <value>; ...`, the hex
    /// kinds), for the viewer's demo (`data-levels`). Pages only.
    #[serde(skip)]
    pub levels: String,
    #[serde(skip)]
    pub phases: String,
    /// The judge's test run on redsim, drawn as a timing diagram on the build page. Pages only.
    #[serde(skip)]
    pub wave: Option<Wave>,
    /// The checker's report, one line per test (`pulse 4: out 2 gt after 5 gt`). Pages only.
    #[serde(skip)]
    pub table: Vec<String>,
    /// Every output cell holds the judge's lamp in the 3D view (`circuit::Circuit::consumer_cells`:
    /// air output cells), so the lamp shows the output; else the orange marker does. Pages only.
    #[serde(skip)]
    pub output_lamp: bool,
    /// The judge's other actions (the BUD kind's block updates at its target and probe cells),
    /// for the viewer's demo (`data-events`): `<frame> <op> <x> <y> <z> [<state>]; ...` with op
    /// `place` (with updates), `clear`, `shape` (shape updates only) or `update` (a pure block
    /// update around the cell), positions in the design's coordinates. Pages only.
    #[serde(skip)]
    pub events: String,
    /// Why the browser's simulator cannot run this design (`block "oak_fence" is not modeled`),
    /// empty when it can: the 3D view then shows it as built (the viewer's own session decides,
    /// `mcview::session::Session::unsimulated`). Pages only.
    #[serde(skip)]
    pub unmodeled: String,
    /// Where the 3D view's demo stops, raw (`tick 44: dropper ... picks a random slot among 9
    /// stacks (not modeled)`), empty when it plays to its end: then the page does not autoplay a
    /// demo that stops after its first pulses, and says why (webrender's QA). Pages only.
    #[serde(skip)]
    pub view_halt: String,
    /// A shulker machine run the way its 3D view runs it (the viewer's session, start lever on):
    /// the boxes it sends out. Pages only.
    #[serde(skip)]
    pub view_run: Option<ViewRun>,
    /// The design file has `extend` headers: the base design of an extendable family (one more
    /// unit per n). The dominated-design audit counts it as a builder's property. Pages only.
    #[serde(skip)]
    pub extends: bool,
    /// An extendable family's values from its base design's headers (`extend axis: 30 + 16n`,
    /// `extend n: 0..10` -> (30, 190, 16)): (lowest, highest, step); its card shows the range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extend_range: Option<(i64, i64, i64)>,
    /// The `extend n: lo..hi` header of an extendable design (a tall flying machine's cargo: up to
    /// `hi` extra blocks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extend_n: Option<(i64, i64)>,
    /// A port above or below the design's own layers: its input or output stacks over it in use
    /// (no "lies flat" chip then).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ports_off_layers: bool,
    /// A button sequence lock's code as built: the buttons in order (`a b a`), read from the
    /// rotations of the item frames on its back (clk-item: a frame's rotation names its step's
    /// button, 0 = the first).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub seq_code: String,
    /// A counter's measured duty in the judge's run: (pulse windows its output is on, N). Pages
    /// only.
    #[serde(skip)]
    pub counter_on: Option<(u32, u32)>,
    /// The design's blocks without their contents (the verdict's type key, NBT stripped): designs
    /// that are one build with other items preloaded share it. Site build only.
    #[serde(skip)]
    pub layout_key: String,
}

/// A machine run like its 3D view runs it ([`Circuit::view_run`]).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ViewRun {
    /// Game ticks run (from the end of the judge's setup).
    pub ticks: u32,
    /// Boxes sent out to the OUT chests.
    pub boxes: u32,
    /// The tick the first box arrived.
    pub first: Option<u32>,
    /// Game ticks per box after the first.
    pub cycle: Option<f64>,
    /// The item the view's endless chests supply (`iron_shovel`...).
    pub item: String,
    /// Items that reached a filter's reject chests.
    pub rejected: u32,
    /// OUT chests (a machine with two lines delivers into two).
    pub lines: u32,
}

/// A timing diagram: the judge's test run on redsim, per input and output its signal per tick.
/// (Serializable for the build's simulation cache; never in site.json.)
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Wave {
    /// Frames 0..ticks.
    pub ticks: u32,
    pub rows: Vec<WaveRow>,
    /// The checker's labeled spans: (from, to, label).
    pub phases: Vec<(u32, u32, String)>,
}

/// One signal of a [`Wave`]: an input (on/off), an output (its level 0..15), or a position track
/// (`scale` > 0: a piston extender's payload, 0 = home .. N = extended; a flying machine's
/// displacement along its travel), as runs of equal value: (from frame, value); or the judge's
/// actions at a BUD's target (`marks`: frame, `place` / `clear` / `shape` / `update`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct WaveRow {
    pub name: String,
    pub input: bool,
    pub steps: Vec<(u32, u8)>,
    /// A position track's full scale (0: a signal, 0..15).
    pub scale: u32,
    /// The value it must reach (an extender's N), drawn as a dashed line.
    pub target: Option<u32>,
    /// A short annotation (`extends in 9 gt, back in 9 gt`).
    pub note: String,
    pub marks: Vec<(u32, String)>,
}

impl WaveRow {
    /// The runs of a per-frame signal.
    pub fn from_levels(name: &str, input: bool, levels: &[u8]) -> WaveRow {
        let mut steps: Vec<(u32, u8)> = Vec::new();
        for (t, l) in levels.iter().enumerate() {
            if steps.last().is_none_or(|s| s.1 != *l) {
                steps.push((t as u32, *l));
            }
        }
        WaveRow { name: name.to_string(), input, steps, ..Default::default() }
    }
}

/// A period or block-count formula in n (the clock study's `extend period:`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "f")]
pub enum Formula {
    /// a + b·n
    #[serde(rename = "lin")]
    Linear { a: i64, b: i64 },
    /// a·k^n + c
    #[serde(rename = "geo")]
    Geometric { a: i64, k: i64, c: i64 },
}

impl Formula {
    pub fn eval(&self, n: i64) -> i64 {
        match *self {
            Formula::Linear { a, b } => a + b * n,
            Formula::Geometric { a, k, c } => a.saturating_mul(k.saturating_pow(n.max(0) as u32)).saturating_add(c),
        }
    }

    /// The n in `lo..=hi` whose value lies in `[min, max]` (the time search's "which member hits
    /// it"), in increasing n.
    pub fn solve(&self, lo: i64, hi: i64, min: i64, max: i64) -> Vec<i64> {
        (lo..=hi).filter(|&n| (min..=max).contains(&self.eval(n))).collect()
    }
}

/// One extendable card.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Card {
    /// Slug, also its page `families/<id>.html`.
    pub id: String,
    pub title: String,
    /// `P = 10 + 4n gt` (several families: the general formula).
    pub heading: String,
    /// The same in seconds (`0.5 s + 0.2 s·n`), when the card's families share one formula.
    #[serde(default)]
    pub seconds: String,
    pub page: String,
    /// The block kinds every family of the card uses, and the blocks that can be swapped in
    /// ("works with any one of these").
    #[serde(default)]
    pub core: Vec<String>,
    #[serde(default)]
    pub works_with: Vec<String>,
    #[serde(default)]
    pub note: String,
    /// The card's families, the pictured (simplest) one first.
    pub families: Vec<Family>,
    /// `circuit` for an extendable circuit family's card (its formula is the study's axis value,
    /// its members carry their `axis`), empty for a clock card; the circuit's study.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub study: String,
}

/// One extendable family (a base design with `extend` headers).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Family {
    pub id: String,
    /// Build id of its base design.
    pub build: String,
    pub period: Formula,
    pub period_text: String,
    pub blocks: Formula,
    pub blocks_text: String,
    /// How to extend it.
    pub rule: String,
    /// The n range verified in the game, and the n of the base design.
    pub n_lo: i64,
    pub n_hi: i64,
    pub base: i64,
    /// The n judged individually in the game (archive/extend.tsv).
    #[serde(default)]
    pub judged: Vec<i64>,
    /// Block kinds beyond the card's core (the swapped relay).
    #[serde(default)]
    pub extra: Vec<String>,
}

/// One member of an extendable family, generated at build time.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Member {
    /// Its own numbers from the clock judge's simulation (a generated clock member; None: not
    /// simulated, or no clock found): its page's, never its base design's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<MemberRun>,
    pub card: String,
    pub family: String,
    pub n: i64,
    /// The formula's period and blocks for this n.
    pub period: u32,
    pub blocks: u32,
    /// Path of its generated design from the site root.
    pub design: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dims: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<u32>,
    /// [`STATUS_JUDGED`] (judged in the real game: archive/extend.tsv), [`STATUS_REDSIM`] (its
    /// generated design simulated in redsim at build time: the formula's period, exactly) or
    /// [`STATUS_FORMULA`] (formula, not individually judged).
    pub status: String,
    /// The archived build of this exact member, if any (its base design, or an archived instance).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    /// An interim picture (absolute URL), for the members the catalog pictured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// A circuit family's member: its axis value as the study writes it (`N = 26 gt`, `L = 50 gt`,
    /// `N = 5`); `period` then holds that value's number. Empty for a clock member.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub axis: String,
    /// The archived design it is built from (its family's base build id): with no family pages
    /// (vjeux 00:33Z), a clock member is listed under its period with its base's words (00:43Z).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub base: String,
}

pub const STATUS_JUDGED: &str = "judged";

/// A generated clock member's numbers, measured by the clock judge's simulation (redsim): its
/// visible warm-up, its output's on-ticks and rising edges per period, its pulse widths (`[2, 2]`)
/// and its pulses' own period when they are even within the cycle.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MemberRun {
    pub warmup: u32,
    pub on: u32,
    pub edges: u32,
    pub pulses: String,
    pub pulse_every: Option<u32>,
}
pub const STATUS_REDSIM: &str = "redsim";
pub const STATUS_FORMULA: &str = "formula";

/// A member's status in words.
pub fn status_label(s: &str) -> &'static str {
    match s {
        STATUS_JUDGED => "tested in the game",
        STATUS_REDSIM => "simulated",
        _ => "formula only",
    }
}

/// The flags, (key, label, the kinds it applies to: comma-separated), in display order.
pub const FLAGS: &[(&str, &str, &str)] = &[
    ("rotation-safe", "rotation-safe", "door"),
    ("slime-free", "no slime or honey", "door"),
    ("flat", "flat (one block thick)", "door"),
    ("wall", "fits a 3-thick wall", "door"),
    ("ground", "on the ground", "door"),
    ("seamless", "seamless (any tier)", "door"),
    ("seamless-full", "seamless FULL or better", "door"),
    ("hallway", "hallway-compatible", "door"),
    ("frame-full", "full frame when open", "door"),
    ("eligible", "record-eligible", "door"),
    ("record", "took a record", "door"),
    ("leader", "the best door for something (the most compact or the fastest of its kind)", "door"),
    ("video", "has a video", "door"),
    ("buildable", "buildable in survival", "door"),
    ("entities", "uses entities", "door"),
    ("robust", "works in any direction, anywhere", "clock,circuit"),
    ("self-starting", "starts by itself", "clock"),
    ("kicked", "needs one kick to start", "clock"),
    ("chiral", "mirror image is a different design", "clock"),
    ("extendable", "comes in every size", "clock,circuit"),
    ("variant", "variant of a smaller design", "door,clock,circuit"),
    ("census", "found by the census, not the smallest of its period", "clock"),
    ("twin", "the same numbers as another clock shown", "clock"),
    ("kind-twin", "the same build as a design of another kind", "circuit"),
    ("tunable", "several versions of one mechanism", "circuit"),
    ("layout", "another layout of a shown design", "circuit"),
    ("composite", "several clocks in one", "clock"),
    ("instant", "instant (reacts in the same tick)", "circuit"),
    ("button", "its own button as the input", "circuit"),
    ("lever", "its own lever as the input", "circuit"),
    ("faster", "faster than the smallest of its value", "circuit"),
    ("mirrored", "its mirror image (A and B swapped) is folded under it", "circuit"),
    ("cargo", "carries extra blocks along (a flying machine with cargo)", "circuit"),
    ("there-and-back", "flies there and back between stations", "circuit"),
    ("traced", "real-game replay", "clock"),
];

pub fn flag_label(key: &str) -> &str {
    FLAGS.iter().find(|f| f.0 == key).map(|f| f.1).unwrap_or(key)
}

// ---- time ------------------------------------------------------------------------------------

/// Just the real time of `gt` game ticks: `1.5 s`, `1 min 55.2 s`, `2 h 5 min` (as the clock
/// catalog writes it).
pub fn real_time(gt: u32) -> String {
    let num = |x: f64| -> String {
        let s = format!("{x:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let secs = gt as f64 / 20.0;
    if secs < 60.0 {
        return format!("{} s", num(secs));
    }
    let mins = (secs / 60.0).floor();
    let rest = secs - mins * 60.0;
    if mins < 60.0 {
        return if rest > 0.0 { format!("{} min {} s", mins, num(rest)) } else { format!("{} min", mins) };
    }
    let hours = (mins / 60.0).floor();
    let m = mins - hours * 60.0;
    if m > 0.0 { format!("{} h {} min", hours, m) } else { format!("{} h", hours) }
}

/// `20 ticks (1 s)`: visitor text says ticks, not gt (critique: the search's count line).
pub fn gt_time(gt: u32) -> String {
    format!("{gt} {} ({})", if gt == 1 { "tick" } else { "ticks" }, real_time(gt))
}

/// Parse a clock time search: a period in game ticks (`48`, `48 gt`, `48 ticks`), a time
/// (`2.5 s`, `1 min`, `1 min 30 s`, `1:30`, `2 h`), or a range of either (`40-60 gt`,
/// `40..60`, `2-3 s`, `2 s to 3 s`, `< 60 gt`, `>= 1 min`). Returns the inclusive range in gt
/// (an exact time is the one period it names; a time between two ticks matches nothing).
pub fn parse_time_query(q: &str) -> Result<(u32, u32), String> {
    parse_time_range(q)
}

/// What a circuit value search compares: the study's axis value (any study, or only the studies
/// whose axis is L, the output pulse length or a timer's hold, N: a delay, a count, a reach, T: an
/// on-delay's threshold, P: a pulse train's period) or the latency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueField {
    Axis,
    L,
    N,
    T,
    P,
    Latency,
}

/// Parse a circuit value search: an optional field (`L` = an output pulse length, `N` = a delay
/// line's delay, a counter's count or an extender's reach, `T` = an on-delay's threshold, `P` = a
/// pulse train's period, `reach` / `speed` / `delay` = the study's axis value; `latency` / `lat`
/// = the latency), an optional `=` or `:`, then a number, a time or a range as
/// [`parse_time_query`] reads them (`L 12`, `N=3`, `T 10`, `12`, `10-20 gt`, `lat 0`).
pub fn parse_value_query(q: &str) -> Result<(ValueField, u32, u32), String> {
    let t = q.trim();
    let lower = t.to_lowercase();
    let mut field = ValueField::Axis;
    let mut rest = lower.as_str();
    for (word, f) in [
        ("latency", ValueField::Latency),
        ("lat", ValueField::Latency),
        ("reach", ValueField::N),
        ("speed", ValueField::Axis),
        ("delay", ValueField::N),
        ("l", ValueField::L),
        ("n", ValueField::N),
        ("t", ValueField::T),
        ("p", ValueField::P),
    ] {
        if let Some(r) = rest.strip_prefix(word) {
            // the word must stand alone: `l 12`, `l=12`, `l12`, not `lever`
            if r.is_empty() || r.starts_with([' ', '=', ':']) || r.starts_with(|c: char| c.is_ascii_digit()) {
                field = f;
                rest = r;
                break;
            }
        }
    }
    let rest = rest.trim_start_matches([' ', '=', ':']).trim();
    if rest.is_empty() {
        return Err(format!("{t}: which value? (e.g. L 12, N 3, latency 0)"));
    }
    let (lo, hi) = parse_time_query(rest)?;
    Ok((field, lo, hi))
}

fn parse_time_range(q: &str) -> Result<(u32, u32), String> {
    let s = q.trim().to_lowercase().replace('–', "-").replace('—', "-").replace('≤', "<=").replace('≥', ">=");
    if s.is_empty() {
        return Err("empty".into());
    }
    for (op, f) in [("<=", 0), (">=", 1), ("<", 2), (">", 3)] {
        if let Some(rest) = s.strip_prefix(op) {
            let v = parse_amount(rest.trim(), None)?;
            return Ok(match f {
                0 => (0, floor_gt(v)),
                1 => (ceil_gt(v), u32::MAX),
                2 => (0, ceil_gt(v).saturating_sub(1)),
                _ => (floor_gt(v).saturating_add(1), u32::MAX),
            });
        }
    }
    // a range: "a-b", "a..b", "a to b" (a unit on the right end only applies to both)
    let split = ["..", " to ", "-"].iter().find_map(|sep| s.find(sep).filter(|&i| i > 0).map(|i| (i, sep.len())));
    if let Some((i, len)) = split {
        let (a, b) = (s[..i].trim(), s[i + len..].trim());
        let unit_b = unit_of(b);
        let hi = parse_amount(b, None)?;
        let lo = parse_amount(a, unit_b)?;
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        let (l, h) = (ceil_gt(lo), floor_gt(hi));
        if l > h {
            return Err(format!("no whole game tick between {} and {}", a, b));
        }
        return Ok((l, h));
    }
    let v = parse_amount(&s, None)?;
    let (l, h) = (ceil_gt(v), floor_gt(v));
    if l > h {
        return Err(format!("{} is not a whole number of game ticks (1 gt = 0.05 s)", q.trim()));
    }
    Ok((l, h))
}

const EPS: f64 = 1e-6;

fn ceil_gt(v: f64) -> u32 {
    (v - EPS).ceil().max(0.0).min(u32::MAX as f64) as u32
}

fn floor_gt(v: f64) -> u32 {
    (v + EPS).floor().max(0.0).min(u32::MAX as f64) as u32
}

/// Game ticks per unit word.
fn unit_scale(u: &str) -> Option<f64> {
    Some(match u {
        "" | "gt" | "t" | "tick" | "ticks" | "game tick" | "game ticks" => 1.0,
        "rt" | "redstone tick" | "redstone ticks" => 2.0,
        "s" | "sec" | "secs" | "second" | "seconds" => 20.0,
        "m" | "min" | "mins" | "minute" | "minutes" => 1200.0,
        "h" | "hr" | "hrs" | "hour" | "hours" => 72000.0,
        _ => return None,
    })
}

/// The unit word at the end of an amount (`"3 s"` -> `s`), if it names one.
fn unit_of(s: &str) -> Option<f64> {
    let t = s.trim();
    let i = t.rfind(|c: char| c.is_ascii_digit() || c == '.')?;
    let u = t[i + 1..].trim();
    if u.is_empty() { None } else { unit_scale(u) }
}

/// An amount in game ticks: `48`, `48gt`, `2.5 s`, `1 min 30 s`, `1:30` (m:ss); a bare number
/// takes `default_unit` (gt when none).
fn parse_amount(s: &str, default_unit: Option<f64>) -> Result<f64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("missing number".into());
    }
    if let Some((m, sec)) = s.split_once(':') {
        let m: f64 = m.trim().parse().map_err(|_| format!("bad time {s:?}"))?;
        let sec: f64 = sec.trim().trim_end_matches('s').trim().parse().map_err(|_| format!("bad time {s:?}"))?;
        return Ok(m * 1200.0 + sec * 20.0);
    }
    // a sequence of "<number> <unit>" parts: "1 min 30 s"
    let mut total = 0.0;
    let mut rest = s;
    let mut parts = 0;
    while !rest.is_empty() {
        let num_end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
        if num_end == 0 {
            return Err(format!("bad time {s:?}"));
        }
        let n: f64 = rest[..num_end].parse().map_err(|_| format!("bad number in {s:?}"))?;
        rest = rest[num_end..].trim_start();
        let unit_end = rest.find(|c: char| c.is_ascii_digit()).unwrap_or(rest.len());
        let unit = rest[..unit_end].trim();
        let scale = if unit.is_empty() { default_unit.unwrap_or(1.0) } else { unit_scale(unit).ok_or_else(|| format!("unknown unit {unit:?}"))? };
        total += n * scale;
        rest = rest[unit_end..].trim_start();
        parts += 1;
    }
    if parts == 0 {
        return Err(format!("bad time {s:?}"));
    }
    Ok(total)
}

// ---- query -----------------------------------------------------------------------------------

/// An inclusive numeric range; `None` ends are open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Range {
    pub min: Option<i64>,
    pub max: Option<i64>,
}

impl Range {
    pub fn is_open(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    /// A value passes when it lies in the range; a missing value passes only an open range.
    pub fn admits(&self, v: Option<i64>) -> bool {
        match v {
            None => self.is_open(),
            Some(v) => self.min.is_none_or(|m| v >= m) && self.max.is_none_or(|m| v <= m),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    /// The build's main number, then blocks (vjeux 12:09Z): doors by size, clocks by period,
    /// circuits by kind, then their value (N, L, P...).
    #[default]
    Main,
    /// Fewest blocks, then volume, then name.
    Blocks,
    Volume,
    Period,
    Open,
    Close,
}

impl Sort {
    /// The sorts the index offers (vjeux 13:35Z deleted "newest"; names are not shown any more,
    /// so no sort by name): an old URL's `sort=newest` or `sort=name` falls back to the default.
    pub const ALL: [(Sort, &'static str, &'static str); 6] = [
        // (short: the select's longest option set the page's width on a phone, critique blocker)
        (Sort::Main, "main", "best first"),
        (Sort::Blocks, "blocks", "fewest blocks"),
        (Sort::Volume, "volume", "least space"),
        (Sort::Period, "period", "shortest period"),
        (Sort::Open, "open", "fastest open"),
        (Sort::Close, "close", "fastest close"),
    ];

    pub fn key(self) -> &'static str {
        Sort::ALL.iter().find(|s| s.0 == self).map(|s| s.1).unwrap_or("main")
    }

    pub fn from_key(k: &str) -> Sort {
        Sort::ALL.iter().find(|s| s.1 == k).map(|s| s.0).unwrap_or_default()
    }
}

/// A query with the players' words for a piston extender's reach (critique C12: "double piston
/// extender" found nothing): `double` / `triple` / `quadruple piston extender`, `dpe` / `tpe` /
/// `qpe` search the piston extenders of that reach.
pub fn with_synonyms(q: &Query) -> Query {
    let mut q = q.clone();
    let lower = q.text.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let reach = |w: &str| match w {
        "double" | "dpe" => Some(2),
        "triple" | "tpe" => Some(3),
        "quadruple" | "qpe" => Some(4),
        _ => None,
    };
    let extender = words.iter().any(|w| *w == "extender" || *w == "dpe" || *w == "tpe" || *w == "qpe");
    if let Some(n) = words.iter().find_map(|w| reach(w)).filter(|_| extender) {
        let rest: Vec<&str> = words.iter().copied().filter(|w| reach(w).is_none() && *w != "piston" && *w != "extender").collect();
        q.text = std::iter::once("piston extender").chain(rest).collect::<Vec<_>>().join(" ");
        if q.value.trim().is_empty() {
            q.value = format!("N {n}");
        }
    }
    q
}

/// What the person asked for. Facets: `groups` (OR), block kinds and flags as include (all
/// required) / exclude (none allowed) sets, numeric ranges, a free-text search, and the clock
/// time search, which also expands extendable families to their members.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    /// `door`, `clock`, `circuit`; empty: every kind.
    pub kind: String,
    pub text: String,
    /// The clock time search, as typed ([`parse_time_query`]).
    pub time: String,
    /// The circuit value search, as typed ([`parse_value_query`]): the study's axis value (`L 12`,
    /// `N 3`, `12`, `10-20 gt`) or the latency (`latency 0`); it expands the tunable groups to the
    /// member that hits it.
    pub value: String,
    pub groups: BTreeSet<String>,
    pub kinds_in: BTreeSet<String>,
    pub kinds_out: BTreeSet<String>,
    pub flags_in: BTreeSet<String>,
    pub flags_out: BTreeSet<String>,
    pub blocks: Range,
    pub volume: Range,
    pub period: Range,
    pub open: Range,
    pub close: Range,
    /// Show the variants (clocks folded under a smaller clock of their period).
    pub variants: bool,
    pub sort: Sort,
}

/// One result: a build, or a generated member of an extendable family (time search).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Hit {
    Build(usize),
    Member(usize),
}

/// Which facet to leave out when counting (the counts of a facet's values are computed with
/// every other filter applied, so picking one value never zeroes its siblings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Skip {
    None,
    Kind,
    Groups,
    Kinds,
    Flags,
}

/// Facet counts for the current query.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facets {
    pub kinds: BTreeMap<String, usize>,
    pub groups: BTreeMap<String, usize>,
    pub block_kinds: BTreeMap<String, usize>,
    pub flags: BTreeMap<String, usize>,
}

/// The fields a hit is filtered on (a member borrows its family's base build for the rest).
struct View<'a> {
    kind: &'a str,
    group: &'a str,
    /// A circuit's [`Circuit::order`] (0 for doors and clocks).
    order: u32,
    /// A circuit's [`Circuit::rank`].
    rank: u32,
    name: &'a str,
    blocks: i64,
    volume: Option<i64>,
    period: Option<i64>,
    /// The main number for sorting: a clock's period, a circuit's axis value.
    main: Option<i64>,
    open: Option<i64>,
    close: Option<i64>,
    kinds: &'a [String],
    flags: &'a [String],
    text: [&'a str; 5],
    /// A circuit's full kind (`gate xor`): an exact function name ranks it first.
    function: &'a str,
    variant: bool,
    /// A tunable group's member other than its shown design (hidden unless a value search hits
    /// it), and the circuit's latency in gt.
    member: bool,
    latency: Option<i64>,
    /// A circuit axis's letter (`L` for `L = 4 gt`, `N` for `N = 3`), else empty.
    axis_letter: &'a str,
    /// A clock that is hard to wire into a circuit (the judge's `usable`: `no: ...`): after the
    /// easy ones of its period (the coordinator 2026-10-21).
    hard: bool,
    /// A door's opening side (3 or 2; 0 for anything else): the 3x3 doors' card first.
    door_side: u32,
}

impl Site {
    /// Build index by id.
    pub fn index(&self) -> BTreeMap<&str, usize> {
        self.builds.iter().enumerate().map(|(i, b)| (b.id.as_str(), i)).collect()
    }

    /// The periods nearest to `[lo, hi]` that some clock or family member has (for an empty time
    /// search): up to `k` (period, how many), closest first, then shorter first.
    pub fn nearest_periods(&self, lo: u32, hi: u32, k: usize) -> Vec<(u32, usize)> {
        let mut count: BTreeMap<u32, usize> = BTreeMap::new();
        for b in &self.builds {
            if let Some(c) = &b.clock {
                if c.host.is_none() && !c.instance {
                    *count.entry(c.every()).or_default() += 1;
                }
            }
        }
        for m in &self.members {
            if m.build.is_none() && m.axis.is_empty() {
                *count.entry(m.period).or_default() += 1;
            }
        }
        let dist = |p: u32| if p < lo { lo - p } else { p.saturating_sub(hi) };
        let mut v: Vec<(u32, usize)> = count.into_iter().collect();
        v.sort_by_key(|(p, _)| (dist(*p), *p));
        v.truncate(k);
        v
    }

    fn base_of(&self, m: &Member, index: &BTreeMap<&str, usize>) -> Option<&Build> {
        if !m.base.is_empty() {
            return index.get(m.base.as_str()).map(|&i| &self.builds[i]);
        }
        let card = self.cards.iter().find(|c| c.id == m.card)?;
        let fam = card.families.iter().find(|f| f.id == m.family)?;
        index.get(fam.build.as_str()).map(|&i| &self.builds[i])
    }

    fn view<'a>(&'a self, h: Hit, index: &BTreeMap<&str, usize>) -> Option<View<'a>> {
        match h {
            Hit::Build(i) => {
                let b = self.builds.get(i)?;
                Some(View {
                    kind: &b.kind,
                    group: &b.group,
                    order: b.circuit.as_ref().map(|c| c.order).unwrap_or(0),
                    rank: b.circuit.as_ref().map(|c| c.rank).unwrap_or(0),
                    name: &b.name,
                    blocks: b.blocks as i64,
                    volume: b.volume.map(|v| v as i64),
                    period: b.clock.as_ref().map(|c| c.every() as i64),
                    main: b.clock.as_ref().map(|c| c.every() as i64).or_else(|| b.circuit.as_ref().and_then(|c| c.value)),
                    open: b.door.as_ref().and_then(|d| d.open).map(|v| v as i64),
                    close: b.door.as_ref().and_then(|d| d.close).map(|v| v as i64),
                    kinds: &b.kinds,
                    flags: &b.flags,
                    text: [&b.name, &b.caption, &b.group, &b.page, html::search_aliases(b)],
                    function: b.circuit.as_ref().map(|c| c.kind.as_str()).unwrap_or(""),
                    variant: b.clock.as_ref().is_some_and(|c| c.host.is_some() || c.instance) || b.circuit.as_ref().is_some_and(|c| c.host.is_some() || c.layout_of.is_some()) || b.door.as_ref().is_some_and(|d| d.host.is_some()) || b.flags.iter().any(|f| f == "census" || f == "twin"),
                    member: b.circuit.as_ref().is_some_and(|c| (!c.tunable.is_empty() && c.span.is_empty()) || c.card_member) || b.flags.iter().any(|f| f == "size"),
                    latency: b.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(k, _)| k == "latency")).and_then(|(_, v)| v.trim().trim_end_matches("gt").trim().parse().ok()),
                    axis_letter: b.circuit.as_ref().and_then(|c| c.axis.split_once(" = ").and_then(|(l, _)| l.split_whitespace().last()).or_else(|| c.metrics.iter().any(|(k, _)| k == "N").then_some("N"))).unwrap_or(""),
                    hard: b.clock.as_ref().is_some_and(|c| c.usable.starts_with("no")),
                    door_side: b.door.as_ref().map(Door::side).unwrap_or(0),
                })
            }
            Hit::Member(i) => {
                let m = self.members.get(i)?;
                let b = self.base_of(m, index)?;
                let circuit = !m.axis.is_empty();
                // a family whose value never changes (a flying machine growing its payload at one
                // speed) is no answer to a value search: its members differ by size only
                let constant = circuit && self.cards.iter().find(|c| c.id == m.card).and_then(|c| c.families.iter().find(|f| f.id == m.family)).is_some_and(|f| matches!(f.period, Formula::Linear { b: 0, .. }));
                Some(View {
                    kind: if circuit { "circuit" } else { "clock" },
                    group: &b.group,
                    order: b.circuit.as_ref().map(|c| c.order).unwrap_or(0),
                    rank: u32::MAX,
                    name: &m.family,
                    blocks: m.blocks as i64,
                    volume: m.volume.map(|v| v as i64),
                    period: (!circuit).then_some(m.period as i64),
                    main: (!constant).then_some(m.period as i64),
                    open: None,
                    close: None,
                    kinds: &b.kinds,
                    flags: &b.flags,
                    text: [&m.family, &b.caption, &b.name, "", html::search_aliases(b)],
                    function: "",
                    variant: false,
                    member: false,
                    latency: None,
                    axis_letter: m.axis.split_once(" = ").and_then(|(l, _)| l.split_whitespace().last()).unwrap_or(""),
                    // (a generated member: its base design's tap, the same mechanism)
                    hard: b.clock.as_ref().is_some_and(|c| c.usable.starts_with("no")),
                    door_side: 0,
                })
            }
        }
    }

    /// What each tab's cards hold (vjeux 21:47Z: one card per kind; critique: "Doors 1 · Clocks 1"
    /// read as one door and one clock): the door records, every clock (each a card under its
    /// period: vjeux 01:24Z), the circuit kinds (the shown circuit cards).
    pub fn holds(&self) -> [usize; 3] {
        let doors = self.builds.iter().filter(|b| b.door.is_some()).count();
        let clocks = self.builds.iter().filter(|b| b.clock.is_some()).count();
        let kinds = self.run(&Query::from_fragment("kind=circuit")).map(|(h, _)| h.len()).unwrap_or(0);
        [doors, clocks, kinds]
    }

    /// The distinct periods of the clocks (the Clocks tab's headings).
    pub fn clock_periods(&self) -> usize {
        self.builds.iter().filter_map(|b| b.clock.as_ref().map(|c| c.every())).collect::<BTreeSet<u32>>().len()
    }

    /// The period a hit's card stands under on the Clocks tab (a clock's), if any.
    pub fn hit_period(&self, h: Hit) -> Option<u32> {
        match h {
            Hit::Build(i) => self.builds.get(i).and_then(|b| b.clock.as_ref()).map(|c| c.every()),
            Hit::Member(i) => self.members.get(i).filter(|m| m.axis.is_empty()).map(|m| m.period),
        }
    }

    /// Run the query: the hits in display order and the facet counts. With a time search, the
    /// clocks of those periods plus every extendable family's members that hit them (an archived
    /// design that is exactly a member shows once, as the member); without one, the builds.
    pub fn run(&self, q: &Query) -> Result<(Vec<Hit>, Facets), String> {
        let q = &with_synonyms(q);
        let index = self.index();
        let time = if q.time.trim().is_empty() { None } else { Some(parse_time_query(&q.time)?) };
        let value = if q.value.trim().is_empty() { None } else { Some(parse_value_query(&q.value)?) };
        let words: Vec<String> = q.text.to_lowercase().split_whitespace().map(String::from).collect();
        let mut cands: Vec<Hit> = (0..self.builds.len()).map(Hit::Build).collect();
        if time.is_some() || value.is_some() {
            // members replace the archived designs they are (same family and n)
            let covered: BTreeSet<&str> = self.members.iter().filter_map(|m| m.build.as_deref()).collect();
            cands.retain(|h| match h {
                Hit::Build(i) => !covered.contains(self.builds[*i].id.as_str()),
                Hit::Member(_) => true,
            });
            cands.extend((0..self.members.len()).map(Hit::Member));
        }
        let pass = |v: &View, skip: Skip| -> bool {
            if skip != Skip::Kind && !q.kind.is_empty() && v.kind != q.kind {
                return false;
            }
            if let Some((lo, hi)) = time {
                match v.period {
                    Some(p) if p >= lo as i64 && p <= hi as i64 => {}
                    _ => return false,
                }
            }
            if let Some((field, lo, hi)) = value {
                if v.kind != "circuit" {
                    return false;
                }
                let x = match field {
                    ValueField::Axis => v.main,
                    ValueField::L => v.main.filter(|_| v.axis_letter == "L"),
                    ValueField::N => v.main.filter(|_| v.axis_letter == "N"),
                    ValueField::T => v.main.filter(|_| v.axis_letter == "T"),
                    ValueField::P => v.main.filter(|_| v.axis_letter == "P"),
                    ValueField::Latency => v.latency,
                };
                match x {
                    Some(p) if p >= lo as i64 && p <= hi as i64 => {}
                    _ => return false,
                }
            } else if !q.variants && v.member && time.is_none() {
                return false;
            }
            if !q.variants && v.variant {
                return false;
            }
            // a build listed under two kinds shows once, unless its own kind is picked
            if !q.variants && q.groups.is_empty() && v.flags.iter().any(|f| f == "kind-twin") {
                return false;
            }
            if skip != Skip::Groups && !q.groups.is_empty() && !q.groups.contains(v.group) {
                return false;
            }
            if skip != Skip::Kinds {
                if !q.kinds_in.iter().all(|k| v.kinds.iter().any(|x| x == k)) {
                    return false;
                }
                if v.kinds.iter().any(|x| q.kinds_out.contains(x)) {
                    return false;
                }
            }
            if skip != Skip::Flags {
                if !q.flags_in.iter().all(|f| v.flags.iter().any(|x| x == f)) {
                    return false;
                }
                if v.flags.iter().any(|x| q.flags_out.contains(x)) {
                    return false;
                }
            }
            if !(q.blocks.admits(Some(v.blocks)) && q.volume.admits(v.volume) && q.period.admits(v.period) && q.open.admits(v.open) && q.close.admits(v.close)) {
                return false;
            }
            if !words.is_empty() {
                let hay = v.text.iter().map(|t| t.to_lowercase()).collect::<Vec<_>>().join(" ");
                if !words.iter().all(|w| hay.contains(w.as_str()) || v.kinds.iter().any(|k| k.contains(w.as_str()))) {
                    return false;
                }
            }
            true
        };
        let views: Vec<(Hit, View)> = cands.iter().filter_map(|h| self.view(*h, &index).map(|v| (*h, v))).collect();
        let mut facets = Facets::default();
        for (_, v) in &views {
            if pass(v, Skip::Kind) {
                *facets.kinds.entry(v.kind.to_string()).or_default() += 1;
            }
            if pass(v, Skip::Groups) {
                *facets.groups.entry(v.group.to_string()).or_default() += 1;
            }
            if pass(v, Skip::Kinds) {
                for k in v.kinds {
                    *facets.block_kinds.entry(k.clone()).or_default() += 1;
                }
            }
            if pass(v, Skip::Flags) {
                for f in v.flags {
                    *facets.flags.entry(f.clone()).or_default() += 1;
                }
            }
        }
        let mut hits: Vec<(Hit, &View)> = views.iter().filter(|(_, v)| pass(v, Skip::None)).map(|(h, v)| (*h, v)).collect();
        let big = i64::MAX;
        // a text search ranks what the words name first (critique C12: "toggle" led with doors):
        // the kind's words and aliases, then the name, then the rest (captions)
        let relevance = |v: &View| -> u8 {
            if words.is_empty() {
                return 0;
            }
            // an exact gate function (`xor`, `xnor gate`) ranks its gates first (critique: "xor"
            // led with OR and AND, every gate's aliases name every function)
            let f = v.function.strip_prefix("gate ").unwrap_or("");
            if !f.is_empty() && words.iter().any(|w| w == f) && words.iter().all(|w| w == f || w == "gate") {
                return 0;
            }
            let kind = format!("{} {}", v.text[2], v.text[4]).to_lowercase();
            if words.iter().all(|w| kind.contains(w.as_str())) {
                1
            } else if words.iter().all(|w| v.text[0].to_lowercase().contains(w.as_str())) {
                2
            } else {
                3
            }
        };
        hits.sort_by(|(ha, a), (hb, b)| {
            let rel = relevance(a).cmp(&relevance(b));
            if rel != std::cmp::Ordering::Equal {
                return rel;
            }
            let primary = match q.sort {
                Sort::Main => {
                    // doors: the solid ones first (the community's main ranking; sculk doors can
                    // be set off by footsteps: critique, coordinator 13:54Z); clocks by period;
                    // circuits by kind then value; then, every list alike, blocks placed, then the
                    // box's volume, then speed (vjeux 13:11Z: "sort by blocks then volume")
                    let door_rank = |g: &str| match g {
                        "solid" => "0",
                        "sculk" => "2",
                        _ => "1",
                    };
                    let key = |v: &View| -> (u8, String, i64) {
                        match v.kind {
                            // (the 3x3 doors' card, then the 2x2's: each size its own records,
                            // the coordinator 2026-10-21)
                            "door" => (0, format!("{} {}", 9 - v.door_side.min(9), door_rank(v.group)), 0),
                            // (the clocks after the circuits on the all-builds list: every clock is
                            // a card now, 2812 of them, vjeux 01:24Z)
                            "clock" => (3, String::new(), v.main.or(v.period).unwrap_or(big)),
                            // circuits: the first of every kind, then the second of every kind...
                            // (critique C40), each round by study (pulse shapers first), the
                            // basics last
                            _ => (2, format!("{:010} {:05} {}", v.rank, v.order, v.group), v.main.unwrap_or(big)),
                        }
                    };
                    // speed: a door's open and close ticks, a circuit's latency (a clock's period
                    // is its key already); within a period, a clock that is hard to wire into a
                    // circuit after the easy ones (the coordinator 2026-10-21)
                    let speed = |v: &View| v.open.zip(v.close).map(|(o, c)| o + c).or(v.latency).unwrap_or(big);
                    key(a).cmp(&key(b)).then(a.hard.cmp(&b.hard)).then(a.blocks.cmp(&b.blocks)).then(a.volume.unwrap_or(big).cmp(&b.volume.unwrap_or(big))).then(speed(a).cmp(&speed(b)))
                }
                Sort::Blocks => a.blocks.cmp(&b.blocks).then(a.volume.unwrap_or(big).cmp(&b.volume.unwrap_or(big))),
                Sort::Volume => a.volume.unwrap_or(big).cmp(&b.volume.unwrap_or(big)).then(a.blocks.cmp(&b.blocks)),
                Sort::Period => a.main.unwrap_or(big).cmp(&b.main.unwrap_or(big)).then(a.hard.cmp(&b.hard)).then(a.blocks.cmp(&b.blocks)).then(a.volume.unwrap_or(big).cmp(&b.volume.unwrap_or(big))),
                Sort::Open => a.open.unwrap_or(big).cmp(&b.open.unwrap_or(big)).then(a.close.unwrap_or(big).cmp(&b.close.unwrap_or(big))),
                Sort::Close => a.close.unwrap_or(big).cmp(&b.close.unwrap_or(big)).then(a.open.unwrap_or(big).cmp(&b.open.unwrap_or(big))),
            };
            primary.then(a.group.cmp(b.group)).then(a.name.cmp(b.name)).then(ha.cmp(hb))
        });
        Ok((hits.into_iter().map(|(h, _)| h).collect(), facets))
    }

    /// How many of `hits` (in [`Site::run`]'s order for the same query) name what the query's
    /// words name, the rest only mentioning them (a caption, a name): 0 when nothing names it or
    /// everything does (critique: "toggle" showed its 4 toggle switches, then doors and clocks
    /// that only mention the word, with nothing telling them apart).
    pub fn named_count(&self, q: &Query, hits: &[Hit]) -> usize {
        let q = with_synonyms(q);
        let words: Vec<String> = q.text.to_lowercase().split_whitespace().map(String::from).collect();
        if words.is_empty() {
            return 0;
        }
        let index: BTreeMap<&str, usize> = self.builds.iter().enumerate().map(|(i, b)| (b.id.as_str(), i)).collect();
        let named = |h: &Hit| -> bool {
            let Some(v) = self.view(*h, &index) else { return false };
            let f = v.function.strip_prefix("gate ").unwrap_or("");
            if !f.is_empty() && words.iter().any(|w| w == f) && words.iter().all(|w| w == f || w == "gate") {
                return true;
            }
            let kind = format!("{} {}", v.text[2], v.text[4]).to_lowercase();
            words.iter().all(|w| kind.contains(w.as_str()))
        };
        let n = hits.iter().take_while(|h| named(h)).count();
        if n == hits.len() { 0 } else { n }
    }
}

/// The query as a URL fragment (`kind=clock&time=2-3%20s&sort=blocks`), and back: shareable
/// links to a search.
impl Query {
    pub fn to_fragment(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut push = |k: &str, v: &str| {
            if !v.is_empty() {
                parts.push(format!("{k}={}", encode(v)));
            }
        };
        push("kind", &self.kind);
        push("q", &self.text);
        push("time", &self.time);
        push("value", &self.value);
        let join = |s: &BTreeSet<String>| s.iter().cloned().collect::<Vec<_>>().join(",");
        push("group", &join(&self.groups));
        push("with", &join(&self.kinds_in));
        push("without", &join(&self.kinds_out));
        push("is", &join(&self.flags_in));
        push("not", &join(&self.flags_out));
        for (k, r) in [("blocks", self.blocks), ("volume", self.volume), ("period", self.period), ("open", self.open), ("close", self.close)] {
            if !r.is_open() {
                push(k, &format!("{}-{}", r.min.map(|v| v.to_string()).unwrap_or_default(), r.max.map(|v| v.to_string()).unwrap_or_default()));
            }
        }
        if self.variants {
            push("variants", "1");
        }
        if self.sort != Sort::Main {
            push("sort", self.sort.key());
        }
        parts.join("&")
    }

    pub fn from_fragment(s: &str) -> Query {
        let mut q = Query::default();
        let set = |v: &str| -> BTreeSet<String> { v.split(',').filter(|x| !x.is_empty()).map(String::from).collect() };
        let range = |v: &str| -> Range {
            let (a, b) = v.split_once('-').unwrap_or((v, v));
            Range { min: a.trim().parse().ok(), max: b.trim().parse().ok() }
        };
        for part in s.trim_start_matches('#').split('&') {
            let Some((k, v)) = part.split_once('=') else { continue };
            let v = decode(v);
            match k {
                "kind" => q.kind = v,
                "q" => q.text = v,
                "time" => q.time = v,
                "value" => q.value = v,
                "group" => q.groups = set(&v),
                "with" => q.kinds_in = set(&v),
                "without" => q.kinds_out = set(&v),
                "is" => q.flags_in = set(&v),
                "not" => q.flags_out = set(&v),
                "blocks" => q.blocks = range(&v),
                "volume" => q.volume = range(&v),
                "period" => q.period = range(&v),
                "open" => q.open = range(&v),
                "close" => q.close = range(&v),
                "variants" => q.variants = v == "1",
                "sort" => q.sort = Sort::from_key(&v),
                _ => {}
            }
        }
        q
    }
}

/// Percent-encoding for a URL fragment value (unreserved characters and `,` kept).
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' | b'+' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let hex = |b: u8| -> Option<u8> { (b as char).to_digit(16).map(|d| d as u8) };
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_queries() {
        assert_eq!(parse_time_query("48"), Ok((48, 48)));
        assert_eq!(parse_time_query("48 gt"), Ok((48, 48)));
        assert_eq!(parse_time_query("48gt"), Ok((48, 48)));
        assert_eq!(parse_time_query("2.5 s"), Ok((50, 50)));
        assert_eq!(parse_time_query("2.5s"), Ok((50, 50)));
        assert_eq!(parse_time_query("1 min"), Ok((1200, 1200)));
        assert_eq!(parse_time_query("1 min 30 s"), Ok((1800, 1800)));
        assert_eq!(parse_time_query("1:30"), Ok((1800, 1800)));
        assert_eq!(parse_time_query("2 h"), Ok((144000, 144000)));
        assert_eq!(parse_time_query("40-60 gt"), Ok((40, 60)));
        assert_eq!(parse_time_query("40..60"), Ok((40, 60)));
        assert_eq!(parse_time_query("2-3 s"), Ok((40, 60)));
        assert_eq!(parse_time_query("2 s to 3 s"), Ok((40, 60)));
        assert_eq!(parse_time_query("1-2 min"), Ok((1200, 2400)));
        assert_eq!(parse_time_query("60 - 40"), Ok((40, 60)));
        assert_eq!(parse_time_query("< 60 gt"), Ok((0, 59)));
        assert_eq!(parse_time_query("<= 3 s"), Ok((0, 60)));
        assert_eq!(parse_time_query(">= 1 min"), Ok((1200, u32::MAX)));
        assert_eq!(parse_time_query("> 10"), Ok((11, u32::MAX)));
        assert_eq!(parse_time_query("2 rt"), Ok((4, 4)));
        assert_eq!(parse_time_query("0.4 S"), Ok((8, 8)));
        // between two ticks: no period matches, said plainly
        assert!(parse_time_query("0.33 s").is_err());
        assert!(parse_time_query("abc").is_err());
        assert!(parse_time_query("5 parsecs").is_err());
        assert_eq!(parse_time_query("0.3-0.33 s"), Ok((6, 6)));
    }

    #[test]
    fn real_times() {
        assert_eq!(real_time(20), "1 s");
        assert_eq!(real_time(30), "1.5 s");
        assert_eq!(real_time(2304), "1 min 55.2 s");
        assert_eq!(real_time(1200), "1 min");
        assert_eq!(real_time(72000 + 6000), "1 h 5 min");
        assert_eq!(gt_time(8), "8 ticks (0.4 s)");
    }

    #[test]
    fn formulas_solve() {
        let f = Formula::Linear { a: 10, b: 4 };
        assert_eq!(f.solve(0, 47, 48, 48), Vec::<i64>::new());
        assert_eq!(f.solve(0, 47, 50, 50), vec![10]);
        assert_eq!(f.solve(0, 47, 40, 60), vec![8, 9, 10, 11, 12]);
        let g = Formula::Geometric { a: 12, k: 2, c: 0 };
        assert_eq!(g.solve(0, 8, 40, 100), vec![2, 3]);
        assert_eq!(g.eval(8), 3072);
    }

    fn clock(id: &str, period: u32, blocks: u32, group: &str, kinds: &[&str], flags: &[&str]) -> Build {
        Build {
            id: format!("clocks/{id}"),
            kind: "clock".into(),
            group: group.into(),
            name: id.into(),
            page: format!("clocks/{id}.html"),
            design: format!("designs/clocks/{id}.txt"),
            blocks,
            volume: Some(blocks),
            kinds: kinds.iter().map(|s| s.to_string()).collect(),
            flags: flags.iter().map(|s| s.to_string()).collect(),
            clock: Some(ClockInfo { period, ..Default::default() }),
            ..Default::default()
        }
    }

    fn circuit(id: &str, kind: &str, value: i64, blocks: u32, tunable: &str, span: &str, latency: Option<&str>) -> Build {
        Build {
            id: format!("circuits/pulse/{id}"),
            kind: "circuit".into(),
            group: kind.into(),
            name: id.into(),
            page: format!("circuits/pulse/{id}.html"),
            blocks,
            circuit: Some(Circuit {
                study: "pulse".into(),
                kind: kind.into(),
                axis: format!("L = {value} gt"),
                value: Some(value),
                tunable: tunable.into(),
                span: span.into(),
                metrics: latency.map(|l| vec![("latency".to_string(), l.to_string())]).unwrap_or_default(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn tunable_groups_fold_and_the_value_search_expands_them() {
        let mut s = Site { version: DATA_VERSION, ..Default::default() };
        s.builds = vec![
            circuit("rise-L6", "rising-edge", 6, 17, "oneshot", "L 6-40 gt, 3 values", None),
            circuit("rise-L12", "rising-edge", 12, 17, "oneshot", "", None),
            circuit("rise-L40", "rising-edge", 40, 25, "oneshot", "", None),
            circuit("obs-L2", "rising-edge", 2, 1, "", "", Some("2 gt")),
        ];
        let mut layout = circuit("obs-L2-b", "rising-edge", 2, 1, "", "", Some("2 gt"));
        layout.circuit.as_mut().unwrap().layout_of = Some("circuits/pulse/obs-L2".into());
        s.builds.push(layout);
        fn names_of(s: &Site, q: &Query) -> Vec<String> {
            s.run(q).unwrap().0.iter().map(|h| match h {
                Hit::Build(i) => s.builds[*i].name.clone(),
                Hit::Member(_) => String::new(),
            }).collect()
        }
        let names = |q: &Query| names_of(&s, q);
        // default: the group's shown design only, no layout
        assert_eq!(names(&Query { kind: "circuit".into(), ..Default::default() }), vec!["obs-L2", "rise-L6"]);
        // a value search expands the group to the member that hits it
        assert_eq!(names(&Query { kind: "circuit".into(), value: "L 12".into(), ..Default::default() }), vec!["rise-L12"]);
        assert_eq!(names(&Query { value: "10-40".into(), ..Default::default() }), vec!["rise-L12", "rise-L40"]);
        assert_eq!(names(&Query { value: "latency 2".into(), ..Default::default() }), vec!["obs-L2"]);
        // variants on: every design
        assert_eq!(names(&Query { kind: "circuit".into(), variants: true, ..Default::default() }).len(), 5);
        assert_eq!(parse_value_query("N=3").unwrap(), (ValueField::N, 3, 3));
        assert_eq!(parse_value_query("12").unwrap(), (ValueField::Axis, 12, 12));
        assert_eq!(parse_value_query("lat 0").unwrap(), (ValueField::Latency, 0, 0));
        assert_eq!(parse_value_query("T 10").unwrap(), (ValueField::T, 10, 10));
        assert_eq!(parse_value_query("p=8").unwrap(), (ValueField::P, 8, 8));
        assert!(parse_value_query("pulse").is_err());
        // `N 12` asks for an N axis: the L = 12 member does not answer it
        assert!(names(&Query { kind: "circuit".into(), value: "N 12".into(), ..Default::default() }).is_empty());
        assert!(parse_value_query("lever").is_err());
        let q = Query { kind: "circuit".into(), value: "L 12".into(), ..Default::default() };
        assert_eq!(Query::from_fragment(&q.to_fragment()), q);
        // an extendable circuit family's generated members answer the value search; a family
        // card's second base design stays out of the default grid
        s.cards.push(Card { id: "rep-line".into(), kind: "circuit".into(), families: vec![Family {
            id: "rep2".into(),
            build: "circuits/pulse/obs-L2".into(),
            period: Formula::Linear { a: 2, b: 8 },
            period_text: "2 + 8n".into(),
            blocks: Formula::Linear { a: 1, b: 2 },
            blocks_text: "1 + 2n".into(),
            rule: String::new(),
            n_lo: 0,
            n_hi: 4,
            base: 0,
            judged: vec![0],
            extra: vec![],
        }], ..Default::default() });
        for n in 0..=4i64 {
            s.members.push(Member {
                run: None,
                card: "rep-line".into(),
                family: "rep2".into(),
                n,
                period: (2 + 8 * n) as u32,
                blocks: (1 + 2 * n) as u32,
                design: String::new(),
                dims: None,
                volume: None,
                status: STATUS_REDSIM.into(),
                build: (n == 0).then(|| "circuits/pulse/obs-L2".to_string()),
                picture: None,
                axis: format!("N = {} gt", 2 + 8 * n),
                base: String::new(),
            });
        }
        let (hits, _) = s.run(&Query { kind: "circuit".into(), value: "N 26".into(), ..Default::default() }).unwrap();
        assert_eq!(hits, vec![Hit::Member(3)]);
        // a clock time search never returns a circuit member
        assert!(s.run(&Query { time: "26".into(), ..Default::default() }).unwrap().0.is_empty());
        let mut second = circuit("obs-L4", "rising-edge", 4, 1, "", "", None);
        second.circuit.as_mut().unwrap().card_member = true;
        s.builds.push(second);
        assert!(!names_of(&s, &Query { kind: "circuit".into(), ..Default::default() }).contains(&"obs-L4".to_string()));
    }

    fn site() -> Site {
        let mut ring = clock("obs-ring", 8, 4, "observer", &["observer"], &["robust", "extendable"]);
        ring.clock.as_mut().unwrap().card = Some("obs-ring".into());
        let mut variant = clock("obs-note", 6, 3, "observer+note_block", &["note_block", "observer"], &["variant"]);
        variant.clock.as_mut().unwrap().host = Some("clocks/obs2".into());
        let door = Build {
            id: "doors/fc2-40b".into(),
            kind: "door".into(),
            group: "solid".into(),
            name: "fc2-40b".into(),
            blocks: 40,
            volume: Some(144),
            kinds: vec!["observer".into(), "slime_block".into(), "sticky_piston".into()],
            flags: vec!["rotation-safe".into(), "wall".into()],
            door: Some(Door { open: Some(14), close: Some(16), ..Default::default() }),
            ..Default::default()
        };
        let members = (0..=10)
            .map(|n| Member {
                run: None,
                card: "obs-ring".into(),
                family: "obs-ring".into(),
                n,
                period: 8 + 4 * n as u32,
                blocks: 4 + 2 * n as u32,
                design: format!("designs/clocks/members/obs-ring-n{n}.txt"),
                dims: None,
                volume: Some(4 + 2 * n as u32),
                status: if n < 3 { STATUS_JUDGED.into() } else { STATUS_FORMULA.into() },
                build: (n == 0).then(|| "clocks/obs-ring".to_string()),
                picture: None,
                axis: String::new(),
                base: String::new(),
            })
            .collect();
        Site {
            version: DATA_VERSION,
            builds: vec![
                clock("obs2", 6, 2, "observer", &["observer"], &["robust"]),
                ring,
                clock("lamp48", 48, 9, "observer+redstone_lamp", &["observer", "redstone_lamp"], &[]),
                variant,
                door,
            ],
            cards: vec![Card {
                id: "obs-ring".into(),
                families: vec![Family {
                    id: "obs-ring".into(),
                    build: "clocks/obs-ring".into(),
                    period: Formula::Linear { a: 8, b: 4 },
                    period_text: "8 + 4n".into(),
                    blocks: Formula::Linear { a: 4, b: 2 },
                    blocks_text: "4 + 2n".into(),
                    rule: "insert 2 observers".into(),
                    n_lo: 0,
                    n_hi: 10,
                    base: 0,
                    judged: vec![0, 1, 2],
                    extra: vec![],
                }],
                ..Default::default()
            }],
            members,
            ..Default::default()
        }
    }

    #[test]
    fn query_kinds_flags_ranges() {
        let s = site();
        let ids = |q: &Query| -> Vec<String> {
            s.run(q).unwrap().0.iter().map(|h| match h {
                Hit::Build(i) => s.builds[*i].id.clone(),
                Hit::Member(i) => format!("{}#n{}", s.members[*i].family, s.members[*i].n),
            }).collect()
        };
        // default: builds, variants hidden, by the main number (doors by size, clocks by period)
        assert_eq!(ids(&Query::default()), vec!["doors/fc2-40b", "clocks/obs2", "clocks/obs-ring", "clocks/lamp48"]);
        // fewest blocks first
        assert_eq!(ids(&Query { sort: Sort::Blocks, ..Default::default() }), vec!["clocks/obs2", "clocks/obs-ring", "clocks/lamp48", "doors/fc2-40b"]);
        let q = Query { kind: "door".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["doors/fc2-40b"]);
        let q = Query { kinds_out: ["redstone_lamp".to_string()].into(), kind: "clock".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/obs2", "clocks/obs-ring"]);
        let q = Query { kinds_in: ["redstone_lamp".to_string()].into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/lamp48"]);
        let q = Query { flags_in: ["robust".to_string()].into(), flags_out: ["extendable".to_string()].into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/obs2"]);
        let q = Query { variants: true, kind: "clock".into(), period: Range { min: Some(6), max: Some(6) }, ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/obs2", "clocks/obs-note"]);
        let q = Query { open: Range { min: None, max: Some(14) }, ..Default::default() };
        assert_eq!(ids(&q), vec!["doors/fc2-40b"]);
        let q = Query { text: "LAMP".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/lamp48"]);
        // facet counts ignore their own facet
        let q = Query { kind: "door".into(), ..Default::default() };
        let (_, f) = s.run(&q).unwrap();
        assert_eq!(f.kinds.get("clock"), Some(&3));
        assert_eq!(f.kinds.get("door"), Some(&1));
        assert_eq!(f.block_kinds.get("slime_block"), Some(&1));
    }

    #[test]
    fn players_words_for_extenders() {
        let q = Query { text: "Double Piston Extender".into(), ..Default::default() };
        let s = with_synonyms(&q);
        assert_eq!((s.text.as_str(), s.value.as_str()), ("piston extender", "N 2"));
        let s = with_synonyms(&Query { text: "tpe flat".into(), ..Default::default() });
        assert_eq!((s.text.as_str(), s.value.as_str()), ("piston extender flat", "N 3"));
        // "double" alone is not an extender search
        assert_eq!(with_synonyms(&Query { text: "double".into(), ..Default::default() }).value, "");
    }

    #[test]
    fn time_search_expands_families() {
        let s = site();
        let ids = |q: &Query| -> Vec<String> {
            s.run(q).unwrap().0.iter().map(|h| match h {
                Hit::Build(i) => s.builds[*i].id.clone(),
                Hit::Member(i) => format!("{}#n{}", s.members[*i].family, s.members[*i].n),
            }).collect()
        };
        // 48 gt: the archived lamp clock (9 blocks) and the ring's n = 10 member (24 blocks)
        let q = Query { time: "48 gt".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/lamp48", "obs-ring#n10"]);
        let q = Query { time: "2.4 s".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/lamp48", "obs-ring#n10"]);
        // the base design (n = 0) shows once, as the member
        let q = Query { time: "8".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["obs-ring#n0"]);
        // 6..12 gt: the variant (6 gt) stays hidden
        let q = Query { time: "0.3-0.6 s".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/obs2", "obs-ring#n0", "obs-ring#n1"]);
        // nothing at 7 gt: the nearest periods that exist
        assert_eq!(s.nearest_periods(7, 7, 3), vec![(6, 1), (8, 1), (12, 1)]);
        assert!(s.run(&Query { time: "0.33 s".into(), ..Default::default() }).is_err());
        // a clock that is hard to wire into a circuit comes after the easy ones of its period,
        // however small (the coordinator 2026-10-21: the judge's `usable`)
        let mut hard = site();
        let lamp = hard.builds.iter().position(|b| b.id == "clocks/lamp48").unwrap();
        if let Some(c) = hard.builds[lamp].clock.as_mut() {
            c.usable = "no: the output is under the build".into();
        }
        let q = Query { time: "48 gt".into(), ..Default::default() };
        let order: Vec<String> = hard.run(&q).unwrap().0.iter().map(|h| match h {
            Hit::Build(i) => hard.builds[*i].id.clone(),
            Hit::Member(i) => format!("{}#n{}", hard.members[*i].family, hard.members[*i].n),
        }).collect();
        assert_eq!(order, vec!["obs-ring#n10", "clocks/lamp48"]);
    }

    #[test]
    fn fragments_round_trip() {
        let q = Query {
            kind: "clock".into(),
            text: "lamp ring".into(),
            time: "2-3 s".into(),
            kinds_in: ["observer".to_string()].into(),
            flags_out: ["variant".to_string(), "chiral".to_string()].into(),
            blocks: Range { min: Some(2), max: None },
            sort: Sort::Period,
            variants: true,
            ..Default::default()
        };
        let f = q.to_fragment();
        assert_eq!(Query::from_fragment(&f), q, "{f}");
        assert_eq!(Query::from_fragment(""), Query::default());
        assert_eq!(decode(&encode("a b/ç&=")), "a b/ç&=");
    }
}
