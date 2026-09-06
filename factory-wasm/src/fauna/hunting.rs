//! Explicit herd targeting and tick-owned windup. Ordinary gathering never targets wildlife.
use super::*;

pub(crate) const HUNT_REACH: i32 = 5000;
pub(crate) const HUNT_TICKS: u64 = 10;

#[derive(Serialize)]
pub(crate) struct HuntPreview {
    pub herd_id: u32,
    pub point: (i32, i32),
    pub player: (i32, i32),
    pub reach: i32,
    pub ready: bool,
    pub reason: String,
    pub remaining_ticks: u64,
    pub duration_ticks: u64,
}

impl Core {
    pub(crate) fn hunt_preview(&self, id: u32) -> Result<HuntPreview, String> {
        let herd = self.herds.get(&id).ok_or("That herd has moved away")?;
        let point = herd.position(self.tick);
        let reason = if self.player.move_x != 0 || self.player.move_y != 0 {
            "Ready to stop and aim"
        } else {
            "Ready — aim for one second, then the herd scatters"
        };
        let blocked = self.boundary_blocks_segment((self.player.x, self.player.y), point);
        let far = squared_distance(self.player.x, self.player.y, point.0, point.1)
            > i64::from(HUNT_REACH).pow(2);
        let alarmed = herd.alarm > 30;
        let aiming = self
            .herds
            .values()
            .find(|h| h.hunt_tick > 0)
            .map(|h| h.hunt_tick.saturating_sub(self.tick))
            .unwrap_or(0);
        let reason = if far {
            "Move closer — keep the herd inside the hunting ring"
        } else if blocked {
            "Blocked — open the gate or find a clear line"
        } else if aiming > 0 {
            "Aiming — hold still; moving cancels"
        } else if alarmed {
            "Herd alarmed — step back and let it settle"
        } else {
            reason
        };
        Ok(HuntPreview {
            herd_id: id,
            point,
            player: (self.player.x, self.player.y),
            reach: HUNT_REACH,
            ready: !far && !blocked && !alarmed && aiming == 0,
            reason: reason.into(),
            remaining_ticks: aiming,
            duration_ticks: HUNT_TICKS,
        })
    }

    pub(crate) fn hunt_herd(&mut self, id: u32) -> Result<(), String> {
        let preview = self.hunt_preview(id)?;
        if !preview.ready {
            return Err(preview.reason);
        }
        self.halt_motion();
        self.set_aim(preview.point.0, preview.point.1)?;
        let herd = self.herds.get_mut(&id).expect("herd");
        // Aiming does not freeze the animal or make it flee before the player can release.
        herd.hunt_tick = self.tick + HUNT_TICKS;
        self.dirty.herds.push(id);
        self.events
            .push("Aiming at herd — hold still for one second; moving cancels".into());
        Ok(())
    }

    pub(crate) fn cancel_hunt(&mut self) {
        for herd in self.herds.values_mut().filter(|h| h.hunt_tick > 0) {
            herd.hunt_tick = 0;
            self.dirty.herds.push(herd.id);
            self.events.push("Hunt cancelled".into());
        }
    }

    pub(crate) fn finish_hunts(&mut self) {
        let ids: Vec<_> = self
            .herds
            .values()
            .filter(|h| h.hunt_tick > 0 && h.hunt_tick <= self.tick)
            .map(|h| h.id)
            .collect();
        for id in ids {
            let mut herd = self.herds.remove(&id).expect("herd");
            let point = herd.position(self.tick);
            herd.hunt_tick = 0;
            if squared_distance(self.player.x, self.player.y, point.0, point.1)
                <= i64::from(HUNT_REACH).pow(2)
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
            // Resolve at the advertised tick, even halfway along a leg, then scatter survivors.
            if let Some(ids) = self.herd_arrivals.get_mut(&herd.arrive_tick) {
                ids.remove(&id);
            }
            herd.from = point;
            herd.to = point;
            herd.left_tick = self.tick;
            herd.arrive_tick = self.tick + 1;
            herd.alarm = 200;
            herd.drive = Drive::Flee;
            if herd.count > 0 {
                self.herd_arrivals
                    .entry(herd.arrive_tick)
                    .or_default()
                    .insert(id);
                self.herds.insert(id, herd);
            }
            self.dirty.herds.push(id);
        }
    }

    pub(crate) fn cut_feed(&mut self, q: i32, r: i32) -> Result<(), String> {
        let preview = self.pasture_preview(q, r);
        if !preview.can_cut {
            return Err(preview.reason);
        }
        let item = self
            .definitions
            .species
            .first()
            .ok_or("No grazing species defined")?
            .feed_item;
        self.graze_grass(q, r, 100);
        *self.player.inventory.entry(item).or_default() += 1;
        self.player.action_cooldown = 30;
        self.events
            .push("Cut feed packed — select ground and place feed to lure a herd".into());
        Ok(())
    }
}
