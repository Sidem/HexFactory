//! Native mobile herds. Wildlife never participates in the resource field or extractor graph.
use super::*;
pub(crate) mod hunting;
mod husbandry;
mod interaction;
pub(crate) use husbandry::PastureAction;
mod needs;
mod pasture;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Species {
    pub id: u16,
    pub speed: u16,
    pub body_radius: u16,
    pub herd_size: u16,
    pub flight_distance: u16,
    pub feed_item: ItemId,
    pub carcass_item: ItemId,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) enum Drive {
    Rest,
    Graze,
    Thirst,
    Flee,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Herd {
    pub id: u32,
    pub species: u16,
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub left_tick: u64,
    pub arrive_tick: u64,
    pub count: u16,
    pub drive: Drive,
    pub hunger: u16,
    pub thirst: u16,
    pub alarm: u16,
    pub healthy_ticks: u16,
    pub shortage_ticks: u16,
    pub hunt_tick: u64,
}

impl Herd {
    pub fn position(&self, tick: u64) -> (i32, i32) {
        let duration = self.arrive_tick.saturating_sub(self.left_tick);
        if duration == 0 || tick >= self.arrive_tick {
            return self.to;
        }
        let elapsed = tick.saturating_sub(self.left_tick).min(duration);
        let axis = |from: i32, to: i32| {
            (i128::from(from)
                + (i128::from(to) - i128::from(from)) * i128::from(elapsed) / i128::from(duration))
                as i32
        };
        (axis(self.from.0, self.to.0), axis(self.from.1, self.to.1))
    }
}

/// Saved facts live in Core. This queue is rebuilt from the legs after loading.
pub(crate) fn schedule(herds: &BTreeMap<u32, Herd>) -> BTreeMap<u64, BTreeSet<u32>> {
    let mut arrivals: BTreeMap<u64, BTreeSet<u32>> = BTreeMap::new();
    for herd in herds.values() {
        arrivals
            .entry(herd.arrive_tick)
            .or_default()
            .insert(herd.id);
    }
    arrivals
}

impl Core {
    /// Every hex around this chunk an animal could walk to a drink from, in one sweep.
    ///
    /// It has to be walked rather than measured. A river cut into its bench is a terrace, so water
    /// three hexes off across the drop is not water a herd can get to at all, and hex distance says
    /// the opposite.
    ///
    /// Asked the other way round — per candidate hex, can this one reach water — the same flood
    /// runs again for every candidate, and seeding becomes the most expensive thing chunk
    /// generation does. Water is the rarer end, so the search starts there instead and one flood
    /// answers for all of them. Reversing it is sound because every leg test is symmetric:
    /// [`Core::grade_blocks`] says so in as many words, and the others are facts about a hex rather
    /// than about a direction.
    ///
    /// This is the physical half of what [`Core::herd_leg_clear`] asks — walkable, dry, and not over
    /// a step the animal cannot take — without the half that belongs to a running game: what the
    /// player has surveyed, and what they have since built. Seeding is a fact about the world the
    /// generator made, so it reads only the world the generator made.
    fn ground_within_reach_of_water(
        &self,
        chunk_q: i32,
        chunk_r: i32,
        reach: i32,
    ) -> BTreeSet<(i32, i32)> {
        let size = self.scenario.chunk_size;
        let (base_q, base_r) = (chunk_q * size, chunk_r * size);
        let mut reached = BTreeSet::new();
        let mut queue = std::collections::VecDeque::new();
        for q in (base_q - reach)..(base_q + size + reach) {
            for r in (base_r - reach)..(base_r + size + reach) {
                if self.walkable_hex(q, r)
                    && self.water_depth_at(q, r) == 0
                    && self.drinking_bank(q, r)
                    && reached.insert((q, r))
                {
                    queue.push_back(((q, r), 0));
                }
            }
        }
        while let Some((at, distance)) = queue.pop_front() {
            if distance >= reach {
                continue;
            }
            for (dq, dr) in DIRECTIONS {
                let next = (at.0 + dq, at.1 + dr);
                if !self.walkable_hex(next.0, next.1)
                    || self.water_depth_at(next.0, next.1) > 0
                    || self.grade_blocks(at, next)
                    || !reached.insert(next)
                {
                    continue;
                }
                queue.push_back((next, distance + 1));
            }
        }
        reached
    }

    pub(crate) fn seed_herds(&mut self, chunk_q: i32, chunk_r: i32) {
        if !self.scenario.generated_environment {
            return;
        }
        let Some(species) = self.definitions.species.first().cloned() else {
            return;
        };
        // The hexes that could carry a herd on their own account, before the expensive question is
        // asked. Most chunks are rock, highland or open water and hold none, and settling that with
        // a hash and a terrain band means those chunks never pay for the flood below.
        let candidates: Vec<(i32, i32)> =
            hexes_in_chunk(chunk_q, chunk_r, self.scenario.chunk_size)
                .into_iter()
                .filter(|&(q, r)| {
                    coordinate_hash(self.seed ^ 0x4772_617a, q, r).is_multiple_of(5)
                        && matches!(self.terrain_at(q, r), Terrain::Lowland | Terrain::Hills)
                        && self.walkable_hex(q, r)
                        && self.water_depth_at(q, r) == 0
                })
                .collect();
        if candidates.is_empty() {
            return;
        }
        // Six, against the ten a thirsty herd searches, so that grazing a few hexes out over the
        // course of a game does not strand an animal that was seeded able to drink.
        let watered = self.ground_within_reach_of_water(chunk_q, chunk_r, 6);
        for (q, r) in candidates {
            if self.herds.len() >= 512 {
                break;
            }
            // Ground that fails this seeds herds which thirst to death before the player has
            // walked to them. Requiring it is also what makes these riverbank grazers rather
            // than animals scattered evenly over the map.
            if !watered.contains(&(q, r)) {
                continue;
            }
            let id = self.next_herd_id;
            self.next_herd_id += 1;
            let arrive_tick = self.tick + 100 + u64::from(id % 100);
            self.herds.insert(
                id,
                Herd {
                    id,
                    species: species.id,
                    from: axial_world(q, r),
                    to: axial_world(q, r),
                    left_tick: self.tick,
                    arrive_tick,
                    count: species.herd_size,
                    drive: Drive::Rest,
                    hunger: 0,
                    thirst: 0,
                    alarm: 0,
                    healthy_ticks: 0,
                    shortage_ticks: 0,
                    hunt_tick: 0,
                },
            );
            self.herd_arrivals
                .entry(arrive_tick)
                .or_default()
                .insert(id);
            self.dirty.herds.push(id);
        }
    }

    pub(crate) fn rebuild_herd_schedule(&mut self) {
        self.herd_arrivals = schedule(&self.herds);
    }

    /// Bounded adjacent legs use the same terrain, grade and boundary authority as walking.
    pub(crate) fn herd_leg_clear(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        let a = world_to_axial(from.0, from.1);
        let b = world_to_axial(to.0, to.1);
        hydrology::WaterField::surveyed(self, b.0, b.1)
            && self.walkable_hex(b.0, b.1)
            && self.water_depth_at(b.0, b.1) == 0
            && !self.grade_blocks(a, b)
            && !self.boundary_blocks_segment(from, to)
            && (0..=8).all(|step| {
                let x = from.0 + ((i64::from(to.0) - i64::from(from.0)) * step / 8) as i32;
                let y = from.1 + ((i64::from(to.1) - i64::from(from.1)) * step / 8) as i32;
                !self.boundary_blocks_circle(x, y, 650)
                    && hexes_in_radius(world_to_axial(x, y), 1)
                        .iter()
                        .all(|&(q, r)| {
                            !self.runtime.occupied.contains_key(&(q, r)) || {
                                let centre = axial_world(q, r);
                                !circles_overlap(x, y, 650, centre.0, centre.1, BUILDING_RADIUS)
                            }
                        })
            })
    }

    pub(crate) fn advance_herds(&mut self) {
        self.regrow_grass();
        while self
            .herd_arrivals
            .first_key_value()
            .is_some_and(|(&tick, _)| tick <= self.tick)
        {
            let (_, ids) = self.herd_arrivals.pop_first().expect("due arrival");
            for id in ids {
                let Some(mut herd) = self.herds.remove(&id) else {
                    continue;
                };
                let here = herd.position(self.tick);
                self.reunite_lone_grazer(&mut herd);
                let hash = coordinate_hash(
                    self.seed ^ self.tick as u32,
                    id as i32,
                    i32::from(herd.count),
                );
                let target = self.decide_herd(&mut herd);
                let species = self
                    .definitions
                    .species
                    .iter()
                    .find(|s| s.id == herd.species)
                    .expect("validated species");
                let distance =
                    integer_sqrt(squared_distance(here.0, here.1, target.0, target.1)) as u64;
                herd.from = here;
                herd.to = target;
                herd.left_tick = self.tick;
                herd.arrive_tick = self.tick
                    + if distance == 0 {
                        100 + u64::from(hash % 100)
                    } else {
                        distance
                            .div_ceil(
                                u64::from(species.speed)
                                    * if herd.drive == Drive::Flee && herd.alarm > 30 {
                                        2
                                    } else {
                                        1
                                    },
                            )
                            .max(1)
                    };
                self.herd_arrivals
                    .entry(herd.arrive_tick)
                    .or_default()
                    .insert(id);
                if herd.count > 0 {
                    self.herds.insert(id, herd);
                }
                self.dirty.herds.push(id);
            }
        }
    }

    /// A fence closed or ground raised across a walking herd stops it where it stands rather than
    /// letting it slide through the new wall, and it decides again on the next tick.
    ///
    /// Only legs actually in flight are checked, and only against the authority that would have
    /// refused them in the first place. A herd standing still is left alone even when a machine
    /// lands on its hex: it leaves through [`Core::decide_herd`] on its own schedule instead, which
    /// keeps construction out of the checksum and so keeps undo exact. No extractor invalidation
    /// happens here at all — wildlife is not a deposit and never was.
    pub(crate) fn invalidate_blocked_legs(&mut self) {
        let ids: Vec<_> = self
            .herds
            .values()
            .filter(|h| h.from != h.to && !self.herd_leg_clear(h.position(self.tick), h.to))
            .map(|h| h.id)
            .collect();
        for id in ids {
            let h = self.herds.get_mut(&id).expect("herd exists");
            if let Some(ids) = self.herd_arrivals.get_mut(&h.arrive_tick) {
                ids.remove(&id);
            }
            h.from = h.position(self.tick);
            h.to = h.from;
            h.left_tick = self.tick;
            h.arrive_tick = self.tick + 1;
            self.herd_arrivals
                .entry(h.arrive_tick)
                .or_default()
                .insert(id);
            self.dirty.herds.push(id);
        }
        self.herd_arrivals.retain(|_, ids| !ids.is_empty());
    }

    pub(crate) fn disturb_habitat_ring(&mut self, q: i32, r: i32) {
        self.refresh_fouling(q, r);
        self.dirty.habitats.push((q, r));
        self.dirty
            .habitats
            .extend(DIRECTIONS.iter().map(|&(dq, dr)| (q + dq, r + dr)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leg_interpolation_is_exact_and_clamped() {
        let herd = Herd {
            id: 1,
            species: 1,
            from: (-1774, -1536),
            to: (1774, 1536),
            left_tick: 10,
            arrive_tick: 40,
            count: 3,
            drive: Drive::Rest,
            hunger: 0,
            thirst: 0,
            alarm: 0,
            healthy_ticks: 0,
            shortage_ticks: 0,
            hunt_tick: 0,
        };
        assert_eq!(herd.position(0), herd.from);
        assert_eq!(herd.position(25), (0, 0));
        assert_eq!(herd.position(40), herd.to);
        assert_eq!(herd.position(u64::MAX), herd.to);
    }
}
