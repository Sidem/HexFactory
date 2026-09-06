//! Explicit herd targeting and tick-owned windup. Ordinary gathering never targets wildlife.
use super::*;

impl Core {
    pub(crate) fn hunt_herd(&mut self, id: u32) -> Result<(), String> {
        if self.herds.values().any(|h| h.hunt_tick > 0) {
            return Err("A hunt is already in progress".into());
        }
        let herd = self.herds.get(&id).ok_or("That herd has moved away")?;
        let point = herd.position(self.tick);
        if squared_distance(self.player.x, self.player.y, point.0, point.1) > 3500i64.pow(2)
            || self.boundary_blocks_segment((self.player.x, self.player.y), point)
        {
            return Err("Walk within hunting reach with a clear line to the herd".into());
        }
        self.cancel_hunt();
        let herd = self.herds.get_mut(&id).expect("herd");
        if let Some(ids) = self.herd_arrivals.get_mut(&herd.arrive_tick) {
            ids.remove(&id);
        }
        herd.from = point;
        herd.to = point;
        herd.left_tick = self.tick;
        herd.arrive_tick = self.tick + 1;
        herd.hunt_tick = self.tick + 20;
        herd.alarm = 200;
        herd.drive = Drive::Flee;
        self.herd_arrivals
            .entry(herd.arrive_tick)
            .or_default()
            .insert(id);
        self.dirty.herds.push(id);
        self.events
            .push("Hunting herd — hold still for two seconds; moving cancels".into());
        Ok(())
    }

    pub(crate) fn cancel_hunt(&mut self) {
        for herd in self.herds.values_mut().filter(|h| h.hunt_tick > 0) {
            herd.hunt_tick = 0;
            self.dirty.herds.push(herd.id);
        }
    }

    pub(crate) fn finish_hunts(&mut self) {
        let ids: Vec<_> = self
            .herds
            .values()
            .filter(|h| {
                h.hunt_tick > 0
                    && h.hunt_tick <= self.tick
                    && (h.from == h.to || h.arrive_tick <= self.tick)
            })
            .map(|h| h.id)
            .collect();
        for id in ids {
            let mut herd = self.herds.remove(&id).expect("herd");
            let point = herd.position(self.tick);
            herd.hunt_tick = 0;
            if squared_distance(self.player.x, self.player.y, point.0, point.1) <= 3500i64.pow(2)
                && !self.boundary_blocks_segment((self.player.x, self.player.y), point)
            {
                let item = self
                    .definitions
                    .species
                    .iter()
                    .find(|s| s.id == herd.species)
                    .expect("species")
                    .carcass_item;
                let (q, r) = world_to_axial(point.0, point.1);
                self.add_ground_item(q, r, item, 1);
                herd.count -= 1;
                self.events
                    .push("Carcass dropped beside the herd; collect it for processing".into());
                if herd.count < 2 {
                    self.events.push(
                        "No breeding pair remains here. Leave nearby herds as a refuge".into(),
                    );
                }
            } else {
                self.events.push("The herd escaped hunting reach".into());
            }
            if herd.count > 0 {
                self.herds.insert(id, herd);
            }
            self.dirty.herds.push(id);
        }
    }

    pub(crate) fn cut_feed(&mut self, q: i32, r: i32) -> Result<(), String> {
        if !self.within_world_range(q, r, 2500) || self.player.action_cooldown > 0 {
            return Err("Walk closer and finish the current action before cutting grass".into());
        }
        if self.grass_stock(q, r) < 100 {
            return Err("Let this pasture recover before cutting more feed".into());
        }
        let item = self
            .definitions
            .species
            .first()
            .ok_or("No grazing species defined")?
            .feed_item;
        self.graze_grass(q, r, 100);
        self.add_ground_item(q, r, item, 1);
        self.player.action_cooldown = 30;
        Ok(())
    }
}
