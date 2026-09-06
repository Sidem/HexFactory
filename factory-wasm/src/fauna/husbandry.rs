use super::*;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PastureAction {
    Harvest,
    Cut,
    Restore,
}

impl Core {
    /// A surviving lone animal can recover by physically meeting another compatible herd.
    /// No new animals appear: an emptied region needs a source brought from living habitat.
    pub(crate) fn reunite_lone_grazer(&mut self, herd: &mut Herd) {
        if herd.alarm > 0 || herd.hunt_tick > 0 {
            return;
        }
        let point = herd.position(self.tick);
        let other = self
            .herds
            .values()
            .find(|h| {
                let at = h.position(self.tick);
                h.species == herd.species
                    && (herd.count == 1 || h.count == 1)
                    && herd.count + h.count <= 8
                    && h.alarm == 0
                    && h.hunt_tick == 0
                    && squared_distance(point.0, point.1, at.0, at.1) <= 650i64.pow(2)
                    && self.herd_leg_clear(point, at)
            })
            .map(|h| h.id);
        if let Some(id) = other {
            let other = self.herds.remove(&id).expect("nearby herd");
            if let Some(ids) = self.herd_arrivals.get_mut(&other.arrive_tick) {
                ids.remove(&id);
            }
            herd.count += other.count;
            herd.hunger = herd.hunger.max(other.hunger);
            herd.thirst = herd.thirst.max(other.thirst);
            herd.healthy_ticks = 0;
            self.dirty.herds.push(id);
            self.events.push(
                "A lone grazer joined a herd — a breeding pair can recover here again".into(),
            );
        }
    }

    /// The reserved, visibly marked working cell behind a pasture station. Outputs leave the front.
    pub(crate) fn pasture_work_cell(&self, index: usize) -> (i32, i32) {
        let p = self.entities[index].placed;
        let offset = rotate_coordinate(Coordinate { q: -1, r: 0 }, p.orientation);
        (p.q + offset.q, p.r + offset.r)
    }

    pub(crate) fn pasture_cell_open(&self, index: usize) -> bool {
        let p = self.entities[index].placed;
        let cell = self.pasture_work_cell(index);
        let point = axial_world(cell.0, cell.1);
        !self.boundary_blocks_segment(axial_world(p.q, p.r), point)
            && !self.grade_blocks((p.q, p.r), cell)
            && self.herd_leg_clear(point, point)
    }

    pub(crate) fn pasture_victim(&self, index: usize) -> Option<u32> {
        let cell = self.pasture_work_cell(index);
        self.herds
            .values()
            .find(|h| {
                h.count > 2
                    && h.alarm == 0
                    && h.hunt_tick == 0
                    && h.hunger < 100
                    && h.thirst < 100
                    && world_to_axial(h.position(self.tick).0, h.position(self.tick).1) == cell
                    && h.from == h.to
            })
            .map(|h| h.id)
    }

    pub(crate) fn pasture_work_ready(&self, index: usize, action: PastureAction) -> bool {
        if !self.pasture_cell_open(index) {
            return false;
        }
        let (q, r) = self.pasture_work_cell(index);
        match action {
            PastureAction::Harvest => self.pasture_victim(index).is_some(),
            PastureAction::Cut => self.grass_stock(q, r) >= 100,
            PastureAction::Restore => {
                self.grass_limit(q, r) > 0
                    && self.grazed.get(&(q, r)).copied().unwrap_or(0) >= 100
                    && self.fouled.get(&(q, r)).copied().unwrap_or(0) == 0
            }
        }
    }

    pub(crate) fn finish_pasture_work(&mut self, index: usize, action: PastureAction) {
        let (q, r) = self.pasture_work_cell(index);
        match action {
            PastureAction::Harvest => {
                let id = self
                    .pasture_victim(index)
                    .expect("revalidated on the completion tick");
                let herd = self.herds.get_mut(&id).expect("herd");
                herd.count -= 1;
                herd.healthy_ticks = 0;
                self.dirty.herds.push(id);
            }
            PastureAction::Cut => {
                self.graze_grass(q, r, 100);
            }
            PastureAction::Restore => {
                let deficit = self.grazed.get_mut(&(q, r)).expect("damaged pasture");
                *deficit = deficit.saturating_sub(100);
                if *deficit == 0 {
                    self.grazed.remove(&(q, r));
                }
                self.dirty.habitats.push((q, r));
            }
        }
    }

    /// Feed remains ordinary routed input. A disabled or fenced-off station advertises nothing.
    pub(crate) fn pasture_feeder(&self, cell: (i32, i32), item: ItemId) -> Option<usize> {
        self.runtime.envelope.get(&cell).copied().filter(|&i| {
            self.entity_running(i)
                && self.pasture_work_cell(i) == cell
                && self.entities[i]
                    .placed
                    .recipe_id
                    .and_then(|id| self.recipe(id))
                    .is_some_and(|r| r.pasture_action == Some(PastureAction::Harvest))
                && self.pasture_cell_open(i)
                && self.stock_quantity(i, StockKind::Input, item) > 0
        })
    }

    pub(crate) fn pasture_station_note(&self, index: usize) -> Option<String> {
        let action = self.entities[index]
            .placed
            .recipe_id
            .and_then(|id| self.recipe(id))?
            .pasture_action?;
        let (q, r) = self.pasture_work_cell(index);
        let note = if !self.pasture_cell_open(index) {
            "Working cell blocked — open the boundary and keep it dry and accessible"
        } else if !self.pasture_work_ready(index, action) {
            match action {
                PastureAction::Harvest => "Reserve protected: keep two animals. Waiting for a calm, healthy surplus animal at the feed apron",
                PastureAction::Cut => "Pasture recovering — mowing pauses below 100 grass",
                PastureAction::Restore => "No restoration needed, or loose waste must be collected first",
            }
        } else {
            "Habitat ready — supply recipe inputs and route the output"
        };
        Some(format!("Working cell {q}, {r}. {note}"))
    }
}
