//! Prepared ground: sparse surfaces and integer elevation over the same bounded transaction.
//!
//! Surfaces and grades share one overlay, one preview, one bill and one undo because they share one
//! answer. Walking speed, route cost, movement legality and building legality all read this map, and
//! splitting the two into separate systems would mean writing that arithmetic twice and designing a
//! road's grade transitions apart from its grades.
//!
//! Elevation is stored as a *graded delta* from the band the generator already produced. The
//! generated world therefore keeps exactly the passability it had — `natural_elevation` is chosen so
//! no two walkable bands can differ by more than [`MAX_WALK_STEP`] — and every step in this map is
//! one the player paid for.
use super::*;

/// A prepared surface, as the data declares it.
#[derive(Clone, Deserialize)]
pub(super) struct SurfaceDefinition {
    pub id: DefinitionId,
    pub key: String,
    pub name: String,
    pub description: String,
    /// Walking speed on this surface, in percent of untreated ground. Integer, like every other
    /// quantity the tick touches: `player_step` multiplies by it and `walk_step_cost` divides by it,
    /// and a float here would put the route search and the player's own feet on different arithmetic.
    pub movement: u32,
    pub construction_cost: Vec<Ingredient>,
    #[serde(default)]
    pub unlock_technology_id: Option<TechnologyId>,
    #[serde(default)]
    pub base_surface_id: Option<DefinitionId>,
}

/// One prepared hex. Absent from the map means untreated ground at its natural band.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct GroundCell {
    pub q: i32,
    pub r: i32,
    /// `0` is untreated. Otherwise a `SurfaceDefinition` id.
    pub surface: DefinitionId,
    /// Steps of cut (negative) or fill (positive) against the generated bed, bounded by the
    /// active ground source's content limit.
    pub elevation: i16,
    /// The surface bill actually paid. Sandbox paving never becomes a material source, on the same
    /// rule as a boundary's `paid`.
    pub paid: Vec<Ingredient>,
}

impl GroundCell {
    fn is_untouched(&self) -> bool {
        self.surface == 0 && self.elevation == 0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum GroundAction {
    /// Lay the chosen surface.
    Pave,
    /// Strip the surface back to untreated ground, recovering exactly what was paid.
    Clear,
    /// Fill every selected cell by [`GroundEdit::steps`].
    Raise,
    /// Cut every selected cell by [`GroundEdit::steps`].
    Lower,
    /// Even every selected cell onto one grade, chosen by [`GroundEdit::reference`].
    Level,
}

/// Which grade a [`GroundAction::Level`] evens onto.
///
/// The datum is the whole decision in levelling, and before this it was implicit in the order the
/// player happened to click. Naming it turns three separate gestures into one control: `Lowest`
/// cuts everything down and fills the spoil heap, `Highest` fills everything up and spends it, and
/// `First` keeps the datum the player picked by hand when neither extreme is what they meant.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum GroundReference {
    #[default]
    First,
    Lowest,
    Highest,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum GroundShape {
    Cell,
    Path,
    /// A rectangle drawn on the world, not on the axial grid: two lattice vertices, and every hex
    /// the rectangle touches. It shares its anchors and its snapping with the walled yard, so a
    /// floor and the wall around it land on exactly the same rectangle.
    Rect,
    /// The one-hex-thick outline of [`GroundShape::Rect`], on exactly the same two anchors.
    Frame,
    /// Every hex within the distance from the centre `q, r` out to the rim hex `to_q, to_r`.
    Disc,
    /// The one-hex-thick outline of [`GroundShape::Disc`] — the hexes at exactly the rim distance.
    Ring,
}

#[derive(Clone, Deserialize)]
pub(super) struct GroundEdit {
    pub q: i32,
    pub r: i32,
    pub to_q: i32,
    pub to_r: i32,
    /// Which corner of hex `q, r` a rectangle is anchored on. Ignored by the other shapes, which
    /// name whole hexes, so it defaults rather than being spelled on every edit.
    #[serde(default)]
    pub corner: u8,
    #[serde(default)]
    pub to_corner: u8,
    pub shape: GroundShape,
    pub definition_id: DefinitionId,
    pub action: GroundAction,
    /// How many steps one raise or lower moves the ground, clamped to `1..=MAX_GRADE_STEPS`.
    ///
    /// Defaulted rather than required so an edit written before this field existed still means what
    /// it meant: one step. A cell that cannot take the whole depth takes what it can and says so,
    /// which is what makes a terrace one gesture instead of three.
    #[serde(default)]
    pub steps: u8,
    /// Which grade a level evens onto. Ignored by every other verb.
    #[serde(default)]
    pub reference: GroundReference,
    /// Explicit confirmation that covering a resource field is intended. Never defaulted true: a
    /// deposit the player cannot see again is exactly the decision that has to be deliberate.
    #[serde(default)]
    pub cover: bool,
}

/// Every hex a world rectangle touches, contact along an edge or at a single corner included.
///
/// Both shapes are convex, so five separating axes settle it exactly: the rectangle's two, and the
/// three its own edges give the hexagon. It is all integer arithmetic on the lattice the rest of
/// the core measures in, so a hex that merely grazes the line is in or out for everyone at once.
fn hexes_touching_rect(rect: ((i32, i32), (i32, i32))) -> Result<Vec<(i32, i32)>, String> {
    let ((left, top), (right, bottom)) = rect;
    let (x0, y0) = (i64::from(left), i64::from(top));
    let (x1, y1) = (i64::from(right), i64::from(bottom));
    let hex_x = i64::from(HEX_X);
    let hex_y = i64::from(HEX_Y);
    let half_x = hex_x / 2;
    let radius = i64::from(HEX_RADIUS);
    // The hexagon's two slanted edge pairs project onto (radius/2, half_x); this is their support.
    let (nx, ny) = (radius / 2, half_x);
    let support = 2 * nx * ny;
    let r_min = (y0 - radius).div_euclid(hex_y) - 1;
    let r_max = (y1 + radius).div_euclid(hex_y) + 1;
    let columns = (x1 - x0 + 2 * half_x) / hex_x + 3;
    // Bounded before anything is scanned, let alone priced, so an accidental drag across the map is
    // refused rather than walked. This is the *scan* bound, deliberately looser than the selection
    // bound: an outline may be drawn round a rectangle far larger than its own filled area, and the
    // count that has to stay small is the one that gets priced.
    if (r_max - r_min + 1).saturating_mul(columns) > MAX_GROUND_CELLS as i64 * 16 {
        return Err("That rectangle is too large to select. Drag a smaller one".into());
    }
    let mut cells = Vec::new();
    for r in r_min..=r_max {
        let first = (x0 - half_x - r * half_x).div_euclid(hex_x) - 1;
        for q in first..=first + columns {
            let (cx, cy) = (q * hex_x + r * half_x, r * hex_y);
            if cx - half_x > x1 || cx + half_x < x0 || cy - radius > y1 || cy + radius < y0 {
                continue;
            }
            let along = nx * cx + ny * cy;
            if along - support > nx * x1 + ny * y1 || along + support < nx * x0 + ny * y0 {
                continue;
            }
            let across = nx * cx - ny * cy;
            if across - support > nx * x1 - ny * y0 || across + support < nx * x0 - ny * y1 {
                continue;
            }
            cells.push((q as i32, r as i32));
        }
    }
    Ok(cells)
}

/// The one-hex-thick outline of a filled selection: every cell of it with a neighbour outside it.
///
/// Outlines are derived from their filled shape rather than drawn by their own geometry, which is
/// what makes them exactly one hex thick for *every* shape and every size, with no rounding rule of
/// their own to disagree with the fill's. A shape already one hex wide is its own outline, which is
/// the right answer rather than a special case.
fn perimeter(cells: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let inside: BTreeSet<(i32, i32)> = cells.iter().copied().collect();
    cells
        .iter()
        .copied()
        .filter(|&(q, r)| {
            DIRECTIONS
                .iter()
                .any(|&(dq, dr)| !inside.contains(&(q + dq, r + dr)))
        })
        .collect()
}

/// The hexes of a disc or its rim, centred on `centre` and reaching `rim`.
///
/// Both are bounded by arithmetic rather than by a scan: a disc of radius `n` is `1 + 3n(n + 1)`
/// hexes and its rim is `6n`, so the refusal is decided before a single cell is enumerated.
fn ground_circle(
    centre: (i32, i32),
    rim: (i32, i32),
    filled: bool,
) -> Result<Vec<(i32, i32)>, String> {
    let radius = i64::from(axial_distance(centre, rim));
    let count = if filled {
        1 + 3 * radius * (radius + 1)
    } else if radius == 0 {
        1
    } else {
        6 * radius
    };
    if count > MAX_GROUND_CELLS as i64 {
        return Err(format!(
            "That {} is too wide: select at most {MAX_GROUND_CELLS} hexes of ground at a time",
            if filled { "circle" } else { "ring" }
        ));
    }
    let radius = radius as i32;
    let mut cells = Vec::new();
    for dq in -radius..=radius {
        for dr in (-radius).max(-dq - radius)..=radius.min(-dq + radius) {
            // On the rim, the cube distance is the largest of the three axes, so this is the same
            // predicate `perimeter` would reach by a different road.
            if filled || dq.abs().max(dr.abs()).max((dq + dr).abs()) == radius {
                cells.push((centre.0 + dq, centre.1 + dr));
            }
        }
    }
    Ok(cells)
}

/// One cell of the preview, as the host draws it.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct GroundPreviewCell {
    pub q: i32,
    pub r: i32,
    /// Finished surface after the edit (`0` = untreated).
    pub surface: DefinitionId,
    /// Finished elevation after the edit, natural band included, so the host draws the ground the
    /// player is about to get rather than re-deriving it.
    pub elevation: i32,
    /// Steps of cut (negative) or fill (positive) this cell contributes.
    pub change: i32,
    /// Whether this cell would cover a resource field.
    pub covers: bool,
    /// Whether this cell would be left standing behind an unwalkable step.
    pub retained: bool,
    /// Why this one cell cannot take the edit, if it cannot.
    ///
    /// A per-cell refusal, so an obstacle stops being a reason to erase the selection around it.
    /// The cells that *can* take the edit still resolve, are still priced, and are still applied;
    /// this one is drawn in the refusal colour with its own reason attached.
    pub blocked: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(super) struct GroundPreview {
    pub cells: Vec<GroundPreviewCell>,
    pub changes: usize,
    pub cost: Vec<Ingredient>,
    pub refund: Vec<Ingredient>,
    /// Steps of material this edit digs out, and steps it puts back.
    pub cut: u32,
    pub fill: u32,
    /// The spoil ledger after the edit, so the tray can show what levelling leaves behind.
    pub spoil: u64,
    /// How many selected cells hold a resource field the surface would cover.
    pub covers: usize,
    /// How many finished edges would be too steep to walk.
    pub retaining: usize,
    /// How many selected cells cannot take this edit and are skipped.
    pub blocked: usize,
    pub error: Option<String>,
}

#[derive(Clone)]
pub(super) struct GroundUndo {
    before: Vec<((i32, i32), Option<GroundCell>)>,
    after: Vec<((i32, i32), Option<GroundCell>)>,
    spoil_before: u64,
    spoil_after: u64,
}

struct GroundTransaction {
    preview: GroundPreview,
    undo: GroundUndo,
    inventory: BTreeMap<ItemId, u32>,
}

fn ingredients(items: &BTreeMap<ItemId, u32>) -> Vec<Ingredient> {
    items
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(&item_id, &quantity)| Ingredient { item_id, quantity })
        .collect()
}

/// The band the generator already put here, in whole steps.
///
/// Chosen so that every pair of terrains a player can walk on lies within [`MAX_WALK_STEP`] of every
/// other: the generated world is exactly as passable after this release as before it, and the only
/// unwalkable steps in a run are ones somebody dug. Shallows sit at ground level rather than below
/// it for that reason — a ford is a wet hex, not a canyon, and giving a river a real depth would
/// have walled off every bank in the world the moment this shipped.
pub(super) fn natural_elevation(terrain: Terrain) -> i32 {
    legacy_band_elevation(terrain)
}

impl Core {
    /// The pure generated facts through the cache when it matches this Core, otherwise through the
    /// full oracle. This is the only route from running simulation to generated ground.
    pub(super) fn generated_ground_at(&self, q: i32, r: i32) -> GeneratedGround {
        self.ground_spine.generated_from(
            &self.world_params,
            self.seed,
            self.scenario.generated_environment,
            q,
            r,
        )
    }

    pub(super) fn surface_definition(&self, id: DefinitionId) -> Option<&SurfaceDefinition> {
        self.definitions.surfaces.iter().find(|d| d.id == id)
    }

    /// The prepared surface on this hex, or `0` for untreated ground.
    pub(super) fn surface_at(&self, q: i32, r: i32) -> DefinitionId {
        self.ground.get(&(q, r)).map_or(0, |cell| cell.surface)
    }

    /// Walking speed here, in percent of untreated ground.
    pub(super) fn movement_factor_at(&self, q: i32, r: i32) -> u32 {
        let surface = self.surface_at(q, r);
        if surface == 0 {
            return UNTREATED_MOVEMENT;
        }
        self.surface_definition(surface)
            .map_or(UNTREATED_MOVEMENT, |d| d.movement)
    }

    /// The separated facts for this finished cell. The erosion delta is zero until geomorphic
    /// epochs exist, but it is a distinct input now so no later implementation can quietly fold it
    /// into what the player cut or filled.
    pub(super) fn finished_ground_at(&self, q: i32, r: i32) -> FinishedGround {
        let cell = self.ground.get(&(q, r));
        FinishedGround {
            generated: self.generated_ground_at(q, r),
            earthwork: GroundDelta::new(cell.map_or(0, |cell| cell.elevation)),
            erosion: GroundDelta::default(),
            surface: cell.map_or(0, |cell| cell.surface),
        }
    }

    /// Whether the heights this world publishes are physical quanta rather than legacy band steps.
    pub(super) fn ground_is_physical(&self) -> bool {
        self.ground_spine.is_physical()
    }

    pub(super) fn walk_step_limit(&self) -> i32 {
        if self.ground_is_physical() {
            scale::MAX_WALK_STEP_QUANTA
        } else {
            MAX_WALK_STEP
        }
    }

    pub(super) fn build_step_limit(&self) -> i32 {
        if self.ground_is_physical() {
            scale::MAX_BUILD_STEP_QUANTA
        } else {
            MAX_BUILD_STEP
        }
    }

    pub(super) fn grade_limit(&self) -> i32 {
        if self.ground_is_physical() {
            scale::EARTHWORK_LIMIT_QUANTA
        } else {
            i32::from(MAX_GRADE_STEPS)
        }
    }

    pub(super) fn grade_step_delta(&self, steps: u8) -> i32 {
        let index = steps.clamp(1, 3) as usize - 1;
        if self.ground_is_physical() {
            scale::EARTHWORK_STEPS_QUANTA[index]
        } else {
            (index + 1) as i32
        }
    }

    /// The finished height of this hex: the generated band plus whatever has been cut or filled.
    pub(super) fn ground_elevation_at(&self, q: i32, r: i32) -> i32 {
        self.finished_ground_at(q, r).elevation().get()
    }

    /// Whether the cliff on this hex has been quarried away.
    ///
    /// A cliff is the one impassable band made of something a player can take apart, and taking it
    /// apart is the cut they could already make everywhere else: the face comes down, the rock
    /// leaves as spoil, and the hex is a hex again. The cut has to reach *below* the band's natural
    /// grade before that happens, so a cliff nobody has worked is still the wall the generator drew.
    ///
    /// One step is enough because `natural_elevation` puts a cliff exactly one above highland: the
    /// first cut brings the face level with the ground beside it, which is the same fact the player
    /// can see in the diorama before they pay for it.
    pub(super) fn cliff_quarried(&self, q: i32, r: i32) -> bool {
        self.finished_ground_at(q, r).cliff_quarried()
    }

    /// Whether the terrain on this hex stops the player's body, as the finished ground leaves it.
    ///
    /// [`Terrain::blocks_movement`] is the rule for ground nobody has worked, and that is all the
    /// band table has ever claimed. Everything that walks, routes, drops or builds asks this
    /// instead, because a quarried cliff is a wall only in the table.
    ///
    /// On the physical source the water half of that table is a *picture of the generated
    /// equilibrium*, and Phase 8 lets the player change it. So the band is overruled by
    /// `water_depth_at`, in both directions: a drained deep-water cell is walkable even though its
    /// band still reads `DeepWater`, and a flooded meadow is not, even though its band still reads
    /// `Lowland`. The legacy source keeps the band's answer exactly — a 1 m² world has no depth to
    /// measure and [`scale::WADE_LIMIT_QUANTA`] would mean nothing in its units.
    pub(super) fn terrain_blocks_movement(&self, q: i32, r: i32) -> bool {
        let finished = self.finished_ground_at(q, r);
        if !self.ground_is_physical() {
            return finished.blocks_movement();
        }
        if self.water_depth_of(finished.generated, q, r) >= scale::WADE_LIMIT_QUANTA {
            return true;
        }
        !finished.generated.presentation.is_water() && finished.blocks_movement()
    }

    /// The construction half of the same rule. Any standing water at all refuses a foundation,
    /// which is what the band said when a ford was the shallowest water the game could describe.
    pub(super) fn terrain_blocks_construction(&self, q: i32, r: i32) -> bool {
        let finished = self.finished_ground_at(q, r);
        if !self.ground_is_physical() {
            return finished.blocks_construction();
        }
        if self.water_depth_of(finished.generated, q, r) > 0 {
            return true;
        }
        !finished.generated.presentation.is_water() && finished.blocks_construction()
    }

    /// Whether the step between two neighbouring hexes is too steep to walk.
    ///
    /// This is the whole of what a retaining wall is. It is deliberately symmetric: a wall keeps the
    /// player out of the pit as firmly as it keeps them in one, and a rule that let them drop down a
    /// face they could not climb would strand them at the bottom of their own excavation.
    pub(super) fn grade_blocks(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        (self.ground_elevation_at(from.0, from.1) - self.ground_elevation_at(to.0, to.1)).abs()
            > self.walk_step_limit()
    }

    /// Memoizes a pure digest of the overlay, on the same terms as the boundary digest: the cache is
    /// never saved or hashed, and the uncached walk stays the checksum oracle.
    pub(super) fn ground_state_hash(&self) -> u32 {
        if let Some(hash) = *self.ground_hash_cache.borrow() {
            return hash;
        }
        let hash = self.uncached_ground_hash();
        *self.ground_hash_cache.borrow_mut() = Some(hash);
        hash
    }

    pub(super) fn uncached_ground_hash(&self) -> u32 {
        let mut hash = 0x811c9dc5u32;
        hash_u64(&mut hash, self.ground.len() as u64);
        hash_u64(&mut hash, self.spoil);
        for cell in self.ground.values() {
            hash_i32(&mut hash, cell.q);
            hash_i32(&mut hash, cell.r);
            hash_u32(&mut hash, u32::from(cell.surface));
            hash_i32(&mut hash, i32::from(cell.elevation));
            hash_u32(&mut hash, cell.paid.len() as u32);
            for i in &cell.paid {
                hash_u32(&mut hash, u32::from(i.item_id));
                hash_u32(&mut hash, i.quantity);
            }
        }
        hash
    }

    pub(super) fn ground_snapshot(&self) -> Vec<GroundCell> {
        self.ground.values().cloned().collect()
    }

    /// The cells one selection covers. Bounded before anything is priced, so an accidental drag
    /// across the map is refused rather than costed.
    fn ground_cells(edit: &GroundEdit) -> Result<Vec<(i32, i32)>, String> {
        for (q, r) in [(edit.q, edit.r), (edit.to_q, edit.to_r)] {
            if q.abs_diff(0) > 100_000 || r.abs_diff(0) > 100_000 {
                return Err("Ground target is outside the supported coordinate range".into());
            }
        }
        let cells: Vec<(i32, i32)> = match edit.shape {
            GroundShape::Cell => vec![(edit.q, edit.r)],
            GroundShape::Path => hex_line_any((edit.q, edit.r), (edit.to_q, edit.to_r)),
            GroundShape::Rect | GroundShape::Frame => {
                if edit.corner >= 6 || edit.to_corner >= 6 {
                    return Err("Ground target is outside the supported coordinate range".into());
                }
                let rect = yard_rect(
                    (edit.q, edit.r, edit.corner),
                    (edit.to_q, edit.to_r, edit.to_corner),
                )?;
                let filled = hexes_touching_rect(rect)?;
                if matches!(edit.shape, GroundShape::Frame) {
                    perimeter(&filled)
                } else {
                    filled
                }
            }
            GroundShape::Disc | GroundShape::Ring => ground_circle(
                (edit.q, edit.r),
                (edit.to_q, edit.to_r),
                matches!(edit.shape, GroundShape::Disc),
            )?,
        };
        if cells.len() as u64 > MAX_GROUND_CELLS {
            return Err(format!(
                "Select at most {MAX_GROUND_CELLS} hexes of ground at a time"
            ));
        }
        if cells.is_empty() {
            return Err("That selection covers no ground".into());
        }
        Ok(cells)
    }

    /// Whether anything is standing on this hex. Grading moves the ground a building rests on, so it
    /// waits until the building is gone; a surface is only the skin on top and may be laid under one.
    fn entity_on(&self, cell: (i32, i32)) -> bool {
        self.entities.iter().any(|entity| {
            self.entity_footprint(entity)
                .iter()
                .any(|c| (c.q, c.r) == cell)
        })
    }

    /// Whether any extractor is currently drawing from this hex.
    fn extractor_draws_from(&self, cell: (i32, i32)) -> bool {
        self.entities.iter().any(|entity| {
            matches!(entity.kind, BuildingKind::Extractor)
                && self.field_covered_at(
                    (entity.placed.q, entity.placed.r),
                    cell,
                    self.extract_radius_of(entity.placed.definition_id),
                )
        })
    }

    fn ground_site_check(&self, cell: (i32, i32), grading: bool) -> Result<(), String> {
        if !self.within_world_range(cell.0, cell.1, self.player.build_range) {
            return Err("Walk closer: this ground is outside build reach".into());
        }
        // Grading a cliff is not asked whether the hex is buildable: that question is exactly what
        // the cut is about to change, and refusing it here is what used to make a cliff permanent.
        // Everything laid *on* the hex still waits for the rock to be below its natural grade.
        let cliff = self.terrain_at(cell.0, cell.1) == Terrain::Cliff;
        if cliff {
            if !grading && !self.cliff_quarried(cell.0, cell.1) {
                return Err(
                    "Cut this cliff down first: lower the face and the hex stops being a wall"
                        .into(),
                );
            }
        } else if self.terrain_at(cell.0, cell.1).blocks_construction() {
            return Err("Ground works need dry, buildable land".into());
        }
        if !grading {
            return Ok(());
        }
        if self.entity_on(cell) {
            return Err("Remove the building standing here before grading it".into());
        }
        // A cliff is the exception, and it is the exception on the guard's own reasoning. Elsewhere
        // a cut would move ground a deposit is measured in while leaving the deposit sitting on top
        // of it, which is a fiction the ground works refuse to write. A cliff face is the one place
        // where taking the ground down is what reaching the stone has always meant, and nearly
        // every cliff carries a scree field — so honouring the guard here would not protect a
        // deposit, it would only put the wall back beyond reach. The stone itself is untouched:
        // the quantity is a per-hex number that no grade has ever entered, and a quarried face is
        // still gathered from, now from on top rather than from beside.
        if !cliff && self.field_at(cell.0, cell.1).is_some() {
            return Err("A deposit sits here; grading would move ground it is measured in".into());
        }
        if world_to_axial(self.player.x, self.player.y) == cell {
            return Err("Step off this hex before grading it".into());
        }
        Ok(())
    }

    /// One selection, in three passes: resolve every cell, draw the footprint, then ask the
    /// questions that are about the selection as a whole.
    ///
    /// The order is the whole point. The footprint is published between the two, so it is published
    /// whatever either says — a refused selection that disappears is a selection the player cannot
    /// correct, and every refusal below names a hex they can only find by looking at it.
    fn ground_transaction(&self, edit: &GroundEdit) -> GroundTransaction {
        let mut transaction = GroundTransaction {
            preview: GroundPreview {
                cells: Vec::new(),
                changes: 0,
                cost: Vec::new(),
                refund: Vec::new(),
                cut: 0,
                fill: 0,
                spoil: self.spoil,
                covers: 0,
                retaining: 0,
                blocked: 0,
                error: None,
            },
            undo: GroundUndo {
                before: Vec::new(),
                after: Vec::new(),
                spoil_before: self.spoil,
                spoil_after: self.spoil,
            },
            inventory: self.player.inventory.clone(),
        };
        // The one refusal that has no footprint to draw: the selection did not resolve to cells at
        // all, so there is nothing to say it about.
        let cells = match Self::ground_cells(edit) {
            Ok(cells) => cells,
            Err(error) => {
                transaction.preview.error = Some(error);
                return transaction;
            }
        };
        let mut blocked = BTreeMap::new();
        let resolved = self.ground_resolve(edit, &cells, &mut transaction, &mut blocked);
        let after: BTreeMap<(i32, i32), Option<GroundCell>> =
            transaction.undo.after.iter().cloned().collect();
        self.ground_footprint(&cells, &after, &blocked, &mut transaction);
        transaction.preview.error = match resolved {
            Err(error) => Some(error),
            Ok(()) => self
                .ground_confirm(edit, &cells, &after, &mut transaction)
                .err(),
        };
        transaction
    }

    /// Turn the selection into a change set, one cell at a time.
    ///
    /// A cell that cannot take the edit is recorded and skipped rather than aborting the pass: an
    /// obstacle in a thirty-hex yard used to erase the other twenty-nine, which made every large
    /// selection a guessing game about which hex was the problem. Only a refusal about the *edit* —
    /// the material, the research, the spoil ledger — stops the whole thing, because none of those
    /// can be answered by grading fewer hexes.
    fn ground_resolve(
        &self,
        edit: &GroundEdit,
        cells: &[(i32, i32)],
        transaction: &mut GroundTransaction,
        blocked: &mut BTreeMap<(i32, i32), String>,
    ) -> Result<(), String> {
        let grading = matches!(
            edit.action,
            GroundAction::Raise | GroundAction::Lower | GroundAction::Level
        );
        let definition = if matches!(edit.action, GroundAction::Pave) {
            Some(
                self.surface_definition(edit.definition_id)
                    .ok_or("Unknown surface material")?,
            )
        } else {
            None
        };
        if let Some(id) = definition.and_then(|surface| surface.unlock_technology_id) {
            if !self.creative && !self.researched.contains(&id) {
                return Err(format!(
                    "Research {} before laying this surface",
                    self.technology(id)
                        .map_or("the required technology", |technology| technology
                            .name
                            .as_str())
                ));
            }
        }
        // Zero is what a host that predates the depth control sends, and one step is what it meant.
        let steps = self.grade_step_delta(edit.steps);
        let limit = self.grade_limit();
        // The grade every other cell is evened onto. Naming the datum explicitly is what makes
        // levelling a decision the player can see before they make it: the lowest cell fills the
        // spoil heap, the highest spends it, and the first one picked is their own eye.
        let target = match edit.reference {
            GroundReference::First => self.ground_elevation_at(cells[0].0, cells[0].1),
            GroundReference::Lowest => cells
                .iter()
                .map(|&(q, r)| self.ground_elevation_at(q, r))
                .min()
                .unwrap_or(0),
            GroundReference::Highest => cells
                .iter()
                .map(|&(q, r)| self.ground_elevation_at(q, r))
                .max()
                .unwrap_or(0),
        };

        let mut cut = 0i64;
        let mut fill = 0i64;
        for &cell in cells {
            let before = self.ground.get(&cell).cloned();
            let natural = self.generated_ground_at(cell.0, cell.1).bed.get();
            let current = i32::from(before.as_ref().map_or(0, |c| c.elevation));
            let mut next = before.clone().unwrap_or(GroundCell {
                q: cell.0,
                r: cell.1,
                surface: 0,
                elevation: 0,
                paid: Vec::new(),
            });
            match edit.action {
                GroundAction::Pave => {
                    let definition = definition.expect("pave definition");
                    if next.surface == definition.id {
                        continue;
                    }
                    if let Some(base) = definition.base_surface_id {
                        if next.surface != base {
                            blocked.insert(
                                cell,
                                format!(
                                    "Lay {} here first; this road needs a prepared base",
                                    self.surface_definition(base)
                                        .map_or("the base surface", |surface| surface
                                            .name
                                            .as_str())
                                ),
                            );
                            continue;
                        }
                    }
                    next.surface = definition.id;
                    next.paid = if self.creative {
                        Vec::new()
                    } else {
                        let mut paid = BTreeMap::new();
                        if definition.base_surface_id.is_some() {
                            add_ingredients(&mut paid, &next.paid);
                        }
                        add_ingredients(&mut paid, &definition.construction_cost);
                        ingredients(&paid)
                    };
                }
                GroundAction::Clear => {
                    next.surface = 0;
                    next.paid = Vec::new();
                }
                // A depth the cell cannot take in full is taken as far as it goes. Clamping rather
                // than refusing is what lets one gesture terrace mixed ground: the cells with room
                // move, the ones already at the bound say so, and the bound itself never moves.
                GroundAction::Raise | GroundAction::Lower => {
                    let wanted = if matches!(edit.action, GroundAction::Raise) {
                        (current + steps).min(limit)
                    } else {
                        (current - steps).max(-limit)
                    };
                    if wanted == current {
                        blocked.insert(
                            cell,
                            format!("Already cut or filled the full {limit} steps"),
                        );
                        continue;
                    }
                    next.elevation = wanted as i16;
                }
                GroundAction::Level => {
                    let wanted = i64::from(target) - i64::from(natural);
                    next.elevation = wanted.clamp(i64::from(-limit), i64::from(limit)) as i16;
                }
            }
            let after = (!next.is_untouched()).then_some(next);
            if before == after {
                continue;
            }
            if let Err(reason) = self.ground_site_check(cell, grading) {
                blocked.insert(cell, reason);
                continue;
            }
            let step = i64::from(after.as_ref().map_or(0, |c| c.elevation))
                - i64::from(before.as_ref().map_or(0, |c| c.elevation));
            if step > 0 {
                fill += step;
            } else {
                cut -= step;
            }
            transaction.undo.before.push((cell, before));
            transaction.undo.after.push((cell, after));
        }
        transaction.preview.changes = transaction.undo.after.len();
        transaction.preview.cut = cut as u32;
        transaction.preview.fill = fill as u32;
        // Nothing at all can be done and something is in the way: then the obstacle *is* the
        // answer, and it is spoken as the selection's refusal rather than left as a tint on one
        // hex nobody has a reason to look at.
        if transaction.preview.changes == 0 {
            if let Some((cell, reason)) = cells
                .iter()
                .find_map(|cell| blocked.get(cell).map(|reason| (cell, reason)))
            {
                return Err(format!("Hex {}, {}: {reason}", cell.0, cell.1));
            }
        }
        // Fill is dug, never conjured. Spoil is the one ledger that makes evening the ground an
        // exchange instead of a wish: to raise anything, something else has to come down.
        let spoil = i64::try_from(self.spoil).unwrap_or(i64::MAX) + cut - fill;
        if spoil < 0 {
            return Err(format!(
                "Not enough spoil: this needs {fill} and you have {}. Cut ground somewhere to raise it here",
                self.spoil
            ));
        }
        transaction.undo.spoil_after = spoil as u64;
        transaction.preview.spoil = spoil as u64;
        Ok(())
    }

    /// The surface and finished grade a hex would be left with, reading the pending change set
    /// before the committed map so a preview never prices itself against ground it is moving.
    fn ground_finished(
        &self,
        after: &BTreeMap<(i32, i32), Option<GroundCell>>,
        cell: (i32, i32),
    ) -> FinishedGround {
        match after.get(&cell) {
            Some(entry) => FinishedGround {
                generated: self.generated_ground_at(cell.0, cell.1),
                earthwork: GroundDelta::new(entry.as_ref().map_or(0, |cell| cell.elevation)),
                erosion: GroundDelta::default(),
                surface: entry.as_ref().map_or(0, |cell| cell.surface),
            },
            None => self.finished_ground_at(cell.0, cell.1),
        }
    }

    /// A cliff coming down in this very selection has stopped being a wall by the time the footing
    /// checks run, and one going back up has become one again.
    fn ground_finished_blocks(
        &self,
        after: &BTreeMap<(i32, i32), Option<GroundCell>>,
        cell: (i32, i32),
    ) -> bool {
        self.ground_finished(after, cell).blocks_movement()
    }

    /// Draw the picture: every selected hex, at the grade it would finish on, with what is about to
    /// happen to it — or why nothing is.
    fn ground_footprint(
        &self,
        cells: &[(i32, i32)],
        after: &BTreeMap<(i32, i32), Option<GroundCell>>,
        blocked: &BTreeMap<(i32, i32), String>,
        transaction: &mut GroundTransaction,
    ) {
        let mut retaining = 0usize;
        for &cell in cells {
            let finished = self.ground_finished(after, cell);
            let surface = finished.surface;
            let elevation = finished.elevation().get();
            let covers = surface != 0 && self.field_at(cell.0, cell.1).is_some();
            if covers {
                transaction.preview.covers += 1;
            }
            let retained = DIRECTIONS.iter().any(|&(dq, dr)| {
                let neighbour = (cell.0 + dq, cell.1 + dr);
                !self.ground_finished_blocks(after, neighbour)
                    && (elevation - self.ground_finished(after, neighbour).elevation().get()).abs()
                        > self.walk_step_limit()
            });
            if retained {
                retaining += 1;
            }
            let reason = blocked.get(&cell).cloned();
            if reason.is_some() {
                transaction.preview.blocked += 1;
            }
            transaction.preview.cells.push(GroundPreviewCell {
                q: cell.0,
                r: cell.1,
                surface,
                elevation,
                change: i32::from(
                    after
                        .get(&cell)
                        .map_or(0, |entry| entry.as_ref().map_or(0, |c| c.elevation)),
                ) - i32::from(self.ground.get(&cell).map_or(0, |c| c.elevation)),
                covers,
                retained,
                blocked: reason,
            });
        }
        transaction.preview.retaining = retaining;
    }

    /// What the finished ground would mean for deposits, for the player's own footing, and for the
    /// pack. Every question here is about the selection as a whole, so every answer refuses it whole.
    fn ground_confirm(
        &self,
        edit: &GroundEdit,
        cells: &[(i32, i32)],
        after: &BTreeMap<(i32, i32), Option<GroundCell>>,
        transaction: &mut GroundTransaction,
    ) -> Result<(), String> {
        if transaction.preview.covers > 0 && !edit.cover {
            return Err(format!(
                "{} selected hex{} hold{} a deposit. Confirm covering to seal it: it stops being reachable until the surface comes up",
                transaction.preview.covers,
                if transaction.preview.covers == 1 { "" } else { "es" },
                if transaction.preview.covers == 1 { "s" } else { "" },
            ));
        }
        for &cell in cells {
            if self.ground_finished(after, cell).surface != 0 && self.extractor_draws_from(cell) {
                return Err(format!(
                    "Hex {}, {} is being worked by an extractor. Relocate it before covering the deposit",
                    cell.0, cell.1
                ));
            }
        }
        // The player must not be able to wall themselves into their own excavation. Only their own
        // hex is checked, and only its six edges: a full reachability sweep would price thinking
        // about a route, and the pit that traps somebody is always the one under their feet.
        let standing = world_to_axial(self.player.x, self.player.y);
        let here = self.ground_finished(after, standing).elevation().get();
        let escapes = DIRECTIONS.iter().any(|&(dq, dr)| {
            let neighbour = (standing.0 + dq, standing.1 + dr);
            !self.ground_finished_blocks(after, neighbour)
                && (here - self.ground_finished(after, neighbour).elevation().get()).abs()
                    <= self.walk_step_limit()
        });
        if !escapes {
            return Err("That grade would leave you with no way out of this hex".into());
        }
        self.ground_price(transaction)
    }

    fn ground_price(&self, transaction: &mut GroundTransaction) -> Result<(), String> {
        let mut old = BTreeMap::new();
        let mut new = BTreeMap::new();
        for (_, cell) in &transaction.undo.before {
            if let Some(cell) = cell {
                add_ingredients(&mut old, &cell.paid);
            }
        }
        for (_, cell) in &transaction.undo.after {
            if let Some(cell) = cell {
                add_ingredients(&mut new, &cell.paid);
            }
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
        if self.slots_used(&transaction.inventory) > self.player.carry_slots {
            return Err("Make room in your pack for the recovered materials".into());
        }
        Ok(())
    }

    pub(super) fn ground_preview(&self, edit: &GroundEdit) -> GroundPreview {
        self.ground_transaction(edit).preview
    }

    fn commit_ground_transaction(&mut self, transaction: &GroundTransaction) {
        let mut covering_changed = false;
        for (cell, after) in &transaction.undo.after {
            let was_paved = self.surface_at(cell.0, cell.1) != 0;
            let now_paved = after.as_ref().is_some_and(|c| c.surface != 0);
            // `field_at` is already blind to a sealed deposit, so asking it here would see the
            // change on the way down and miss it on the way back up. The buried view sees both.
            if was_paved != now_paved && self.buried_field_at(cell.0, cell.1).is_some() {
                covering_changed = true;
            }
            match after {
                Some(cell_state) => {
                    self.ground.insert(*cell, cell_state.clone());
                }
                None => {
                    self.ground.remove(cell);
                }
            }
        }
        self.spoil = transaction.undo.spoil_after;
        self.player.inventory = transaction.inventory.clone();
        *self.ground_hash_cache.borrow_mut() = None;
        self.dirty.ground = true;
        if covering_changed {
            // A covered deposit leaves the published field, so the host's ordering has to be the
            // native one again rather than a patch against a list that no longer has the row.
            self.dirty.resources_replace = true;
            // Every resolved extractor reach is stale for the same reason a generated chunk makes
            // it stale, and every extractor's status is derived from one, so the two are dropped
            // together rather than left to disagree.
            self.deposit_links.clear();
            self.mark_all_entities_dirty();
            self.rebuild_flora_regrowth();
        }
        self.replan_walk();
    }

    pub(super) fn edit_ground(&mut self, edit: &GroundEdit) -> Result<(), String> {
        let transaction = self.ground_transaction(edit);
        if let Some(error) = &transaction.preview.error {
            return Err(error.clone());
        }
        if transaction.preview.changes == 0 {
            return Err("This ground already matches the selection; nothing spent".into());
        }
        let (cut, fill, changes) = (
            transaction.preview.cut,
            transaction.preview.fill,
            transaction.preview.changes,
        );
        self.commit_ground_transaction(&transaction);
        self.ground_undo.push(transaction.undo);
        if self.ground_undo.len() > MAX_UNDO_DEPTH {
            self.ground_undo.remove(0);
        }
        self.events.push(if cut > 0 || fill > 0 {
            format!(
                "Graded {changes} hex{} · cut {cut}, filled {fill}, spoil {}",
                if changes == 1 { "" } else { "es" },
                self.spoil
            )
        } else {
            format!(
                "Prepared {changes} hex{} of ground",
                if changes == 1 { "" } else { "es" }
            )
        });
        Ok(())
    }

    pub(super) fn undo_ground(&mut self) -> Result<(), String> {
        let undo = self
            .ground_undo
            .last()
            .ok_or("No ground edit to undo in this session")?
            .clone();
        // The world has to still be holding what that edit left, or the undo would be rewriting
        // someone else's work rather than reversing its own.
        for (cell, after) in &undo.after {
            if self.ground.get(cell) != after.as_ref() {
                return Err("This ground changed since that edit".into());
            }
        }
        let mut transaction = GroundTransaction {
            preview: GroundPreview {
                cells: Vec::new(),
                changes: undo.before.len(),
                cost: Vec::new(),
                refund: Vec::new(),
                cut: 0,
                fill: 0,
                spoil: undo.spoil_before,
                covers: 0,
                retaining: 0,
                blocked: 0,
                error: None,
            },
            undo: GroundUndo {
                before: undo.after.clone(),
                after: undo.before.clone(),
                spoil_before: undo.spoil_after,
                spoil_after: undo.spoil_before,
            },
            inventory: self.player.inventory.clone(),
        };
        // Undo is construction too, and it is priced by the same arithmetic: a surface coming back
        // is bought again, and one coming up is recovered again. Spoil returns to exactly the count
        // the edit found, which is what keeps an undone excavation from becoming a material source.
        for (cell, after) in &transaction.undo.after {
            self.ground_site_check(
                *cell,
                after.as_ref().map_or(0, |c| c.elevation)
                    != self.ground.get(cell).map_or(0, |c| c.elevation),
            )?;
        }
        self.ground_price(&mut transaction)?;
        self.commit_ground_transaction(&transaction);
        self.ground_undo.pop();
        self.events.push("Undid the last ground edit".into());
        Ok(())
    }
}

pub(super) fn validate_surfaces(definitions: &DefinitionsInput) -> Result<(), String> {
    unique_positive_ids(definitions.surfaces.iter().map(|s| s.id), "surface")?;
    let mut keys = BTreeSet::new();
    for s in &definitions.surfaces {
        if s.key.is_empty()
            || !keys.insert(&s.key)
            || s.name.is_empty()
            || s.description.is_empty()
            // A surface below untreated ground would be a trap dressed as a road, and one above the
            // ceiling would outrun the route search's admissible heuristic.
            || s.movement < UNTREATED_MOVEMENT
            || s.movement > MAX_SURFACE_MOVEMENT
            || s.construction_cost.iter().any(|i| {
                i.quantity == 0
                    || i.quantity > 1000
                    || !definitions.items.iter().any(|d| d.id == i.item_id)
            })
        {
            return Err("Invalid surface definition or construction bill".into());
        }
        unique_positive_ids(
            s.construction_cost.iter().map(|i| i.item_id),
            "surface cost item",
        )?;
        if let Some(base) = s.base_surface_id {
            if !definitions.surfaces.iter().any(|surface| {
                surface.id == base && surface.id != s.id && surface.base_surface_id.is_none()
            }) {
                return Err("Surface base must be a different, single-layer surface".into());
            }
        }
    }
    Ok(())
}

pub(super) fn validate_saved_ground(
    definitions: &DefinitionsInput,
    saved: &[GroundCell],
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for cell in saved {
        if !seen.insert((cell.q, cell.r))
            || i32::from(cell.elevation.abs()) > scale::EARTHWORK_LIMIT_QUANTA
        {
            return Err("Invalid saved ground identity or grade".into());
        }
        if cell.surface == 0 {
            if !cell.paid.is_empty() {
                return Err("Untreated ground cannot carry a paid bill".into());
            }
            continue;
        }
        let d = definitions
            .surfaces
            .iter()
            .find(|d| d.id == cell.surface)
            .ok_or("Unknown saved surface")?;
        if !cell.paid.is_empty() && cell.paid != d.construction_cost {
            return Err("Invalid saved surface paid bill".into());
        }
    }
    Ok(())
}
