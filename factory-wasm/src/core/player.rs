//! player — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    /// The host's own movement intent — a key going down, or coming back up.
    ///
    /// Any such command cancels an autonomous walk, including the zero one a key release sends. The
    /// moment the player touches the controls they are driving, and a walk that kept steering
    /// against them would be fighting for the same two numbers. This is why the walk writes
    /// `move_x`/`move_y` directly in [`Core::steer_walk`] rather than calling through here: the
    /// command path is the *cancellation* path, and routing the walk through it would cancel it on
    /// its own first step.
    pub(crate) fn set_move_intent(&mut self, x: i16, y: i16) -> Result<(), String> {
        if !(-1000..=1000).contains(&x) || !(-1000..=1000).contains(&y) {
            return Err("movement intent must be in -1000..1000".into());
        }
        self.clear_walk();
        self.player.move_x = x;
        self.player.move_y = y;
        if x != 0 || y != 0 {
            self.player.facing_x = x;
            self.player.facing_y = y;
        }
        Ok(())
    }

    /// Start walking to a hex, resolving the route here and now.
    ///
    /// A refusal is an event rather than a silent no-op: the player pointed at something, and
    /// "there is no way there" is the answer to what they asked.
    pub(crate) fn walk_to(&mut self, q: i32, r: i32) -> Result<(), String> {
        let here = world_to_axial(self.player.x, self.player.y);
        if (q, r) == here {
            // Already standing on it. Cancelling any walk in flight is the useful reading of a
            // click on your own feet, and it costs no search to answer.
            self.clear_walk();
            return Ok(());
        }
        if axial_distance(here, (q, r)) > MAX_WALK_DISTANCE {
            self.clear_walk();
            return Err("That is too far to walk to in one go".into());
        }
        let Some(path) = self.walk_route(here, (q, r)) else {
            self.clear_walk();
            return Err("No way through to there".into());
        };
        self.player.walk_goal = Some(Coordinate { q, r });
        self.walk_path = path;
        self.walk_stall = 0;
        self.walk_last_position = (self.player.x, self.player.y);
        Ok(())
    }

    /// Stop walking, wherever the walk had got to. Idempotent, and it drops the intent it was
    /// holding so a cancelled walk does not leave the player drifting.
    pub(crate) fn clear_walk(&mut self) {
        if self.player.walk_goal.is_none() {
            return;
        }
        self.player.walk_goal = None;
        self.walk_path.clear();
        self.walk_stall = 0;
        self.player.move_x = 0;
        self.player.move_y = 0;
    }

    /// Rebuild the route to the standing goal against the world as it now is.
    ///
    /// Called from [`Core::rebuild_runtime_index`], which every edit and every load funnels through,
    /// so a wall built across the player's own route is answered the moment it is built rather than
    /// when they arrive at it. That matters as much for the drawing as for the walking: the ribbon
    /// on screen is this path, and a path through a building the player just placed would be the
    /// host promising a walk the simulation will not take.
    ///
    /// A route that no longer exists empties the path and leaves the goal standing. Ending the walk
    /// is [`Core::steer_walk`]'s job alone, for two reasons: it keeps one place deciding what a
    /// finished walk means, and this runs inside a load's `compile_graph`, where clearing a goal the
    /// file recorded would move the checksum out from under the very check that is about to verify
    /// it.
    pub(crate) fn replan_walk(&mut self) {
        let Some(goal) = self.player.walk_goal else {
            return;
        };
        let here = world_to_axial(self.player.x, self.player.y);
        self.walk_path = self.walk_route(here, (goal.q, goal.r)).unwrap_or_default();
        self.walk_stall = 0;
    }

    /// Whether the player's body can stand at the centre of this hex.
    ///
    /// The centre, and not the whole hex: an adjacent pair of walkable centres is 1774 apart, and
    /// the nearest a blocking building's centre can sit to the segment between them is 1536 — both
    /// comfortably clear of the 1270 that `PLAYER_RADIUS + BUILDING_RADIUS` needs. So a route made
    /// of hex centres is one the continuous collision in `player_blocked` will actually let the
    /// player walk, without the route having to model the body it is routing.
    ///
    /// It asks `terrain_at`, which is a pure function of the world parameters and the seed, and
    /// `runtime.occupied`, which is maintained with the compiled topology. Neither generates a
    /// chunk — deliberately. `generated_chunks` is a checksum input, so a search that surveyed the
    /// ground it considered would make *thinking about* a route change the run's checksum.
    pub(crate) fn walkable_hex(&self, q: i32, r: i32) -> bool {
        if self.player_terrain_blocks_movement(q, r) {
            return false;
        }
        !self
            .runtime
            .occupied
            .get(&(q, r))
            .map(|&index| {
                self.entities
                    .get(index)
                    .and_then(|entity| self.building_definition(entity.placed.definition_id))
                    .map(|definition| definition.blocks_movement)
                    .unwrap_or(true)
            })
            .unwrap_or(false)
    }

    /// What entering this hex from `from` costs the route, in hundredths of a dry-ground hex.
    ///
    /// Three things, and each of them is something `player_step` or `advance_player` actually does.
    /// A ford is a fifth speed, so it is priced at five hexes; a prepared surface is faster, so it is
    /// priced at less than one; a step up is real work, so it costs extra. Charging the route for
    /// anything the walk does not pay, or failing to charge it for something the walk does, produces
    /// a route that is short on the map and slow in the hand.
    ///
    /// The surface does not modify the ford. Shallows are a 5 m/s crawl in `player_step` regardless,
    /// and pretending a decked river bank crosses faster would be the search inventing a preference.
    pub(crate) fn walk_step_cost(&self, from: (i32, i32), q: i32, r: i32) -> u32 {
        let base = if self.deep_water_at(q, r) {
            WALK_SWIM_COST
        } else if self.shallow_water_at(q, r) {
            WALK_SHALLOW_COST
        } else {
            WALK_STEP_COST * UNTREATED_MOVEMENT / self.movement_factor_at(q, r)
        };
        let climb = (self.ground_elevation_at(q, r) - self.ground_elevation_at(from.0, from.1))
            .max(0) as u32;
        base + climb * WALK_CLIMB_COST
    }

    /// A* over hex centres, returning the cells still to be walked — nearest first, ending on the
    /// goal — or `None` when there is no route inside the bounds.
    ///
    /// Read-only and integer-only, so it is as reproducible as everything else the checksum covers.
    /// Ties break on `(f, g, q, r)`, which is a total order over distinct cells, so the frontier
    /// never depends on how a heap happened to order two equal keys.
    ///
    /// Three separate bounds hold it: the goal must be within `MAX_WALK_DISTANCE`, the frontier
    /// never leaves that disc around the *start*, and `MAX_WALK_SEARCH_NODES` caps the settle count
    /// for the case the bounds cannot help with — a goal that is reachable-looking and walled off,
    /// where an unbounded search would sweep the whole disc before admitting it.
    pub(crate) fn walk_route(&self, from: (i32, i32), goal: (i32, i32)) -> Option<Vec<Coordinate>> {
        if from == goal {
            return Some(Vec::new());
        }
        if axial_distance(from, goal) > MAX_WALK_DISTANCE || !self.walkable_hex(goal.0, goal.1) {
            return None;
        }

        let mut open: BinaryHeap<Reverse<(u32, u32, i32, i32)>> = BinaryHeap::new();
        let mut best: BTreeMap<(i32, i32), u32> = BTreeMap::new();
        let mut came_from: BTreeMap<(i32, i32), (i32, i32)> = BTreeMap::new();
        best.insert(from, 0);
        open.push(Reverse((
            axial_distance(from, goal) as u32 * MIN_WALK_STEP_COST,
            0,
            from.0,
            from.1,
        )));

        let mut settled = 0usize;
        while let Some(Reverse((_, cost, q, r))) = open.pop() {
            let cell = (q, r);
            if cell == goal {
                return self.walk_path_from(&came_from, from, goal);
            }
            // A cheaper way here was found after this entry was pushed; the heap keeps no
            // decrease-key, so the stale entry is simply skipped.
            if best.get(&cell).copied().is_some_and(|known| known < cost) {
                continue;
            }
            settled += 1;
            if settled > MAX_WALK_SEARCH_NODES {
                return None;
            }
            for (dq, dr) in DIRECTIONS {
                let next = (q.saturating_add(dq), r.saturating_add(dr));
                if axial_distance(from, next) > MAX_WALK_DISTANCE
                    || !self.walkable_hex(next.0, next.1)
                    || self.grade_blocks(cell, next)
                    || self.boundary_blocks_segment(axial_world(q, r), axial_world(next.0, next.1))
                {
                    continue;
                }
                let step = cost + self.walk_step_cost(cell, next.0, next.1);
                if best.get(&next).copied().is_some_and(|known| known <= step) {
                    continue;
                }
                best.insert(next, step);
                came_from.insert(next, cell);
                open.push(Reverse((
                    step + axial_distance(next, goal) as u32 * MIN_WALK_STEP_COST,
                    step,
                    next.0,
                    next.1,
                )));
            }
        }
        None
    }

    /// Walk the predecessor map back from the goal, dropping the cell the player is already on.
    ///
    /// A route longer than `MAX_WALK_PATH_CELLS` is reported as no route at all. It is not a real
    /// shape — inside a 96-hex disc it would have to double back on itself for hundreds of cells —
    /// and the alternative is an unbounded list crossing the wire every frame of the walk.
    pub(crate) fn walk_path_from(
        &self,
        came_from: &BTreeMap<(i32, i32), (i32, i32)>,
        from: (i32, i32),
        goal: (i32, i32),
    ) -> Option<Vec<Coordinate>> {
        let mut path = Vec::new();
        let mut cursor = goal;
        while cursor != from {
            path.push(Coordinate {
                q: cursor.0,
                r: cursor.1,
            });
            if path.len() > MAX_WALK_PATH_CELLS {
                return None;
            }
            cursor = *came_from.get(&cursor)?;
        }
        path.reverse();
        Some(path)
    }

    /// One player-clock step of an autonomous walk: retire the waypoints reached, then aim at the
    /// next one. Writes the intent directly, for the reason [`Core::set_move_intent`] explains.
    pub(crate) fn steer_walk(&mut self) {
        if self.player.walk_goal.is_none() {
            return;
        }
        let (px, py) = (self.player.x, self.player.y);

        // Ground actually covered, not intent issued. A walk pressed against something the route
        // did not predict gets a second to slide out of it and is then handed back to the player,
        // rather than jogging into a wall until they take the controls themselves.
        if (px, py) == self.walk_last_position {
            self.walk_stall += 1;
            if self.walk_stall >= WALK_STALL_STEPS {
                self.clear_walk();
                self.events.push("Stopped — the way is blocked".into());
                return;
            }
        } else {
            self.walk_stall = 0;
        }
        self.walk_last_position = (px, py);

        let reach = i64::from(WALK_ARRIVE_RADIUS).pow(2);
        while let Some(&next) = self.walk_path.first() {
            let (wx, wy) = axial_world(next.q, next.r);
            if squared_distance(px, py, wx, wy) > reach {
                break;
            }
            self.walk_path.remove(0);
        }
        let Some(&target) = self.walk_path.first() else {
            // The goal is the last waypoint, so a route that has run out is either arrival or a
            // route `replan_walk` could not rebuild. Standing on the goal tells the two apart —
            // `WALK_ARRIVE_RADIUS` is the inradius, so being inside it means being in that hex —
            // and only the second is worth saying out loud.
            let here = world_to_axial(px, py);
            let blocked = self.player.walk_goal
                != Some(Coordinate {
                    q: here.0,
                    r: here.1,
                });
            self.clear_walk();
            if blocked {
                self.events.push("The way there is blocked".into());
            }
            return;
        };

        let (tx, ty) = axial_world(target.q, target.r);
        let dx = i64::from(tx) - i64::from(px);
        let dy = i64::from(ty) - i64::from(py);
        let length = integer_sqrt(dx * dx + dy * dy);
        if length == 0 {
            return;
        }
        self.player.move_x = (dx * i64::from(AUTO_WALK_INTENT) / length) as i16;
        self.player.move_y = (dy * i64::from(AUTO_WALK_INTENT) / length) as i16;
        self.player.facing_x = self.player.move_x;
        self.player.facing_y = self.player.move_y;
    }

    /// Face the world position the host is pointing at, resolved here in integer arithmetic so the
    /// checksummed facing vector is native's answer rather than the host's.
    ///
    /// [`Core::set_move_intent`] still sets facing, and an aim wins by arriving later in the same
    /// batch — which is what lets a touch layout that sends no aim keep facing the way it walks,
    /// without a stored aiming mode that the save format and the checksum would then have to carry.
    pub(crate) fn set_aim(&mut self, x: i32, y: i32) -> Result<(), String> {
        let dx = i64::from(x) - i64::from(self.player.x);
        let dy = i64::from(y) - i64::from(self.player.y);
        if dx.abs() > MAX_AIM_DISTANCE || dy.abs() > MAX_AIM_DISTANCE {
            return Err("aim target is out of range".into());
        }
        // The cursor resting exactly on the player names no direction, so the last one stands.
        let length = integer_sqrt(dx * dx + dy * dy);
        if length == 0 {
            return Ok(());
        }
        self.player.facing_x = (dx * 1000 / length) as i16;
        self.player.facing_y = (dy * 1000 / length) as i16;
        Ok(())
    }

    pub(crate) fn advance_player(&mut self) {
        let (dx, dy) = self.player_step();
        if dx == 0 && dy == 0 {
            return;
        }
        self.ensure_neighborhood(self.player.x + dx, self.player.y + dy);
        let next_x = self.player.x + dx;
        if !self.player_blocked(next_x, self.player.y) {
            self.player.x = next_x;
        }
        let next_y = self.player.y + dy;
        if !self.player_blocked(self.player.x, next_y) {
            self.player.y = next_y;
        }
    }

    /// One player-clock step, in world units. Land uses the host's intent against `PLAYER_SPEED`,
    /// scaled by the surface underfoot — the same integer percentage the route search prices, so
    /// the road that looked faster on the map is the road that is faster in the hand.
    /// Shallows are a 5 m/s ford: walk and run collapse to the same crawl, so holding Shift in a
    /// river does not buy a faster crossing, and neither does decking its bank.
    pub(crate) fn player_step(&self) -> (i32, i32) {
        let mut intent_x = self.player.move_x;
        let mut intent_y = self.player.move_y;
        let (q, r) = world_to_axial(self.player.x, self.player.y);
        let mut speed =
            PLAYER_SPEED * self.movement_factor_at(q, r) as i32 / UNTREATED_MOVEMENT as i32;
        if self.can_swim() && self.in_or_entering_deep_water() {
            speed = PLAYER_SPEED / SWIM_SPEED_DIVISOR;
            let diagonal = intent_x != 0 && intent_y != 0;
            let magnitude = if diagonal { 707 } else { 1000 };
            intent_x = intent_x.signum() * magnitude;
            intent_y = intent_y.signum() * magnitude;
        } else if self.in_or_entering_shallows() {
            speed = PLAYER_SPEED / 5;
            let diagonal = intent_x != 0 && intent_y != 0;
            let magnitude = if diagonal { 707 } else { 1000 };
            intent_x = intent_x.signum() * magnitude;
            intent_y = intent_y.signum() * magnitude;
        }
        (
            i32::from(intent_x) * speed / 1000,
            i32::from(intent_y) * speed / 1000,
        )
    }

    pub(crate) fn in_or_entering_shallows(&self) -> bool {
        let dx = i32::from(self.player.move_x) * PLAYER_SPEED / 1000;
        let dy = i32::from(self.player.move_y) * PLAYER_SPEED / 1000;
        self.shallows_at(self.player.x, self.player.y)
            || self.shallows_at(self.player.x + dx, self.player.y)
            || self.shallows_at(self.player.x, self.player.y + dy)
    }

    pub(crate) fn shallows_at(&self, x: i32, y: i32) -> bool {
        let (q, r) = world_to_axial(x, y);
        self.shallow_water_at(q, r)
    }

    /// The wading half of the one native water predicate. Physical water is shallow by depth,
    /// including a flood on a meadow and excluding a drained river band; the legacy fixture keeps
    /// its presentation-only rule.
    pub(crate) fn shallow_water_at(&self, q: i32, r: i32) -> bool {
        if !self.ground_is_physical() {
            return self.terrain_at(q, r) == Terrain::ShallowWater;
        }
        let depth = self.water_depth_at(q, r);
        depth > 0 && depth < scale::WADE_LIMIT_QUANTA
    }

    /// Deep water is a depth, not a colour. The legacy band path exists only for the pinned old
    /// fixture; production reads the same native water predicate as pumps and earthworks.
    pub(crate) fn deep_water_at(&self, q: i32, r: i32) -> bool {
        if !self.ground_is_physical() {
            return self.terrain_at(q, r) == Terrain::DeepWater;
        }
        self.water_depth_at(q, r) >= scale::WADE_LIMIT_QUANTA
    }

    pub(crate) fn can_swim(&self) -> bool {
        self.skills.bonuses(&self.technologies).can_swim
    }

    /// Only the player movement rule is relaxed. Deep water remains blocking terrain for every
    /// construction, machine, and transport predicate.
    pub(crate) fn player_terrain_blocks_movement(&self, q: i32, r: i32) -> bool {
        self.terrain_blocks_movement(q, r) && !(self.can_swim() && self.deep_water_at(q, r))
    }

    pub(crate) fn in_or_entering_deep_water(&self) -> bool {
        let dx = i32::from(self.player.move_x) * PLAYER_SPEED / 1000;
        let dy = i32::from(self.player.move_y) * PLAYER_SPEED / 1000;
        let deep_at = |x: i32, y: i32| {
            let (q, r) = world_to_axial(x, y);
            self.deep_water_at(q, r)
        };
        deep_at(self.player.x, self.player.y)
            || deep_at(self.player.x + dx, self.player.y)
            || deep_at(self.player.x, self.player.y + dy)
    }

    pub(crate) fn player_blocked(&self, x: i32, y: i32) -> bool {
        let (q, r) = world_to_axial(x, y);
        let feature_collision = self.player_terrain_blocks_movement(q, r);
        feature_collision
            // A retaining face stops the body exactly where it stops the route. The step is measured
            // between the hex being left and the hex being entered, so standing still on a terrace
            // is always legal and only the crossing is refused.
            || self.grade_blocks(world_to_axial(self.player.x, self.player.y), (q, r))
            || self.boundary_blocks_player(x, y)
            || self.boundary_blocks_segment((self.player.x, self.player.y), (x, y))
            || self.entities.iter().any(|entity| {
                self.building_definition(entity.placed.definition_id)
                    .map(|definition| definition.blocks_movement)
                    .unwrap_or(true)
                    && self.entity_footprint(entity).iter().any(|cell| {
                        let (building_x, building_y) = axial_world(cell.q, cell.r);
                        circles_overlap(
                            x,
                            y,
                            PLAYER_RADIUS,
                            building_x,
                            building_y,
                            BUILDING_RADIUS,
                        )
                    })
            })
    }

    pub(crate) fn gather(&mut self) -> Result<(), String> {
        if self.player.action_cooldown > 0 {
            return Err("action cooling down".into());
        }
        self.ensure_neighborhood(self.player.x, self.player.y);
        let (player_q, player_r) = world_to_axial(self.player.x, self.player.y);
        let origin = (player_q, player_r);
        if let Some(pos) = self
            .ground_items
            .iter()
            .position(|item| axial_distance(origin, (item.q, item.r)) <= EXTRACT_RADIUS)
        {
            let item_id = self.ground_items[pos].item_id;
            let room = self.player_room_for(item_id);
            if room == 0 {
                return Err("carrying capacity is full".into());
            }
            let quantity = self.ground_items[pos].quantity.min(room);
            *self.player.inventory.entry(item_id).or_default() += quantity;
            let name = self
                .item_definition(item_id)
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| format!("item {item_id}"));
            self.events.push(format!("Picked up {quantity} × {name}"));
            if quantity == self.ground_items[pos].quantity {
                self.ground_items.remove(pos);
            } else {
                self.ground_items[pos].quantity -= quantity;
            }
            self.dirty.ground_items = true;
            return Ok(());
        }
        // The same question placement and every extractor ask — the field cells the player's own
        // hex covers, nearest first — so a gather can never reach a cell an extractor standing
        // here could not. Facing is deliberately not part of it. Nothing on screen shows which way
        // the player points, so weighting the target by facing drained a neighbour's number while
        // the hex underfoot stayed full, which is a change the player cannot connect to an action.
        let key = self
            .resource_at_world(self.player.x, self.player.y)
            .ok_or("stand on or beside a field hex to gather")?;
        self.gather_from(key)
    }

    /// Harvest one named hex rather than whichever the nearest-first order picks.
    ///
    /// This is the argument the facing invariant asked for, and it is a different argument.
    /// Facing-weighted targeting was refused because *where the mouse rests* is not something a
    /// player reads as aiming at a hex, so the harvest moved to a neighbour with no visible cause.
    /// A right-click **is** the cause: the player named that hex, on screen, deliberately. So the
    /// target is explicit and the reach is unchanged — `field_covered_at` at the player's own
    /// reach, the same predicate placement and every extractor use, so a right-click can still
    /// never take from a cell an extractor standing here could not.
    pub(crate) fn gather_at(&mut self, q: i32, r: i32) -> Result<(), String> {
        if self.player.action_cooldown > 0 {
            return Err("action cooling down".into());
        }
        self.ensure_neighborhood(self.player.x, self.player.y);
        let origin = world_to_axial(self.player.x, self.player.y);
        if axial_distance(origin, (q, r)) > EXTRACT_RADIUS {
            return Err("that hex is out of reach".into());
        }
        if let Some(pos) = self
            .ground_items
            .iter()
            .position(|item| item.q == q && item.r == r)
        {
            let item_id = self.ground_items[pos].item_id;
            let room = self.player_room_for(item_id);
            if room == 0 {
                return Err("carrying capacity is full".into());
            }
            let quantity = self.ground_items[pos].quantity.min(room);
            *self.player.inventory.entry(item_id).or_default() += quantity;
            let name = self
                .item_definition(item_id)
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| format!("item {item_id}"));
            self.events.push(format!("Picked up {quantity} × {name}"));
            if quantity == self.ground_items[pos].quantity {
                self.ground_items.remove(pos);
            } else {
                self.ground_items[pos].quantity -= quantity;
            }
            self.dirty.ground_items = true;
            return Ok(());
        }
        if !self.field_covered_at(origin, (q, r), EXTRACT_RADIUS) {
            return Err("that hex is out of reach".into());
        }
        self.gather_from((q, r))
    }

    /// Start working a field cell that has already been resolved and range-checked. Both gathers
    /// land here, so the work a material costs, the carrying rule, and the refusals are one
    /// implementation and cannot drift apart.
    ///
    /// Nothing is taken here. The swing is armed and `finish_gather` pays it out when the player's
    /// clock has actually spent the work — the deposit counts down and the item appears together,
    /// at the end, which is the only moment either of them is true.
    pub(crate) fn gather_from(&mut self, key: (i32, i32)) -> Result<(), String> {
        let (_, steps) = self.gather_check(key)?;
        self.player.action_cooldown = steps;
        self.last_action_cooldown_total = steps;
        self.pending_gather = Some(Coordinate { q: key.0, r: key.1 });
        Ok(())
    }

    /// Everything that has to hold for one unit to come out of a field cell: it is a field, it
    /// still holds stock, the hand can work that material at all, and there is room to carry what
    /// comes back. Answered twice for every harvest — once when the swing starts, so a refusal is
    /// immediate and says why, and once when it lands, because a swing takes real time and the
    /// world may have moved under it.
    pub(crate) fn gather_check(&self, key: (i32, i32)) -> Result<(ItemId, u32), String> {
        let field = self
            .field_at(key.0, key.1)
            .ok_or("stand on or beside a field hex to gather")?;
        // `resource_at_world` filters empty cells for the untargeted gather, but a named hex has
        // not been through that filter — and an empty one would underflow the subtraction the
        // payout makes.
        if self.deposit_quantity(key) == 0 {
            return Err("this deposit is worked out".into());
        }
        if self.player_room_for(field.item_id) == 0 {
            return Err("carrying capacity is full".into());
        }
        let steps = self
            .item_definition(field.item_id)
            .and_then(|item| item.hand_gather_steps)
            .ok_or_else(|| {
                format!(
                    "{} cannot be gathered by hand — place an extractor on the field",
                    self.item_name(field.item_id)
                )
            })?;
        Ok((field.item_id, steps))
    }

    /// The swing lands: one unit leaves the deposit and enters the pack, in the same step.
    ///
    /// It asks again what it asked when the swing started, because the work took real time: the
    /// cell may have run out under an extractor, the pack may have filled from an erase refund, and
    /// the player may have walked off the hex they were working. Reach is the same predicate the
    /// start used, so a swing can never land on a cell an extractor standing here could not reach —
    /// walking away cancels the harvest rather than dragging it along.
    ///
    /// A swing that no longer holds pays nothing and says nothing. The refusal for a harvest the
    /// player can still start is the one they get when they start it, and the ring already showed
    /// them the work; a toast at the end of it would be an error message for an action they had
    /// already stopped taking.
    pub(crate) fn finish_gather(&mut self) {
        let Some(target) = self.pending_gather.take() else {
            return;
        };
        let key = (target.q, target.r);
        let origin = world_to_axial(self.player.x, self.player.y);
        if !self.field_covered_at(origin, key, EXTRACT_RADIUS) {
            return;
        }
        if self.gather_check(key).is_err() {
            return;
        }
        let Some(field) = self.field_at(key.0, key.1) else {
            return;
        };
        let remaining = self.deposit_quantity(key) - 1;
        self.write_overlay(
            key.0,
            key.1,
            field.item_id,
            remaining,
            field.initial_quantity,
        );
        self.dirty.resources.push(key);
        *self.player.inventory.entry(field.item_id).or_default() += 1;
        // Named, not numbered. "Gathered item 6" was serviceable when the world held three items;
        // against a material base of twenty-three it tells the player nothing they can act on.
        self.events
            .push(format!("Gathered {}", self.item_name(field.item_id)));
        if remaining == 0 {
            // Any extractor covering this deposit may now report a different status.
            self.mark_all_entities_dirty();
            self.events.push("Deposit depleted".into());
        }
    }

    #[allow(dead_code)]
    pub(crate) fn deposit_inventory(&mut self) -> Result<(), String> {
        self.deposit_item(None)
    }

    pub(crate) fn deposit_item(&mut self, target_item: Option<ItemId>) -> Result<(), String> {
        let hub = self
            .entities
            .iter()
            .position(|entity| entity.kind == BuildingKind::Hub);
        let Some(hub) = hub else {
            return Err("this scenario has no landing hub".into());
        };
        if !self.within_hex_range_of_entity(hub, HUB_REACH_HEXES) {
            return Err("move beside the landing hub to deliver".into());
        }
        if self.player.inventory.is_empty() {
            return Err("inventory is empty".into());
        }
        if let Some(target) = target_item {
            if !self.player.inventory.contains_key(&target) {
                return Err("you are not carrying that item".into());
            }
        }
        // Only what the hub is actually asking for, and only as much of it as is still wanted. If
        // a target item was specified, deliver only that item; otherwise deliver all demanded items.
        let cargo: Vec<(ItemId, u32)> = self
            .player
            .inventory
            .iter()
            .filter(|(&item, _)| target_item.map_or(true, |target| item == target))
            .map(|(&item, &carried)| (item, self.hub_demand(item).min(u64::from(carried)) as u32))
            .filter(|&(_, quantity)| quantity > 0)
            .collect();
        if cargo.is_empty() {
            if target_item.is_some() {
                return Err("the landing hub is not asking for that item".into());
            }
            return Err("the landing hub is not asking for anything you carry".into());
        }
        let handed: u32 = cargo.iter().map(|&(_, quantity)| quantity).sum();
        self.events
            .push(format!("Delivered {handed} to the landing hub"));
        for (item, quantity) in cargo {
            let carried = self.player.inventory.entry(item).or_default();
            *carried -= quantity;
            if *carried == 0 {
                self.player.inventory.remove(&item);
            }
            self.deliver_to_hub(item, quantity);
        }
        Ok(())
    }

    /// Mark every technology this completed stage grants. Insight is not charged, and a
    /// technology already researched is left untouched so a legacy factory that bought the
    /// same unlock is neither refunded nor double-granted.
    pub(crate) fn grant_contract_stage(&mut self, stage_key: &str) {
        let granted: Vec<(TechnologyId, String)> = self
            .technologies
            .technologies
            .iter()
            .filter(|technology| {
                matches!(
                    &technology.grant,
                    TechnologyGrant::ContractStage { key, .. } if key == stage_key
                ) && !self.researched.contains(&technology.id)
            })
            .map(|technology| (technology.id, technology.name.clone()))
            .collect();
        if granted.is_empty() {
            return;
        }
        for (id, name) in granted {
            self.researched.insert(id);
            self.events.push(format!("The hub grants {name}"));
        }
        self.apply_research_effects();
        self.refill_requests();
    }

    pub(crate) fn research_availability(
        &self,
        technology: &TechnologyDefinition,
    ) -> ResearchAvailability {
        ResearchAvailability {
            technology_id: technology.id,
            complete: self.researched.contains(&technology.id),
            missing_prerequisites: technology
                .prerequisites
                .iter()
                .copied()
                .filter(|id| !self.researched.contains(id))
                .collect(),
            insight_shortfall: u64::from(technology.cost).saturating_sub(self.insight),
        }
    }

    pub(crate) fn research_availability_snapshot(&self) -> Vec<ResearchAvailability> {
        self.technologies
            .technologies
            .iter()
            .map(|technology| self.research_availability(technology))
            .collect()
    }

    pub(crate) fn research(&mut self, technology_id: TechnologyId) -> Result<(), String> {
        let technology = self
            .technology(technology_id)
            .cloned()
            .ok_or_else(|| format!("unknown technology {technology_id}"))?;
        let availability = self.research_availability(&technology);
        if availability.complete {
            return Err("technology already researched".into());
        }
        if !technology.purchasable() {
            return Err(match &technology.grant {
                TechnologyGrant::ContractStage { name, .. } => {
                    format!("granted by completing {name}")
                }
                TechnologyGrant::Purchase => "technology cannot be purchased".into(),
            });
        }
        if !availability.missing_prerequisites.is_empty() {
            return Err("technology prerequisites are not complete".into());
        }
        if availability.insight_shortfall > 0 {
            return Err(format!("requires {} insight", technology.cost));
        }
        self.insight -= u64::from(technology.cost);
        self.researched.insert(technology_id);
        self.apply_research_effects();
        // A breakthrough can make a request reachable that was not, which matters only when the
        // board is short of a slot — the usual case is a full board that turns over on its own.
        self.refill_requests();
        self.events.push(format!("Researched {}", technology.name));
        Ok(())
    }
}
