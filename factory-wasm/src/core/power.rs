//! power — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    pub(crate) fn compile_power(&mut self) {
        let n = self.entities.len();
        self.power_of = vec![None; n];
        self.power_supply.clear();
        self.power_demand.clear();
        if n == 0 {
            self.runtime.rebuild_power(&self.power_of);
            return;
        }
        let mut parent: Vec<usize> = (0..n).collect();
        let find = |parent: &mut [usize], mut index: usize| -> usize {
            while parent[index] != index {
                parent[index] = parent[parent[index]];
                index = parent[index];
            }
            index
        };
        let union = |parent: &mut [usize], a: usize, b: usize, ids: &[u32]| {
            let pa = find(parent, a);
            let pb = find(parent, b);
            if pa == pb {
                return;
            }
            if ids[pa] < ids[pb] {
                parent[pb] = pa;
            } else {
                parent[pa] = pb;
            }
        };
        let ids: Vec<u32> = self.entities.iter().map(|entity| entity.id).collect();
        let poles: Vec<usize> = (0..n)
            .filter(|&index| {
                self.building_definition(self.entities[index].placed.definition_id)
                    .is_some_and(|definition| definition.kind == BuildingKind::Pole)
            })
            .collect();
        let machines: Vec<usize> = (0..n)
            .filter(|&index| {
                let Some(definition) =
                    self.building_definition(self.entities[index].placed.definition_id)
                else {
                    return false;
                };
                definition.kind != BuildingKind::Pole
                    && (definition.power_output.unwrap_or(0) > 0
                        || definition.power_draw.unwrap_or(0) > 0)
            })
            .collect();
        // Poles form the long-range graph, and each pole supplies the machines inside its own
        // coverage. Machines attach to poles rather than to each other at range, so a plant of
        // extractors with no poles is linear rather than quadratic.
        for (offset, &left) in poles.iter().enumerate() {
            for &right in &poles[offset + 1..] {
                if self.power_linked(left, right) {
                    union(&mut parent, left, right, &ids);
                }
            }
        }
        for &machine in &machines {
            for &pole in &poles {
                if self.power_linked(machine, pole) {
                    union(&mut parent, machine, pole, &ids);
                }
            }
        }
        // Touching machines conduct. A generator standing beside a smelter runs it, a block of
        // machines built shoulder to shoulder wires itself, and a pole becomes what *distance*
        // costs rather than what power costs — which is what the balance tool's opening prices
        // have always assumed.
        //
        // Only buildings that draw or generate conduct. If belts and containers carried current,
        // a line of the cheapest building in the game would be free wire across the map and no
        // player would ever place the second pole.
        //
        // Walked through a cell index rather than pairwise, so the pass is linear in machines
        // instead of quadratic: `entity_at` is a scan, and six of them per footprint cell per
        // machine is the shape of a compile that gets slower the more factory there is.
        let mut cells: BTreeMap<(i32, i32), usize> = BTreeMap::new();
        for &machine in &machines {
            for cell in self.entity_footprint(&self.entities[machine]) {
                cells.insert((cell.q, cell.r), machine);
            }
        }
        for &machine in &machines {
            for cell in self.entity_footprint(&self.entities[machine]) {
                for &(dq, dr) in &DIRECTIONS {
                    if let Some(&other) = cells.get(&(cell.q + dq, cell.r + dr)) {
                        if other != machine {
                            union(&mut parent, machine, other, &ids);
                        }
                    }
                }
            }
        }
        for index in poles.into_iter().chain(machines) {
            let root = find(&mut parent, index);
            self.power_of[index] = Some(ids[root]);
        }
        self.runtime.rebuild_power(&self.power_of);
        self.refresh_power_meters();
    }

    pub(crate) fn power_linked(&self, left: usize, right: usize) -> bool {
        let Some(a) = self.building_definition(self.entities[left].placed.definition_id) else {
            return false;
        };
        let Some(b) = self.building_definition(self.entities[right].placed.definition_id) else {
            return false;
        };
        let distance = self.power_distance(left, right);
        let a_pole = a.kind == BuildingKind::Pole;
        let b_pole = b.kind == BuildingKind::Pole;
        if a_pole && b_pole {
            let reach = i32::max(
                a.pole_reach.unwrap_or(DEFAULT_POLE_REACH as u32) as i32,
                b.pole_reach.unwrap_or(DEFAULT_POLE_REACH as u32) as i32,
            );
            return distance <= reach;
        }
        if a_pole || b_pole {
            // Coverage is the pole's, not the machine's. This is the whole of the upgrade: a
            // better pole lights a wider disc, and every machine already standing in it connects
            // without being touched.
            let pole = if a_pole { a } else { b };
            let radius = pole
                .supply_radius
                .unwrap_or(DEFAULT_POLE_SUPPLY_RADIUS as u32) as i32;
            return distance <= radius;
        }
        false
    }

    pub(crate) fn power_distance(&self, left: usize, right: usize) -> i32 {
        let mut best = i32::MAX;
        for a in self.entity_footprint(&self.entities[left]) {
            for b in self.entity_footprint(&self.entities[right]) {
                best = best.min(axial_distance((a.q, a.r), (b.q, b.r)));
            }
        }
        best
    }

    /// What the meters read: live generation and the standing draw of the machines that have work.
    ///
    /// Deliberately *not* the buffer-fill requests `distribute_power` allocates against. A machine
    /// with a full bank asks for nothing that tick, and a needle that dropped to zero every time
    /// the factory got comfortable would be telling the player about the accounting rather than
    /// about their grid. Supply against standing draw is the number that answers "can this plant
    /// carry this factory".
    pub(crate) fn refresh_power_meters(&mut self) {
        let previous_supply = std::mem::take(&mut self.power_supply);
        let previous_demand = std::mem::take(&mut self.power_demand);
        for offset in 0..self.runtime.power_order.len() {
            let index = self.runtime.power_order[offset];
            let Some(net) = self.power_of.get(index).copied().flatten() else {
                continue;
            };
            let definition_id = self.entities[index].placed.definition_id;
            let draw = self
                .building_definition(definition_id)
                .and_then(|definition| definition.power_draw)
                .unwrap_or(0);
            let output = self
                .building_definition(definition_id)
                .and_then(|definition| definition.power_output)
                .unwrap_or(0);
            if draw > 0 && self.power_work_wanted(index) {
                *self.power_demand.entry(net).or_default() += draw;
            }
            if output > 0 {
                *self.power_supply.entry(net).or_default() += self.generator_output_now(index);
            }
        }
        if self.power_unmetered {
            self.power_supply = self.power_demand.clone();
        }
        if self.power_supply != previous_supply || self.power_demand != previous_demand {
            for offset in 0..self.runtime.power_order.len() {
                let index = self.runtime.power_order[offset];
                self.dirty.entities.push(self.entities[index].id);
            }
        }
    }

    /// Whether this machine has work its next tick of power would actually buy.
    ///
    /// This predicate is the entire fuel rule. A machine with nothing to do asks the grid for
    /// nothing, so the grid draws nothing from its plants, so the plants burn nothing — there is no
    /// separate "throttle the generator" step anywhere, because there is nothing to throttle.
    pub(crate) fn power_work_wanted(&self, index: usize) -> bool {
        let entity = &self.entities[index];
        // A machine switched off has no work its next tick of power would buy, so it asks for
        // none — and by the rule above, nothing burns anywhere to supply it.
        if entity.disabled {
            return false;
        }
        match entity.kind {
            // A blocked extractor or pump has produced something nobody has taken. It is not
            // waiting on power and must not hold a share of it.
            BuildingKind::Extractor | BuildingKind::Pump => {
                self.room_for_stock(index, StockKind::Output, 0) > 0
            }
            BuildingKind::Composer => {
                let Some(recipe) = entity.placed.recipe_id.and_then(|id| self.recipe(id)) else {
                    return false;
                };
                if !self.room_for_recipe(index, recipe) {
                    return false;
                }
                // Mid-craft always wants power: the inputs are already spent and the only thing
                // between the machine and its output is time it has to be paid for.
                if entity.progress > 0 {
                    return true;
                }
                let stocked = recipe.inputs.iter().all(|ingredient| {
                    self.stock_quantity(index, StockKind::Input, ingredient.item_id)
                        >= ingredient.quantity
                });
                stocked && self.fuel_ready(entity)
            }
            _ => false,
        }
    }

    /// How much electricity this machine wants banked: `POWER_BUFFER_CYCLES` whole cycles of the
    /// work it is set up to do. A machine with no recipe, or no work, wants nothing.
    pub(crate) fn power_capacity(&self, index: usize) -> u32 {
        let draw = self
            .building_definition(self.entities[index].placed.definition_id)
            .and_then(|definition| definition.power_draw)
            .unwrap_or(0);
        if draw == 0 {
            return 0;
        }
        // `progress_total` is already the length of one cycle for every kind that has one — a
        // cadence for an extractor or pump, a recipe duration for a composer — so the buffer is
        // sized off the same number the progress bar fills against rather than a second opinion.
        draw.saturating_mul(self.progress_total(index).max(1))
            .saturating_mul(POWER_BUFFER_CYCLES)
    }

    /// One tick of the grid: every network is filled from its plants, and every plant burns for
    /// exactly the energy it was asked to hand over.
    ///
    /// Energy is conserved. What machines bank equals what plants produced, to the unit, which is
    /// why throughput comes out exactly proportional to generation without a slowdown factor
    /// anywhere: an undersupplied factory is not scaled down, it is simply given less to spend.
    pub(crate) fn distribute_power(&mut self) {
        self.refresh_power_meters();
        if self.power_unmetered {
            return;
        }
        // Requests, by network, in ascending entity id — which is index order, so every
        // apportionment below is over a list whose order is a save's order.
        let mut requests: BTreeMap<u32, Vec<(usize, u64)>> = BTreeMap::new();
        let mut plants: BTreeMap<u32, Vec<(usize, u64)>> = BTreeMap::new();
        for offset in 0..self.runtime.power_order.len() {
            let index = self.runtime.power_order[offset];
            let Some(net) = self.power_of.get(index).copied().flatten() else {
                continue;
            };
            let definition_id = self.entities[index].placed.definition_id;
            let Some(definition) = self.building_definition(definition_id) else {
                continue;
            };
            let draw = definition.power_draw.unwrap_or(0);
            let output = definition.power_output.unwrap_or(0);
            if output > 0 {
                let live = self.generator_output_now(index);
                if live > 0 {
                    plants
                        .entry(net)
                        .or_default()
                        .push((index, u64::from(live)));
                }
            } else if draw > 0 && self.power_work_wanted(index) {
                let want = u64::from(self.power_capacity(index))
                    .saturating_sub(u64::from(self.entities[index].power_charge));
                if want > 0 {
                    requests.entry(net).or_default().push((index, want));
                }
            }
        }
        for (net, asked) in requests {
            let Some(offers) = plants.get(&net) else {
                continue;
            };
            let available: u64 = offers.iter().map(|&(_, offer)| offer).sum();
            let wanted: u64 = asked.iter().map(|&(_, want)| want).sum();
            let used = available.min(wanted);
            if used == 0 {
                continue;
            }
            let weights: Vec<u64> = asked.iter().map(|&(_, want)| want).collect();
            for (&(index, _), granted) in asked.iter().zip(apportion(used, &weights)) {
                if granted == 0 {
                    continue;
                }
                self.entities[index].power_charge += granted as u32;
                let id = self.entities[index].id;
                self.dirty.entities.push(id);
            }
            // The same split over the plants, so what was produced equals what was banked and no
            // generator burns for a unit that never reached a machine.
            let offered: Vec<u64> = offers.iter().map(|&(_, offer)| offer).collect();
            for (&(index, _), produced) in offers.iter().zip(apportion(used, &offered)) {
                self.burn_for_output(index, produced as u32);
            }
        }
    }

    /// Charge a plant for the electricity it just produced.
    ///
    /// A generator running flat out spends one unit of fuel energy per tick, so `power_output` is
    /// the exchange rate, and a plant carrying a fifth of the load pays a fifth as often.
    /// `burn_progress` is where the fraction waits, which is what keeps a lightly loaded burner
    /// honest instead of either free or rounded up to a whole coal every tick.
    pub(crate) fn burn_for_output(&mut self, index: usize, produced: u32) {
        if produced == 0 {
            return;
        }
        let Some(definition) = self.building_definition(self.entities[index].placed.definition_id)
        else {
            return;
        };
        let (source, rate) = (
            definition.power_source,
            definition.power_output.unwrap_or(0),
        );
        if rate == 0 {
            return;
        }
        // Wind and water are paid for once, at construction. Only a plant with a bill has one.
        if !matches!(
            source,
            Some(PowerSource::Burner) | Some(PowerSource::Turbine)
        ) {
            return;
        }
        self.entities[index].burn_progress += produced;
        let units = self.entities[index].burn_progress / rate;
        if units == 0 {
            return;
        }
        self.entities[index].burn_progress -= units * rate;
        match source {
            Some(PowerSource::Burner) => {
                if self.charge_fuel(index, units, &[]) {
                    self.entities[index].fuel_charge -= units;
                }
            }
            // A turbine has no firebox of its own: the bill lands on the boiler beside it, which is
            // where the coal and the water actually are.
            Some(PowerSource::Turbine) => {
                if let Some(boiler) = self.adjacent_live_boiler_index(index) {
                    let water = self.entities[boiler]
                        .input_inventory
                        .get(&WATER_ITEM)
                        .copied()
                        .unwrap_or(0)
                        .saturating_add(
                            self.entities[boiler]
                                .inventory
                                .get(&WATER_ITEM)
                                .copied()
                                .unwrap_or(0),
                        )
                        .min(units);
                    if water > 0 {
                        self.subtract_stock(boiler, StockKind::Input, WATER_ITEM, water);
                    }
                    if self.charge_fuel(boiler, units, &[]) {
                        self.entities[boiler].fuel_charge -= units;
                    }
                    let id = self.entities[boiler].id;
                    self.dirty.entities.push(id);
                }
            }
            _ => {}
        }
        let id = self.entities[index].id;
        self.dirty.entities.push(id);
    }

    pub(crate) fn generator_output_now(&self, index: usize) -> u32 {
        // A plant switched off offers nothing to its network, which is what stops a burner eating
        // coal on behalf of a line the player has deliberately stopped.
        if self.entities[index].disabled {
            return 0;
        }
        let Some(definition) = self.building_definition(self.entities[index].placed.definition_id)
        else {
            return 0;
        };
        let output = definition.power_output.unwrap_or(0);
        if output == 0 {
            return 0;
        }
        match definition.power_source {
            Some(PowerSource::Burner) => {
                if self.generator_has_fuel(index) {
                    output
                } else {
                    0
                }
            }
            Some(PowerSource::Wind) => output,
            Some(PowerSource::Hydro) => {
                let placed = self.entities[index].placed;
                let radius = definition.extract_radius.unwrap_or(PUMP_RADIUS as u32) as i32;
                if self.water_within_reach(placed.q, placed.r, radius) {
                    output
                } else {
                    0
                }
            }
            Some(PowerSource::Turbine) => {
                if self.adjacent_live_boiler(index) {
                    output
                } else {
                    0
                }
            }
            None => 0,
        }
    }

    pub(crate) fn generator_has_fuel(&self, index: usize) -> bool {
        let entity = &self.entities[index];
        entity.fuel_charge > 0
            || self.burnable_item(&entity.fuel_inventory, &[]).is_some()
            || self.burnable_item(&entity.inventory, &[]).is_some()
    }

    pub(crate) fn boiler_live(&self, index: usize) -> bool {
        let entity = &self.entities[index];
        // A boiler switched off raises no steam, so the turbines beside it read as having no
        // boiler at all — the switch travels the pair the same way fuel and water do.
        !entity.disabled
            && self.stock_quantity(index, StockKind::Input, WATER_ITEM) >= 1
            && (entity.fuel_charge > 0
                || self.burnable_item(&entity.fuel_inventory, &[]).is_some()
                || self.burnable_item(&entity.inventory, &[]).is_some())
    }

    pub(crate) fn adjacent_live_boiler(&self, index: usize) -> bool {
        self.adjacent_live_boiler_index(index).is_some()
    }

    /// The boiler a turbine's bill lands on: the lowest-id live one it touches, so a turbine
    /// wedged between two boilers always empties the same one and a save reproduces which.
    pub(crate) fn adjacent_live_boiler_index(&self, index: usize) -> Option<usize> {
        let mut best: Option<usize> = None;
        for cell in self.entity_footprint(&self.entities[index]) {
            for &(dq, dr) in &DIRECTIONS {
                if let Some(other) = self.entity_at(cell.q + dq, cell.r + dr) {
                    if self.entities[other].kind == BuildingKind::Boiler && self.boiler_live(other)
                    {
                        best = Some(match best {
                            Some(current)
                                if self.entities[current].id <= self.entities[other].id =>
                            {
                                current
                            }
                            _ => other,
                        });
                    }
                }
            }
        }
        best
    }

    /// Spend banked electricity on `base` ticks of progress, returning the ticks actually paid for.
    ///
    /// The machine buys work out of its own bank rather than out of a network ratio. A brownout is
    /// therefore not a slowdown factor applied to a machine: it is a machine that ran out of what
    /// it was given, and it resumes at full speed the moment the grid hands it more.
    pub(crate) fn power_progress(&mut self, index: usize, base: u32) -> u32 {
        if self.power_unmetered || base == 0 {
            return base;
        }
        let draw = self
            .building_definition(self.entities[index].placed.definition_id)
            .and_then(|definition| definition.power_draw)
            .unwrap_or(0);
        if draw == 0 {
            return base;
        }
        let charge = self.entities[index].power_charge;
        let afforded = base.min(charge / draw);
        if afforded == 0 {
            return 0;
        }
        self.entities[index].power_charge = charge - afforded * draw;
        afforded
    }

    /// Whether this machine can pay for a tick of work right now — it holds at least one tick's
    /// draw. What gates a craft is the bank, not the network, so a machine on a dead grid keeps
    /// running until the energy it was already given runs out.
    pub(crate) fn entity_powered(&self, index: usize) -> bool {
        if self.power_unmetered {
            return true;
        }
        let draw = self
            .building_definition(self.entities[index].placed.definition_id)
            .and_then(|definition| definition.power_draw)
            .unwrap_or(0);
        draw == 0 || self.entities[index].power_charge >= draw
    }

    /// Whether this machine is wired to anything that is generating. Separates "no power" from
    /// "brownout": the first is a grid problem the player fixes with a pole or a plant, the second
    /// is a capacity problem they fix with more generation.
    pub(crate) fn entity_connected(&self, index: usize) -> bool {
        if self.power_unmetered {
            return true;
        }
        let Some(net) = self.power_of.get(index).copied().flatten() else {
            return false;
        };
        self.power_supply.get(&net).copied().unwrap_or(0) > 0
    }

    pub(crate) fn network_of(&self, index: usize) -> (u32, u32) {
        let Some(net) = self.power_of.get(index).copied().flatten() else {
            return (0, 0);
        };
        (
            self.power_supply.get(&net).copied().unwrap_or(0),
            self.power_demand.get(&net).copied().unwrap_or(0),
        )
    }
}
