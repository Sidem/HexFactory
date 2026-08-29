//! Sparse boundary construction on the hex *vertex* lattice. The same bounded transaction resolves
//! previews and commits.
//!
//! A boundary is a **chord**: a straight line between two corners of one hex. Three of a hex's
//! fifteen chords are the edges it shares with a neighbour — those are the only ones that existed
//! before this release, and they are stored under exactly the same identity they always were. The
//! other twelve run through the hex's interior, and they are what lets a wall hold one heading for
//! twenty segments instead of zig-zagging around hex centres.
//!
//! Every vertex of the lattice has twelve chords leaving it, one per thirty degrees, so a straight
//! run between two vertices is exact on twelve headings and never more than half a hex off the line
//! on any other. `DIRECTIONS` is untouched: adjacency is still six-sided, and always will be.
use super::*;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum BoundaryFamily {
    #[default]
    Fence,
    Wall,
}

#[derive(Clone, Deserialize)]
pub(super) struct BoundaryDefinition {
    pub id: DefinitionId,
    pub key: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub family: BoundaryFamily,
    pub gate: bool,
    #[serde(default)]
    pub unlock_technology_id: Option<TechnologyId>,
    pub construction_cost: Vec<Ingredient>,
}

/// The six corners of a hex, as world offsets from its centre. Index 0 is due north, then
/// clockwise, so corners `k + 1` and `k + 2` are the ends of the hex edge in `DIRECTIONS[k]`.
pub(super) const CORNERS: [(i32, i32); 6] = [
    (0, -HEX_RADIUS),
    (HEX_X / 2, -HEX_RADIUS / 2),
    (HEX_X / 2, HEX_RADIUS / 2),
    (0, HEX_RADIUS),
    (-HEX_X / 2, HEX_RADIUS / 2),
    (-HEX_X / 2, -HEX_RADIUS / 2),
];

/// How many chords one hex names. `0..3` are the edges this hex owns, `3..6` the other three edges,
/// always rewritten onto the neighbour that owns them, `6..12` the six short diagonals (corner `k`
/// to corner `k + 2`) and `12..15` the three long diagonals (corner `k` to corner `k + 3`).
const CHORDS: u8 = 15;

/// The longest run one selection may draw. Bounded before anything is priced, so a stray drag is
/// refused rather than costed, and comfortably above the twenty segments a straight run has to
/// reach on every heading.
const MAX_BOUNDARY_SEGMENTS: usize = 32;

/// The two corners a chord joins.
fn chord_corners(chord: u8) -> (u8, u8) {
    match chord {
        0..=5 => ((chord + 1) % 6, (chord + 2) % 6),
        6..=11 => (chord - 6, (chord - 4) % 6),
        _ => (chord - 12, chord - 9),
    }
}

/// The chord joining two distinct corners of one hex. Inverse of [`chord_corners`].
fn chord_between(from: u8, to: u8) -> u8 {
    match (to + 6 - from) % 6 {
        1 => (from + 5) % 6,
        5 => (to + 5) % 6,
        2 => 6 + from,
        4 => 6 + to,
        _ => 12 + from % 3,
    }
}

/// Where corner `corner` of hex `q, r` sits in world units.
pub(super) fn corner_world(q: i32, r: i32, corner: u8) -> (i32, i32) {
    let (x, y) = axial_world(q, r);
    let (dx, dy) = CORNERS[(corner % 6) as usize];
    (x + dx, y + dy)
}

fn distance2(a: (i32, i32), b: (i32, i32)) -> i64 {
    (i64::from(a.0) - i64::from(b.0)).pow(2) + (i64::from(a.1) - i64::from(b.1)).pow(2)
}

/// The lattice vertex nearest a world point, named by the hex the point falls in and one of its
/// corners. Every vertex belongs to three hexes; naming it from the containing hex keeps the answer
/// local and deterministic, and [`Segment::new`] folds the three spellings back together.
pub(super) fn nearest_corner(x: i32, y: i32) -> (i32, i32, u8) {
    let (q, r) = world_to_axial(x, y);
    let corner = (0..6u8)
        .min_by_key(|&corner| (distance2(corner_world(q, r, corner), (x, y)), corner))
        .unwrap_or(0);
    (q, r, corner)
}

/// The three hexes meeting at corner `corner` of hex `q, r`.
pub(super) fn corner_hexes(q: i32, r: i32, corner: u8) -> [(i32, i32); 3] {
    let k = (corner % 6) as usize;
    let a = DIRECTIONS[(k + 5) % 6];
    let b = DIRECTIONS[(k + 4) % 6];
    [
        (q, r),
        (q.saturating_add(a.0), r.saturating_add(a.1)),
        (q.saturating_add(b.0), r.saturating_add(b.1)),
    ]
}

/// Every atomic straight step out of one vertex: the twelve chords that start there, one per
/// thirty degrees. Derived from the three hexes meeting at the vertex rather than tabulated, so the
/// rose cannot drift out of step with [`CORNERS`].
fn corner_steps(q: i32, r: i32, corner: u8) -> Vec<((i32, i32, u8), Segment)> {
    let origin = corner_world(q, r, corner);
    let mut steps: Vec<((i32, i32, u8), Segment)> = Vec::with_capacity(12);
    for (hq, hr) in corner_hexes(q, r, corner) {
        let Some(from) = (0..6u8).find(|&k| corner_world(hq, hr, k) == origin) else {
            continue;
        };
        for to in (0..6u8).filter(|&to| to != from) {
            let Ok(segment) = Segment::new(hq, hr, chord_between(from, to)) else {
                continue;
            };
            if steps.iter().any(|(_, other)| *other == segment) {
                continue;
            }
            steps.push(((hq, hr, to), segment));
        }
    }
    steps
}

/// A straight run of chords between two lattice vertices.
///
/// Greedy on the twelve-heading rose: a step is only considered if it strictly closes the distance
/// to the far end, and among those the one deviating least from the straight line wins. On the
/// twelve exact headings the deviation is zero at every step, so the run is exactly straight; off
/// them it is the lattice's own best staircase. Strict closing makes the walk monotone, so it
/// always terminates and never revisits a vertex.
fn chord_chain(
    start: (i32, i32, u8),
    end: (i32, i32, u8),
    budget: usize,
) -> Result<Vec<Segment>, String> {
    let from = corner_world(start.0, start.1, start.2);
    let to = corner_world(end.0, end.1, end.2);
    let mut chain = Vec::new();
    let mut here = start;
    let mut at = from;
    while at != to {
        let remaining = distance2(at, to);
        let mut best: Option<((i128, i64), Segment, (i32, i32, u8), (i32, i32))> = None;
        for (target, segment) in corner_steps(here.0, here.1, here.2) {
            let next = corner_world(target.0, target.1, target.2);
            let closing = distance2(next, to);
            if closing >= remaining {
                continue;
            }
            let drift = ((i128::from(to.0) - i128::from(from.0))
                * (i128::from(next.1) - i128::from(from.1))
                - (i128::from(to.1) - i128::from(from.1))
                    * (i128::from(next.0) - i128::from(from.0)))
            .abs();
            if best
                .as_ref()
                .is_none_or(|(key, ..)| (drift, closing) < *key)
            {
                best = Some(((drift, closing), segment, target, next));
            }
        }
        let Some((_, segment, target, next)) = best else {
            return Err("No straight run reaches there from that anchor".into());
        };
        chain.push(segment);
        if chain.len() > budget {
            return Err(format!("Draw at most {budget} boundary segments at a time"));
        }
        here = target;
        at = next;
    }
    Ok(chain)
}

fn round_to(value: i32, step: i32) -> i32 {
    let half = step / 2;
    if value >= 0 {
        (value + half) / step * step
    } else {
        -((half - value) / step * step)
    }
}

/// The rectangle two picked vertices define, snapped so that all four of its corners are lattice
/// vertices and every side is an exactly straight run.
///
/// Columns are one hex wide. Rows follow the vertex ladder at a fixed world-x, which alternates
/// steps of one and two hex radii and so repeats every three: from an even corner the reachable
/// rises are `0` and `2r` modulo `3r`, from an odd corner `0` and `r`. Snapping to that ladder is
/// what makes "wall this rectangle" and "pave this rectangle" line up on the same anchors.
pub(super) fn yard_rect(
    start: (i32, i32, u8),
    end: (i32, i32, u8),
) -> Result<((i32, i32), (i32, i32)), String> {
    let (ax, ay) = corner_world(start.0, start.1, start.2);
    let (bx, by) = corner_world(end.0, end.1, end.2);
    let width = round_to(bx - ax, HEX_X);
    let rise = if start.2 % 2 == 0 {
        HEX_RADIUS * 2
    } else {
        HEX_RADIUS
    };
    let period = HEX_RADIUS * 3;
    let height = [0, rise]
        .into_iter()
        .map(|offset| round_to((by - ay) - offset, period) + offset)
        .min_by_key(|&candidate| ((candidate - (by - ay)).abs(), Reverse(candidate.abs())))
        .unwrap_or(0);
    if width == 0 || height == 0 {
        return Err("Drag out a rectangle at least one hex across".into());
    }
    Ok((
        ((ax + width).min(ax), (ay + height).min(ay)),
        ((ax + width).max(ax), (ay + height).max(ay)),
    ))
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Segment {
    pub q: i32,
    pub r: i32,
    /// Which chord of hex `q, r` this is. Saves written before the vertex lattice spell this field
    /// `direction` and only ever held `0..3`, which are the same three chords under the same
    /// identity — the alias loads them in place, unchanged and unmigrated.
    #[serde(alias = "direction")]
    pub chord: u8,
}

impl Segment {
    fn new(q: i32, r: i32, chord: u8) -> Result<Self, String> {
        // Keeps all world geometry exact and comfortably within i32 axial_world arithmetic.
        if q.abs_diff(0) > 100_000 || r.abs_diff(0) > 100_000 || chord >= CHORDS {
            return Err("Boundary target is outside the supported coordinate range".into());
        }
        if (3..6).contains(&chord) {
            let (dq, dr) = DIRECTIONS[chord as usize];
            if (q + dq).abs_diff(0) > 100_000 || (r + dr).abs_diff(0) > 100_000 {
                return Err("Boundary target is outside the supported coordinate range".into());
            }
            Ok(Self {
                q: q + dq,
                r: r + dr,
                chord: chord - 3,
            })
        } else {
            Ok(Self { q, r, chord })
        }
    }

    /// Whether this chord is a shared hex edge rather than a line through one hex's interior.
    fn is_edge(self) -> bool {
        self.chord < 3
    }

    /// The hexes this chord divides: two for a shared edge, one for an interior chord.
    fn hexes(self) -> ((i32, i32), Option<(i32, i32)>) {
        if self.is_edge() {
            let (dq, dr) = DIRECTIONS[self.chord as usize];
            (
                (self.q, self.r),
                Some((self.q.saturating_add(dq), self.r.saturating_add(dr))),
            )
        } else {
            ((self.q, self.r), None)
        }
    }

    pub(super) fn ends(self) -> ((i32, i32), (i32, i32)) {
        let (a, b) = chord_corners(self.chord);
        (
            corner_world(self.q, self.r, a),
            corner_world(self.q, self.r, b),
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct Boundary {
    #[serde(flatten)]
    pub segment: Segment,
    pub definition_id: DefinitionId,
    pub open: bool,
    /// Actual paid bill: sandbox construction never becomes a material source.
    pub paid: Vec<Ingredient>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum BoundaryAction {
    Build,
    Remove,
    Open,
    Close,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum BoundaryShape {
    /// A straight run from one lattice vertex to another.
    #[default]
    Line,
    /// The four straight sides of the rectangle two vertices define.
    Yard,
}

#[derive(Clone, Deserialize)]
pub(super) struct BoundaryEdit {
    pub q: i32,
    pub r: i32,
    /// Which corner of hex `q, r` the run starts on.
    pub corner: u8,
    pub to_q: i32,
    pub to_r: i32,
    pub to_corner: u8,
    #[serde(default)]
    pub shape: BoundaryShape,
    pub definition_id: DefinitionId,
    pub action: BoundaryAction,
}

#[derive(Serialize)]
pub(super) struct BoundaryPreview {
    pub segments: Vec<Segment>,
    pub changes: usize,
    pub cost: Vec<Ingredient>,
    pub refund: Vec<Ingredient>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub(super) struct BoundaryUndo {
    before: Vec<(Segment, Option<Boundary>)>,
    after: Vec<(Segment, Option<Boundary>)>,
}

struct BoundaryTransaction {
    preview: BoundaryPreview,
    undo: BoundaryUndo,
    inventory: BTreeMap<ItemId, u32>,
}

fn ingredients(items: &BTreeMap<ItemId, u32>) -> Vec<Ingredient> {
    items
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(&item_id, &quantity)| Ingredient { item_id, quantity })
        .collect()
}

fn bill(boundary: &Option<Boundary>, items: &mut BTreeMap<ItemId, u32>) {
    if let Some(boundary) = boundary {
        add_ingredients(items, &boundary.paid);
    }
}

/// Inclusive intersections close the vertex loophole for corner transport headings.
fn segments_cross(a: (i32, i32), b: (i32, i32), c: (i32, i32), d: (i32, i32)) -> bool {
    let cross = |p: (i32, i32), q: (i32, i32), r: (i32, i32)| -> i128 {
        (i128::from(q.0) - i128::from(p.0)) * (i128::from(r.1) - i128::from(p.1))
            - (i128::from(q.1) - i128::from(p.1)) * (i128::from(r.0) - i128::from(p.0))
    };
    a.0.min(b.0) <= c.0.max(d.0)
        && c.0.min(d.0) <= a.0.max(b.0)
        && a.1.min(b.1) <= c.1.max(d.1)
        && c.1.min(d.1) <= a.1.max(b.1)
        && cross(a, b, c).signum() * cross(a, b, d).signum() <= 0
        && cross(c, d, a).signum() * cross(c, d, b).signum() <= 0
}

fn near_segment(segment: Segment, p: (i32, i32), radius: i32) -> bool {
    let (a, b) = segment.ends();
    let dx = i128::from(b.0 - a.0);
    let dy = i128::from(b.1 - a.1);
    let px = i128::from(p.0) - i128::from(a.0);
    let py = i128::from(p.1) - i128::from(a.1);
    let length = dx * dx + dy * dy;
    let dot = px * dx + py * dy;
    let radius2 = i128::from(radius).pow(2);
    if dot <= 0 {
        return px * px + py * py <= radius2;
    }
    if dot >= length {
        return (px - dx).pow(2) + (py - dy).pow(2) <= radius2;
    }
    (px * dy - py * dx).pow(2) <= radius2 * length
}

impl Core {
    /// Every boundary keyed on one hex, in one ordered range scan. Chords are stored under the hex
    /// that owns them, so a hex and its six neighbours name every segment that can touch it however
    /// many boundaries the world holds.
    fn segments_in(&self, q: i32, r: i32) -> impl Iterator<Item = (&Segment, &Boundary)> {
        self.boundaries.range(
            Segment { q, r, chord: 0 }..=Segment {
                q,
                r,
                chord: CHORDS - 1,
            },
        )
    }

    pub(super) fn boundary_crosses_footprint(&self, footprint: &[Coordinate]) -> bool {
        footprint.iter().any(|cell| {
            // A chord through the interior runs across the machine's own floor, not beside it.
            self.segments_in(cell.q, cell.r)
                .any(|(segment, _)| !segment.is_edge())
                || (0..6).any(|direction| {
                    let (dq, dr) = DIRECTIONS[direction as usize];
                    footprint
                        .iter()
                        .any(|other| other.q == cell.q + dq && other.r == cell.r + dr)
                        && self.boundary_between(cell.q, cell.r, direction)
                })
        })
    }

    pub(super) fn boundary_between(&self, q: i32, r: i32, direction: u8) -> bool {
        Segment::new(q, r, direction).is_ok_and(|segment| self.boundaries.contains_key(&segment))
    }

    fn boundary_definition(&self, id: DefinitionId) -> Option<&BoundaryDefinition> {
        self.definitions.boundaries.iter().find(|d| d.id == id)
    }

    /// Memoizes a pure digest of source records. Cache presence is never saved or hashed;
    /// the uncached implementation is the checksum oracle, including after load and every edit.
    pub(super) fn boundary_state_hash(&self) -> u32 {
        if let Some(hash) = *self.boundary_hash_cache.borrow() {
            return hash;
        }
        let hash = self.uncached_boundary_hash();
        *self.boundary_hash_cache.borrow_mut() = Some(hash);
        hash
    }

    pub(super) fn uncached_boundary_hash(&self) -> u32 {
        let mut hash = 0x811c9dc5u32;
        hash_u64(&mut hash, self.boundaries.len() as u64);
        for b in self.boundaries.values() {
            hash_i32(&mut hash, b.segment.q);
            hash_i32(&mut hash, b.segment.r);
            hash_u32(&mut hash, u32::from(b.segment.chord));
            hash_u32(&mut hash, u32::from(b.definition_id));
            hash_u32(&mut hash, u32::from(b.open));
            hash_u32(&mut hash, b.paid.len() as u32);
            for i in &b.paid {
                hash_u32(&mut hash, u32::from(i.item_id));
                hash_u32(&mut hash, i.quantity);
            }
        }
        hash
    }

    pub(super) fn boundary_snapshot(&self) -> Vec<Boundary> {
        self.boundaries.values().cloned().collect()
    }

    /// Only a segment's local hex neighbourhood is visited, independent of world boundary count.
    pub(super) fn boundary_blocks_segment(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        if self.boundaries.is_empty() {
            return false;
        }
        let a = world_to_axial(from.0, from.1);
        let b = world_to_axial(to.0, to.1);
        for (q, r) in hex_line_any(a, b) {
            for (dq, dr) in [(0, 0)].into_iter().chain(DIRECTIONS) {
                for (segment, boundary) in
                    self.segments_in(q.saturating_add(dq), r.saturating_add(dr))
                {
                    let (c, d) = segment.ends();
                    if !boundary.open && segments_cross(from, to, c, d) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub(super) fn boundary_blocks_player(&self, x: i32, y: i32) -> bool {
        if self.boundaries.is_empty() {
            return false;
        }
        let (q, r) = world_to_axial(x, y);
        for (dq, dr) in [(0, 0)].into_iter().chain(DIRECTIONS) {
            for (segment, boundary) in self.segments_in(q.saturating_add(dq), r.saturating_add(dr))
            {
                if !boundary.open && near_segment(*segment, (x, y), PLAYER_RADIUS) {
                    return true;
                }
            }
        }
        false
    }

    /// Anchors stop one hex inside the segment limit, so every chord a run can reach from a valid
    /// anchor — including the three that canonicalize onto a neighbour — is still representable.
    fn anchor(q: i32, r: i32, corner: u8) -> Result<(i32, i32, u8), String> {
        if q.abs_diff(0) >= 100_000 || r.abs_diff(0) >= 100_000 || corner >= 6 {
            return Err("Boundary target is outside the supported coordinate range".into());
        }
        Ok((q, r, corner))
    }

    fn boundary_segments(edit: &BoundaryEdit) -> Result<Vec<Segment>, String> {
        let start = Self::anchor(edit.q, edit.r, edit.corner)?;
        let end = Self::anchor(edit.to_q, edit.to_r, edit.to_corner)?;
        match edit.shape {
            BoundaryShape::Line => {
                let chain = chord_chain(start, end, MAX_BOUNDARY_SEGMENTS)?;
                if chain.is_empty() {
                    return Err("Pick the far end of the run".into());
                }
                Ok(chain)
            }
            BoundaryShape::Yard => {
                let ((left, top), (right, bottom)) = yard_rect(start, end)?;
                let corners = [(left, top), (right, top), (right, bottom), (left, bottom)]
                    .map(|(x, y)| nearest_corner(x, y));
                let mut segments = BTreeSet::new();
                for side in 0..4 {
                    for segment in chord_chain(
                        corners[side],
                        corners[(side + 1) % 4],
                        MAX_BOUNDARY_SEGMENTS,
                    )? {
                        segments.insert(segment);
                    }
                    if segments.len() > MAX_BOUNDARY_SEGMENTS {
                        return Err(format!(
                            "Draw at most {MAX_BOUNDARY_SEGMENTS} boundary segments at a time"
                        ));
                    }
                }
                Ok(segments.into_iter().collect())
            }
        }
    }

    fn boundary_site_check(&self, segment: Segment, closing: bool) -> Result<(), String> {
        let (hex, other) = segment.hexes();
        let sides = [Some(hex), other];
        if !sides
            .iter()
            .flatten()
            .any(|&(q, r)| self.within_world_range(q, r, self.player.build_range))
        {
            return Err("Walk closer: boundary is outside build reach".into());
        }
        if !closing {
            return Ok(());
        }
        if sides
            .iter()
            .flatten()
            .any(|&(q, r)| self.terrain_at(q, r).blocks_construction())
        {
            return Err("Boundaries need dry, buildable ground on both sides".into());
        }
        if near_segment(segment, (self.player.x, self.player.y), PLAYER_RADIUS) {
            return Err("Step away from this line before closing or building it".into());
        }
        for entity in &self.entities {
            let footprint = self.entity_footprint(entity);
            let on = |cell: (i32, i32)| footprint.iter().any(|c| (c.q, c.r) == cell);
            let blocked = match other {
                Some(other) => on(hex) && on(other),
                None => on(hex),
            };
            if blocked {
                return Err("A building stands across this line".into());
            }
        }
        let (a, b) = segment.ends();
        for (index, links) in self.graph.iter().enumerate() {
            let source = &self.entities[index].placed;
            for target in links.iter() {
                let target = &self.entities[target].placed;
                if segments_cross(
                    a,
                    b,
                    axial_world(source.q, source.r),
                    axial_world(target.q, target.r),
                ) {
                    return Err("Reroute the transport crossing this line before closing it".into());
                }
            }
        }
        Ok(())
    }

    fn boundary_transaction(&self, edit: &BoundaryEdit) -> BoundaryTransaction {
        let mut transaction = BoundaryTransaction {
            preview: BoundaryPreview {
                segments: Vec::new(),
                changes: 0,
                cost: Vec::new(),
                refund: Vec::new(),
                error: None,
            },
            undo: BoundaryUndo {
                before: Vec::new(),
                after: Vec::new(),
            },
            inventory: self.player.inventory.clone(),
        };
        let result = (|| -> Result<(), String> {
            transaction.preview.segments = Self::boundary_segments(edit)?;
            let definition = if matches!(edit.action, BoundaryAction::Build) {
                let d = self
                    .boundary_definition(edit.definition_id)
                    .ok_or("Unknown boundary material")?;
                if d.gate && transaction.preview.segments.len() > 1 {
                    return Err("Place gates one segment at a time".into());
                }
                if let Some(required) = d.unlock_technology_id {
                    if !self.researched.contains(&required) {
                        let name = self
                            .technology(required)
                            .map(|technology| technology.name.as_str())
                            .unwrap_or("its technology");
                        return Err(format!("Research {name} before building {}", d.name));
                    }
                }
                Some(d)
            } else {
                None
            };
            for &segment in &transaction.preview.segments {
                let before = self.boundaries.get(&segment).cloned();
                let after = match edit.action {
                    BoundaryAction::Build => {
                        let definition = definition.expect("build definition");
                        if before
                            .as_ref()
                            .is_some_and(|b| b.definition_id == definition.id)
                        {
                            continue;
                        }
                        Some(Boundary {
                            segment,
                            definition_id: definition.id,
                            open: definition.gate,
                            paid: if self.creative {
                                Vec::new()
                            } else {
                                definition.construction_cost.clone()
                            },
                        })
                    }
                    BoundaryAction::Remove => None,
                    BoundaryAction::Open | BoundaryAction::Close => {
                        let mut boundary = before.clone().ok_or("Select a gate first")?;
                        if !self
                            .boundary_definition(boundary.definition_id)
                            .is_some_and(|d| d.gate)
                        {
                            let kind = self
                                .boundary_definition(boundary.definition_id)
                                .map(|d| match d.family {
                                    BoundaryFamily::Wall => "wall",
                                    BoundaryFamily::Fence => "fence",
                                })
                                .unwrap_or("boundary");
                            return Err(format!(
                                "This {kind} has no gate. Place a gate to create a crossing"
                            ));
                        }
                        boundary.open = matches!(edit.action, BoundaryAction::Open);
                        Some(boundary)
                    }
                };
                if before == after {
                    continue;
                }
                // Even an open gate must not bisect a building or stand in water.
                self.boundary_site_check(
                    segment,
                    after.as_ref().is_some_and(|b| !b.open)
                        || matches!(edit.action, BoundaryAction::Build),
                )
                .map_err(|reason| format!("Hex {}, {}: {reason}", segment.q, segment.r))?;
                transaction.undo.before.push((segment, before));
                transaction.undo.after.push((segment, after));
            }
            transaction.preview.changes = transaction.undo.after.len();
            self.boundary_price(&mut transaction)?;
            Ok(())
        })();
        transaction.preview.error = result.err();
        transaction
    }

    fn boundary_price(&self, transaction: &mut BoundaryTransaction) -> Result<(), String> {
        let mut old = BTreeMap::new();
        let mut new = BTreeMap::new();
        for (_, boundary) in &transaction.undo.before {
            bill(boundary, &mut old);
        }
        for (_, boundary) in &transaction.undo.after {
            bill(boundary, &mut new);
        }
        let ids: BTreeSet<_> = old.keys().chain(new.keys()).copied().collect();
        let mut cost = BTreeMap::new();
        let mut refund = BTreeMap::new();
        for id in ids {
            let was = old.get(&id).copied().unwrap_or(0);
            let now = new.get(&id).copied().unwrap_or(0);
            if now > was {
                cost.insert(id, now - was);
            }
            if was > now {
                refund.insert(id, was - now);
            }
        }
        transaction.preview.cost = ingredients(&cost);
        transaction.preview.refund = ingredients(&refund);
        if !has_ingredients(&self.player.inventory, &transaction.preview.cost) {
            return Err("Not enough materials in your pack for this entire selection".into());
        }
        for (&id, &count) in &cost {
            subtract_item(&mut transaction.inventory, id, count);
        }
        add_inventory(&mut transaction.inventory, &refund);
        // Price the final pack, so materials being spent can free slots for the refund.
        if self.slots_used(&transaction.inventory) > self.player.carry_slots {
            return Err("Make room in your pack for the recovered materials".into());
        }
        Ok(())
    }

    pub(super) fn boundary_preview(&self, edit: &BoundaryEdit) -> BoundaryPreview {
        self.boundary_transaction(edit).preview
    }

    fn commit_boundary_transaction(&mut self, transaction: &BoundaryTransaction) {
        let old_links = self.graph_links_by_id();
        let mut cells = BTreeSet::new();
        for (segment, after) in &transaction.undo.after {
            let (hex, other) = segment.hexes();
            cells.insert(hex);
            cells.extend(other);
            if let Some(boundary) = after {
                self.boundaries.insert(*segment, boundary.clone());
            } else {
                self.boundaries.remove(segment);
            }
        }
        self.player.inventory = transaction.inventory.clone();
        *self.boundary_hash_cache.borrow_mut() = None;
        self.dirty.boundaries = true;
        let nearby = self
            .entities
            .iter()
            .filter(|e| {
                cells.iter().any(|&cell| {
                    axial_distance(cell, (e.placed.q, e.placed.r)) <= GRAPH_TRACE_LIMIT * 2
                })
            })
            .map(|e| e.id)
            .collect();
        self.recompile_graph_components(&old_links, &cells, &nearby);
        self.replan_walk();
    }

    pub(super) fn edit_boundaries(&mut self, edit: &BoundaryEdit) -> Result<(), String> {
        let transaction = self.boundary_transaction(edit);
        if let Some(error) = &transaction.preview.error {
            return Err(error.clone());
        }
        if transaction.preview.changes == 0 {
            return Err("No boundary changes needed; no materials spent".into());
        }
        self.commit_boundary_transaction(&transaction);
        self.boundary_undo.push(transaction.undo);
        if self.boundary_undo.len() > MAX_UNDO_DEPTH {
            self.boundary_undo.remove(0);
        }
        self.events.push(format!(
            "Updated {} boundary segment{}",
            transaction.preview.changes,
            if transaction.preview.changes == 1 {
                ""
            } else {
                "s"
            }
        ));
        Ok(())
    }

    pub(super) fn undo_boundary(&mut self) -> Result<(), String> {
        let undo = self
            .boundary_undo
            .last()
            .ok_or("No boundary edit to undo in this session")?
            .clone();
        let mut transaction = BoundaryTransaction {
            preview: BoundaryPreview {
                segments: Vec::new(),
                changes: undo.before.len(),
                cost: Vec::new(),
                refund: Vec::new(),
                error: None,
            },
            undo: BoundaryUndo {
                before: undo.after,
                after: undo.before,
            },
            inventory: self.player.inventory.clone(),
        };
        for (segment, before) in &transaction.undo.before {
            if self.boundaries.get(segment) != before.as_ref() {
                return Err("Boundary changed since that edit".into());
            }
        }
        for (segment, after) in &transaction.undo.after {
            // Restoring an open gate is construction too: a newly placed building may span it.
            self.boundary_site_check(*segment, after.is_some())?;
        }
        self.boundary_price(&mut transaction)?;
        self.commit_boundary_transaction(&transaction);
        self.boundary_undo.pop();
        self.events.push("Undid the last boundary edit".into());
        Ok(())
    }
}

pub(super) fn validate_boundaries(definitions: &DefinitionsInput) -> Result<(), String> {
    unique_positive_ids(definitions.boundaries.iter().map(|b| b.id), "boundary")?;
    let mut keys = BTreeSet::new();
    for b in &definitions.boundaries {
        if b.key.is_empty()
            || !keys.insert(&b.key)
            || b.name.is_empty()
            || b.description.is_empty()
            || b.construction_cost.is_empty()
            || b.construction_cost.iter().any(|i| {
                i.quantity == 0
                    || i.quantity > 1000
                    || !definitions.items.iter().any(|d| d.id == i.item_id)
            })
        {
            return Err("Invalid boundary definition or construction bill".into());
        }
        unique_positive_ids(
            b.construction_cost.iter().map(|i| i.item_id),
            "boundary cost item",
        )?;
    }
    Ok(())
}

pub(super) fn validate_saved_boundaries(
    definitions: &DefinitionsInput,
    saved: &[Boundary],
) -> Result<(), String> {
    let mut segments = BTreeSet::new();
    for b in saved {
        let d = definitions
            .boundaries
            .iter()
            .find(|d| d.id == b.definition_id)
            .ok_or("Unknown saved boundary")?;
        if Segment::new(b.segment.q, b.segment.r, b.segment.chord)? != b.segment
            || !segments.insert(b.segment)
            || (b.open && !d.gate)
            || (!b.paid.is_empty() && b.paid != d.construction_cost)
        {
            return Err("Invalid saved boundary identity, state or paid bill".into());
        }
    }
    Ok(())
}
