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
    /// Steps of cut (negative) or fill (positive) against the natural band, bounded by
    /// [`MAX_GRADE_STEPS`].
    pub elevation: i8,
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
    /// One step of fill on every selected cell.
    Raise,
    /// One step of cut on every selected cell.
    Lower,
    /// Even every selected cell onto the first cell's finished grade.
    Level,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum GroundShape {
    Cell,
    Path,
    Area,
}

#[derive(Clone, Deserialize)]
pub(super) struct GroundEdit {
    pub q: i32,
    pub r: i32,
    pub to_q: i32,
    pub to_r: i32,
    pub shape: GroundShape,
    pub definition_id: DefinitionId,
    pub action: GroundAction,
    /// Explicit confirmation that covering a resource field is intended. Never defaulted true: a
    /// deposit the player cannot see again is exactly the decision that has to be deliberate.
    #[serde(default)]
    pub cover: bool,
}

/// One cell of the preview, as the host draws it.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
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
    match terrain {
        Terrain::DeepWater => -1,
        Terrain::ShallowWater | Terrain::Shore | Terrain::Lowland => 0,
        Terrain::Hills => 1,
        Terrain::Highland => 2,
        Terrain::Cliff => 3,
    }
}

impl Core {
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

    /// The finished height of this hex: the generated band plus whatever has been cut or filled.
    pub(super) fn ground_elevation_at(&self, q: i32, r: i32) -> i32 {
        natural_elevation(self.terrain_at(q, r))
            + i32::from(self.ground.get(&(q, r)).map_or(0, |cell| cell.elevation))
    }

    /// Whether the step between two neighbouring hexes is too steep to walk.
    ///
    /// This is the whole of what a retaining wall is. It is deliberately symmetric: a wall keeps the
    /// player out of the pit as firmly as it keeps them in one, and a rule that let them drop down a
    /// face they could not climb would strand them at the bottom of their own excavation.
    pub(super) fn grade_blocks(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        (self.ground_elevation_at(from.0, from.1) - self.ground_elevation_at(to.0, to.1)).abs()
            > MAX_WALK_STEP
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
            GroundShape::Area => {
                let width = u64::from(edit.q.abs_diff(edit.to_q)) + 1;
                let height = u64::from(edit.r.abs_diff(edit.to_r)) + 1;
                if width * height > MAX_GROUND_CELLS {
                    return Err(format!(
                        "Select at most {MAX_GROUND_CELLS} hexes of ground at a time"
                    ));
                }
                let mut cells = Vec::new();
                for q in edit.q.min(edit.to_q)..=edit.q.max(edit.to_q) {
                    for r in edit.r.min(edit.to_r)..=edit.r.max(edit.to_r) {
                        cells.push((q, r));
                    }
                }
                cells
            }
        };
        if cells.len() as u64 > MAX_GROUND_CELLS {
            return Err(format!(
                "Select at most {MAX_GROUND_CELLS} hexes of ground at a time"
            ));
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
        if self.terrain_at(cell.0, cell.1).blocks_construction() {
            return Err("Ground works need dry, buildable land".into());
        }
        if !grading {
            return Ok(());
        }
        if self.entity_on(cell) {
            return Err("Remove the building standing here before grading it".into());
        }
        if self.field_at(cell.0, cell.1).is_some() {
            return Err("A deposit sits here; grading would move ground it is measured in".into());
        }
        if world_to_axial(self.player.x, self.player.y) == cell {
            return Err("Step off this hex before grading it".into());
        }
        Ok(())
    }

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
        let result = (|| -> Result<(), String> {
            let cells = Self::ground_cells(edit)?;
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
            // The first cell of the selection is the grade every other cell is evened onto. Naming
            // the reference by the click that started the drag is what makes levelling a decision
            // the player can see before they make it, rather than an average they have to guess.
            let target = self.ground_elevation_at(cells[0].0, cells[0].1);

            let mut cut = 0i64;
            let mut fill = 0i64;
            for &cell in &cells {
                let before = self.ground.get(&cell).cloned();
                let natural = natural_elevation(self.terrain_at(cell.0, cell.1));
                let current = before.as_ref().map_or(0, |c| c.elevation);
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
                                return Err(format!(
                                    "Lay {} on hex {}, {} first; this road needs a prepared base",
                                    self.surface_definition(base)
                                        .map_or("the base surface", |surface| surface
                                            .name
                                            .as_str()),
                                    cell.0,
                                    cell.1
                                ));
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
                    GroundAction::Raise => next.elevation = current.saturating_add(1),
                    GroundAction::Lower => next.elevation = current.saturating_sub(1),
                    GroundAction::Level => {
                        let wanted = i64::from(target) - i64::from(natural);
                        next.elevation = wanted
                            .clamp(i64::from(-MAX_GRADE_STEPS), i64::from(MAX_GRADE_STEPS))
                            as i8;
                    }
                }
                if grading && next.elevation.abs() > MAX_GRADE_STEPS {
                    return Err(format!(
                        "Hex {}, {} is already cut or filled the full {MAX_GRADE_STEPS} steps",
                        cell.0, cell.1
                    ));
                }
                let after = (!next.is_untouched()).then_some(next);
                if before == after {
                    continue;
                }
                self.ground_site_check(cell, grading)
                    .map_err(|reason| format!("Hex {}, {}: {reason}", cell.0, cell.1))?;
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
            self.ground_finish(edit, &cells, &mut transaction)?;
            Ok(())
        })();
        transaction.preview.error = result.err();
        transaction
    }

    /// Draw the picture and the bill from the resolved change set, then check what the finished
    /// ground would mean for deposits, for the player's own footing, and for the pack.
    fn ground_finish(
        &self,
        edit: &GroundEdit,
        cells: &[(i32, i32)],
        transaction: &mut GroundTransaction,
    ) -> Result<(), String> {
        let after: BTreeMap<(i32, i32), Option<GroundCell>> =
            transaction.undo.after.iter().cloned().collect();
        let finished = |cell: (i32, i32)| -> (DefinitionId, i32) {
            match after.get(&cell) {
                Some(entry) => (
                    entry.as_ref().map_or(0, |c| c.surface),
                    natural_elevation(self.terrain_at(cell.0, cell.1))
                        + i32::from(entry.as_ref().map_or(0, |c| c.elevation)),
                ),
                None => (
                    self.surface_at(cell.0, cell.1),
                    self.ground_elevation_at(cell.0, cell.1),
                ),
            }
        };

        let mut retaining = 0usize;
        for &cell in cells {
            let (surface, elevation) = finished(cell);
            let covers = surface != 0 && self.field_at(cell.0, cell.1).is_some();
            if covers {
                transaction.preview.covers += 1;
            }
            let retained = DIRECTIONS.iter().any(|&(dq, dr)| {
                let neighbour = (cell.0 + dq, cell.1 + dr);
                !self.terrain_at(neighbour.0, neighbour.1).blocks_movement()
                    && (elevation - finished(neighbour).1).abs() > MAX_WALK_STEP
            });
            if retained {
                retaining += 1;
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
            });
        }
        transaction.preview.retaining = retaining;
        if transaction.preview.covers > 0 && !edit.cover {
            return Err(format!(
                "{} selected hex{} hold{} a deposit. Confirm covering to seal it: it stops being reachable until the surface comes up",
                transaction.preview.covers,
                if transaction.preview.covers == 1 { "" } else { "es" },
                if transaction.preview.covers == 1 { "s" } else { "" },
            ));
        }
        for &cell in cells {
            if finished(cell).0 != 0 && self.extractor_draws_from(cell) {
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
        let (_, here) = finished(standing);
        let escapes = DIRECTIONS.iter().any(|&(dq, dr)| {
            let neighbour = (standing.0 + dq, standing.1 + dr);
            !self.terrain_at(neighbour.0, neighbour.1).blocks_movement()
                && (here - finished(neighbour).1).abs() <= MAX_WALK_STEP
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
        if !seen.insert((cell.q, cell.r)) || cell.elevation.abs() > MAX_GRADE_STEPS {
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
