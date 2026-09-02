//! construction — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    pub(crate) fn placement_legality(
        &self,
        q: i32,
        r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
        check_cost: bool,
    ) -> Result<(), String> {
        let definition = self
            .building_definition(definition_id)
            .ok_or_else(|| format!("unknown building definition {definition_id}"))?;
        if !definition.buildable {
            return Err("this scenario object cannot be constructed".into());
        }
        if !definition.orientation_axis.allows(orientation) {
            let range = definition.orientation_axis.range();
            return Err(format!(
                "{} must be oriented in {}..{}",
                definition.name, range.start, range.end
            ));
        }
        for (gate, message) in definition.gates_at(orientation).into_iter().zip([
            "building is locked by research",
            "this heading is locked by research",
        ]) {
            if let Some(required) = gate {
                if !self.researched.contains(&required) {
                    return Err(message.into());
                }
            }
        }
        let placed = PlacedBuilding {
            q,
            r,
            definition_id,
            orientation,
            recipe_id,
            scenario_owned: false,
        };
        let footprint = self.footprint_for(placed, orientation);
        if footprint.is_empty() {
            return Err("building footprint is empty".into());
        }
        if !footprint
            .iter()
            .any(|cell| self.within_world_range(cell.q, cell.r, self.player.build_range))
        {
            return Err("placement is outside build range".into());
        }
        if self.boundary_crosses_footprint(&footprint) {
            return Err("A boundary crosses this building footprint; remove it first".into());
        }
        let envelope = self.envelope_for(placed, orientation);
        let clearance = self.clearance_for(placed, orientation);
        if self.boundary_crosses_footprint(&envelope) {
            return Err(
                "A boundary crosses this building's service envelope; remove it first".into(),
            );
        }
        for cell in &footprint {
            let supported_transport = definition.kind == BuildingKind::Belt
                && self.bridge_at(cell.q, cell.r)
                && self
                    .entity_at(cell.q, cell.r)
                    .is_some_and(|index| self.entities[index].kind == BuildingKind::Bridge);
            self.reservation_conflict(cell.q, cell.r, definition.kind, None, supported_transport)?;
            let (cell_x, cell_y) = axial_world(cell.q, cell.r);
            if circles_overlap(
                self.player.x,
                self.player.y,
                PLAYER_RADIUS,
                cell_x,
                cell_y,
                BUILDING_RADIUS,
            ) {
                return Err("the player blocks this footprint".into());
            }
            let shallow_support = definition.placement_rule == PlacementRule::Shallows
                && self.shallow_water_at(cell.q, cell.r);
            let bridged_transport = definition.kind == BuildingKind::Belt
                && self.shallow_water_at(cell.q, cell.r)
                && self.bridge_at(cell.q, cell.r);
            if self.terrain_blocks_construction(cell.q, cell.r)
                && !shallow_support
                && !bridged_transport
            {
                return Err("environment blocks construction".into());
            }
        }
        for cell in &envelope {
            self.reservation_conflict(cell.q, cell.r, definition.kind, None, false)?;
            let shallow_support = definition.placement_rule == PlacementRule::Shallows
                && self.shallow_water_at(cell.q, cell.r);
            if self.terrain_blocks_construction(cell.q, cell.r) && !shallow_support {
                return Err("environment blocks construction".into());
            }
        }
        for cell in &clearance {
            // Clearance is air: low infrastructure may already stand here, and the ground does
            // not have to be a pad. Other machines, envelopes and rotors still cannot share it.
            self.reservation_conflict(cell.q, cell.r, definition.kind, None, true)?;
            if let Some(index) = self.entity_at(cell.q, cell.r) {
                if !Self::is_low_infrastructure(self.entities[index].kind) {
                    return Err("building footprint overlaps an occupied hex".into());
                }
            }
        }
        // A footprint has to sit on ground level enough to stand a building on. Measuring the whole
        // occupied foundation's spread rather than each neighbouring pair is what makes a level pad
        // worth grading: a multi-hex machine on a hillside asks the player to prepare a site first,
        // and the site they prepare is exactly the one the preview showed them. Envelope and
        // clearance are reservations, not the pad.
        if let (Some(low), Some(high)) = (
            footprint
                .iter()
                .map(|cell| self.ground_elevation_at(cell.q, cell.r))
                .min(),
            footprint
                .iter()
                .map(|cell| self.ground_elevation_at(cell.q, cell.r))
                .max(),
        ) {
            if high - low > self.pad_step_limit(definition.foundation_class) {
                return Err("This ground is too uneven; level a pad for this footprint".into());
            }
        }
        if definition.placement_rule == PlacementRule::Resource
            && !self.extractable_deposit(definition.id, (q, r))
        {
            return Err(if let Some(item) = definition.output_item_id {
                format!(
                    "{} requires a non-empty {} deposit",
                    definition.name,
                    self.item_name(item)
                )
            } else if self
                .field_at(q, r)
                .and_then(|field| self.item_definition(field.item_id))
                .is_some_and(|item| item.extraction_building_id.is_some())
            {
                "This deposit requires an oil well, not an ordinary extractor".into()
            } else {
                "extractors require a non-empty deposit".into()
            });
        }
        let source_radius = definition.extract_radius.unwrap_or(PUMP_RADIUS as u32) as i32;
        if definition.placement_rule == PlacementRule::Water
            && !self.water_within_reach(q, r, source_radius)
        {
            return Err("must be placed beside open water".into());
        }
        if definition.placement_rule == PlacementRule::Elevated {
            let terrain = self.terrain_at(q, r);
            if !matches!(terrain, Terrain::Hills | Terrain::Highland) {
                return Err("wind turbines must stand on hills or highland".into());
            }
        }
        if definition.placement_rule == PlacementRule::Shallows && !self.shallow_water_at(q, r) {
            return Err("bridges require shallow water".into());
        }
        if definition.kind == BuildingKind::Composer {
            let id = recipe_id.ok_or("this machine requires a recipe")?;
            let recipe = self
                .recipe(id)
                .ok_or_else(|| format!("unknown recipe {id}"))?;
            // One field, one check: a kiln cannot be given a circuit recipe because the categories
            // disagree, not because there is a separate building kind for every machine.
            if !definition.supports_recipe(recipe) {
                return Err(format!(
                    "{} cannot run a {} recipe",
                    definition.name, recipe.category
                ));
            }
        }
        // Transport exists to deliver. A belt aimed at something that can never take an item is not
        // a slow belt, it is a dead one, and the old game only told the player so much later, when
        // the line silently backed up. So the question moves from delivery time to construction
        // time, and the refusal names the hex that is refusing and why.
        //
        // Only the facing is judged, and only for transport. A splitter's flanks may legitimately
        // point at anything, and a machine that happens to face a power pole is still a perfectly
        // good machine — refusing those would be hostile. A belt exists for one purpose and a drag
        // chooses its own heading, so it is the one that can be held to it.
        if definition.kind == BuildingKind::Belt {
            if let Some((target, (cell_q, cell_r))) =
                self.prospective_output(&footprint, definition, orientation)
            {
                let blocked = &self.entities[target];
                // A bridge is a support a belt may itself stand on, so a belt aimed at a bare
                // bridge hex is aimed at the belt that will stand there: not accepting *yet*,
                // rather than never.
                if never_accepts_deliveries(blocked.kind) && blocked.kind != BuildingKind::Bridge {
                    let name = self
                        .building_definition(blocked.placed.definition_id)
                        .map(|value| value.name.clone())
                        .unwrap_or_else(|| "that building".into());
                    return Err(format!(
                        "this {} would deliver into the {name} at {cell_q}, {cell_r}, which never takes items",
                        definition.name.to_lowercase()
                    ));
                }
                if !self.prospective_transport_target_compatible(definition, target) {
                    let name = self
                        .building_definition(blocked.placed.definition_id)
                        .map(|value| value.name.clone())
                        .unwrap_or_else(|| "that building".into());
                    return Err(format!(
                        "the {} and {name} carry incompatible cargo",
                        definition.name.to_lowercase()
                    ));
                }
            }
        }
        // Creative builds for free, so it is asked for nothing. Every other rule above still
        // applies — terrain, footprint, overlap, reach, orientation — because a creative layout that
        // could not be built in a priced run would be no use as a test of one.
        if check_cost && !self.creative {
            let missing: Vec<String> = definition
                .cost_at(orientation)
                .iter()
                .filter_map(|ingredient| {
                    let have = self
                        .player
                        .inventory
                        .get(&ingredient.item_id)
                        .copied()
                        .unwrap_or(0);
                    if have >= ingredient.quantity {
                        return None;
                    }
                    let name = self
                        .item_definition(ingredient.item_id)
                        .map(|item| item.name.as_str())
                        .unwrap_or("item");
                    Some(format!("{} {name} (have {have})", ingredient.quantity))
                })
                .collect();
            if !missing.is_empty() {
                return Err(format!("need {}", missing.join(" · ")));
            }
        }
        Ok(())
    }

    pub(crate) fn place(
        &mut self,
        q: i32,
        r: i32,
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> Result<(), String> {
        let old_links = self.graph_links_by_id();
        let (x, y) = axial_world(q, r);
        self.ensure_neighborhood(x, y);
        self.placement_legality(q, r, definition_id, orientation, recipe_id, true)?;
        let definition = self.building_definition(definition_id).unwrap().clone();
        if !self.creative {
            for ingredient in definition.cost_at(orientation) {
                subtract_item(
                    &mut self.player.inventory,
                    ingredient.item_id,
                    ingredient.quantity,
                );
            }
        }
        let id = self.next_entity_id;
        let placed = PlacedBuilding {
            q,
            r,
            definition_id,
            orientation,
            recipe_id,
            scenario_owned: false,
        };
        self.entities.push(Entity {
            id,
            placed,
            kind: definition.kind,
            cargo: None,
            inventory: BTreeMap::new(),
            input_inventory: BTreeMap::new(),
            fuel_inventory: BTreeMap::new(),
            output_inventory: BTreeMap::new(),
            reserved_inputs: BTreeMap::new(),
            progress: 0,
            fuel_charge: 0,
            power_charge: 0,
            burn_progress: 0,
            disabled: definition.manual_work,
            route_cursor: 0,
            merge_cursor: 0,
            lane: Vec::new(),
        });
        self.next_entity_id += 1;
        self.undo_stack.push(id);
        if self.undo_stack.len() > MAX_UNDO_DEPTH {
            self.undo_stack.remove(0);
        }
        self.dirty.entities.push(id);
        // A chunk's reported entity count changes with the blueprint.
        self.dirty.chunks = true;
        let changed_cells = self
            .footprint_for(placed, orientation)
            .into_iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        self.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([id]));
        self.events.push(format!("Placed {}", definition.name));
        Ok(())
    }

    /// One drag of construction. The host sends the endpoints it dragged between and nothing else:
    /// every cell, orientation, legality result, and cost is resolved here, and each cell goes
    /// through the same `place` the single-cell command uses, so a drag can only ever produce what
    /// the equivalent individual placements would have produced.
    ///
    /// Illegal cells are skipped rather than aborting the run, so dragging a belt past a rock or
    /// past the end of the materials builds everything it legally can. The per-cell events are
    /// replaced by one summary, because ten "Placed Belt" lines is not feedback.
    pub(crate) fn place_line(
        &mut self,
        from: (i32, i32),
        to: (i32, i32),
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> Result<(), String> {
        let definition = self
            .building_definition(definition_id)
            .ok_or_else(|| format!("unknown building definition {definition_id}"))?;
        let routed = definition.kind == BuildingKind::Belt;
        let paired_underpass = definition.underpass_span.is_some() && from != to;
        let name = definition.name.clone();
        let cells = self.drag_route(from, to, definition_id, orientation, recipe_id);
        if paired_underpass {
            let preview = self.line_preview(from, to, definition_id, orientation, recipe_id);
            if preview.len() != 2 || preview.iter().any(|cell| !cell.legal) {
                return Err(preview
                    .iter()
                    .find_map(|cell| cell.reason.clone())
                    .unwrap_or_else(|| {
                        "both underpass portals must be clear and affordable".into()
                    }));
            }
        }
        let before = self.events.len();
        let mut placed = 0usize;
        let mut last_error = None;
        for (index, &(q, r)) in cells.iter().enumerate() {
            // A belt run points every cell at the next one, so the drag routes the line and the
            // player never orients a segment by hand. The final cell keeps the run's heading.
            let cell_orientation = if routed {
                Self::run_orientation(&cells, index, orientation)
            } else {
                orientation
            };
            match self.place(q, r, definition_id, cell_orientation, recipe_id) {
                Ok(()) => placed += 1,
                Err(error) => last_error = Some(error),
            }
        }
        self.events.truncate(before);
        match (placed, last_error) {
            (0, Some(error)) => Err(error),
            (0, None) => Err("nothing to build along that drag".into()),
            (count, reason) => {
                self.events.push(if paired_underpass && count == 2 {
                    format!("Placed {name} pair")
                } else if count == 1 {
                    format!("Placed {name}")
                } else {
                    format!("Placed {count} × {name}")
                });
                // A run that stopped short says why. Silently building four of ten is the kind of
                // thing a player notices only much later, when the line does not work.
                if let Some(reason) = reason {
                    self.events.push(format!("Run stopped: {reason}"));
                }
                Ok(())
            }
        }
    }

    /// The heading a belt takes at `index` along `cells`: toward its successor, or — for the last
    /// cell — continuing the heading it arrived on. Shared by the drag and its preview so the two
    /// cannot disagree.
    pub(crate) fn run_orientation(cells: &[(i32, i32)], index: usize, fallback: u8) -> u8 {
        let (q, r) = cells[index];
        match cells.get(index + 1) {
            Some(&next) => step_direction((q, r), next),
            None => index
                .checked_sub(1)
                .and_then(|previous| cells.get(previous))
                .and_then(|&previous| step_direction(previous, (q, r))),
        }
        .unwrap_or(fallback)
    }

    /// What a construction drag between these endpoints would do, without doing it. Materials are
    /// spent against a copy of the player's inventory as the run is walked, so the preview shows
    /// exactly where a run will stop for cost rather than implying the whole line is affordable.
    /// `recipe_id` travels with the preview for the same reason it travels with the drag: a
    /// machine's legality now depends on whether its recipe belongs to its category, so a preview
    /// that asked without one would refuse every cell of a run the drag would happily build.
    pub(crate) fn line_preview(
        &self,
        from: (i32, i32),
        to: (i32, i32),
        definition_id: DefinitionId,
        orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> Vec<LinePreviewCell> {
        let Some(definition) = self.building_definition(definition_id) else {
            return Vec::new();
        };
        let routed = definition.kind == BuildingKind::Belt;
        let definition = definition.clone();
        let cells = self.drag_route(from, to, definition_id, orientation, recipe_id);
        let mut budget = self.player.inventory.clone();
        let mut taken = BTreeSet::new();
        cells
            .iter()
            .enumerate()
            .map(|(index, &(q, r))| {
                let cell_orientation = if routed {
                    Self::run_orientation(&cells, index, orientation)
                } else {
                    orientation
                };
                // A run that turns can change price partway along it, so the budget is charged the
                // heading each cell actually takes rather than the heading the drag started at.
                let cost = definition.cost_at(cell_orientation);
                let reason = self
                    .placement_legality(q, r, definition_id, cell_orientation, recipe_id, false)
                    .err();
                let legal = !taken.contains(&(q, r))
                    && reason.is_none()
                    && (self.creative || has_ingredients(&budget, cost));
                if legal {
                    if !self.creative {
                        for ingredient in cost {
                            subtract_item(&mut budget, ingredient.item_id, ingredient.quantity);
                        }
                    }
                    taken.insert((q, r));
                }
                LinePreviewCell {
                    q,
                    r,
                    orientation: cell_orientation,
                    legal,
                    reason,
                }
            })
            .collect()
    }

    /// The path a construction drag uses. Ordinary buildings retain the exact line resolver they
    /// have always used. Belts additionally get a bounded deterministic *cheapest* path around
    /// cells on which that belt cannot be placed, so an obstacle produces a connected detour rather
    /// than a straight run with a hole in it.
    ///
    /// The search walks every heading the definition's axis allows — all twelve, for a belt that
    /// has both periods — rather than the six edges the old breadth-first version knew about. Four
    /// keys order it, in this order: what the run costs, how many belts it takes, how often it
    /// turns, and how far it strays from the straight line between the endpoints.
    ///
    /// Cost first because a detour that spends less is the one a player would have drawn. Cells
    /// second because a corner step is priced at what it covers, which leaves the two periods level
    /// on cost and lets the route that turns the same distance into fewer entities win. Turns third
    /// because two runs that cost the same and are the same length are told apart by which one
    /// staircases. Straying last, and it is what settles the ordinary case: an unobstructed drag has
    /// several equally short, equally straight routes, and the one the player was shown while
    /// dragging — and the one the reverse *erase* drag retraces, since removal still resolves by
    /// straight line — is the line itself. Every key only grows along a path, which is what makes
    /// this a shortest-path search and not a guess.
    ///
    /// Counting turns is why a search node is a cell *and* the heading it was reached on: whether a
    /// step turns is a fact about the step before it, so a cell reached along two headings is two
    /// states rather than one that has to forget how it got there.
    ///
    /// A heading whose research is not done is not offered to the search at all, so the route
    /// simply does not use it — the path a player gets widens when they unlock the two-row reach,
    /// with no separate branch here to say so.
    ///
    /// Start and destination are allowed into the route even when occupied. That preserves the
    /// useful gesture of dragging out of, or into, an existing belt: the ordinary `place` call will
    /// skip that endpoint while the neighbouring new segment still points at it. Interior cells
    /// must pass the ordinary placement predicate with cost disabled. Heading order is the explicit
    /// tie-break, and the route never exceeds `MAX_LINE_CELLS`.
    pub(crate) fn drag_route(
        &self,
        from: (i32, i32),
        to: (i32, i32),
        definition_id: DefinitionId,
        _orientation: u8,
        recipe_id: Option<RecipeId>,
    ) -> Vec<(i32, i32)> {
        let Some(definition) = self.building_definition(definition_id) else {
            return Vec::new();
        };
        let axis = definition.orientation_axis;
        if let Some(span) = definition.underpass_span.filter(|_| from != to) {
            // One drag places the two portals, never a carpet of underpass entities. The endpoint
            // snaps to the closest reachable heading/length, so a pointer does not need pixel-
            // perfect axial alignment; the native preview publishes the exact snapped pair.
            let target_world = axial_world(to.0, to.1);
            let best = axis
                .range()
                .filter(|&heading| {
                    definition
                        .gates_at(heading)
                        .into_iter()
                        .flatten()
                        .all(|required| self.researched.contains(&required))
                })
                .flat_map(|heading| {
                    let (dq, dr) = TRANSPORT_DIRECTIONS[usize::from(heading)];
                    (2..=span).map(move |steps| {
                        let candidate = (from.0 + dq * steps as i32, from.1 + dr * steps as i32);
                        let world = axial_world(candidate.0, candidate.1);
                        let dx = i64::from(world.0 - target_world.0);
                        let dy = i64::from(world.1 - target_world.1);
                        ((dx * dx + dy * dy, steps, heading), candidate)
                    })
                })
                .min_by_key(|(key, _)| *key)
                .map(|(_, candidate)| candidate);
            return best.map_or_else(|| vec![from], |end| vec![from, end]);
        }
        if definition.kind != BuildingKind::Belt || axis == OrientationAxis::Corner || from == to {
            return line_between(from, to, axis);
        }

        let weights: Vec<u32> = (0..TRANSPORT_DIRECTIONS.len() as u8)
            .map(|heading| {
                definition
                    .cost_at(heading)
                    .iter()
                    .map(|ingredient| ingredient.quantity)
                    .sum()
            })
            .collect();
        // Research is a fact about the heading, so it is settled once here rather than per step.
        // The per-cell predicate below would say the same thing everywhere except the destination,
        // which this search lets the route reach even when it is occupied — and occupancy is the
        // only thing that exemption was ever meant to forgive.
        let headings: Vec<u8> = axis
            .range()
            .filter(|&heading| {
                definition
                    .gates_at(heading)
                    .into_iter()
                    .flatten()
                    .all(|required| self.researched.contains(&required))
            })
            .collect();

        // The line to stay near is the one the player could actually draw, which is the one their
        // research allows: measured against a line that uses a heading no route here may take, the
        // key would push every route toward cells none of them can reach.
        let reachable = if headings.iter().any(|&heading| is_corner_heading(heading)) {
            axis
        } else {
            OrientationAxis::Edge
        };
        let line: BTreeSet<(i32, i32)> = line_between(from, to, reachable).into_iter().collect();

        // The heading a node was reached on, for the start, which turned nothing to get there.
        const UNTURNED: u8 = u8::MAX;
        // Key first, node second, so the heap orders on the key and the node only ever breaks a tie
        // — which is what keeps two equal routes from resolving differently on two machines.
        type Node = ((i32, i32), u8);
        type Key = (u32, usize, usize, usize);
        let start: Node = (from, UNTURNED);
        let mut frontier =
            BinaryHeap::from([std::cmp::Reverse(((0u32, 0usize, 0usize, 0usize), start))]);
        let mut best: BTreeMap<Node, Key> = BTreeMap::from([(start, (0, 0, 0, 0))]);
        let mut previous: BTreeMap<Node, Node> = BTreeMap::new();
        while let Some(std::cmp::Reverse((key, current))) = frontier.pop() {
            // A cheaper route to this node already left the heap, so this entry is stale.
            if best.get(&current).is_some_and(|&known| known < key) {
                continue;
            }
            let (cell, arrived_on) = current;
            if cell == to {
                let mut route = vec![to];
                let mut node = current;
                while let Some(&step) = previous.get(&node) {
                    route.push(step.0);
                    node = step;
                }
                route.reverse();
                return route;
            }
            let (spent, cells, turns, strayed) = key;
            if cells + 1 >= MAX_LINE_CELLS {
                continue;
            }
            for &heading in &headings {
                let (dq, dr) = TRANSPORT_DIRECTIONS[usize::from(heading)];
                let next: Node = ((cell.0 + dq, cell.1 + dr), heading);
                if next.0 != to
                    && self
                        .placement_legality(
                            next.0 .0,
                            next.0 .1,
                            definition_id,
                            heading,
                            recipe_id,
                            false,
                        )
                        .is_err()
                {
                    continue;
                }
                let candidate = (
                    spent + weights[usize::from(heading)],
                    cells + 1,
                    turns + usize::from(arrived_on != UNTURNED && arrived_on != heading),
                    strayed + usize::from(!line.contains(&next.0)),
                );
                if best.get(&next).is_some_and(|&known| known <= candidate) {
                    continue;
                }
                best.insert(next, candidate);
                previous.insert(next, current);
                frontier.push(std::cmp::Reverse((candidate, next)));
            }
        }

        // A destination outside the bounded legal search still gets the historical line preview,
        // including its visible refused cells, instead of disappearing from the drag entirely.
        line_between(from, to, axis)
    }

    /// What a removal drag between these endpoints would take back. Refunds accumulate against a
    /// copy of the player's inventory as the run is walked, for the same reason the construction
    /// preview spends materials against one: the cell a run stops at has to be visible before the
    /// drag is released, whether it stops for cost or for carrying space.
    pub(crate) fn erase_line_preview(
        &self,
        from: (i32, i32),
        to: (i32, i32),
    ) -> Vec<LinePreviewCell> {
        let mut taken = BTreeSet::new();
        line_between(from, to, self.erase_line_axis(from))
            .into_iter()
            .map(|(q, r)| {
                let in_range = self.within_build_range_of_target(q, r);
                let removable = self.entity_at(q, r).filter(|&index| {
                    !self.entities[index].placed.scenario_owned
                        && !taken.contains(&self.entities[index].id)
                });
                // A full pack no longer refuses a recovery — whatever will not fit falls at the
                // site — so the preview no longer walks a running total of what the pack could
                // still take. `taken` stays: a multi-cell footprint is reached from several cells
                // of the drag, and only the first of them removes anything.
                if in_range {
                    if let Some(index) = removable {
                        taken.insert(self.entities[index].id);
                    }
                }
                let legal = in_range && removable.is_some();
                LinePreviewCell {
                    q,
                    r,
                    orientation: 0,
                    legal,
                    reason: None,
                }
            })
            .collect()
    }

    /// Which axis a removal drag walks. Erasure carries no definition to ask, so it asks the hex
    /// the drag started on: a run that begins on a two-row belt takes back the two-row column, and
    /// every other run walks the six edges exactly as it did before v0.14. Deterministic and
    /// native, like the path itself.
    ///
    /// A definition that takes every heading cannot answer this on its own — that was the one thing
    /// the riser's separate definition was carrying that the unified belt does not. The *entity's*
    /// heading carries it instead, which is the same fact in the place it actually belongs: this
    /// run is in the period the belt under the player's cursor is in.
    pub(crate) fn erase_line_axis(&self, from: (i32, i32)) -> OrientationAxis {
        let Some(index) = self.entity_at(from.0, from.1) else {
            return OrientationAxis::default();
        };
        let orientation = self.entities[index].placed.orientation;
        match self
            .building_definition(self.entities[index].placed.definition_id)
            .map(|definition| definition.orientation_axis)
        {
            Some(OrientationAxis::Any) if is_corner_heading(orientation) => OrientationAxis::Corner,
            Some(OrientationAxis::Any) => OrientationAxis::Edge,
            Some(axis) => axis,
            None => OrientationAxis::default(),
        }
    }

    /// One drag of removal, resolved exactly as `place_line` resolves construction.
    pub(crate) fn erase_line(&mut self, from: (i32, i32), to: (i32, i32)) -> Result<(), String> {
        let cells = line_between(from, to, self.erase_line_axis(from));
        let before = self.events.len();
        let mut removed = 0usize;
        let mut last_error = None;
        for &(q, r) in &cells {
            // A multi-cell footprint is reached from several cells of the drag; the first one
            // removes it and the rest simply find nothing there.
            match self.erase(q, r) {
                Ok(()) => removed += 1,
                Err(error) => last_error = Some(error),
            }
        }
        self.events.truncate(before);
        match (removed, last_error) {
            (0, Some(error)) => Err(error),
            (0, None) => Err("nothing to remove along that drag".into()),
            (count, _) => {
                // Dragging across ground that holds nothing is the normal case for erasure, so
                // unlike construction it is not worth reporting a reason for each empty cell.
                self.events.push(if count == 1 {
                    "Recovered 1 building".into()
                } else {
                    format!("Recovered {count} buildings")
                });
                Ok(())
            }
        }
    }

    /// Take back the most recent construction this session made, through the ordinary erase path so
    /// the refund is the tested one. A construction that has already been removed is skipped, and a
    /// failed undo keeps its entry so the player can walk back into range and retry.
    pub(crate) fn undo(&mut self) -> Result<(), String> {
        while let Some(&id) = self.undo_stack.last() {
            let Some(index) = self.index_of_entity(id) else {
                self.undo_stack.pop();
                continue;
            };
            let (q, r) = (self.entities[index].placed.q, self.entities[index].placed.r);
            let before = self.events.len();
            let result = self.erase(q, r);
            self.events.truncate(before);
            result?;
            self.undo_stack.pop();
            self.events.push("Undid the last construction".into());
            return Ok(());
        }
        Err("nothing to undo".into())
    }

    pub(crate) fn erase(&mut self, q: i32, r: i32) -> Result<(), String> {
        if !self.within_build_range_of_target(q, r) {
            return Err("erase target is outside build range".into());
        }
        let index = self.entity_at(q, r).ok_or("no building to erase")?;
        if self.entities[index].placed.scenario_owned {
            return Err("scenario-owned objects are protected".into());
        }
        // Construction cost and stored contents come back to the pack, and whatever will not fit
        // falls at the site as real ground items — the same treatment in-transit cargo has always
        // had. Refusing the demolition instead was the worse trade: it left the player holding a
        // full pack and a full building with no order of operations that emptied either, and the
        // building they wanted gone stayed. The host warns first and says the ground items are on a
        // timer, so the loss is a decision rather than a surprise.
        let refund = self.erase_refund(index);
        let (carried, spilled) = self.split_by_carry(&refund);
        let old_links = self.graph_links_by_id();
        let changed_cells = self
            .entity_footprint(&self.entities[index])
            .into_iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        let entity = self.entities.remove(index);
        self.deposit_links.remove(&entity.id);
        self.output_routes.remove(&entity.id);
        self.legacy_fluid_belts.remove(&entity.id);
        self.dirty.removed.insert(entity.id);
        self.dirty.chunks = true;
        let name = self
            .building_definition(entity.placed.definition_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| "building".into());
        add_inventory(&mut self.player.inventory, &carried);
        // Everything the belt was carrying, not just what had reached its far end: an item halfway
        // along a conveyor is as real as the one waiting at the end of it, and demolishing the
        // conveyor under it drops it on the ground rather than deleting it.
        for cargo in Self::belt_contents(&entity).collect::<Vec<_>>() {
            self.add_ground_item(
                entity.placed.q,
                entity.placed.r,
                cargo.item_id,
                cargo.quantity,
            );
        }
        for (&item, &quantity) in &spilled {
            self.add_ground_item(entity.placed.q, entity.placed.r, item, quantity);
        }
        self.recompile_graph_components(&old_links, &changed_cells, &BTreeSet::from([entity.id]));
        self.events.push(format!("Recovered {name}"));
        if !spilled.is_empty() {
            let total: u32 = spilled.values().sum();
            self.events.push(format!(
                "{total} items would not fit your pack and fell at the site"
            ));
        }
        Ok(())
    }

    /// Everything erasing this entity hands back: its construction cost, stored inventory, and
    /// reserved recipe inputs. In-transit cargo is deliberately absent because `erase` spills it
    /// on the ground at the removed entity's anchor.
    ///
    /// Creative recovers nothing. Building costs nothing there, so there is nothing owed back, and
    /// nothing to spill either — a creative player clearing a full factory leaves no litter behind
    /// them. One rule here covers every route: single erase, drag erase, the drag's preview, and
    /// undo.
    pub(crate) fn erase_refund(&self, index: usize) -> BTreeMap<ItemId, u32> {
        if self.creative {
            return BTreeMap::new();
        }
        let entity = &self.entities[index];
        let mut refund = BTreeMap::new();
        if let Some(definition) = self.building_definition(entity.placed.definition_id) {
            add_ingredients(&mut refund, definition.cost_at(entity.placed.orientation));
        }
        add_inventory(&mut refund, &entity.inventory);
        add_inventory(&mut refund, &entity.input_inventory);
        add_inventory(&mut refund, &entity.fuel_inventory);
        add_inventory(&mut refund, &entity.output_inventory);
        add_inventory(&mut refund, &entity.reserved_inputs);
        refund
    }
}
