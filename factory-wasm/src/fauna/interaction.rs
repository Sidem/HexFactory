use super::*;

#[derive(Serialize)]
pub(crate) struct PasturePreview {
    pub q: i32,
    pub r: i32,
    pub grass: u16,
    pub limit: u16,
    pub can_cut: bool,
    pub can_feed: bool,
    pub feed_held: u32,
    pub reason: String,
    pub station_note: Option<String>,
    pub work_point: Option<(i32, i32)>,
}

impl Core {
    pub(crate) fn pasture_preview(&self, q: i32, r: i32) -> PasturePreview {
        let limit = self.grass_limit(q, r);
        let grass = self.grass_stock(q, r);
        let item = self.definitions.species.first().map(|s| s.feed_item);
        let held = item
            .and_then(|i| self.player.inventory.get(&i))
            .copied()
            .unwrap_or(0);
        let accessible = self.within_world_range(q, r, 3500)
            && self.herd_leg_clear(axial_world(q, r), axial_world(q, r))
            && !self.boundary_blocks_segment((self.player.x, self.player.y), axial_world(q, r));
        let can_cut = accessible
            && grass >= 100
            && self.player.action_cooldown == 0
            && item.is_some_and(|i| self.player_room_for(i) > 0);
        let reason = if !accessible {
            "Walk closer with a clear route to dry pasture"
        } else if grass < 100 {
            "Pasture recovering — cut elsewhere"
        } else if self.player.action_cooldown > 0 {
            "Finish the current action before cutting"
        } else {
            "Cut feed, then place a trail through an open gate. Step aside so animals can settle and follow it."
        };
        // The reserved apron is a service cell, not occupied foundation, so selecting either hex
        // has to name the station: the building itself through occupancy, the working cell through
        // the envelope map the graph already keeps.
        let station = self
            .entity_at(q, r)
            .or_else(|| self.runtime.envelope.get(&(q, r)).copied())
            .filter(|&i| self.pasture_station_note(i).is_some());
        PasturePreview {
            q,
            r,
            grass,
            limit,
            can_cut,
            can_feed: accessible && held > 0,
            feed_held: held,
            reason: reason.into(),
            station_note: station.and_then(|i| self.pasture_station_note(i)),
            work_point: station.map(|i| {
                let c = self.pasture_work_cell(i);
                axial_world(c.0, c.1)
            }),
        }
    }

    pub(crate) fn place_feed(&mut self, q: i32, r: i32) -> Result<(), String> {
        let preview = self.pasture_preview(q, r);
        if !preview.can_feed {
            return Err("Carry cut feed and select nearby dry, accessible ground".into());
        }
        let item = self
            .definitions
            .species
            .first()
            .ok_or("No grazing species defined")?
            .feed_item;
        subtract_item(&mut self.player.inventory, item, 1);
        self.add_ground_item(q, r, item, 1);
        // Ordinary drops skip auto-pickup while remaining life is above lifetime − 30 ticks.
        // Stretching despawn to the lure window keeps walking across the bait from undoing it.
        for pile in self
            .ground_items
            .iter_mut()
            .filter(|i| (i.q, i.r, i.item_id) == (q, r, item))
        {
            pile.despawn_tick = self.tick + FEED_LURE_TICKS;
        }
        self.wake_nearby_grazers(q, r);
        self.events.push("Feed placed for ten simulation minutes — step aside and leave a route through the gate".into());
        Ok(())
    }

    pub(crate) fn wake_nearby_grazers(&mut self, q: i32, r: i32) {
        for herd in self.herds.values_mut() {
            let point = herd.position(self.tick);
            if herd.alarm > 0
                || herd.from != herd.to
                || axial_distance(world_to_axial(point.0, point.1), (q, r)) > 4
            {
                continue;
            }
            if let Some(ids) = self.herd_arrivals.get_mut(&herd.arrive_tick) {
                ids.remove(&herd.id);
            }
            // Do not reset left_tick: needs account for the time already spent resting.
            herd.arrive_tick = self.tick + 1;
            self.herd_arrivals
                .entry(herd.arrive_tick)
                .or_default()
                .insert(herd.id);
            self.dirty.herds.push(herd.id);
        }
    }

    pub(crate) fn drive_herd(&mut self, id: u32) -> Result<(), String> {
        let herd = self.herds.get(&id).ok_or("That herd has moved away")?;
        let point = herd.position(self.tick);
        if squared_distance(point.0, point.1, self.player.x, self.player.y) > 5000i64.pow(2)
            || self.boundary_blocks_segment((self.player.x, self.player.y), point)
        {
            return Err("Walk within the hunting ring with a clear line to drive this herd".into());
        }
        self.cancel_hunt();
        self.halt_motion();
        let herd = self.herds.get_mut(&id).expect("herd exists");
        if let Some(ids) = self.herd_arrivals.get_mut(&herd.arrive_tick) {
            ids.remove(&id);
        }
        herd.from = point;
        herd.to = point;
        herd.left_tick = self.tick;
        herd.arrive_tick = self.tick + 1;
        herd.alarm = 80;
        herd.drive = Drive::Flee;
        self.herd_arrivals
            .entry(herd.arrive_tick)
            .or_default()
            .insert(id);
        self.dirty.herds.push(id);
        self.events.push(
            "Driving herd away from you — stand opposite the open gate, then give it space".into(),
        );
        Ok(())
    }
}
