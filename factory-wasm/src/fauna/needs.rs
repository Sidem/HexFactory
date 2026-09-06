//! Bounded reachable targets and a small, inspectable drive ladder.
use super::*;

impl Core {
    pub(crate) fn drinking_bank(&self, q: i32, r: i32) -> bool {
        self.water_depth_at(q, r) == 0
            && DIRECTIONS.iter().any(|&(dq, dr)| {
                let water = self.water_surface_at(q + dq, r + dr);
                self.water_depth_at(q + dq, r + dr) > 0
                    && water > scale::SEA_LEVEL_QUANTA
                    && !self.boundary_blocks_segment(axial_world(q, r), axial_world(q + dq, r + dr))
            })
    }

    /// Where the valley this hex sits in drains to, if that is *fresh* water within a bounded walk.
    ///
    /// This is what an animal that cannot see a river steers by. Drainage has no local minima — that
    /// is what makes it drainage — so a herd following it downstream arrives at a channel, where one
    /// descending the bare slope parks in the first hollow it happens to find and dies there. The
    /// walk reads the generator's own pure oracle and opens no chunk.
    ///
    /// Every drainage ends in the sea, and no animal drinks the sea. A guide that stopped at the
    /// first water it met therefore marched thirsty herds onto a beach to stand beside undrinkable
    /// water until they starved of it, so reaching salt ends the walk with no answer at all. What
    /// counts as an answer is fresh standing water, a bank it can drink from, or the generator's own
    /// statement that a channel runs through this cell's bench — the last is what lets the guide name
    /// a river from further off than the herd could see one.
    pub(crate) fn downstream_water(&self, from: (i32, i32)) -> Option<(i32, i32)> {
        let mut at = from;
        for _ in 0..24 {
            at = self.ground_spine.downstream_at(at.0, at.1)?;
            let wet = self.water_depth_at(at.0, at.1) > 0;
            if wet && self.water_surface_at(at.0, at.1) <= scale::SEA_LEVEL_QUANTA {
                return None;
            }
            if wet
                || self.drinking_bank(at.0, at.1)
                || self.ground_spine.river_bench_class_at(at.0, at.1).is_some()
            {
                return Some(at);
            }
        }
        None
    }

    pub(crate) fn decide_herd(&mut self, herd: &mut Herd) -> (i32, i32) {
        let here = herd.position(self.tick);
        let cell = world_to_axial(here.0, here.1);
        let elapsed = self.tick.saturating_sub(herd.left_tick).min(300) as u16;
        herd.hunger = herd.hunger.saturating_add(elapsed / 8).min(2000);
        herd.thirst = herd.thirst.saturating_add(elapsed / 3).min(2000);
        herd.alarm = herd.alarm.saturating_sub(elapsed);
        let species = self
            .definitions
            .species
            .iter()
            .find(|s| s.id == herd.species)
            .expect("species")
            .clone();
        if herd.alarm == 0 {
            if self.drinking_bank(cell.0, cell.1) {
                herd.thirst = 0;
            }
            if herd.hunger > 0 {
                let feed = self.ground_items.iter().position(|i| {
                    (i.q, i.r) == cell
                        && i.item_id == species.feed_item
                        && i.quantity > 0
                        && u32::from(herd.hunger) * u32::from(herd.count) >= 100
                });
                if let Some(index) = feed {
                    self.ground_items[index].quantity -= 1;
                    self.ground_items.retain(|i| i.quantity > 0);
                    self.dirty.ground_items = true;
                    herd.hunger = herd.hunger.saturating_sub(100 / herd.count.max(1));
                } else if let Some(station) = self
                    .pasture_feeder(cell, species.feed_item)
                    .filter(|_| u32::from(herd.hunger) * u32::from(herd.count) >= 100)
                {
                    self.subtract_stock(station, StockKind::Input, species.feed_item, 1);
                    self.dirty.entities.push(self.entities[station].id);
                    herd.hunger = herd.hunger.saturating_sub(100 / herd.count.max(1));
                } else {
                    let eaten = self.graze_grass(
                        cell.0,
                        cell.1,
                        herd.count.saturating_mul(herd.hunger.min(100)),
                    );
                    herd.hunger = herd.hunger.saturating_sub(eaten / herd.count.max(1));
                }
            }
        }
        let shortage = herd.hunger >= 800 || herd.thirst >= 800;
        if shortage {
            let before = herd.shortage_ticks;
            herd.shortage_ticks = herd.shortage_ticks.saturating_add(elapsed);
            herd.healthy_ticks = 0;
            if before < 1000 && herd.shortage_ticks >= 1000 {
                self.events.push(format!(
                    "Herd {} is struggling: {}. Losses will follow if access is not restored",
                    herd.id,
                    if herd.thirst >= 800 {
                        "open a drinking route"
                    } else {
                        "rest the pasture or bring feed"
                    }
                ));
            }
            if herd.shortage_ticks >= 3000 {
                herd.count = herd.count.saturating_sub(1);
                herd.shortage_ticks = 2000;
            }
        } else {
            herd.shortage_ticks = herd.shortage_ticks.saturating_sub(elapsed);
            if herd.hunger < 100 && herd.thirst < 100 && herd.alarm == 0 && herd.count >= 2 {
                herd.healthy_ticks = herd.healthy_ticks.saturating_add(elapsed);
                if herd.healthy_ticks >= 1200 && herd.count < 8 {
                    herd.count += 1;
                    herd.healthy_ticks = 0;
                }
            } else {
                herd.healthy_ticks = 0;
            }
        }
        herd.drive = if herd.alarm > 0 {
            Drive::Flee
        } else if herd.thirst >= 400 || herd.drive == Drive::Thirst && herd.thirst > 80 {
            Drive::Thirst
        } else if herd.hunger >= 60
            || herd.drive == Drive::Graze && herd.hunger > 10
            || self.ground_items.iter().any(|i| {
                i.item_id == species.feed_item
                    && i.quantity > 0
                    && axial_distance(cell, (i.q, i.r)) <= 4
            })
            || hexes_in_radius(cell, 4)
                .into_iter()
                .any(|at| self.pasture_feeder(at, species.feed_item).is_some())
        {
            Drive::Graze
        } else {
            Drive::Rest
        };
        // Animals yield rather than veto a footprint, so a machine can land on a resting herd.
        // Leaving ground it can no longer stand on comes before every drive, `Rest` included:
        // otherwise a resting herd would keep returning its own position and stand inside the
        // building forever. The best neighbouring grass wins, and ties break on the coordinate so
        // the choice is the same on every machine.
        if self.runtime.occupied.contains_key(&cell) || !self.walkable_hex(cell.0, cell.1) {
            let escape = DIRECTIONS
                .iter()
                .map(|&(dq, dr)| (cell.0 + dq, cell.1 + dr))
                .filter(|&(q, r)| self.herd_leg_clear(here, axial_world(q, r)))
                .max_by_key(|&(q, r)| (self.grass_stock(q, r), q, r));
            if let Some(step) = escape {
                return axial_world(step.0, step.1);
            }
        }
        if herd.drive == Drive::Rest {
            return here;
        }

        // How far the animal ranges for this, which is not the same question for food and water.
        // Grass is everywhere it grows, so four hexes always holds some and looking further would
        // only cost. Water is sparse and, worse, terraced: a river cut into its bench is walkable
        // only where the bank steps down, so water three hexes off is routinely six or seven legs
        // off, and a thirsty herd that only ever looked four abandoned a reachable river to wander
        // into the first hollow it found and die there.
        let (reach, guide) = if herd.drive == Drive::Thirst {
            (10, self.downstream_water(cell))
        } else {
            (4, None)
        };

        // Search only reachable cells; closed fences cannot advertise food or water through walls.
        // Each node carries its first step, so only one adjacent leg is ever committed.
        let mut seen = BTreeSet::from([cell]);
        let mut queue = std::collections::VecDeque::from([(cell, cell, 0u16)]);
        let mut best: Option<(i64, (i32, i32))> = None;
        let mut onward: Option<(i64, (i32, i32))> = None;
        while let Some((at, first, distance)) = queue.pop_front() {
            let score = match herd.drive {
                Drive::Thirst => self
                    .drinking_bank(at.0, at.1)
                    .then_some(10_000 - i64::from(distance) * 100),
                Drive::Graze => {
                    let grass = self.grass_stock(at.0, at.1);
                    let feed = self.pasture_feeder(at, species.feed_item).is_some()
                        || self.ground_items.iter().any(|i| {
                            (i.q, i.r) == at && i.item_id == species.feed_item && i.quantity > 0
                        });
                    (grass > 0 || feed).then_some(
                        i64::from(grass) + if feed { 2000 } else { 0 } - i64::from(distance) * 80,
                    )
                }
                Drive::Flee => {
                    let point = axial_world(at.0, at.1);
                    Some(
                        squared_distance(point.0, point.1, self.player.x, self.player.y) / 1000
                            - i64::from(distance) * 10,
                    )
                }
                Drive::Rest => None,
            };
            if let Some(score) = score {
                if best.is_none_or(|(old, _)| score > old) {
                    best = Some((score, first));
                }
            }
            // What to do when the search comes back empty. Standing still is the one answer that
            // cannot be right: a need the herd cannot satisfy where it is only grows, so a herd
            // that waits is a herd that dies where the player will never see why. Each drive names
            // the direction its need lies in instead, and the animal walks it one leg at a time.
            if distance > 0 {
                let heading = match herd.drive {
                    // Downstream while the drainage names fresh water, and inland once it stops.
                    // Downhill is the obvious second answer and the wrong one: the only thing a
                    // drainage with no fresh water in it runs to is the sea, so descending it walks
                    // the herd to salt. Climbing leaves the catchment instead, and the two headings
                    // hand over exactly at the divide, where the far side drains somewhere else and
                    // the guide answers again. This is the whole of migration here: no route and no
                    // memory of a river, only the gradient the terrain already publishes.
                    Drive::Thirst => Some(match guide {
                        Some(goal) => -i64::from(axial_distance(at, goal)),
                        None => i64::from(self.ground_elevation_at(at.0, at.1)),
                    }),
                    // Stripped ground still says what it could carry, so pressure moves off to
                    // pasture that can recover rather than sitting where there is nothing to eat.
                    Drive::Graze => Some(i64::from(self.grass_limit(at.0, at.1))),
                    Drive::Flee | Drive::Rest => None,
                };
                if let Some(heading) = heading {
                    if onward.is_none_or(|(old, _)| heading > old) {
                        onward = Some((heading, first));
                    }
                }
            }
            if distance >= reach {
                continue;
            }
            for (dq, dr) in DIRECTIONS {
                let next = (at.0 + dq, at.1 + dr);
                if seen.contains(&next)
                    || !self.herd_leg_clear(axial_world(at.0, at.1), axial_world(next.0, next.1))
                {
                    continue;
                }
                seen.insert(next);
                queue.push_back((next, if distance == 0 { next } else { first }, distance + 1));
            }
        }
        best.or(onward)
            .map_or(here, |(_, first)| axial_world(first.0, first.1))
    }

    pub(crate) fn alarm_near_player(&mut self) {
        let ids: Vec<_> = self
            .herds
            .values()
            .filter(|h| {
                let species = self
                    .definitions
                    .species
                    .iter()
                    .find(|s| s.id == h.species)
                    .expect("species");
                let point = h.position(self.tick);
                h.alarm == 0
                    && squared_distance(point.0, point.1, self.player.x, self.player.y)
                        < i64::from(species.flight_distance / 2).pow(2)
                    && !self.boundary_blocks_segment((self.player.x, self.player.y), point)
            })
            .map(|h| h.id)
            .collect();
        for id in ids {
            let h = self.herds.get_mut(&id).expect("herd");
            if let Some(ids) = self.herd_arrivals.get_mut(&h.arrive_tick) {
                ids.remove(&id);
            }
            h.from = h.position(self.tick);
            h.to = h.from;
            h.left_tick = self.tick;
            h.arrive_tick = self.tick + 1;
            h.alarm = 20;
            h.drive = Drive::Flee;
            self.herd_arrivals
                .entry(h.arrive_tick)
                .or_default()
                .insert(id);
            self.dirty.herds.push(id);
        }
        self.herd_arrivals.retain(|_, ids| !ids.is_empty());
    }
}
