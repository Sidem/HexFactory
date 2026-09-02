//! graph — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    pub(crate) fn compile_graph(&mut self) {
        let (occupied, envelope, clearance) = self.occupancy_maps();
        self.graph = self
            .entities
            .iter()
            .enumerate()
            .map(|(index, _)| self.compile_links(index, &occupied))
            .collect();
        self.rebuild_runtime_index(occupied, envelope, clearance);
        self.compile_power();
        // A full compile can move any entity's outgoing link, and `next_id` is part of its snapshot.
        self.mark_all_entities_dirty();
    }

    pub(crate) fn rebuild_runtime_index(
        &mut self,
        occupied: BTreeMap<(i32, i32), usize>,
        envelope: BTreeMap<(i32, i32), usize>,
        clearance: BTreeMap<(i32, i32), usize>,
    ) {
        let mergers = (0..self.entities.len())
            .map(|index| self.is_merger(index))
            .collect();
        self.runtime.rebuild(
            &self.entities,
            &self.graph,
            mergers,
            occupied,
            envelope,
            clearance,
        );
        // Every edit and every load funnels through here, and `occupied` is half of what a route is
        // made of, so this is the one place a standing walk has to be re-answered against the world
        // it is crossing. It is also what builds the route after a load, since the goal is saved
        // and the path deliberately is not.
        self.replan_walk();
    }

    #[cfg(test)]
    pub(crate) fn occupied_entities(&self) -> BTreeMap<(i32, i32), usize> {
        self.occupancy_maps().0
    }

    /// Occupied foundation, service envelope and overhead clearance as three derived maps.
    ///
    /// Occupied is the only one the transport graph and the walk read. Envelope and clearance are
    /// placement reservations: they are never saved or checksummed, and they rebuild with the
    /// occupancy index after every edit.
    pub(crate) fn occupancy_maps(
        &self,
    ) -> (
        BTreeMap<(i32, i32), usize>,
        BTreeMap<(i32, i32), usize>,
        BTreeMap<(i32, i32), usize>,
    ) {
        let mut occupied = BTreeMap::new();
        let mut envelope = BTreeMap::new();
        let mut clearance = BTreeMap::new();
        for (index, entity) in self.entities.iter().enumerate() {
            for cell in self.entity_footprint(entity) {
                occupied.insert((cell.q, cell.r), index);
            }
            for cell in self.entity_envelope(entity) {
                envelope.insert((cell.q, cell.r), index);
            }
            for cell in self.entity_clearance(entity) {
                clearance.insert((cell.q, cell.r), index);
            }
        }
        (occupied, envelope, clearance)
    }

    pub(crate) fn reserved_name(&self, index: usize) -> String {
        self.entities
            .get(index)
            .and_then(|entity| self.building_definition(entity.placed.definition_id))
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| "that building".into())
    }

    /// Whether this cell is already claimed in a way this kind cannot share.
    ///
    /// `ignore` is the building whose own envelope or clearance we are growing into, so an
    /// upgrade does not refuse the reservation it already holds.
    pub(crate) fn reservation_conflict(
        &self,
        q: i32,
        r: i32,
        kind: BuildingKind,
        ignore: Option<usize>,
        occupied_ok: bool,
    ) -> Result<(), String> {
        if !occupied_ok {
            if let Some(index) = self.entity_at(q, r) {
                if ignore != Some(index) {
                    return Err("building footprint overlaps an occupied hex".into());
                }
            }
        }
        if let Some(&index) = self.runtime.envelope.get(&(q, r)) {
            if ignore != Some(index) {
                return Err(format!(
                    "this hex is reserved around the {}",
                    self.reserved_name(index).to_lowercase()
                ));
            }
        }
        if let Some(&index) = self.runtime.clearance.get(&(q, r)) {
            if ignore != Some(index) && !Self::is_low_infrastructure(kind) {
                return Err(format!(
                    "the {}'s overhead clearance occupies this hex",
                    self.reserved_name(index).to_lowercase()
                ));
            }
        }
        Ok(())
    }

    /// Every outgoing transport edge one entity compiles.
    ///
    /// One edge for everything the game had before splitters: its facing. A splitter additionally
    /// rays the two headings 60° either side, which is the entire difference between it and a belt
    /// — the tick still walks compiled edges and never discovers a neighbour.
    pub(crate) fn compile_links(
        &self,
        index: usize,
        occupied: &BTreeMap<(i32, i32), usize>,
    ) -> Links {
        let entity = &self.entities[index];
        let Some(definition) = self.building_definition(entity.placed.definition_id) else {
            return Links::default();
        };
        if let Some(routes) = self.output_routes.get(&entity.id) {
            if !routes.is_empty() {
                let mut links = Links::default();
                for (&item_id, &route) in routes {
                    let origin = (entity.placed.q + route.q, entity.placed.r + route.r);
                    if let Some(target) =
                        self.trace_output_from(index, origin, route.direction, None, occupied)
                    {
                        links.push_item(Some(item_id), target);
                    }
                }
                return links;
            }
        }
        let facing = entity.placed.orientation;
        let span = definition.underpass_span;
        let mut links = Links::single(self.trace_output(index, facing, span, occupied));
        if definition.splits {
            for flank in flanks_of(facing) {
                if let Some(target) = self.trace_output(index, flank, span, occupied) {
                    links.push(target);
                }
            }
        }
        links
    }

    /// The building one output ray binds to, on one heading.
    ///
    /// An underpass tries its partner first and falls back to the ordinary ray, and that fallback
    /// is what makes the pair work with one definition and no placement mode: the *entrance* is
    /// simply the underpass that found a partner ahead of it, and the *exit* is the one that did
    /// not, so it delivers to whatever it is pointed at like any other belt.
    pub(crate) fn trace_output(
        &self,
        index: usize,
        orientation: u8,
        underpass_span: Option<u32>,
        occupied: &BTreeMap<(i32, i32), usize>,
    ) -> Option<usize> {
        let placed = self.entities[index].placed;
        self.trace_output_from(
            index,
            (placed.q, placed.r),
            orientation,
            underpass_span,
            occupied,
        )
    }

    pub(crate) fn trace_output_from(
        &self,
        index: usize,
        origin: (i32, i32),
        orientation: u8,
        underpass_span: Option<u32>,
        occupied: &BTreeMap<(i32, i32), usize>,
    ) -> Option<usize> {
        let target = underpass_span
            .and_then(|span| self.trace_underpass(index, orientation, span, occupied))
            .or_else(|| self.trace_ray_from(index, origin, orientation, occupied));
        target.filter(|&target| {
            // A dead edge is not compiled at all. Binding one and letting every tick's transfer
            // refuse it spends arbitration on a delivery that can never land, and draws the player
            // a connected line that silently is not one.
            if never_accepts_deliveries(self.entities[target].kind) {
                return false;
            }
            if !self.transport_target_compatible(index, target) {
                return false;
            }
            let to = &self.entities[target].placed;
            !self.boundary_blocks_segment(axial_world(origin.0, origin.1), axial_world(to.q, to.r))
        })
    }

    /// The entity an output ray on this heading would bind to for a building that is not placed
    /// yet, with the hex it binds at.
    ///
    /// Deliberately mirrors `trace_underpass` then `trace_ray`, in that order and with the same
    /// step table, limit, and skip-own-footprint rule, so construction refuses exactly the edge the
    /// graph compile would otherwise go on to build. An entrance that finds its partner delivers
    /// past whatever stands between, so it must not be judged on what stands between.
    pub(crate) fn prospective_output(
        &self,
        footprint: &[Coordinate],
        definition: &BuildingDefinition,
        orientation: u8,
    ) -> Option<(usize, (i32, i32))> {
        let anchor = footprint.first().map(|cell| (cell.q, cell.r))?;
        let (dq, dr) = TRANSPORT_DIRECTIONS[usize::from(orientation) % TRANSPORT_DIRECTIONS.len()];
        if let Some(span) = definition.underpass_span {
            let (mut q, mut r) = (anchor.0 + dq, anchor.1 + dr);
            for _ in 1..=span.min(GRAPH_TRACE_LIMIT as u32) {
                if let Some(target) = self.entity_at(q, r) {
                    let placed = &self.entities[target].placed;
                    if placed.definition_id == definition.id && placed.orientation == orientation {
                        return None;
                    }
                }
                q += dq;
                r += dr;
            }
        }
        let (mut q, mut r) = (anchor.0 + dq, anchor.1 + dr);
        for _ in 0..GRAPH_TRACE_LIMIT {
            if footprint.iter().any(|cell| cell.q == q && cell.r == r) {
                q += dq;
                r += dr;
                continue;
            }
            let target = self.entity_at(q, r)?;
            let to = &self.entities[target].placed;
            if self
                .boundary_blocks_segment(axial_world(anchor.0, anchor.1), axial_world(to.q, to.r))
            {
                return None;
            }
            return Some((target, (q, r)));
        }
        None
    }

    /// The ordinary transport ray, unchanged since the graph existed.
    ///
    /// Routing, so twelve. The loop is a ray-cast: it steps `(dq, dr)` up to `GRAPH_TRACE_LIMIT`,
    /// skipping its own footprint, and returns the first other occupied cell. Nothing in it ever
    /// assumed the step was a unit vector, which is why the six corner headings cost table rows
    /// here and nothing else.
    pub(crate) fn trace_ray_from(
        &self,
        index: usize,
        origin: (i32, i32),
        orientation: u8,
        occupied: &BTreeMap<(i32, i32), usize>,
    ) -> Option<usize> {
        let (dq, dr) = TRANSPORT_DIRECTIONS[usize::from(orientation) % TRANSPORT_DIRECTIONS.len()];
        let mut q = origin.0 + dq;
        let mut r = origin.1 + dr;
        for _ in 0..GRAPH_TRACE_LIMIT {
            match occupied.get(&(q, r)).copied() {
                Some(target) if target == index => {
                    q += dq;
                    r += dr;
                }
                target => return target,
            }
        }
        None
    }

    /// The partner an underpass hands its cargo to, or `None` if there is none within its span.
    ///
    /// This is the whole of "belts cross belts": the ray passes *over* every occupied cell instead
    /// of binding to the first one, so the line that runs between the two ends is untouched, keeps
    /// its own cargo, and never sees the cargo going over it. What stops that from being a free
    /// belt of unlimited reach is the span, and what stops it from stealing an ordinary delivery is
    /// that it binds to nothing except another underpass of the same definition on the same
    /// heading. The covered hexes stay ordinary ground: buildable, walkable, and erasable.
    pub(crate) fn trace_underpass(
        &self,
        index: usize,
        orientation: u8,
        span: u32,
        occupied: &BTreeMap<(i32, i32), usize>,
    ) -> Option<usize> {
        let entity = &self.entities[index];
        let definition_id = entity.placed.definition_id;
        let (dq, dr) = TRANSPORT_DIRECTIONS[usize::from(orientation) % TRANSPORT_DIRECTIONS.len()];
        let mut q = entity.placed.q + dq;
        let mut r = entity.placed.r + dr;
        let reach = span.min(GRAPH_TRACE_LIMIT as u32);
        for _ in 1..=reach {
            if let Some(target) = occupied.get(&(q, r)).copied() {
                let partner = target != index
                    && self.entities[target].placed.definition_id == definition_id
                    && self.entities[target].placed.orientation == orientation;
                if partner {
                    return Some(target);
                }
            }
            q += dq;
            r += dr;
        }
        None
    }
}
