//! Phase 8 slice 1: a drainage-first world prototype, with no production toggle.
//!
//! `docs/HEXFACTORY-PLAN.md#phase-8--flowing-water` refuses to put a flowing-water front on the
//! shipped ridge-noise rivers, and refuses to let a flow simulation grow the generated world. This
//! module exists to falsify the replacement cheaply, before the save boundary makes it expensive.
//! It is **native only** — like `capacity` and `survey`, it is never compiled into the wasm
//! artifact — so nothing in the game can read it by accident. Slice 2 promotes what survives here
//! into the production ground spine.
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
use std::time::Instant;

use crate::scale::{BED_MAX_QUANTA, BED_MIN_QUANTA, SEA_LEVEL_QUANTA};
use crate::{axial_distance, coordinate_hash, cube_round_num, floor_div, value_noise, DIRECTIONS};

/// The noise ceiling `value_noise` interpolates against. Mirrors the private constant in `lib.rs`;
/// re-stated rather than exported because a prototype should not widen the production surface.
const NOISE_MAX: i32 = 65_535;

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

/// Moisture above which a channel head is a spring, on the `0..=NOISE_MAX` noise scale.
const SPRING_MOISTURE: i32 = 38_000;
/// Wavelength of the moisture channel, in cells.
const MOISTURE_CELL: i32 = 96;

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
}

/// The province a cell belongs to.
pub fn province_of(q: i32, r: i32) -> (i32, i32) {
    (floor_div(q, PROVINCE_CELL), floor_div(r, PROVINCE_CELL))
}

fn province_origin(pq: i32, pr: i32) -> (i32, i32) {
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
const MQ: i64 = 1_000;

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
fn base_mq(seed: u32, q: i32, r: i32) -> i32 {
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
        let low_head = continental_mq(seed, low_cell.0, low_cell.1);
        let high_head = continental_mq(seed, high_cell.0, high_cell.1);
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
pub const CHANNEL_CLASS_MIN: u8 = 1;

/// How wide a class of channel cuts, in cells, and how deep, in height quanta.
///
/// Depth grows with class and class grows monotonically downstream, so the incision itself leans
/// the long profile downhill. That is the mechanism that turns "channels descend" from a hope into
/// a tendency the survey can measure.
fn valley_half_width(class: u8) -> i32 {
    (3 + i32::from(class.saturating_sub(CHANNEL_CLASS_MIN)) * 3).min(VALLEY_RADIUS)
}

fn channel_depth(class: u8) -> i32 {
    2 + i32::from(class.saturating_sub(CHANNEL_CLASS_MIN)) * 3
}

fn river_depth(class: u8) -> i32 {
    i32::from(class)
}

fn river_half_width(class: u8) -> i32 {
    i32::from(class.saturating_sub(CHANNEL_CLASS_MIN)) / 2
}

/// Catchment in cells to a discharge class. Logarithmic, base five: 2,048 cells is a stream and
/// 32 million is a continental river, which is the range the classes have to stay distinct across.
pub fn discharge_class(catchment_cells: u64) -> u8 {
    let mut class = 0u8;
    let mut threshold = 2_048u64;
    while class < 7 && catchment_cells >= threshold {
        class += 1;
        threshold = threshold.saturating_mul(5);
    }
    class
}

/// Whether a spanning-tree edge is preferred. Lower is grown first.
///
/// The hash makes the network dendritic rather than radial; the climb term makes it hug low ground
/// before it climbs, which is what turns a maze into something that reads as a catchment.
fn spine_edge_cost(seed: u32, from: (i32, i32), to: (i32, i32)) -> i64 {
    let hash = i64::from(coordinate_hash(seed ^ OCT_SPINE, from.0 + to.0, from.1 + to.1) >> 8);
    let climb =
        (continental_mq(seed, to.0, to.1) - continental_mq(seed, from.0, from.1)).max(0) as i64;
    hash + climb * 4
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

/// Builds a province's channel network. Bounded: 256 nodes, four faces, and one rasterised path
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
                    continental_mq(seed, node.cell.0, node.cell.1),
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

    // Prim from the root: every node ends with a parent chain that reaches the root, so the
    // network is a tree by construction and the accumulation below can be a single reverse pass.
    let mut in_tree = vec![false; count];
    let mut order: Vec<usize> = Vec::with_capacity(count);
    let mut frontier: BinaryHeap<Reverse<(i64, usize, usize)>> = BinaryHeap::new();
    in_tree[root] = true;
    order.push(root);
    push_spine_edges(seed, &nodes, side, root, &in_tree, &mut frontier);
    while let Some(Reverse((_, slot, parent))) = frontier.pop() {
        if in_tree[slot] {
            continue;
        }
        in_tree[slot] = true;
        nodes[slot].parent = Some(parent);
        order.push(slot);
        push_spine_edges(seed, &nodes, side, slot, &in_tree, &mut frontier);
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
        if class_of[slot] >= CHANNEL_CLASS_MIN
            && !has_channel_child[slot]
            && moisture(seed, cell.0, cell.1) > SPRING_MOISTURE
            && continental_mq(seed, cell.0, cell.1) > SEA_LEVEL_QUANTA
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
    let mut mark = |cell: (i32, i32), class: u8, wet: bool| {
        if class < CHANNEL_CLASS_MIN {
            return;
        }
        let entry = channels.entry(cell).or_insert(Channel { class, wet });
        // Where two branches share a cell the wider one owns it, so a confluence never narrows.
        if class > entry.class {
            entry.class = class;
        }
        entry.wet |= wet;
    };

    for slot in 0..count {
        let node = nodes[slot];
        mark(node.cell, class_of[slot], node.wet);
        if let Some(parent) = node.parent {
            for cell in axial_line(node.cell, nodes[parent].cell) {
                mark(cell, class_of[slot], node.wet);
            }
        }
    }

    // Both sides of an active seam draw a stub to the shared pour point, so the channel crosses
    // rather than stopping four cells short on each side of every province boundary.
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
        for cell in axial_line(node.cell, own_cell) {
            mark(cell, class_of[slot], node.wet);
        }
    }

    let springs = nodes
        .iter()
        .filter(|node| node.spring)
        .map(|node| node.cell)
        .collect();
    Spine { channels, springs }
}

fn push_spine_edges(
    seed: u32,
    nodes: &[SpineNode],
    side: i32,
    slot: usize,
    in_tree: &[bool],
    frontier: &mut BinaryHeap<Reverse<(i64, usize, usize)>>,
) {
    let i = slot as i32 % side;
    let j = slot as i32 / side;
    for (dq, dr) in DIRECTIONS {
        let (ni, nj) = (i + dq, j + dr);
        if ni < 0 || nj < 0 || ni >= side || nj >= side {
            continue;
        }
        let next = (nj * side + ni) as usize;
        if in_tree[next] {
            continue;
        }
        let cost = spine_edge_cost(seed, nodes[slot].cell, nodes[next].cell);
        frontier.push(Reverse((cost, next, slot)));
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

/// The hexes a straight line between two cells passes through.
fn axial_line(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let steps = axial_distance(from, to);
    if steps == 0 {
        return vec![from];
    }
    let den = i64::from(steps);
    let mut cells = Vec::with_capacity(steps as usize + 1);
    for step in 0..=steps {
        let t = i64::from(step);
        let q = i64::from(from.0) * (den - t) + i64::from(to.0) * t;
        let r = i64::from(from.1) * (den - t) + i64::from(to.1) * t;
        cells.push(cube_round_num(q, r, -q - r, den));
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
    /// The channel each domain cell is closest to under the incision solve, as a domain index, or
    /// `usize::MAX` where no channel reaches.
    nearest_channel: Vec<usize>,
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

        // 2. Cut the valleys. A widest-first sweep: pop the strongest remaining influence, keep it
        // if it beats what a cell already has, and hand the weakened influence to the neighbours.
        // Each cell settles once, at its maximum, which is what makes the result independent of
        // which channel happened to be visited first.
        //
        // The cut is a constant depth per class, which is why it shapes the valley sides but does
        // nothing for the long profile. Carving the whole flow tree this way was measured and made
        // the closed-basin count worse, not better — see `docs/BENCHMARKS.md`. Grading the profile
        // to a descending floor elevation is slice 2's work, because it changes what head means.
        let mut incision = vec![0i32; cells];
        let mut frontier: BinaryHeap<(i32, i32, i32, i32, usize)> = BinaryHeap::new();
        for (&cell, &channel) in &province.channels {
            let Some(index) = province.domain_index(cell.0, cell.1) else {
                continue;
            };
            let depth = channel_depth(channel.class) * MQ as i32;
            let rate = depth / valley_half_width(channel.class).max(1);
            frontier.push((depth, cell.0, cell.1, rate, index));
        }
        while let Some((value, _, _, rate, index)) = frontier.pop() {
            if value <= incision[index] {
                continue;
            }
            incision[index] = value;
            let (q, r) = province.domain_cell(index);
            let next = value - rate;
            if next <= 0 {
                continue;
            }
            for (dq, dr) in DIRECTIONS {
                if let Some(neighbour) = province.domain_index(q + dq, r + dr) {
                    if next > incision[neighbour] {
                        frontier.push((next, q + dq, r + dr, rate, neighbour));
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
            let (source_q, source_r) = province.domain_cell(source);
            let class = province
                .channels
                .get(&(source_q, source_r))
                .map_or(0, |channel| channel.class);
            if distance >= river_half_width(class) {
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

        // 4. Finished height. Both terms are already milli-quanta, so nothing is rounded here.
        for index in 0..cells {
            province.head_mq[index] -= incision[index];
        }

        // 5. Flow: the strictly lower neighbour under one total order. No fill, so there is no way
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

        // 6. Close what is left over. A pit is a real basin; the flood finds the rim it spills at,
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
        if !channel.wet {
            return Water::Dry;
        }
        let surface = province.head_mq[source] + river_depth(channel.class) * MQ as i32;
        if surface - head >= MQ as i32 {
            Water::River {
                depth: quanta(surface - head),
                class: channel.class,
            }
        } else {
            Water::Dry
        }
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

/// What a seed's landscape actually contains, counted rather than described.
///
/// Every claim the Phase 8 brief makes about drainage is a claim about proportions or invariants,
/// and both are things to be measured. `cycles` and `uphill_edges` are here to be zero; if they
/// are ever not, the model has been falsified and no amount of tuning is the answer.
#[derive(Clone, Debug)]
pub struct TerraSurvey {
    pub seed: u32,
    /// The province the square is centred on. Recorded because "0 walks reached the sea" means
    /// something entirely different inland than it does on a coast.
    pub centre: (i32, i32),
    pub provinces: u32,
    pub cells: u64,
    pub min_quanta: i32,
    pub max_quanta: i32,
    pub mean_quanta: i32,
    /// Neighbour height differences, bucketed by height quanta: 0, 1, 2-3, 4-7, 8-15, 16+.
    pub slope_histogram: [u64; 6],
    /// Neighbour pairs a player could step between under
    /// [`crate::scale::MAX_WALK_STEP_QUANTA`], and pairs a building pad could span under
    /// [`crate::scale::MAX_BUILD_STEP_QUANTA`].
    ///
    /// These are the numbers that decide whether the scale contract and the generator agree. A
    /// world can satisfy every drainage invariant and still be unplayable, and the only way that
    /// shows up is by asking what fraction of it a person can walk across.
    pub walkable_edges: u64,
    pub buildable_edges: u64,
    pub total_edges: u64,
    pub springs: u32,
    pub lakes: u32,
    pub lake_cells: u64,
    pub frontier_basins: u32,
    pub cycles: u64,
    pub uphill_edges: u64,
    /// Channel cells per discharge class.
    pub discharge_histogram: [u64; 8],
    pub sea_cells: u64,
    pub lake_water_cells: u64,
    pub river_cells: u64,
    /// Where a downstream walk ended, over a sampled set of starts.
    pub walks: u64,
    pub reached_sea: u64,
    pub reached_lake: u64,
    pub reached_frontier: u64,
    /// Walks that were still running when they left the surveyed square. Not a failure: a river
    /// crossing the edge of the sample is a river, and following it would mean solving provinces
    /// the survey never asked about.
    pub left_survey: u64,
    pub walk_budget_exhausted: u64,
    pub longest_walk: u32,
    pub solve_micros: u128,
    pub sweep_micros: u128,
}

impl TerraSurvey {
    pub fn water_per_mille(&self) -> u64 {
        if self.cells == 0 {
            return 0;
        }
        (self.sea_cells + self.lake_water_cells + self.river_cells) * 1_000 / self.cells
    }

    pub fn walkable_per_mille(&self) -> u64 {
        self.walkable_edges * 1_000 / self.total_edges.max(1)
    }

    pub fn buildable_per_mille(&self) -> u64 {
        self.buildable_edges * 1_000 / self.total_edges.max(1)
    }

    /// Channel cells as a share of the sample: the drainage density, which is the number that
    /// separates a river network from a crazed glaze.
    pub fn channel_per_mille(&self) -> u64 {
        self.discharge_histogram.iter().sum::<u64>() * 1_000 / self.cells.max(1)
    }

    /// The invariants the brief names as acceptance. A survey that fails this has falsified the
    /// model rather than found a bug to paper over.
    pub fn invariants_hold(&self) -> bool {
        self.cycles == 0 && self.uphill_edges == 0 && self.walk_budget_exhausted == 0
    }
}

/// How far a downstream walk is allowed to run before the survey calls it unterminated.
const WALK_BUDGET: u32 = 20_000;

/// Surveys a square of `span` by `span` provinces, with the origin province at its centre.
/// Surveys a square of provinces centred on the origin.
pub fn survey(seed: u32, span: i32) -> TerraSurvey {
    survey_at(seed, span, (0, 0))
}

/// Surveys a square of provinces centred anywhere.
///
/// The origin is not a representative place. Whether it is mountain, plain or seabed is a property
/// of the seed, and a survey that only ever looks there will report "no walk reached the sea" for a
/// sample with no sea in it and call that a drainage result. [`coast_province`] finds somewhere the
/// question can actually be asked.
pub fn survey_at(seed: u32, span: i32, centre: (i32, i32)) -> TerraSurvey {
    let span = span.max(1);
    let half = (span - 1) / 2;
    let (cq, cr) = centre;
    let mut terra = Terra::new(seed);

    let solve_started = Instant::now();
    let mut provinces = Vec::new();
    for pr in -half..(span - half) {
        for pq in -half..(span - half) {
            provinces.push((cq + pq, cr + pr));
            terra.province(cq + pq, cr + pr);
        }
    }
    let solve_micros = solve_started.elapsed().as_micros();

    let sweep_started = Instant::now();
    let mut result = TerraSurvey {
        seed,
        centre,
        provinces: provinces.len() as u32,
        cells: 0,
        min_quanta: i32::MAX,
        max_quanta: i32::MIN,
        mean_quanta: 0,
        slope_histogram: [0; 6],
        walkable_edges: 0,
        buildable_edges: 0,
        total_edges: 0,
        springs: 0,
        lakes: 0,
        lake_cells: 0,
        frontier_basins: 0,
        cycles: 0,
        uphill_edges: 0,
        discharge_histogram: [0; 8],
        sea_cells: 0,
        lake_water_cells: 0,
        river_cells: 0,
        walks: 0,
        reached_sea: 0,
        reached_lake: 0,
        reached_frontier: 0,
        left_survey: 0,
        walk_budget_exhausted: 0,
        longest_walk: 0,
        solve_micros,
        sweep_micros: 0,
    };

    let mut height_total: i64 = 0;
    for &(pq, pr) in &provinces {
        let province = terra.province(pq, pr);
        result.springs += province.springs().len() as u32;
        result.lakes += province.lakes().len() as u32;
        for lake in province.lakes() {
            result.lake_cells += u64::from(lake.cells);
        }
        let (origin_q, origin_r) = province_origin(pq, pr);
        for y in 0..PROVINCE_CELL {
            for x in 0..PROVINCE_CELL {
                let (q, r) = (origin_q + x, origin_r + y);
                let head = province.head(q, r).expect("own cell");
                result.cells += 1;
                height_total += i64::from(head);
                result.min_quanta = result.min_quanta.min(head);
                result.max_quanta = result.max_quanta.max(head);

                for (dq, dr) in DIRECTIONS {
                    if let Some(neighbour) = province.head(q + dq, r + dr) {
                        let step = (head - neighbour).abs();
                        result.slope_histogram[slope_bucket(step)] += 1;
                        result.total_edges += 1;
                        if step <= crate::scale::MAX_WALK_STEP_QUANTA {
                            result.walkable_edges += 1;
                        }
                        if step <= crate::scale::MAX_BUILD_STEP_QUANTA {
                            result.buildable_edges += 1;
                        }
                    }
                }

                if let Some(channel) = province.channel(q, r) {
                    result.discharge_histogram[usize::from(channel.class)] += 1;
                }

                match province.flow(q, r) {
                    Some(Flow::To(direction)) => {
                        let (dq, dr) = DIRECTIONS[direction as usize];
                        let (nq, nr) = (q + dq, r + dr);
                        let neighbour = province.head(nq, nr).expect("halo covers one ring");
                        if neighbour > head {
                            result.uphill_edges += 1;
                        }
                        // Strict decrease in one total order is what forbids a cycle, so the
                        // survey checks the order rather than walking every chain to prove it.
                        // The order is the one flow is decided in, milli-quanta, because two cells
                        // can share a published quantum without being at the same height.
                        let here_mq = province.head_mq(q, r).expect("own cell");
                        let there_mq = province.head_mq(nq, nr).expect("halo covers one ring");
                        if (there_mq, nq, nr) >= (here_mq, q, r) {
                            result.cycles += 1;
                        }
                    }
                    Some(Flow::Frontier) => result.frontier_basins += 1,
                    _ => {}
                }
            }
        }
    }
    if result.cells > 0 {
        result.mean_quanta = (height_total / result.cells as i64) as i32;
    }

    // Water and walk termination, sampled: every 16th cell in each direction, which is 1/256 of
    // the sweep and still tens of thousands of starts.
    for &(pq, pr) in &provinces {
        let (origin_q, origin_r) = province_origin(pq, pr);
        for y in (0..PROVINCE_CELL).step_by(4) {
            for x in (0..PROVINCE_CELL).step_by(4) {
                let (q, r) = (origin_q + x, origin_r + y);
                match terra.water(q, r) {
                    Water::Sea { .. } => result.sea_cells += 16,
                    Water::Lake { .. } => result.lake_water_cells += 16,
                    Water::River { .. } => result.river_cells += 16,
                    Water::Dry => {}
                }
            }
        }
        for y in (0..PROVINCE_CELL).step_by(16) {
            for x in (0..PROVINCE_CELL).step_by(16) {
                let (mut q, mut r) = (origin_q + x, origin_r + y);
                result.walks += 1;
                let mut steps = 0u32;
                loop {
                    // Stopping at the sample's edge is what keeps the survey's cost the size of
                    // the square it was asked about: a walk that followed a river out of the
                    // sample would solve provinces nobody asked to see.
                    let (wq, wr) = province_of(q, r);
                    if wq < cq - half
                        || wr < cr - half
                        || wq >= cq + span - half
                        || wr >= cr + span - half
                    {
                        result.left_survey += 1;
                        break;
                    }
                    if terra.head(q, r) < SEA_LEVEL_QUANTA {
                        result.reached_sea += 1;
                        break;
                    }
                    match terra.flow(q, r) {
                        Flow::To(direction) => {
                            let (dq, dr) = DIRECTIONS[direction as usize];
                            q += dq;
                            r += dr;
                            steps += 1;
                            if steps >= WALK_BUDGET {
                                result.walk_budget_exhausted += 1;
                                break;
                            }
                        }
                        Flow::Lake(_) => {
                            result.reached_lake += 1;
                            break;
                        }
                        Flow::Frontier => {
                            result.reached_frontier += 1;
                            break;
                        }
                    }
                }
                result.longest_walk = result.longest_walk.max(steps);
            }
        }
    }
    result.sweep_micros = sweep_started.elapsed().as_micros();
    result
}

fn slope_bucket(difference: i32) -> usize {
    match difference {
        0 => 0,
        1 => 1,
        2..=3 => 2,
        4..=7 => 3,
        8..=15 => 4,
        _ => 5,
    }
}

/// Height quanta as a readable metre figure, to one decimal place.
fn metres(quanta: i32) -> String {
    let tenths = i64::from(quanta) * i64::from(crate::scale::HEIGHT_QUANTUM_MM) / 100;
    format!("{}.{}", tenths / 10, (tenths % 10).abs())
}

pub fn format_report(survey: &TerraSurvey) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "terra prototype | seed {} | centre ({},{}) | {} provinces | {} cells\n",
        survey.seed, survey.centre.0, survey.centre.1, survey.provinces, survey.cells
    ));
    out.push_str(&format!(
        "  elevation      {} m to {} m, mean {} m\n",
        metres(survey.min_quanta),
        metres(survey.max_quanta),
        metres(survey.mean_quanta)
    ));
    let labels = ["0", "1", "2-3", "4-7", "8-15", "16+"];
    let edges: u64 = survey.slope_histogram.iter().sum::<u64>().max(1);
    out.push_str("  slope (quanta between neighbours)\n");
    for (label, count) in labels.iter().zip(survey.slope_histogram.iter()) {
        out.push_str(&format!(
            "    {label:>5}  {count:>12}  {:>4} per mille\n",
            count * 1_000 / edges
        ));
    }
    out.push_str(&format!(
        "  terrain        {} per mille walkable at {} quanta, {} per mille buildable at {}\n",
        survey.walkable_per_mille(),
        crate::scale::MAX_WALK_STEP_QUANTA,
        survey.buildable_per_mille(),
        crate::scale::MAX_BUILD_STEP_QUANTA
    ));
    out.push_str(&format!(
        "  discharge class (channel cells, {} per mille of the sample)\n",
        survey.channel_per_mille()
    ));
    for (class, count) in survey.discharge_histogram.iter().enumerate() {
        if *count == 0 {
            continue;
        }
        out.push_str(&format!(
            "    {class:>5}  {count:>12}  valley half-width {} cells, river depth {} m\n",
            valley_half_width(class as u8),
            metres(river_depth(class as u8))
        ));
    }
    out.push_str(&format!(
        "  hydrology      {} springs, {} lakes over {} cells, {} frontier basins\n",
        survey.springs, survey.lakes, survey.lake_cells, survey.frontier_basins
    ));
    out.push_str(&format!(
        "  water          {} per mille wet (sea {}, lake {}, river {})\n",
        survey.water_per_mille(),
        survey.sea_cells,
        survey.lake_water_cells,
        survey.river_cells
    ));
    out.push_str(&format!(
        "  invariants     {} cycles, {} uphill edges\n",
        survey.cycles, survey.uphill_edges
    ));
    out.push_str(&format!(
        "  drainage walks {} starts: {} to sea, {} to lake, {} to frontier, {} off the sample, {} unterminated, longest {}\n",
        survey.walks,
        survey.reached_sea,
        survey.reached_lake,
        survey.reached_frontier,
        survey.left_survey,
        survey.walk_budget_exhausted,
        survey.longest_walk
    ));
    let per_province = survey.solve_micros / u128::from(survey.provinces.max(1));
    out.push_str(&format!(
        "  cost           {} ms to solve ({} ms per province), {} ms to sweep\n",
        survey.solve_micros / 1_000,
        per_province / 1_000,
        survey.sweep_micros / 1_000
    ));
    out
}

pub fn format_json(survey: &TerraSurvey) -> String {
    serde_json::json!({
        "seed": survey.seed,
        "centre_pq": survey.centre.0,
        "centre_pr": survey.centre.1,
        "provinces": survey.provinces,
        "cells": survey.cells,
        "min_quanta": survey.min_quanta,
        "max_quanta": survey.max_quanta,
        "mean_quanta": survey.mean_quanta,
        "slope_histogram": survey.slope_histogram,
        "walkable_per_mille": survey.walkable_per_mille(),
        "buildable_per_mille": survey.buildable_per_mille(),
        "channel_per_mille": survey.channel_per_mille(),
        "springs": survey.springs,
        "lakes": survey.lakes,
        "lake_cells": survey.lake_cells,
        "frontier_basins": survey.frontier_basins,
        "cycles": survey.cycles,
        "uphill_edges": survey.uphill_edges,
        "discharge_histogram": survey.discharge_histogram,
        "sea_cells": survey.sea_cells,
        "lake_water_cells": survey.lake_water_cells,
        "river_cells": survey.river_cells,
        "water_per_mille": survey.water_per_mille(),
        "walks": survey.walks,
        "reached_sea": survey.reached_sea,
        "reached_lake": survey.reached_lake,
        "reached_frontier": survey.reached_frontier,
        "left_survey": survey.left_survey,
        "walk_budget_exhausted": survey.walk_budget_exhausted,
        "longest_walk": survey.longest_walk,
        "solve_micros": survey.solve_micros,
        "sweep_micros": survey.sweep_micros,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: u32 = 0x5EED_A17E;

    /// A modest patch, so the invariant tests cover several provinces and a seam in each
    /// direction without making `cargo test` slow.
    fn patch() -> Vec<(i32, i32)> {
        let mut cells = Vec::new();
        for r in -PROVINCE_CELL..(2 * PROVINCE_CELL) {
            for q in -PROVINCE_CELL..(2 * PROVINCE_CELL) {
                cells.push((q, r));
            }
        }
        cells
    }

    /// The macro graph is a forest: every edge strictly decreases `(rank, pq, pr)`, so following
    /// outlets from anywhere terminates rather than looping.
    #[test]
    fn province_outlets_strictly_descend() {
        for pr in -6..6 {
            for pq in -6..6 {
                if let Outlet::Province { pq: nq, pr: nr } = province_outlet(SEED, pq, pr) {
                    let here = (province_rank(SEED, pq, pr), pq, pr);
                    let there = (province_rank(SEED, nq, nr), nq, nr);
                    assert!(
                        there < here,
                        "province ({pq},{pr}) drains to a higher outlet"
                    );
                }
            }
        }
    }

    #[test]
    fn province_outlet_chains_terminate() {
        for pr in -6..6 {
            for pq in -6..6 {
                let mut current = (pq, pr);
                let mut steps = 0;
                while let Outlet::Province { pq: nq, pr: nr } =
                    province_outlet(SEED, current.0, current.1)
                {
                    current = (nq, nr);
                    steps += 1;
                    assert!(
                        steps < 4_096,
                        "outlet chain from ({pq},{pr}) did not terminate"
                    );
                }
            }
        }
    }

    /// Both sides of a seam name the same pour point. Without this a channel would arrive at a
    /// different cell depending on which province was asked, and every province boundary in the
    /// world would show a tear.
    #[test]
    fn seam_pour_points_agree_from_both_sides() {
        for pr in -3..3 {
            for pq in -3..3 {
                for (dq, dr) in PROVINCE_FACES {
                    let neighbour = (pq + dq, pr + dr);
                    let (mine, theirs) = seam_pour(SEED, (pq, pr), neighbour);
                    let (their_side, my_side) = seam_pour(SEED, neighbour, (pq, pr));
                    assert_eq!(mine, my_side);
                    assert_eq!(theirs, their_side);
                    assert_eq!(province_of(mine.0, mine.1), (pq, pr));
                    assert_eq!(province_of(theirs.0, theirs.1), neighbour);
                    assert_eq!(axial_distance(mine, theirs), 1);
                }
            }
        }
    }

    /// The claim caching is allowed to make, and the only one: the same answer either way.
    #[test]
    fn cached_and_uncached_height_agree() {
        let mut terra = Terra::new(SEED);
        for (q, r) in [
            (0, 0),
            (-1, -1),
            (PROVINCE_CELL - 1, 0),
            (PROVINCE_CELL, 0),
            (0, PROVINCE_CELL - 1),
            (0, PROVINCE_CELL),
            (-PROVINCE_CELL, -PROVINCE_CELL),
            (3 * PROVINCE_CELL + 7, -2 * PROVINCE_CELL - 5),
        ] {
            assert_eq!(
                terra.head(q, r),
                Terra::head_uncached(SEED, q, r),
                "cached and uncached height disagree at ({q},{r})"
            );
        }
    }

    /// Query order cannot matter. Two caches walked in opposite directions must agree cell for
    /// cell, height, flow and water alike.
    #[test]
    fn query_order_does_not_change_the_world() {
        let cells = patch();
        let mut forward = Terra::new(SEED);
        let mut backward = Terra::new(SEED);
        let mut readings = Vec::with_capacity(cells.len());
        for &(q, r) in &cells {
            readings.push((forward.head(q, r), forward.flow(q, r), forward.water(q, r)));
        }
        for (index, &(q, r)) in cells.iter().enumerate().rev() {
            let expected = readings[index];
            assert_eq!(
                (
                    backward.head(q, r),
                    backward.flow(q, r),
                    backward.water(q, r)
                ),
                expected,
                "reverse query order changed ({q},{r})"
            );
        }
    }

    /// A province's own cells are computed identically whether the province was solved on its own
    /// or after its neighbours. This is the seam version of the test above: it is what lets the
    /// halo be an implementation detail rather than a place results are approximate.
    #[test]
    fn a_seam_reads_the_same_from_either_province() {
        let mut alone = Terra::new(SEED);
        let border = alone.province(0, 0);
        let mut surrounded = Terra::new(SEED);
        for pr in -1..=1 {
            for pq in -1..=1 {
                surrounded.province(pq, pr);
            }
        }
        let after = surrounded.province(0, 0);
        for r in 0..PROVINCE_CELL {
            for q in 0..PROVINCE_CELL {
                assert_eq!(border.head(q, r), after.head(q, r), "height at ({q},{r})");
                assert_eq!(border.flow(q, r), after.flow(q, r), "flow at ({q},{r})");
            }
        }
    }

    /// The drainage invariants the brief names as acceptance, over a real patch of world.
    #[test]
    fn drainage_never_runs_uphill_and_never_cycles() {
        let mut terra = Terra::new(SEED);
        for (q, r) in patch() {
            let here = terra.head(q, r);
            if let Some((nq, nr)) = terra.downstream(q, r) {
                let there = terra.head(nq, nr);
                assert!(there <= here, "({q},{r}) at {here} flows uphill to {there}");
                let (here_mq, there_mq) = (terra.head_mq(q, r), terra.head_mq(nq, nr));
                assert!(
                    (there_mq, nq, nr) < (here_mq, q, r),
                    "({q},{r}) flows to an equal-or-greater key, which would admit a cycle"
                );
            }
        }
    }

    /// Every path that is not a lake reaches a declared outlet: the sea, a lake, or an honestly
    /// reported frontier basin. Nothing wanders forever.
    #[test]
    fn every_path_reaches_a_declared_outlet() {
        let mut terra = Terra::new(SEED);
        for r in (-PROVINCE_CELL..(2 * PROVINCE_CELL)).step_by(7) {
            for q in (-PROVINCE_CELL..(2 * PROVINCE_CELL)).step_by(7) {
                let (mut cq, mut cr) = (q, r);
                let mut steps = 0u32;
                loop {
                    if terra.head(cq, cr) < SEA_LEVEL_QUANTA {
                        break;
                    }
                    match terra.flow(cq, cr) {
                        Flow::To(direction) => {
                            let (dq, dr) = DIRECTIONS[direction as usize];
                            cq += dq;
                            cr += dr;
                        }
                        Flow::Lake(_) | Flow::Frontier => break,
                    }
                    steps += 1;
                    assert!(
                        steps < WALK_BUDGET,
                        "the path from ({q},{r}) never terminated"
                    );
                }
            }
        }
    }

    /// A retained lake reports the rim it spills over, and that rim stands at or above every cell
    /// the lake covers. A lake surface below its own bed would be the model lying about water.
    #[test]
    fn lakes_report_a_spill_level_above_their_bed() {
        let mut terra = Terra::new(SEED);
        let mut found = 0;
        for pr in -1..=1 {
            for pq in -1..=1 {
                let province = terra.province(pq, pr);
                for lake in province.lakes() {
                    assert!(lake.cells > 0);
                }
                let (origin_q, origin_r) = province_origin(pq, pr);
                for r in origin_r..(origin_r + PROVINCE_CELL) {
                    for q in origin_q..(origin_q + PROVINCE_CELL) {
                        if let Some(Flow::Lake(id)) = province.flow(q, r) {
                            let lake = province.lake(id);
                            let head = province.head_mq(q, r).expect("own cell");
                            assert!(
                                lake.spill_mq >= head,
                                "lake surface {} is below its bed {head} at ({q},{r})",
                                lake.spill_mq
                            );
                            found += 1;
                        }
                    }
                }
            }
        }
        // The prototype is worth nothing if it produces no basins at all; a landscape with no
        // closed depression anywhere has been smoothed until it stopped being terrain.
        assert!(found > 0, "no lake cells anywhere in nine provinces");
    }

    /// Springs sit above sea level, on damp ground, at the head of a channel and inside their own
    /// province.
    ///
    /// Deliberately not "every province has a spring". A spring needs a channel head, and a
    /// channel needs [`CHANNEL_CLASS_MIN`] — about five hectares of catchment — so a province gets
    /// roughly one. Nine provinces finding none is ordinary, which is why the sample is 49 and the
    /// assertion is about the predicate rather than the density.
    #[test]
    fn springs_are_wet_high_ground() {
        let (cq, cr) = highest_province(SEED);
        let mut found = 0;
        for pr in cr - 3..=cr + 3 {
            for pq in cq - 3..=cq + 3 {
                let spine = build_spine(SEED, pq, pr, &[]);
                for &(q, r) in &spine.springs {
                    assert_eq!(
                        province_of(q, r),
                        (pq, pr),
                        "a spring outside its own province"
                    );
                    assert!(moisture(SEED, q, r) > SPRING_MOISTURE, "a dry spring");
                    assert!(
                        continental_mq(SEED, q, r) > SEA_LEVEL_QUANTA,
                        "a spring below sea level"
                    );
                    // A spring is the top of a channel, so the cell it names has to be one.
                    let channel = spine
                        .channels
                        .get(&(q, r))
                        .expect("a spring off the channel");
                    assert!(channel.class >= CHANNEL_CLASS_MIN);
                    assert!(channel.wet, "a spring that starts no water");
                    found += 1;
                }
            }
        }
        assert!(found > 0, "no springs anywhere in forty-nine provinces");
    }

    /// The province with the most height in it, within a couple of continental wavelengths.
    ///
    /// The origin is not land. At [`SEED`] it sits about 300 m under water, which is the generator
    /// working — a world with a sea in it has to put some seeds in the sea. Tests that are about
    /// hills, springs and rivers have to say where the hills are rather than assuming the origin.
    fn highest_province(seed: u32) -> (i32, i32) {
        let reach = CONTINENT_PROVINCES * 2;
        let mut best = ((0, 0), i32::MIN);
        for pr in (-reach..=reach).step_by(2) {
            for pq in (-reach..=reach).step_by(2) {
                let (q, r) = province_origin(pq, pr);
                let height = continental_mq(seed, q, r);
                if height > best.1 {
                    best = ((pq, pr), height);
                }
            }
        }
        assert!(
            best.1 > SEA_LEVEL_QUANTA,
            "no land within two continental wavelengths of the origin"
        );
        best.0
    }

    /// Discharge classes are monotone in catchment and saturate rather than overflowing.
    #[test]
    fn discharge_classes_are_monotone() {
        let mut last = 0;
        for exponent in 0..40 {
            let class = discharge_class(1u64 << exponent);
            assert!(class >= last);
            last = class;
        }
        assert_eq!(discharge_class(u64::MAX), 7);
        assert_eq!(discharge_class(0), 0);
    }

    /// A valley is never wider than the halo the solve computes, which is what makes a cell's
    /// height complete before anyone outside the province reads it.
    #[test]
    fn no_valley_is_wider_than_the_halo() {
        for class in 0..=7u8 {
            assert!(valley_half_width(class) <= VALLEY_RADIUS);
            assert!(river_half_width(class) <= VALLEY_RADIUS);
        }
        assert!(HALO > VALLEY_RADIUS);
    }

    /// Solving one cell costs a bounded number of provinces, and reading a whole province does not
    /// pull in a continent. Nine is the nine that a halo can reach.
    #[test]
    fn one_cell_costs_a_bounded_number_of_provinces() {
        let mut terra = Terra::new(SEED);
        terra.head(0, 0);
        assert_eq!(terra.provinces_solved(), 1);
        for r in 0..PROVINCE_CELL {
            for q in 0..PROVINCE_CELL {
                terra.head(q, r);
            }
        }
        assert_eq!(terra.provinces_solved(), 1);
    }

    /// A different seed is a different world; the same seed is the same world twice.
    #[test]
    fn seeds_separate_worlds_and_repeat_them() {
        let mut first = Terra::new(SEED);
        let mut again = Terra::new(SEED);
        let mut other = Terra::new(SEED ^ 0x9999);
        let mut differences = 0;
        for r in 0..64 {
            for q in 0..64 {
                assert_eq!(first.head(q, r), again.head(q, r));
                if first.head(q, r) != other.head(q, r) {
                    differences += 1;
                }
            }
        }
        assert!(
            differences > 3_000,
            "two seeds produced nearly the same world"
        );
    }

    /// The survey is the falsification instrument, so it has to run and it has to report the
    /// invariants as clean. A single province keeps the test quick.
    #[test]
    fn the_survey_reports_clean_invariants() {
        let result = survey(SEED, 1);
        assert_eq!(result.provinces, 1);
        assert_eq!(result.cells, (PROVINCE_CELL * PROVINCE_CELL) as u64);
        assert_eq!(result.cycles, 0);
        assert_eq!(result.uphill_edges, 0);
        assert_eq!(result.walk_budget_exhausted, 0);
        assert!(result.invariants_hold());
        assert!(result.max_quanta > result.min_quanta);
        assert!(!format_report(&result).is_empty());
        assert!(format_json(&result).contains("\"uphill_edges\":0"));
    }

    /// Relief has to be worth the rescale: a world that is flat at 25 m² per cell has not earned
    /// the compatibility break. Kilometre-scale variation is the point of the phase.
    ///
    /// Measured on the bare height field rather than on a survey, because the claim is about the
    /// continental wavelength — 33 km — and no sample small enough to solve inside a test can span
    /// one. Three provinces is 2.1 km, six per cent of a wavelength; asking that for hundreds of
    /// metres of relief would only be asking for a noisier generator.
    #[test]
    fn the_landscape_has_relief_worth_the_rescale() {
        let step = PROVINCE_CELL;
        let reach = 32; // 32 provinces each way: 68 km, two continental wavelengths.
        let mut low = i32::MAX;
        let mut high = i32::MIN;
        for r in -reach..=reach {
            for q in -reach..=reach {
                let height = base_mq(SEED, q * step, r * step);
                low = low.min(height);
                high = high.max(height);
            }
        }
        let range = (high - low) / MQ as i32;
        // 1,200 quanta is 300 m. Below that the rescale buys nothing a band enum could not fake.
        assert!(
            range > 1_200,
            "only {range} quanta of relief across 68 km of the height field"
        );

        // And the relief has to be there locally too, or the world is a single smooth ramp with
        // nothing to walk around.
        let result = survey(SEED, 3);
        let local = result.max_quanta - result.min_quanta;
        assert!(
            local > 100,
            "only {local} quanta of relief across three provinces"
        );
    }
}
