//! Phase 8 slice 1: a drainage-first world prototype, with no production toggle.
//!
//! `docs/HEXFACTORY-PLAN.md#phase-8--flowing-water` refuses to put a flowing-water front on the
//! shipped ridge-noise rivers, and refuses to let a flow simulation grow the generated world. This
//! module exists to falsify the replacement cheaply, before the save boundary makes it expensive.
//! Slice 3 selects this generator for new worlds. The survey harness stays native-only.
//!
//! # What is being tested
//!
//! The brief's ordering is *drainage first, terrain fitted to it*. This prototype does that, and
//! then re-derives drainage from the fitted terrain, which is the claim worth falsifying: a
//! landscape carved around a channel network should hand that same network back when you ask it
//! which way water runs.
//!
//! # Why the invariants hold by construction rather than by luck
//!
//! Three separate structures each carry one invariant, so no part of the pipeline has to be
//! trusted to behave:
//!
//! 1. **The province graph cannot cycle.** A province drains to the neighbour that is strictly
//!    lower in the total order `(rank, pq, pr)`. Every macro edge strictly decreases a total
//!    order, so the macro graph is a forest rooted at ocean provinces and closed basins.
//! 2. **The cell graph cannot cycle and cannot run uphill.** A cell's downstream neighbour is the
//!    minimum of the same shape of total order, `(head, q, r)`, and only when that key is strictly
//!    smaller than the cell's own. There is no fill step between the height field and the flow
//!    field, so "non-increasing head" is the definition of a flow edge rather than a property
//!    someone has to preserve.
//! 3. **Query order cannot matter.** `head` is a pure function of seed and coordinate. It reads
//!    channels from the nine provinces around a cell and nothing else, and a valley is never wider
//!    than [`VALLEY_RADIUS`], so the same cell computed from either side of a province seam is
//!    computed from the same inputs. Caching is therefore an optimisation with nothing to get
//!    wrong, which is what [`Terra::head_uncached`] checks against the cache.
//!
//! What is left over — cells with no lower neighbour at all — are real pits, and they become
//! lakes with a measured spill level or, past a budget, a declared frontier basin. Counting them
//! is the point: a model that produces a landscape of ponds has been falsified, and the survey is
//! how that shows up as a number instead of an opinion.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};
use std::rc::Rc;

use crate::scale::{BED_MAX_QUANTA, BED_MIN_QUANTA, SEA_LEVEL_QUANTA};
use crate::{axial_distance, coordinate_hash, floor_div, hexes_in_radius, value_noise, DIRECTIONS};

/// The noise ceiling `value_noise` interpolates against. Mirrors the private constant in `lib.rs`;
/// re-stated rather than exported because a prototype should not widen the production surface.
pub(crate) const NOISE_MAX: i32 = 65_535;

/// Cells along one side of a drainage province: about 687 m at the Phase 8 scale.
///
/// The province is the unit of bounded work, not a unit of content. Nothing a player can see
/// aligns to it — the macro height field is continuous across a seam, and the channel that crosses
/// one is drawn from both sides to a pour point both sides derive independently.
pub const PROVINCE_CELL: i32 = 128;

/// Wavelength of the continental term, in provinces: about 33 km.
///
/// Deliberately far past the shipped `MAX_FEATURE_CELL = 1024`, because the brief says a regional
/// structure may exceed it. The pure unbounded query contract is what survives, not the ceiling.
///
/// The wavelength is what decides whether the world is buildable. 2,400 m of relief over 8 km is a
/// mountain front; over 33 km it is ordinary country with mountains in it, and the difference shows
/// up directly in [`TerraSurvey::walkable_per_mille`].
const CONTINENT_PROVINCES: i32 = 48;

/// Wavelength of the massif term, in provinces: about 8.3 km.
const MASSIF_PROVINCES: i32 = 12;

/// Wavelength of the hillslope term, in cells: about 515 m. This is what puts a shoulder between
/// two valleys rather than a smooth ramp.
///
/// Raising this term's amplitude, or adding a shorter meso-scale one beneath it, was measured
/// against [`TerraSurvey::viewport_relief_median`] and rejected: the field already carries about
/// 53 m of relief inside one viewport, so the extra amplitude bought no visible landform and spent
/// 64 per mille of the world's buildable ground. What reads as flat is the material map, not the
/// height field — see [`crate::ground_spine`].
const HILLSLOPE_CELL: i32 = 96;
/// Amplitude of the hillslope term, in height quanta: 10 m.
const HILLSLOPE_QUANTA: i32 = 40;

/// Wavelength of the fine surface term, in cells — about 64 m — and its amplitude in height
/// quanta, 0.75 m. Small on purpose: fine noise is what manufactures pits, and a landscape of
/// puddles is a landscape nobody can route a belt across.
const RELIEF_CELL: i32 = 12;
const RELIEF_QUANTA: i32 = 3;

/// Spacing of the channel-network nodes inside a province, in cells: about 43 m.
pub const SPINE_CELL: i32 = 8;

/// The widest valley any discharge class carves, in cells. Also the halo the incision solve needs
/// on every side, so that a cell's height is complete before anyone reads it.
pub const VALLEY_RADIUS: i32 = 24;

/// How far outside its own block a province computes, in cells. One cell more than
/// [`VALLEY_RADIUS`], so the ring the block's own flow directions read is itself exact.
const HALO: i32 = VALLEY_RADIUS + 1;

/// Side of the array a province solves over.
const DOMAIN_SIDE: i32 = PROVINCE_CELL + 2 * HALO;

/// The largest closed basin a province will resolve into a lake. Past this the basin is reported
/// as a frontier basin — unresolved, counted, and never quietly filled.
pub const LAKE_CELL_BUDGET: usize = 4_096;

/// How far up the province tree a catchment estimate walks before it saturates. Discharge classes
/// are logarithmic, so a saturated estimate names the top class rather than a wrong number.
pub const UPSTREAM_PROVINCE_BUDGET: usize = 96;

/// Moisture above which a channel head at sea level is a spring, on the `0..=NOISE_MAX` noise
/// scale.
const SPRING_MOISTURE: i32 = 38_000;
/// Wavelength of the moisture channel, in cells.
const MOISTURE_CELL: i32 = 96;
/// How much easier a spring is per height quantum, and the most that easing may buy.
///
/// Orographic lift is the reason headwaters are in the mountains rather than spread evenly over
/// the moisture field, and a river that starts high is the one that has a fall worth using.
const SPRING_ALTITUDE_EASE: i32 = 4;
const SPRING_ALTITUDE_CAP: i32 = 24_000;

/// Moisture a channel head at this height must carry to be a spring.
fn spring_threshold(head_mq: i32) -> i32 {
    let above_sea = (head_mq / MQ as i32 - SEA_LEVEL_QUANTA).max(0);
    SPRING_MOISTURE - (above_sea.saturating_mul(SPRING_ALTITUDE_EASE)).min(SPRING_ALTITUDE_CAP)
}

/// Bounded search used to choose a new world's valley shelf. The coastal search is centred on a
/// province that has both land and sea nearby, so a new game begins on the low plain rather than
/// at an arbitrary inland coordinate. Province ranks are `O(1)`; only eight low dry candidates are
/// sampled in full, plus the bounded neighbouring provinces touched by exact beach checks.
const LANDING_PROVINCE_RADIUS: i32 = 32;
const LANDING_COAST_RADIUS: i32 = 6;
const LANDING_PROVINCE_BUDGET: usize = 8;
#[cfg(test)]
const LANDING_PROVINCE_SOLVE_BUDGET: usize = LANDING_PROVINCE_BUDGET * 4;
const LANDING_SAMPLE_STRIDE: i32 = 8;
/// Where the opening stands relative to the sea, in cells: a beach between 22 and 28 cells out,
/// about 115 m to 150 m.
///
/// The ceiling is the first pump's opening walk — a beach further than that is a beach the opening
/// cannot use. The floor is what makes the shore a place rather than the ground underfoot: nothing
/// in the old rule stopped a shelf three cells from the surf, and an opening on the sand has no
/// coastal plain around it to hold the reaches that run to the ocean, which are the widest water
/// in the world and the reason to open here at all. A site inside the floor is still taken when
/// the search finds nothing standing further back, because a workable shelf near the water beats
/// an unworkable one at the right distance.
const LANDING_BEACH_MIN: i32 = 22;
const LANDING_BEACH_RADIUS: i32 = 28;
/// Coastal plain ceiling for the landing itself: 100 m above the fixed sea datum.
const LANDING_ALTITUDE_CEILING: i32 = 400;
/// The inner pad must carry the largest initial footprint; the outer clearing must be dry and
/// traversable so opening material searches do not begin behind a river or retaining face.
pub const LANDING_PAD_RADIUS: i32 = 1;
pub const LANDING_CLEAR_RADIUS: i32 = 7;

/// Octave salts. Distinct from every channel `lib.rs` already samples, so the prototype cannot
/// accidentally correlate with the shipped generator.
const OCT_CONTINENT: u32 = 0x7E44A_1;
const OCT_MASSIF: u32 = 0x7E44A_2;
const OCT_HILLSLOPE: u32 = 0x7E44A_3;
const OCT_RELIEF: u32 = 0x7E44A_4;
const OCT_MOISTURE: u32 = 0x7E44A_5;
const OCT_SPINE: u32 = 0x7E44A_6;

/// The four province neighbours that share a whole face.
///
/// Provinces are axial-rectangular blocks, so only `±q` and `±r` put a full column or row of cells
/// against a neighbour. The other two hex directions share exactly one corner cell, which is not
/// somewhere a river can be routed through, so they are not drainage adjacencies.
const PROVINCE_FACES: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Where a province's water goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outlet {
    /// The province stands below sea level: it is coast, and the ocean is the root.
    Ocean,
    /// A local minimum of the province lattice above sea level. A closed basin, kept as a lake
    /// rather than filled away.
    Basin,
    /// Drains into a strictly lower neighbouring province.
    Province { pq: i32, pr: i32 },
}

/// Where one cell's water goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Into the neighbour at `DIRECTIONS[index]`.
    To(u8),
    /// The cell is under a lake surface; the lake's spill point is its outlet.
    Lake(u32),
    /// No lower neighbour and no resolved lake: a frontier basin the province could not close
    /// inside [`LAKE_CELL_BUDGET`].
    Frontier,
}

/// A resolved closed basin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LakeInfo {
    /// Surface height, in milli-quanta: the lowest rim the basin spills over.
    pub spill_mq: i32,
    /// The rim cell the basin spills at.
    pub spill: (i32, i32),
    pub cells: u32,
}

impl LakeInfo {
    /// The lake's surface, in published height quanta.
    pub fn spill_quanta(&self) -> i32 {
        quanta(self.spill_mq)
    }
}

/// What is standing on a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Water {
    Dry,
    /// Below sea level.
    Sea {
        depth: i32,
    },
    Lake {
        depth: i32,
    },
    River {
        depth: i32,
        class: u8,
    },
}

impl Water {
    pub fn depth(self) -> i32 {
        match self {
            Water::Dry => 0,
            Water::Sea { depth } | Water::Lake { depth } | Water::River { depth, .. } => depth,
        }
    }

    pub fn is_wet(self) -> bool {
        !matches!(self, Water::Dry)
    }

    /// Rated discharge carried by this initial-water cell. Seas and lakes are settled boundary
    /// conditions rather than river reaches, so only a river publishes a non-zero class.
    pub fn discharge_class(self) -> u8 {
        match self {
            Water::River { class, .. } => class,
            Water::Dry | Water::Sea { .. } | Water::Lake { .. } => 0,
        }
    }
}

/// A deterministic valley shelf used as the physical world's local origin.
///
/// The generated drainage remains in its own unbounded coordinate system. New-world activation
/// translates that system so the player starts on a naturally dry, walkable shelf instead of
/// flattening whatever the seed happened to put at `(0, 0)`. Translation preserves every seam,
/// outlet and flow invariant because every physical query applies the same offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LandingSite {
    pub q: i32,
    pub r: i32,
    pub bed_quanta: i32,
}

/// The province a cell belongs to.
pub fn province_of(q: i32, r: i32) -> (i32, i32) {
    (floor_div(q, PROVINCE_CELL), floor_div(r, PROVINCE_CELL))
}

pub(crate) fn province_origin(pq: i32, pr: i32) -> (i32, i32) {
    (pq * PROVINCE_CELL, pr * PROVINCE_CELL)
}

/// The continental height field, in **milli-quanta**, before any valley is cut into it.
///
/// Everything inside this module carries height in thousandths of a quantum, and publishes whole
/// quanta. The reason is drainage, not precision for its own sake: a 0.25 m integer field over
/// gentle country is *exactly* flat across a fifth of its neighbour pairs, and a flat has no
/// steepest descent, so every flat patch would collapse into a pit and the world would drain into
/// tens of thousands of one-cell puddles. Carrying the field at the resolution it is actually
/// computed at means a flow direction follows the real surface rather than the rounded one.
///
/// The squared curve is what keeps the mass of the world low: an uncurved blend of value noise
/// puts the median halfway up the range, which would make an 800 m plateau the ordinary case and
/// leave no coastline worth walking to. Squaring the normalised sample moves the median to about
/// 200 m and leaves summits genuinely rare, which is a claim the survey checks rather than states.
fn continental_mq(seed: u32, q: i32, r: i32) -> i32 {
    let continent = value_noise(
        seed,
        q,
        r,
        PROVINCE_CELL * CONTINENT_PROVINCES,
        OCT_CONTINENT,
    );
    let massif = value_noise(seed, q, r, PROVINCE_CELL * MASSIF_PROVINCES, OCT_MASSIF);
    let blended = (continent * 68 + massif * 32) / 100;
    let curved = i64::from(blended) * i64::from(blended) / i64::from(NOISE_MAX);
    let span = i64::from(BED_MAX_QUANTA - BED_MIN_QUANTA) * MQ;
    (i64::from(BED_MIN_QUANTA) * MQ + curved * span / i64::from(NOISE_MAX)) as i32
}

/// Milli-quanta in one height quantum.
pub(crate) const MQ: i64 = 1_000;

/// Height quanta from milli-quanta. Floors, so that the published height of a cell never rounds up
/// past a neighbour it is genuinely below.
fn quanta(mq: i32) -> i32 {
    mq.div_euclid(MQ as i32)
}

/// The hillslope and fine-relief terms, in milli-quanta, which give a hillside its shape without
/// moving the continent. Kept separate from [`continental_mq`] because the province lattice reads
/// the continental term alone: a province's rank must not wobble with a metre of surface texture.
fn texture_mq(seed: u32, q: i32, r: i32) -> i32 {
    let hill = value_noise(seed, q, r, HILLSLOPE_CELL, OCT_HILLSLOPE);
    let fine = value_noise(seed, q, r, RELIEF_CELL, OCT_RELIEF);
    let half = NOISE_MAX / 2;
    let scale = MQ as i32;
    (hill - half) * HILLSLOPE_QUANTA * scale / half + (fine - half) * RELIEF_QUANTA * scale / half
}

/// Height before any channel is cut, in milli-quanta. Pure, unbounded, `O(1)`.
pub(crate) fn base_mq(seed: u32, q: i32, r: i32) -> i32 {
    continental_mq(seed, q, r) + texture_mq(seed, q, r)
}

fn moisture(seed: u32, q: i32, r: i32) -> i32 {
    value_noise(seed, q, r, MOISTURE_CELL, OCT_MOISTURE)
}

/// A province's place in the macro drainage order, in milli-quanta: the continental field at the
/// province's own origin cell. Ranks are therefore samples of the very field the cells interpolate,
/// so the macro graph and the visible landscape cannot drift apart.
pub fn province_rank(seed: u32, pq: i32, pr: i32) -> i32 {
    let (q, r) = province_origin(pq, pr);
    continental_mq(seed, q, r)
}

/// Which way a province drains. `O(1)`: five continental samples and nothing else.
pub fn province_outlet(seed: u32, pq: i32, pr: i32) -> Outlet {
    let rank = province_rank(seed, pq, pr);
    if rank < SEA_LEVEL_QUANTA {
        return Outlet::Ocean;
    }
    let mut best: Option<(i32, i32, i32)> = None;
    for (dq, dr) in PROVINCE_FACES {
        let (nq, nr) = (pq + dq, pr + dr);
        let key = (province_rank(seed, nq, nr), nq, nr);
        if key < (rank, pq, pr) && best.is_none_or(|current| key < current) {
            best = Some(key);
        }
    }
    match best {
        Some((_, nq, nr)) => Outlet::Province { pq: nq, pr: nr },
        None => Outlet::Basin,
    }
}

/// Whether `up` drains directly into `down`.
fn drains_into(seed: u32, up: (i32, i32), down: (i32, i32)) -> bool {
    province_outlet(seed, up.0, up.1)
        == Outlet::Province {
            pq: down.0,
            pr: down.1,
        }
}

/// A bounded walk up the province tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Upstream {
    /// Provinces draining into this one, this one included.
    pub provinces: u32,
    /// True when the walk hit [`UPSTREAM_PROVINCE_BUDGET`] and stopped counting. A saturated
    /// catchment reports the top discharge class, which is honest: past that width the classes
    /// stop distinguishing anything a player can see.
    pub saturated: bool,
}

/// The seam between two provinces, named canonically so that both sides derive the same one.
///
/// The lower province along the axis owns the name, which is the whole trick: a pour point is a
/// property of an unordered pair, and a channel that arrives at a different cell depending on which
/// side asked would tear at every seam in the world.
fn seam_pour(seed: u32, a: (i32, i32), b: (i32, i32)) -> ((i32, i32), (i32, i32)) {
    let (lo, along_q) = if a.0 == b.0 {
        (if a.1 < b.1 { a } else { b }, true)
    } else {
        (if a.0 < b.0 { a } else { b }, false)
    };
    let (origin_q, origin_r) = province_origin(lo.0, lo.1);
    let mut best: Option<(i32, (i32, i32), (i32, i32))> = None;
    for step in 0..PROVINCE_CELL {
        // The two cells that actually touch across the seam, enumerated from the lower province so
        // that the pair, not the block, is what gets minimised.
        let (low_cell, high_cell) = if along_q {
            (
                (origin_q + step, origin_r + PROVINCE_CELL - 1),
                (origin_q + step, origin_r + PROVINCE_CELL),
            )
        } else {
            (
                (origin_q + PROVINCE_CELL - 1, origin_r + step),
                (origin_q + PROVINCE_CELL, origin_r + step),
            )
        };
        // The full field, not the continental term: a pour point is where water actually crosses
        // the face, and the hillslope carries ten metres the continental term cannot see.
        let low_head = base_mq(seed, low_cell.0, low_cell.1);
        let high_head = base_mq(seed, high_cell.0, high_cell.1);
        let key = low_head.min(high_head);
        if best.is_none_or(|(current, cell, _)| (key, low_cell) < (current, cell)) {
            best = Some((key, low_cell, high_cell));
        }
    }
    let (_, low_cell, high_cell) = best.expect("a province face is never empty");
    if lo == a {
        (low_cell, high_cell)
    } else {
        (high_cell, low_cell)
    }
}

/// One node of a province's channel network.
#[derive(Clone, Copy, Debug)]
struct SpineNode {
    cell: (i32, i32),
    parent: Option<usize>,
    /// Ground height under the node after the routing flood filled its depressions, in
    /// milli-quanta. Strictly greater than the parent's, which is what makes the long profile
    /// descend rather than merely tend to.
    filled_mq: i32,
    /// Catchment in cells, accumulated from the leaves.
    catchment: u64,
    /// True when a spring stands at or above this node.
    wet: bool,
    spring: bool,
}

/// A province's channel network: the drainage, built before any terrain is fitted to it.
#[derive(Debug)]
pub struct Spine {
    /// Channel cells and the discharge class carried through each, ordered so that iteration is
    /// deterministic.
    pub channels: BTreeMap<(i32, i32), Channel>,
    pub springs: Vec<(i32, i32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channel {
    /// Always at least [`CHANNEL_CLASS_MIN`]; class 0 is the hillslope, not a channel.
    pub class: u8,
    /// The water surface this reach carries, in milli-quanta: the hydraulic grade line.
    ///
    /// This, not the bed, is what makes the model gravitational. It is interpolated between node
    /// heights that strictly descend downstream, so the surface strictly descends too, and every
    /// other water fact — depth, bank, bed — is derived from it rather than from the noise field
    /// the reach happens to cross.
    pub surface_mq: i32,
    /// The bed under that surface, in milli-quanta: the deepest the rock let this reach cut.
    ///
    /// Usually `surface_mq` less the class's own depth. Where a bed of resistant rock crosses the
    /// reach it is higher, the channel runs shallow over the sill, and the water backs up behind
    /// it — which is why depth is read from these two numbers rather than declared per class.
    pub floor_mq: i32,
    /// A channel with a spring at or above it carries water; the rest are dry valley floors, which
    /// is what stops the network from being a river system with no source.
    pub wet: bool,
}

/// The discharge class at which the flow network becomes a cut channel.
///
/// The spanning tree reaches every node, because routing has to; incision does not follow it all
/// the way up. Carving every twig would put a gully every 43 m, which is two orders of magnitude
/// denser than real drainage and would leave nowhere flat enough to build. Below this class the
/// water is on the hillslope, where it belongs.
///
/// Class 1 is hillslope drainage. Cutting it made the default inland world a mesh of one-cell
/// streams with one-cell shore stripes, so visible water now starts once five class-1 catchments
/// have joined. The drainage still exists above this threshold; it simply has no incised bed or
/// permanent surface water yet.
pub const CHANNEL_CLASS_MIN: u8 = 2;

/// The widest a class of valley may spread, in cells. The bank grade normally stops the cut well
/// inside this; the cap is what keeps every valley inside [`VALLEY_RADIUS`] and so inside the halo.
pub(crate) fn valley_half_width(class: u8) -> i32 {
    (4 + i32::from(class.saturating_sub(CHANNEL_CLASS_MIN)) * 4).min(VALLEY_RADIUS)
}

/// How far a reach's water surface stands below the ground it was routed over, in height quanta:
/// 1.5 m for the smallest channel, 4.5 m for the largest.
///
/// This is what incises a valley. A bigger river has cut longer, so it runs deeper below its own
/// shoulders, and the two effects the eye reads as a valley — depth and width — both follow from
/// this one number through [`VALLEY_BANK_MQ`].
fn surface_cut(class: u8) -> i32 {
    6 + i32::from(class.saturating_sub(CHANNEL_CLASS_MIN)) * 2
}

/// Water depth at the centreline, in height quanta: 0.5 m for a stream, 2 m for a continental
/// river. Class 3 and above passes [`crate::scale::WADE_LIMIT_QUANTA`], so a small stream is a ford
/// and a large one is not.
pub(crate) fn bed_depth(class: u8) -> i32 {
    1 + i32::from(class)
}

/// The thinnest sheet of water a reach carries over a sill it could not cut, in milli-quanta:
/// one quantum, which is the least depth [`Terra::water`] will publish at all.
///
/// A river crossing resistant rock runs shallow and fast rather than stopping, so this is what
/// keeps a knickpoint a rapid instead of a dry gap in the channel.
const MIN_FLOW_MQ: i32 = MQ as i32;

/// The water surface and the bed one cell of a reach carries, in milli-quanta, given the routed
/// ground level there.
///
/// Two numbers rather than a depth per class: the grade line says where the water wants to be and
/// [`crate::terra_rock`] says how far down the rock let it get, and everything the eye reads as a
/// pool, a rapid or a fall is the gap between them.
fn graded_bed(seed: u32, cell: (i32, i32), class: u8, level_mq: i32) -> (i32, i32) {
    let surface = level_mq - surface_cut(class) * MQ as i32;
    let target = surface - bed_depth(class) * MQ as i32;
    let floor = crate::terra_rock::erodible_floor_mq(seed, cell.0, cell.1, class, target);
    (surface.max(floor + MIN_FLOW_MQ), floor)
}

/// Half-width of the wetted channel, in cells: three cells across for the first visible stream,
/// rising one cell each side per class to thirteen for a continental river.
///
/// One cell of half-width per class is what makes the network read as a hierarchy rather than as a
/// set of threads: every confluence widens the water visibly, because the class it produces is the
/// class its two branches earned. The previous table spent its first three classes at zero, so the
/// only two classes this generator actually reaches both drew a thread — the mouth of a river looked
/// like its own headwater.
///
/// This is stated against the absolute discharge class rather than [`CHANNEL_CLASS_MIN`], so
/// suppressing hillslope drainage never accidentally narrows the rivers that remain.
pub(crate) fn river_half_width(class: u8) -> i32 {
    match class {
        0..=1 => 0,
        _ => i32::from(class) - 1,
    }
}

/// Dry alluvial bench outside each wetted bank, in cells. Large rivers earn two readable rows;
/// small channels keep one so their banks do not consume more ground than their water.
pub(crate) fn river_bench_width(class: u8) -> i32 {
    if class >= 5 {
        2
    } else {
        1
    }
}

/// How far the graded valley floor is laid down exactly rather than only cut into, in cells.
///
/// Inside this the generated bed *is* the graded floor, so the ±10.75 m hillslope term cannot leave
/// a lip in the middle of a river. That flattening is the whole reason the water surface can be a
/// smooth descending line instead of the ragged one the noise field would impose.
fn bed_radius(class: u8) -> i32 {
    river_half_width(class) + river_bench_width(class)
}

/// How fast the bed climbs from the thalweg out to the waterline, in milli-quanta per cell: one
/// quantum, a quarter of a metre.
///
/// Not zero, because a perfectly flat bed leaves the flow direction across a wide channel a tie,
/// and a tie is how a flow edge ends up pointing at the far bank instead of downstream. Not the
/// bank grade either: at 4 m to 6.4 m per cell the ground clears the water surface before the first
/// neighbour, which is what kept every river one cell wide. [`bed_depth`] gains a quantum per class
/// and [`river_half_width`] a cell, so the outermost wetted cell of every class carries exactly two
/// quanta — half a metre — and the middle of the river is always its deepest part.
const CHANNEL_CROSS_GRADE_MQ: i32 = MQ as i32;

/// The most alluvium the graded floor may lay into a hollow it crosses, in milli-quanta: 1 m.
///
/// A channel fills the centimetre-scale pits the relief term manufactures, because a river bed with
/// a lip in it is the defect. It does not fill a real hollow: past this the floor stops rising, the
/// graded surface stands over the ground, and the result is a pond in the reach — which is what a
/// river crossing a depression actually does.
const FILL_LIMIT_MQ: i32 = 4_000;

/// Catchment in cells to a discharge class.
///
/// The ladder does two different jobs, so it climbs at two different rates. Up to
/// [`CHANNEL_CLASS_MIN`] it is a *threshold*: below 2,048 cells the water is on the hillslope and
/// above 10,240 it has cut a channel, and moving either number changes how dense the network is
/// rather than how big its rivers are. Above that it is a *width scale*, and it doubles, because a
/// class buys one more cell of half-width and doubling the catchment is about what a river needs to
/// earn one.
///
/// Base five the whole way was calibrated for a 32-million-cell continent this generator does not
/// produce. Measured over the reference seed's 81 provinces around the landing, the largest basin
/// reaching the sea held between 65,536 and 131,072 cells, so classes 4 and up never appeared and
/// the entire world was two river widths: a one-cell thread and a three-cell one. Doubling puts the
/// top of the ladder where the water actually is — 327,680 cells, twenty provinces — so a headwater,
/// its confluences and the reach that meets the sea are told apart by the width the eye reads.
pub fn discharge_class(catchment_cells: u64) -> u8 {
    let mut class = 0u8;
    let mut threshold = 2_048u64;
    while class < 7 && catchment_cells >= threshold {
        class += 1;
        threshold = threshold.saturating_mul(if class < CHANNEL_CLASS_MIN { 5 } else { 2 });
    }
    class
}

/// The least a node must stand above the one it drains into, in milli-quanta.
///
/// Small on purpose — 0.01 m over 43 m. It is not a gradient the eye reads; it is what makes
/// "downstream is strictly lower" true across a filled depression as well as on open hillside, so
/// the long profile has no flat on it for a flow direction to be undefined on.
const NODE_DROP_MQ: i32 = 40;

/// How far the routing flood may prefer one neighbour over a lower one, in milli-quanta: 0.375 m.
///
/// Purely dendritic character. Adjacent nodes stand 43 m apart and the hillslope term moves several
/// quanta over that distance, so this perturbs which of two near-equal saddles a headwater takes
/// without ever letting a thread choose real high ground over real low ground.
const SPINE_JITTER_MQ: i32 = 1_500;

fn spine_jitter(seed: u32, cell: (i32, i32)) -> i32 {
    (coordinate_hash(seed ^ OCT_SPINE, cell.0, cell.1) % SPINE_JITTER_MQ as u32) as i32
}

fn spine_nodes_per_side() -> i32 {
    PROVINCE_CELL / SPINE_CELL
}

fn node_cell(pq: i32, pr: i32, i: i32, j: i32) -> (i32, i32) {
    let (origin_q, origin_r) = province_origin(pq, pr);
    (
        origin_q + i * SPINE_CELL + SPINE_CELL / 2,
        origin_r + j * SPINE_CELL + SPINE_CELL / 2,
    )
}

/// Builds a province's channel network. Bounded: 256 nodes, four faces, and one traced path
/// per edge.
pub fn build_spine(seed: u32, pq: i32, pr: i32, inflow: &[SeamInflow]) -> Spine {
    let side = spine_nodes_per_side();
    let count = (side * side) as usize;
    let mut nodes: Vec<SpineNode> = Vec::with_capacity(count);
    for j in 0..side {
        for i in 0..side {
            let cell = node_cell(pq, pr, i, j);
            nodes.push(SpineNode {
                cell,
                parent: None,
                filled_mq: i32::MIN,
                catchment: (SPINE_CELL * SPINE_CELL) as u64,
                wet: false,
                spring: false,
            });
        }
    }

    // The root is the node nearest wherever this province's water leaves it. For a coastal or
    // basin province there is no seam to leave by, so the lowest node is the low point of the
    // basin itself.
    let outlet = province_outlet(seed, pq, pr);
    let root = match outlet {
        Outlet::Province { pq: nq, pr: nr } => {
            let (own_cell, _) = seam_pour(seed, (pq, pr), (nq, nr));
            nearest_node(pq, pr, own_cell)
        }
        Outlet::Ocean | Outlet::Basin => {
            let mut best: Option<(i32, i32, i32, usize)> = None;
            for (slot, node) in nodes.iter().enumerate() {
                let key = (
                    base_mq(seed, node.cell.0, node.cell.1),
                    node.cell.0,
                    node.cell.1,
                    slot,
                );
                if best.is_none_or(|current| key < current) {
                    best = Some(key);
                }
            }
            best.expect("a province always has nodes").3
        }
    };

    // A priority flood outward from the outlet, which is what makes this a drainage network rather
    // than a maze. A node enters the tree at `max(its own ground, its parent's level + a drop)`, so
    // every node stands strictly above the one it drains into whether the ground between them fell
    // or rose — the filled level *is* the long profile, and a depression on the way out becomes a
    // level reach rather than a place the thread runs uphill. The jitter perturbs which of two
    // near-equal saddles a headwater takes and never the order of two genuinely different ones.
    let mut order: Vec<usize> = Vec::with_capacity(count);
    let mut frontier: BinaryHeap<Reverse<(i32, i32, i32, i32, usize, usize)>> = BinaryHeap::new();
    let root_cell = nodes[root].cell;
    let root_level = base_mq(seed, root_cell.0, root_cell.1);
    frontier.push(Reverse((
        root_level + spine_jitter(seed, root_cell),
        root_level,
        root_cell.0,
        root_cell.1,
        root,
        root,
    )));
    while let Some(Reverse((_, level, _, _, slot, parent))) = frontier.pop() {
        if nodes[slot].filled_mq != i32::MIN {
            continue;
        }
        nodes[slot].filled_mq = level;
        if slot != root {
            nodes[slot].parent = Some(parent);
        }
        order.push(slot);
        push_spine_edges(seed, &nodes, side, slot, level, &mut frontier);
    }

    // Water arriving across a seam joins at the node nearest its pour point.
    for seam in inflow {
        let slot = nearest_node(pq, pr, seam.cell);
        nodes[slot].catchment = nodes[slot].catchment.saturating_add(seam.catchment);
    }

    // `order` is a breadth-ordered walk away from the root, so walking it backwards settles every
    // child before its parent.
    for &slot in order.iter().rev() {
        let catchment = nodes[slot].catchment;
        if let Some(parent) = nodes[slot].parent {
            nodes[parent].catchment = nodes[parent].catchment.saturating_add(catchment);
        }
    }

    let class_of: Vec<u8> = nodes
        .iter()
        .map(|node| discharge_class(node.catchment))
        .collect();

    // A spring is where a stream starts: the highest point of a cut channel, with no cut channel
    // above it, on wet ground. Putting them at the tree's leaves instead would put a source on
    // every hilltop in the world.
    let mut has_channel_child = vec![false; count];
    for slot in 0..count {
        if class_of[slot] >= CHANNEL_CLASS_MIN {
            if let Some(parent) = nodes[slot].parent {
                has_channel_child[parent] = true;
            }
        }
    }
    for slot in 0..count {
        let cell = nodes[slot].cell;
        let head = nodes[slot].filled_mq;
        if class_of[slot] >= CHANNEL_CLASS_MIN
            && !has_channel_child[slot]
            && moisture(seed, cell.0, cell.1) > spring_threshold(head)
            && head > SEA_LEVEL_QUANTA * MQ as i32
        {
            nodes[slot].spring = true;
            nodes[slot].wet = true;
        }
    }
    for seam in inflow {
        let slot = nearest_node(pq, pr, seam.cell);
        nodes[slot].wet |= seam.wet;
    }
    for &slot in order.iter().rev() {
        let wet = nodes[slot].wet;
        if let Some(parent) = nodes[slot].parent {
            nodes[parent].wet |= wet;
        }
    }

    let mut channels: BTreeMap<(i32, i32), Channel> = BTreeMap::new();
    let mut mark = |cell: (i32, i32), class: u8, surface_mq: i32, floor_mq: i32, wet: bool| {
        if class < CHANNEL_CLASS_MIN {
            return;
        }
        let entry = channels.entry(cell).or_insert(Channel {
            class,
            surface_mq,
            floor_mq,
            wet,
        });
        // Where two branches share a cell the wider one owns it, so a confluence never narrows,
        // and the lower grade line owns it, so a tributary never perches over its own main stem.
        if class > entry.class {
            entry.class = class;
        }
        entry.surface_mq = entry.surface_mq.min(surface_mq);
        entry.floor_mq = entry.floor_mq.min(floor_mq);
        entry.wet |= wet;
    };

    // A reach is drawn from its node to its parent's, with the grade line interpolated between the
    // two filled node levels. Both ends descend, so every cell between them does.
    let mut profile: Vec<(i32, i32)> = Vec::with_capacity(2 * SPINE_CELL as usize);
    let mut reach = |mark: &mut dyn FnMut((i32, i32), u8, i32, i32, bool),
                     from: (i32, i32),
                     from_mq: i32,
                     to: (i32, i32),
                     to_mq: i32,
                     class: u8,
                     wet: bool| {
        let path = descend_line(seed, from, to);
        let last = (path.len() - 1).max(1) as i64;
        for (step, &cell) in path.iter().enumerate() {
            let t = step as i64;
            let level = (i64::from(from_mq) * (last - t) + i64::from(to_mq) * t) / last;
            profile.push(graded_bed(seed, cell, class, level as i32));
        }
        // A sill is a local base level: the water upstream of it stands level with its lip, and
        // the fall is the step that leaves. One backward pass is the whole of that, and it is also
        // what keeps the published surface non-increasing downstream through a stepped profile.
        for step in (0..profile.len().saturating_sub(1)).rev() {
            profile[step].0 = profile[step].0.max(profile[step + 1].0);
        }
        for (cell, (surface_mq, floor_mq)) in path.into_iter().zip(profile.drain(..)) {
            mark(cell, class, surface_mq, floor_mq, wet);
        }
    };

    for slot in 0..count {
        let node = nodes[slot];
        match node.parent {
            Some(parent) => reach(
                &mut mark,
                node.cell,
                node.filled_mq,
                nodes[parent].cell,
                nodes[parent].filled_mq,
                class_of[slot],
                node.wet,
            ),
            None => {
                let (surface, floor) = graded_bed(seed, node.cell, class_of[slot], node.filled_mq);
                mark(node.cell, class_of[slot], surface, floor, node.wet);
            }
        }
    }

    // Both sides of an active seam draw a stub to the shared pour point, so the channel crosses
    // rather than stopping four cells short on each side of every province boundary. The stub ends
    // on the pour cell's own ground, which both sides read from the same field, so the two grade
    // lines meet at the seam instead of each ending at its own invented height.
    for (dq, dr) in PROVINCE_FACES {
        let neighbour = (pq + dq, pr + dr);
        let active =
            drains_into(seed, (pq, pr), neighbour) || drains_into(seed, neighbour, (pq, pr));
        if !active {
            continue;
        }
        let (own_cell, _) = seam_pour(seed, (pq, pr), neighbour);
        let slot = nearest_node(pq, pr, own_cell);
        let node = nodes[slot];
        let pour_mq = base_mq(seed, own_cell.0, own_cell.1).min(node.filled_mq);
        reach(
            &mut mark,
            node.cell,
            node.filled_mq,
            own_cell,
            pour_mq,
            class_of[slot],
            node.wet,
        );
    }

    let springs = nodes
        .iter()
        .filter(|node| node.spring)
        .map(|node| node.cell)
        .collect();
    Spine { channels, springs }
}

#[allow(clippy::type_complexity)]
fn push_spine_edges(
    seed: u32,
    nodes: &[SpineNode],
    side: i32,
    slot: usize,
    level: i32,
    frontier: &mut BinaryHeap<Reverse<(i32, i32, i32, i32, usize, usize)>>,
) {
    let i = slot as i32 % side;
    let j = slot as i32 / side;
    for (dq, dr) in DIRECTIONS {
        let (ni, nj) = (i + dq, j + dr);
        if ni < 0 || nj < 0 || ni >= side || nj >= side {
            continue;
        }
        let next = (nj * side + ni) as usize;
        if nodes[next].filled_mq != i32::MIN {
            continue;
        }
        let cell = nodes[next].cell;
        let raised = base_mq(seed, cell.0, cell.1).max(level + NODE_DROP_MQ);
        frontier.push(Reverse((
            raised + spine_jitter(seed, cell),
            raised,
            cell.0,
            cell.1,
            next,
            slot,
        )));
    }
}

fn nearest_node(pq: i32, pr: i32, cell: (i32, i32)) -> usize {
    let side = spine_nodes_per_side();
    let (origin_q, origin_r) = province_origin(pq, pr);
    let i = ((cell.0 - origin_q) / SPINE_CELL).clamp(0, side - 1);
    let j = ((cell.1 - origin_r) / SPINE_CELL).clamp(0, side - 1);
    (j * side + i) as usize
}

/// Water crossing into a province across one seam.
#[derive(Clone, Copy, Debug)]
pub struct SeamInflow {
    pub cell: (i32, i32),
    pub catchment: u64,
    pub wet: bool,
}

/// The cells one reach passes through: at every step the neighbour that closes the distance to the
/// next node and stands lowest on the ground.
///
/// A straight rasterised line is what made the shipped rivers read as angular segments crossing
/// whatever was in the way. Only one or two of the six neighbours ever shorten an axial distance,
/// so choosing between them by height costs almost nothing, arrives in exactly `axial_distance`
/// steps, and puts the thread in the hollow rather than over the shoulder beside it.
fn descend_line(seed: u32, from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let steps = axial_distance(from, to);
    let mut cells = Vec::with_capacity(steps as usize + 1);
    cells.push(from);
    let mut current = from;
    for _ in 0..steps {
        let remaining = axial_distance(current, to);
        let mut best: Option<(i32, i32, i32)> = None;
        for (dq, dr) in DIRECTIONS {
            let next = (current.0 + dq, current.1 + dr);
            if axial_distance(next, to) >= remaining {
                continue;
            }
            let key = (base_mq(seed, next.0, next.1), next.0, next.1);
            if best.is_none_or(|current| key < current) {
                best = Some(key);
            }
        }
        let Some((_, q, r)) = best else { break };
        current = (q, r);
        cells.push(current);
    }
    cells
}

/// One province, solved.
#[derive(Debug)]
pub struct Province {
    pq: i32,
    pr: i32,
    origin_q: i32,
    origin_r: i32,
    /// Finished height over block plus halo, in milli-quanta.
    head_mq: Vec<i32>,
    /// The channel each domain cell is closest to across the wetted course and its dry bench, as a
    /// domain index, or `usize::MAX` where no channel reaches.
    nearest_channel: Vec<usize>,
    /// Hex distance to `nearest_channel`. The bounded reach is at most six cells, so one byte keeps
    /// the generated province compact while letting water and dry alluvium share one exact shape.
    channel_distance: Vec<u8>,
    channels: BTreeMap<(i32, i32), Channel>,
    /// Flow and lake membership, for the province's own cells only.
    flow: Vec<Flow>,
    lakes: Vec<LakeInfo>,
    springs: Vec<(i32, i32)>,
}

impl Province {
    fn domain_index(&self, q: i32, r: i32) -> Option<usize> {
        let x = q - self.origin_q + HALO;
        let y = r - self.origin_r + HALO;
        if x < 0 || y < 0 || x >= DOMAIN_SIDE || y >= DOMAIN_SIDE {
            return None;
        }
        Some((y * DOMAIN_SIDE + x) as usize)
    }

    fn own_index(&self, q: i32, r: i32) -> Option<usize> {
        let x = q - self.origin_q;
        let y = r - self.origin_r;
        if x < 0 || y < 0 || x >= PROVINCE_CELL || y >= PROVINCE_CELL {
            return None;
        }
        Some((y * PROVINCE_CELL + x) as usize)
    }

    fn domain_cell(&self, index: usize) -> (i32, i32) {
        let x = index as i32 % DOMAIN_SIDE;
        let y = index as i32 / DOMAIN_SIDE;
        (self.origin_q + x - HALO, self.origin_r + y - HALO)
    }

    /// Published height, in whole quanta, at any cell in the block or its halo.
    pub fn head(&self, q: i32, r: i32) -> Option<i32> {
        self.head_mq(q, r).map(quanta)
    }

    /// Height in milli-quanta: what flow directions are decided on. Everything a player sees uses
    /// [`Province::head`]; this exists so that a flat quantum is not mistaken for flat ground.
    pub fn head_mq(&self, q: i32, r: i32) -> Option<i32> {
        self.domain_index(q, r).map(|index| self.head_mq[index])
    }

    pub fn flow(&self, q: i32, r: i32) -> Option<Flow> {
        self.own_index(q, r).map(|index| self.flow[index])
    }

    pub fn lake(&self, id: u32) -> LakeInfo {
        self.lakes[id as usize]
    }

    pub fn lakes(&self) -> &[LakeInfo] {
        &self.lakes
    }

    pub fn springs(&self) -> &[(i32, i32)] {
        &self.springs
    }

    pub fn channel(&self, q: i32, r: i32) -> Option<Channel> {
        self.channels.get(&(q, r)).copied()
    }

    pub fn coordinate(&self) -> (i32, i32) {
        (self.pq, self.pr)
    }

    /// Whether a cell belongs to this province rather than to its halo.
    pub fn owns(&self, q: i32, r: i32) -> bool {
        self.own_index(q, r).is_some()
    }
}

/// The prototype's cache: provinces and spines, solved on demand.
///
/// A `Terra` is an accelerator, never an authority. Every value it returns is a pure function of
/// `(seed, q, r)`, which is what [`Terra::head_uncached`] exists to keep true.
pub struct Terra {
    seed: u32,
    provinces: BTreeMap<(i32, i32), Rc<Province>>,
    spines: BTreeMap<(i32, i32), Rc<Spine>>,
    upstream: BTreeMap<(i32, i32), Upstream>,
    spring_counts: BTreeMap<(i32, i32), u32>,
    solved: usize,
}

impl Terra {
    pub fn new(seed: u32) -> Terra {
        Terra {
            seed,
            provinces: BTreeMap::new(),
            spines: BTreeMap::new(),
            upstream: BTreeMap::new(),
            spring_counts: BTreeMap::new(),
            solved: 0,
        }
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// How many provinces have been solved. The survey reports work per province from this, and a
    /// bounded-work test asserts on it.
    pub fn provinces_solved(&self) -> usize {
        self.solved
    }

    /// The spine of one province, including whatever crosses its seams.
    pub fn spine(&mut self, pq: i32, pr: i32) -> Rc<Spine> {
        if let Some(spine) = self.spines.get(&(pq, pr)) {
            return Rc::clone(spine);
        }
        let mut inflow = Vec::new();
        for (dq, dr) in PROVINCE_FACES {
            let neighbour = (pq + dq, pr + dr);
            if !drains_into(self.seed, neighbour, (pq, pr)) {
                continue;
            }
            let (cell, _) = seam_pour(self.seed, (pq, pr), neighbour);
            let upstream = self.upstream(neighbour.0, neighbour.1);
            inflow.push(SeamInflow {
                cell,
                // A saturated walk names the top class rather than a number it did not finish
                // counting. That is honest: past the budget the classes stop distinguishing
                // anything, so "the largest river there is" is the true answer.
                catchment: if upstream.saturated {
                    u64::MAX
                } else {
                    u64::from(upstream.provinces) * (PROVINCE_CELL as u64 * PROVINCE_CELL as u64)
                },
                wet: self.springs_upstream(neighbour.0, neighbour.1),
            });
        }
        let spine = Rc::new(build_spine(self.seed, pq, pr, &inflow));
        self.spines.insert((pq, pr), Rc::clone(&spine));
        spine
    }

    /// A bounded count of the provinces draining into this one.
    pub fn upstream(&mut self, pq: i32, pr: i32) -> Upstream {
        if let Some(found) = self.upstream.get(&(pq, pr)) {
            return *found;
        }
        let mut seen = 0u32;
        let mut saturated = false;
        let mut queue = VecDeque::from([(pq, pr)]);
        while let Some(current) = queue.pop_front() {
            seen += 1;
            if seen as usize >= UPSTREAM_PROVINCE_BUDGET {
                saturated = true;
                break;
            }
            for (dq, dr) in PROVINCE_FACES {
                let candidate = (current.0 + dq, current.1 + dr);
                if drains_into(self.seed, candidate, current) {
                    queue.push_back(candidate);
                }
            }
        }
        let result = Upstream {
            provinces: seen,
            saturated,
        };
        self.upstream.insert((pq, pr), result);
        result
    }

    /// How many springs a province's own network holds. Used to decide whether a seam carries
    /// water, so a channel with no source anywhere above it stays a dry valley floor.
    fn spring_count(&mut self, pq: i32, pr: i32) -> u32 {
        if let Some(found) = self.spring_counts.get(&(pq, pr)) {
            return *found;
        }
        // The bare network, without seam inflow: inflow only ever adds water, and asking for it
        // here is what would make this recursive.
        let bare = build_spine(self.seed, pq, pr, &[]);
        let count = bare.springs.len() as u32;
        self.spring_counts.insert((pq, pr), count);
        count
    }

    fn springs_upstream(&mut self, pq: i32, pr: i32) -> bool {
        let mut seen = 0usize;
        let mut queue = VecDeque::from([(pq, pr)]);
        while let Some(current) = queue.pop_front() {
            seen += 1;
            if seen > UPSTREAM_PROVINCE_BUDGET {
                return true;
            }
            if self.spring_count(current.0, current.1) > 0 {
                return true;
            }
            for (dq, dr) in PROVINCE_FACES {
                let candidate = (current.0 + dq, current.1 + dr);
                if drains_into(self.seed, candidate, current) {
                    queue.push_back(candidate);
                }
            }
        }
        false
    }

    /// Solves one province, or returns the solved one.
    pub fn province(&mut self, pq: i32, pr: i32) -> Rc<Province> {
        if let Some(province) = self.provinces.get(&(pq, pr)) {
            return Rc::clone(province);
        }
        let province = Rc::new(self.solve(pq, pr));
        self.provinces.insert((pq, pr), Rc::clone(&province));
        self.solved += 1;
        province
    }

    fn solve(&mut self, pq: i32, pr: i32) -> Province {
        let (origin_q, origin_r) = province_origin(pq, pr);
        let cells = (DOMAIN_SIDE * DOMAIN_SIDE) as usize;

        // Channels from the nine provinces the halo can reach. Nothing further away can carve a
        // cell in this domain, because no valley is wider than VALLEY_RADIUS.
        let mut channels: BTreeMap<(i32, i32), Channel> = BTreeMap::new();
        let mut springs: Vec<(i32, i32)> = Vec::new();
        for dr in -1..=1 {
            for dq in -1..=1 {
                let spine = self.spine(pq + dq, pr + dr);
                for (&cell, &channel) in &spine.channels {
                    channels
                        .entry(cell)
                        .and_modify(|existing| {
                            if channel.class > existing.class {
                                existing.class = channel.class;
                            }
                            existing.surface_mq = existing.surface_mq.min(channel.surface_mq);
                            existing.floor_mq = existing.floor_mq.min(channel.floor_mq);
                            existing.wet |= channel.wet;
                        })
                        .or_insert(channel);
                }
                if dq == 0 && dr == 0 {
                    springs.extend(spine.springs.iter().copied());
                }
            }
        }

        let mut province = Province {
            pq,
            pr,
            origin_q,
            origin_r,
            head_mq: vec![0; cells],
            nearest_channel: vec![usize::MAX; cells],
            channel_distance: vec![u8::MAX; cells],
            channels,
            flow: vec![Flow::Frontier; (PROVINCE_CELL * PROVINCE_CELL) as usize],
            lakes: Vec::new(),
            springs,
        };

        // 1. Height before any channel is cut.
        for index in 0..cells {
            let (q, r) = province.domain_cell(index);
            province.head_mq[index] = base_mq(self.seed, q, r);
        }

        // 2. Cut the valleys down to an absolute floor. A reach carries a known water surface and
        // a bed the rock allowed under it, so the cut is an elevation and not a depth, and the
        // long profile descends because the grade line it was seeded from descends. The sweep is a
        // Dijkstra on that floor: pop the lowest remaining influence, settle the cell at it, and
        // offer the floor one bank step higher to the neighbours. Each cell settles once, at the
        // deepest floor that reaches it, so the result does not depend on which channel was
        // visited first. The bank step is the rock's, not a constant, which is what makes one
        // valley a gorge and the next a floodplain.
        let untouched = province.head_mq.clone();
        let mut valley = vec![i32::MAX; cells];
        let mut frontier: BinaryHeap<Reverse<(i32, i32, i32, i32, u8, usize)>> = BinaryHeap::new();
        for (&cell, &channel) in &province.channels {
            let Some(index) = province.domain_index(cell.0, cell.1) else {
                continue;
            };
            frontier.push(Reverse((
                channel.floor_mq,
                cell.0,
                cell.1,
                0,
                channel.class,
                index,
            )));
        }
        while let Some(Reverse((floor, q, r, distance, class, index))) = frontier.pop() {
            if valley[index] != i32::MAX {
                continue;
            }
            valley[index] = floor;
            let base = untouched[index];
            // Inside the wetted width the floor is imposed, so a hollow under the thread is filled
            // rather than left as a hole the water would have to climb out of; the cap keeps that
            // from raising a coastline. Outside it, and under the sea, the floor may only cut.
            //
            // Ground already below sea level is never filled, however close the channel is. The
            // metre of alluvium is there to take a lip out of a river's own bed, and a reach that
            // ends in the ocean has bed cells on the far side of the shoreline: allowing them to
            // rise laid a causeway a bed-radius wide down every drowned reach in the archipelago,
            // turning 3,925 hexes of sea into land and taking the landing's clay with it.
            //
            // The cut leaves a smooth ramp, and fading [`texture_mq`] back in across it was tried
            // and rejected: valley-side roughness traps drainage. Lakes went 7 → 49 and walks
            // leaving the sample 292 → 36. A graded valley is smooth because that is what letting
            // the water out costs; the material map answers for its own thresholds.
            let drowned = base < SEA_LEVEL_QUANTA * MQ as i32;
            province.head_mq[index] = if distance <= bed_radius(class) && !drowned {
                floor.min(base + FILL_LIMIT_MQ)
            } else {
                base.min(floor)
            };
            if distance >= valley_half_width(class) {
                continue;
            }
            for (dq, dr) in DIRECTIONS {
                if let Some(neighbour) = province.domain_index(q + dq, r + dr) {
                    if valley[neighbour] == i32::MAX {
                        // Inside its own wetted width a channel has no bank: the bed climbs to the
                        // waterline at the shallow cross grade, and the valley side starts outside
                        // it. Without this the floor rose a bank grade — 1 m to 1.6 m — from the
                        // centreline out, which is more than any class's water is deep, so every
                        // river in the world was one cell wide however wide [`river_half_width`]
                        // said it was.
                        let grade = if distance < river_half_width(class) {
                            CHANNEL_CROSS_GRADE_MQ
                        } else {
                            crate::terra_rock::bank_grade_mq(
                                self.seed,
                                q + dq,
                                r + dr,
                                untouched[neighbour],
                            )
                        };
                        frontier.push(Reverse((
                            floor + grade,
                            q + dq,
                            r + dr,
                            distance + 1,
                            class,
                            neighbour,
                        )));
                    }
                }
            }
        }

        // 3. Record, for every cell, the channel whose water surface could stand over it. Done as
        // its own bounded sweep rather than threaded through the incision, so that "which channel
        // is nearest" answers the water question and not the carving one.
        let mut reach: BinaryHeap<Reverse<(i32, i32, i32, usize, usize)>> = BinaryHeap::new();
        for &cell in province.channels.keys() {
            let Some(index) = province.domain_index(cell.0, cell.1) else {
                continue;
            };
            reach.push(Reverse((0, cell.0, cell.1, index, index)));
        }
        let mut reached = vec![i32::MAX; cells];
        while let Some(Reverse((distance, _, _, index, source))) = reach.pop() {
            if distance >= reached[index] {
                continue;
            }
            reached[index] = distance;
            province.nearest_channel[index] = source;
            province.channel_distance[index] = distance as u8;
            let (source_q, source_r) = province.domain_cell(source);
            let class = province
                .channels
                .get(&(source_q, source_r))
                .map_or(0, |channel| channel.class);
            if distance >= bed_radius(class) {
                continue;
            }
            let (q, r) = province.domain_cell(index);
            for (dq, dr) in DIRECTIONS {
                if let Some(neighbour) = province.domain_index(q + dq, r + dr) {
                    if distance + 1 < reached[neighbour] {
                        reach.push(Reverse((distance + 1, q + dq, r + dr, neighbour, source)));
                    }
                }
            }
        }

        // 4. Flow: the strictly lower neighbour under one total order. No fill, so there is no way
        // for this step to invent an uphill edge or a cycle.
        let mut pits: Vec<(i32, i32)> = Vec::new();
        for own in 0..(PROVINCE_CELL * PROVINCE_CELL) as usize {
            let q = origin_q + own as i32 % PROVINCE_CELL;
            let r = origin_r + own as i32 / PROVINCE_CELL;
            match steepest_descent(&province, q, r) {
                Some(direction) => province.flow[own] = Flow::To(direction),
                None => pits.push((q, r)),
            }
        }

        // 5. Close what is left over. A pit is a real basin; the flood finds the rim it spills at,
        // or reports that the basin is wider than one province can answer for.
        let mut resolved: Vec<Option<LakeInfo>> = Vec::new();
        for pit in pits {
            if province.flow(pit.0, pit.1) != Some(Flow::Frontier) {
                continue; // Already inside a basin an earlier pit resolved.
            }
            let Some((info, members)) = flood_basin(&province, pit) else {
                // A basin wider than one province can answer for. Left as a declared frontier
                // rather than filled to an invented level.
                continue;
            };
            let id = resolved.len() as u32;
            resolved.push(Some(info));
            for member in members {
                let Some(own) = province.own_index(member.0, member.1) else {
                    continue;
                };
                // A basin that reaches an existing lake encloses it: the rim this flood found is
                // the outer one, so the smaller lake was a pool inside a larger one and its
                // surface was never the real answer.
                if let Flow::Lake(nested) = province.flow[own] {
                    resolved[nested as usize] = None;
                }
                province.flow[own] = Flow::Lake(id);
            }
        }

        // Compact away the enclosed lakes, so that an id always names a lake that owns cells.
        let mut remap = vec![u32::MAX; resolved.len()];
        for (id, lake) in resolved.iter().enumerate() {
            if let Some(lake) = lake {
                remap[id] = province.lakes.len() as u32;
                province.lakes.push(*lake);
            }
        }
        for slot in province.flow.iter_mut() {
            if let Flow::Lake(id) = *slot {
                *slot = match remap[id as usize] {
                    u32::MAX => Flow::Frontier,
                    fresh => Flow::Lake(fresh),
                };
            }
        }
        // A lake's recorded extent has to match the cells that ended up pointing at it, or the
        // survey would be counting an area that was overwritten.
        let mut owned = vec![0u32; province.lakes.len()];
        for slot in &province.flow {
            if let Flow::Lake(id) = *slot {
                owned[id as usize] += 1;
            }
        }
        for (lake, count) in province.lakes.iter_mut().zip(owned) {
            lake.cells = count;
        }

        province
    }

    /// Height at a cell, from the cache.
    pub fn head(&mut self, q: i32, r: i32) -> i32 {
        let (pq, pr) = province_of(q, r);
        self.province(pq, pr)
            .head(q, r)
            .expect("a cell is always inside its own province")
    }

    /// Height at a cell in milli-quanta: the resolution flow directions are decided at.
    pub fn head_mq(&mut self, q: i32, r: i32) -> i32 {
        let (pq, pr) = province_of(q, r);
        self.province(pq, pr)
            .head_mq(q, r)
            .expect("a cell is always inside its own province")
    }

    /// Height at a cell, computed from nothing. The cached and uncached forms must agree; that is
    /// the whole claim caching is allowed to make.
    pub fn head_uncached(seed: u32, q: i32, r: i32) -> i32 {
        let mut fresh = Terra::new(seed);
        fresh.head(q, r)
    }

    pub fn flow(&mut self, q: i32, r: i32) -> Flow {
        let (pq, pr) = province_of(q, r);
        self.province(pq, pr)
            .flow(q, r)
            .expect("a cell is always inside its own province")
    }

    pub fn lake_at(&mut self, q: i32, r: i32) -> Option<LakeInfo> {
        let (pq, pr) = province_of(q, r);
        let province = self.province(pq, pr);
        match province.flow(q, r) {
            Some(Flow::Lake(id)) => Some(province.lake(id)),
            _ => None,
        }
    }

    pub fn channel_at(&mut self, q: i32, r: i32) -> Option<Channel> {
        let (pq, pr) = province_of(q, r);
        self.province(pq, pr).channel(q, r)
    }

    /// The wet channel's drainage class when this cell lies on its generated alluvial bench.
    pub fn river_bench_class_at(&mut self, q: i32, r: i32) -> Option<u8> {
        let (pq, pr) = province_of(q, r);
        let province = self.province(pq, pr);
        let Some(index) = province.domain_index(q, r) else {
            return None;
        };
        let source = province.nearest_channel[index];
        if source == usize::MAX {
            return None;
        }
        let (source_q, source_r) = province.domain_cell(source);
        let Some(channel) = province.channels.get(&(source_q, source_r)) else {
            return None;
        };
        let distance = i32::from(province.channel_distance[index]);
        (channel.wet
            && distance > river_half_width(channel.class)
            && distance <= river_half_width(channel.class) + river_bench_width(channel.class))
        .then_some(channel.class)
    }

    /// The cell this one's water runs to, or `None` at a lake, the sea or a frontier basin.
    pub fn downstream(&mut self, q: i32, r: i32) -> Option<(i32, i32)> {
        match self.flow(q, r) {
            Flow::To(direction) => {
                let (dq, dr) = DIRECTIONS[direction as usize];
                Some((q + dq, r + dr))
            }
            Flow::Lake(_) | Flow::Frontier => None,
        }
    }

    /// What is standing on a cell.
    pub fn water(&mut self, q: i32, r: i32) -> Water {
        let (pq, pr) = province_of(q, r);
        let province = self.province(pq, pr);
        let head = province.head_mq(q, r).expect("own cell");
        let sea = SEA_LEVEL_QUANTA * MQ as i32;
        if head < sea {
            return Water::Sea {
                depth: quanta(sea - head),
            };
        }
        // A depth below one quantum is a damp patch, not water. Counting it would inflate every
        // coverage figure the survey reports with ground nobody would call wet.
        if let Some(Flow::Lake(id)) = province.flow(q, r) {
            let lake = province.lake(id);
            if lake.spill_mq - head >= MQ as i32 {
                return Water::Lake {
                    depth: quanta(lake.spill_mq - head),
                };
            }
        }
        let index = province.domain_index(q, r).expect("own cell");
        let source = province.nearest_channel[index];
        if source == usize::MAX {
            return Water::Dry;
        }
        let (source_q, source_r) = province.domain_cell(source);
        let Some(channel) = province.channels.get(&(source_q, source_r)) else {
            return Water::Dry;
        };
        if !channel.wet
            || i32::from(province.channel_distance[index]) > river_half_width(channel.class)
        {
            return Water::Dry;
        }
        // The reach's own grade line, not a depth over the nearest bed: the surface has to be the
        // same height all the way across a river, and it has to fall along it.
        let surface = channel.surface_mq;
        if surface - head >= MQ as i32 {
            Water::River {
                depth: quanta(surface - head),
                class: channel.class,
            }
        } else {
            Water::Dry
        }
    }

    /// Locate the nearest deterministic dry shelf suitable for the opening.
    ///
    /// The search ranks macro provinces without solving them, then samples at most eight complete
    /// provinces. A candidate is judged on the exact generated bed and water the running source
    /// will publish: no preview approximation, no rounded band and no query-order state. The first
    /// exact shelf wins in distance order; if a hostile seed offers none inside the bound, the
    /// least-bad dry candidate is still deterministic and its score remains testable.
    pub fn landing_site(&mut self) -> LandingSite {
        let coastal_anchor = coast_province(self.seed);
        let anchor = coastal_anchor.unwrap_or((0, 0));
        let search_radius = if coastal_anchor.is_some() {
            LANDING_COAST_RADIUS
        } else {
            LANDING_PROVINCE_RADIUS
        };
        let mut provinces = Vec::new();
        for pq in anchor.0 - search_radius..=anchor.0 + search_radius {
            for pr in anchor.1 - search_radius..=anchor.1 + search_radius {
                let rank = province_rank(self.seed, pq, pr);
                if rank >= SEA_LEVEL_QUANTA {
                    provinces.push((rank, axial_distance(anchor, (pq, pr)), pq, pr));
                }
            }
        }
        provinces.sort_unstable();

        let pad_offsets = hexes_in_radius((0, 0), LANDING_PAD_RADIUS);
        let clear_offsets = hexes_in_radius((0, 0), LANDING_CLEAR_RADIUS);
        let mut best: Option<((u32, i32, u32, i32, i32, i32, i32), LandingSite)> = None;
        // A workable shelf that sits closer to the surf than [`LANDING_BEACH_MIN`], kept in case no
        // seed-legal site stands further back.
        let mut close: Option<LandingSite> = None;
        for &(_, _, pq, pr) in provinces.iter().take(LANDING_PROVINCE_BUDGET) {
            let (origin_q, origin_r) = province_origin(pq, pr);
            let margin = LANDING_CLEAR_RADIUS;
            let mut local_r = margin;
            while local_r < PROVINCE_CELL - margin {
                let mut local_q = margin;
                while local_q < PROVINCE_CELL - margin {
                    let q = origin_q + local_q;
                    let r = origin_r + local_r;
                    let mut water_cells = 0u32;
                    let mut clear_min = i32::MAX;
                    let mut clear_max = i32::MIN;
                    let mut walk_edges = 0u32;
                    for &(dq, dr) in &clear_offsets {
                        let cell = (q + dq, r + dr);
                        let head = self.head(cell.0, cell.1);
                        clear_min = clear_min.min(head);
                        clear_max = clear_max.max(head);
                        water_cells += u32::from(self.water(cell.0, cell.1).is_wet());
                        for &(step_q, step_r) in &DIRECTIONS {
                            if (head - self.head(cell.0 + step_q, cell.1 + step_r)).abs()
                                > crate::scale::MAX_WALK_STEP_QUANTA
                            {
                                walk_edges += 1;
                            }
                        }
                    }
                    let mut pad_min = i32::MAX;
                    let mut pad_max = i32::MIN;
                    for &(dq, dr) in &pad_offsets {
                        let head = self.head(q + dq, r + dr);
                        pad_min = pad_min.min(head);
                        pad_max = pad_max.max(head);
                    }
                    let pad_spread = pad_max - pad_min;
                    let clear_spread = clear_max - clear_min;
                    let score = (
                        water_cells,
                        pad_spread,
                        walk_edges,
                        clear_spread,
                        axial_distance((0, 0), (q, r)),
                        q,
                        r,
                    );
                    let site = LandingSite {
                        q,
                        r,
                        bed_quanta: self.head(q, r),
                    };
                    if best.as_ref().is_none_or(|(current, _)| score < *current) {
                        best = Some((score, site));
                    }
                    if water_cells == 0
                        && pad_spread <= crate::scale::MAX_BUILD_STEP_QUANTA
                        && walk_edges == 0
                        && site.bed_quanta <= LANDING_ALTITUDE_CEILING
                    {
                        match self.sea_distance((q, r), LANDING_BEACH_RADIUS) {
                            Some(beach) if beach >= LANDING_BEACH_MIN => return site,
                            Some(_) => {
                                close.get_or_insert(site);
                            }
                            None => {}
                        }
                    }
                    local_q += LANDING_SAMPLE_STRIDE;
                }
                local_r += LANDING_SAMPLE_STRIDE;
            }
        }
        close
            .or_else(|| best.map(|(_, site)| site))
            .expect("the bounded landing search contains dry-ranked provinces")
    }

    /// Cells to the nearest ocean hex, or `None` past `radius`.
    fn sea_distance(&mut self, centre: (i32, i32), radius: i32) -> Option<i32> {
        for distance in 0..=radius {
            for dq in -distance..=distance {
                for dr in -distance..=distance {
                    if (dq.abs() + dr.abs() + (dq + dr).abs()) / 2 != distance {
                        continue;
                    }
                    if matches!(self.water(centre.0 + dq, centre.1 + dr), Water::Sea { .. }) {
                        return Some(distance);
                    }
                }
            }
        }
        None
    }
}

/// The neighbour a cell's water runs to: the minimum of `(head, q, r)`, and only when it is
/// strictly below the cell's own key. Returning `None` is what makes a pit a pit.
fn steepest_descent(province: &Province, q: i32, r: i32) -> Option<u8> {
    let here = province.head_mq(q, r)?;
    let mut best: Option<(i32, i32, i32, u8)> = None;
    for (index, (dq, dr)) in DIRECTIONS.iter().enumerate() {
        let (nq, nr) = (q + dq, r + dr);
        let Some(head) = province.head_mq(nq, nr) else {
            continue;
        };
        let key = (head, nq, nr, index as u8);
        if (key.0, key.1, key.2) < (here, q, r) && best.is_none_or(|current| key < current) {
            best = Some(key);
        }
    }
    best.map(|(_, _, _, index)| index)
}

/// Grows a basin from a pit until it finds the rim it spills over.
///
/// Returns `None` when the basin is wider than [`LAKE_CELL_BUDGET`] or runs off the province's
/// domain: an unresolved basin is reported as a frontier rather than filled to an invented level,
/// because a lake whose rim nobody has seen has no honest surface height.
fn flood_basin(province: &Province, pit: (i32, i32)) -> Option<(LakeInfo, Vec<(i32, i32)>)> {
    let mut members: Vec<(i32, i32)> = vec![pit];
    let mut inside: BTreeSet<(i32, i32)> = BTreeSet::new();
    inside.insert(pit);
    let mut rim: BinaryHeap<Reverse<(i32, i32, i32)>> = BinaryHeap::new();
    let mut level = province.head_mq(pit.0, pit.1)?;

    let push_rim = |cell: (i32, i32), rim: &mut BinaryHeap<Reverse<(i32, i32, i32)>>| -> bool {
        for (dq, dr) in DIRECTIONS {
            let (nq, nr) = (cell.0 + dq, cell.1 + dr);
            match province.head_mq(nq, nr) {
                Some(head) => rim.push(Reverse((head, nq, nr))),
                // The basin reaches the edge of what this province computed exactly.
                None => return false,
            }
        }
        true
    };
    if !push_rim(pit, &mut rim) {
        return None;
    }

    while let Some(Reverse((head, q, r))) = rim.pop() {
        if inside.contains(&(q, r)) {
            continue;
        }
        // A rim cell that can send water somewhere outside the basin is the spill point.
        let mut escapes = false;
        for (dq, dr) in DIRECTIONS {
            let (nq, nr) = (q + dq, r + dr);
            if inside.contains(&(nq, nr)) {
                continue;
            }
            let Some(neighbour) = province.head_mq(nq, nr) else {
                return None;
            };
            if (neighbour, nq, nr) < (head, q, r) {
                escapes = true;
                break;
            }
        }
        if escapes {
            return Some((
                LakeInfo {
                    spill_mq: head.max(level),
                    spill: (q, r),
                    cells: members.len() as u32,
                },
                members,
            ));
        }
        inside.insert((q, r));
        members.push((q, r));
        level = level.max(head);
        if members.len() > LAKE_CELL_BUDGET {
            return None;
        }
        if !push_rim((q, r), &mut rim) {
            return None;
        }
    }
    None
}

/// A province on the shoreline, or `None` if the seed puts no coast within reach.
///
/// Picks the province whose origin sits closest to sea level while still having both land and
/// seabed among the provinces around it, so a survey centred there contains a river mouth rather
/// than only its headwaters.
pub fn coast_province(seed: u32) -> Option<(i32, i32)> {
    let reach = CONTINENT_PROVINCES * 2;
    let mut best: Option<((i32, i32), i32)> = None;
    for pr in (-reach..=reach).step_by(2) {
        for pq in (-reach..=reach).step_by(2) {
            let (q, r) = province_origin(pq, pr);
            let here = continental_mq(seed, q, r);
            if here <= SEA_LEVEL_QUANTA {
                continue;
            }
            let wet = PROVINCE_FACES.iter().any(|&(dq, dr)| {
                let (nq, nr) = province_origin(pq + dq * 2, pr + dr * 2);
                continental_mq(seed, nq, nr) <= SEA_LEVEL_QUANTA
            });
            if !wet {
                continue;
            }
            if best.is_none_or(|(_, height)| here < height) {
                best = Some(((pq, pr), here));
            }
        }
    }
    best.map(|(province, _)| province)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The generator's own claims: that a query answers the same whoever asks, that channels descend,
    // that a class is a width the eye can read, and that a new game opens on a shelf the landing
    // contract describes.
    include!("terra/tests.rs");
}
