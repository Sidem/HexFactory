//! Sparse edge construction. The same bounded transaction resolves previews and commits.
use super::*;

#[derive(Clone, Deserialize)]
pub(super) struct BoundaryDefinition {
    pub id: DefinitionId,
    pub key: String,
    pub name: String,
    pub description: String,
    pub gate: bool,
    pub construction_cost: Vec<Ingredient>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Edge {
    pub q: i32,
    pub r: i32,
    pub direction: u8,
}

impl Edge {
    fn new(q: i32, r: i32, direction: u8) -> Result<Self, String> {
        // Keeps all world geometry exact and comfortably within i32 axial_world arithmetic.
        if q.abs_diff(0) > 100_000 || r.abs_diff(0) > 100_000 || direction >= 6 {
            return Err("Boundary target is outside the supported coordinate range".into());
        }
        if direction < 3 {
            Ok(Self { q, r, direction })
        } else {
            let (dq, dr) = DIRECTIONS[direction as usize];
            if (q + dq).abs_diff(0) > 100_000 || (r + dr).abs_diff(0) > 100_000 {
                return Err("Boundary target is outside the supported coordinate range".into());
            }
            Ok(Self {
                q: q + dq,
                r: r + dr,
                direction: direction - 3,
            })
        }
    }

    fn neighbour(self) -> (i32, i32) {
        let (dq, dr) = DIRECTIONS[self.direction as usize];
        (self.q + dq, self.r + dr)
    }

    fn ends(self) -> ((i32, i32), (i32, i32)) {
        let (x, y) = axial_world(self.q, self.r);
        let vertices = [
            (HEX_X / 2, -HEX_RADIUS / 2),
            (HEX_X / 2, HEX_RADIUS / 2),
            (0, HEX_RADIUS),
            (-HEX_X / 2, HEX_RADIUS / 2),
        ];
        let a = vertices[self.direction as usize];
        let b = vertices[self.direction as usize + 1];
        ((x + a.0, y + a.1), (x + b.0, y + b.1))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct Boundary {
    #[serde(flatten)]
    pub edge: Edge,
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

#[derive(Clone, Deserialize)]
pub(super) struct BoundaryEdit {
    pub q: i32,
    pub r: i32,
    pub to_q: i32,
    pub to_r: i32,
    pub direction: u8,
    pub area: bool,
    pub definition_id: DefinitionId,
    pub action: BoundaryAction,
}

#[derive(Serialize)]
pub(super) struct BoundaryPreview {
    pub edges: Vec<Edge>,
    pub changes: usize,
    pub cost: Vec<Ingredient>,
    pub refund: Vec<Ingredient>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub(super) struct BoundaryUndo {
    before: Vec<(Edge, Option<Boundary>)>,
    after: Vec<(Edge, Option<Boundary>)>,
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

fn near_edge(edge: Edge, p: (i32, i32), radius: i32) -> bool {
    let (a, b) = edge.ends();
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
    pub(super) fn boundary_crosses_footprint(&self, footprint: &[Coordinate]) -> bool {
        footprint.iter().any(|cell| {
            (0..6).any(|direction| {
                let (dq, dr) = DIRECTIONS[direction as usize];
                footprint
                    .iter()
                    .any(|other| other.q == cell.q + dq && other.r == cell.r + dr)
                    && self.boundary_between(cell.q, cell.r, direction)
            })
        })
    }

    pub(super) fn boundary_between(&self, q: i32, r: i32, direction: u8) -> bool {
        Edge::new(q, r, direction).is_ok_and(|edge| self.boundaries.contains_key(&edge))
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
            hash_i32(&mut hash, b.edge.q);
            hash_i32(&mut hash, b.edge.r);
            hash_u32(&mut hash, u32::from(b.edge.direction));
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
                for direction in 0..6 {
                    let Ok(edge) = Edge::new(q + dq, r + dr, direction) else {
                        continue;
                    };
                    if self
                        .boundaries
                        .get(&edge)
                        .is_some_and(|boundary| !boundary.open)
                    {
                        let (c, d) = edge.ends();
                        if segments_cross(from, to, c, d) {
                            return true;
                        }
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
            for direction in 0..6 {
                let Ok(edge) = Edge::new(q + dq, r + dr, direction) else {
                    continue;
                };
                if self
                    .boundaries
                    .get(&edge)
                    .is_some_and(|boundary| !boundary.open)
                    && near_edge(edge, (x, y), PLAYER_RADIUS)
                {
                    return true;
                }
            }
        }
        false
    }

    fn boundary_edges(edit: &BoundaryEdit) -> Result<Vec<Edge>, String> {
        Edge::new(edit.q, edit.r, edit.direction)?;
        Edge::new(edit.to_q, edit.to_r, edit.direction)?;
        if !edit.area {
            return Ok(vec![Edge::new(edit.q, edit.r, edit.direction)?]);
        }
        if !matches!(edit.action, BoundaryAction::Build | BoundaryAction::Remove) {
            return Err("Operate one gate at a time".into());
        }
        let width = edit.q.abs_diff(edit.to_q) + 1;
        let height = edit.r.abs_diff(edit.to_r) + 1;
        if u64::from(width) * u64::from(height) > 32 {
            return Err("Select at most 32 hexes per enclosure".into());
        }
        let qs = edit.q.min(edit.to_q)..=edit.q.max(edit.to_q);
        let rs = edit.r.min(edit.to_r)..=edit.r.max(edit.to_r);
        let mut edges = BTreeSet::new();
        for q in qs.clone() {
            for r in rs.clone() {
                for (direction, (dq, dr)) in DIRECTIONS.iter().enumerate() {
                    if !qs.contains(&(q + dq)) || !rs.contains(&(r + dr)) {
                        edges.insert(Edge::new(q, r, direction as u8)?);
                    }
                }
            }
        }
        Ok(edges.into_iter().collect())
    }

    fn boundary_site_check(&self, edge: Edge, closing: bool) -> Result<(), String> {
        let other = edge.neighbour();
        if !self.within_world_range(edge.q, edge.r, self.player.build_range)
            && !self.within_world_range(other.0, other.1, self.player.build_range)
        {
            return Err("Walk closer: boundary is outside build reach".into());
        }
        if !closing {
            return Ok(());
        }
        if self.terrain_at(edge.q, edge.r).blocks_construction()
            || self.terrain_at(other.0, other.1).blocks_construction()
        {
            return Err("Boundaries need dry, buildable ground on both sides".into());
        }
        if near_edge(edge, (self.player.x, self.player.y), PLAYER_RADIUS) {
            return Err("Step away from this edge before closing or building it".into());
        }
        for entity in &self.entities {
            let footprint = self.entity_footprint(entity);
            if footprint.iter().any(|c| (c.q, c.r) == (edge.q, edge.r))
                && footprint.iter().any(|c| (c.q, c.r) == other)
            {
                return Err("A building spans this edge".into());
            }
        }
        let (a, b) = edge.ends();
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
                    return Err("Reroute the transport crossing this edge before closing it".into());
                }
            }
        }
        Ok(())
    }

    fn boundary_transaction(&self, edit: &BoundaryEdit) -> BoundaryTransaction {
        let mut transaction = BoundaryTransaction {
            preview: BoundaryPreview {
                edges: Vec::new(),
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
            transaction.preview.edges = Self::boundary_edges(edit)?;
            let definition = if matches!(edit.action, BoundaryAction::Build) {
                let d = self
                    .boundary_definition(edit.definition_id)
                    .ok_or("Unknown boundary material")?;
                if edit.area && d.gate {
                    return Err("Place gates on individual edges after enclosing the area".into());
                }
                Some(d)
            } else {
                None
            };
            for &edge in &transaction.preview.edges {
                let before = self.boundaries.get(&edge).cloned();
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
                            edge,
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
                        let mut boundary = before.clone().ok_or("Select a gate edge first")?;
                        if !self
                            .boundary_definition(boundary.definition_id)
                            .is_some_and(|d| d.gate)
                        {
                            return Err(
                                "This is a fence. Replace it with a gate to create a crossing"
                                    .into(),
                            );
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
                    edge,
                    after.as_ref().is_some_and(|b| !b.open)
                        || matches!(edit.action, BoundaryAction::Build),
                )
                .map_err(|reason| {
                    format!(
                        "{} edge of hex {}, {}: {reason}",
                        ["East", "Southeast", "Southwest"][edge.direction as usize],
                        edge.q,
                        edge.r
                    )
                })?;
                transaction.undo.before.push((edge, before));
                transaction.undo.after.push((edge, after));
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
        for (edge, after) in &transaction.undo.after {
            cells.insert((edge.q, edge.r));
            cells.insert(edge.neighbour());
            if let Some(boundary) = after {
                self.boundaries.insert(*edge, boundary.clone());
            } else {
                self.boundaries.remove(edge);
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
            "Updated {} boundary edge{}",
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
                edges: Vec::new(),
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
        for (edge, before) in &transaction.undo.before {
            if self.boundaries.get(edge) != before.as_ref() {
                return Err("Boundary changed since that edit".into());
            }
        }
        for (edge, after) in &transaction.undo.after {
            // Restoring an open gate is construction too: a newly placed building may span it.
            self.boundary_site_check(*edge, after.is_some())?;
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
    let mut edges = BTreeSet::new();
    for b in saved {
        let d = definitions
            .boundaries
            .iter()
            .find(|d| d.id == b.definition_id)
            .ok_or("Unknown saved boundary")?;
        if Edge::new(b.edge.q, b.edge.r, b.edge.direction)? != b.edge
            || !edges.insert(b.edge)
            || (b.open && !d.gate)
            || (!b.paid.is_empty() && b.paid != d.construction_cost)
        {
            return Err("Invalid saved boundary identity, state or paid bill".into());
        }
    }
    Ok(())
}
