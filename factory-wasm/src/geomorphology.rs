//! Sparse, coarse live erosion over the drainage field.
//!
//! Geological-age erosion belongs to `terra`; this module owns only the slow answer a surveyed
//! river gives to player-built ground. There is no per-cell tick. Once per coarse epoch, at most
//! [`CHUNK_BUDGET`] surveyed chunks, [`CELL_BUDGET`] cells and [`EDGE_BUDGET`] wet river edges are
//! inspected in deterministic coordinate order, and at most [`CHANGE_BUDGET`] banks can move by one
//! height quantum. Only non-zero stress and bed departures are saved.

use super::*;

/// One geomorphic look per in-game hour at the shipped ten ticks per second.
pub(super) const EPOCH_TICKS: u64 = 36_000;
/// Hard surveyed-region bound. A deterministic epoch key rotates the window through large worlds.
pub(super) const CHUNK_BUDGET: usize = 256;
/// Hard generated-cell read bound even if a future scenario uses larger chunks.
pub(super) const CELL_BUDGET: usize = 65_536;
/// Hard read bound for one epoch, independent of how much world has been surveyed.
pub(super) const EDGE_BUDGET: usize = 4_096;
/// Hard write bound for one epoch. A bank can move no more than once per epoch.
pub(super) const CHANGE_BUDGET: usize = 64;
const STRESS_QUANTUM: u32 = 32;
const PROTECTED_RESISTANCE: u32 = u16::MAX as u32;
pub(super) const EROSION_LIMIT_QUANTA: i32 = 16_000;
const STRESS_LIMIT: u32 = 1_000_000;

/// Saved accumulated stress on one outside bank.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct StressCell {
    pub q: i32,
    pub r: i32,
    pub stress: u32,
}

/// Sparse stress and nothing else. A zero is removed immediately.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct BankStress {
    cells: BTreeMap<(i32, i32), u32>,
}

impl BankStress {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub(super) fn len(&self) -> usize {
        self.cells.len()
    }

    fn get(&self, cell: (i32, i32)) -> u32 {
        self.cells.get(&cell).copied().unwrap_or(0)
    }

    fn set(&mut self, cell: (i32, i32), stress: u32) {
        if stress == 0 {
            self.cells.remove(&cell);
        } else {
            self.cells.insert(cell, stress);
        }
    }

    pub(super) fn cells(&self) -> Vec<StressCell> {
        self.cells
            .iter()
            .map(|(&(q, r), &stress)| StressCell { q, r, stress })
            .collect()
    }

    pub(super) fn from_cells(cells: &[StressCell]) -> Self {
        let mut result = Self::new();
        for cell in cells {
            result.set((cell.q, cell.r), cell.stress);
        }
        result
    }

    pub(super) fn hash_into(&self, hash: &mut u32) {
        hash_u64(hash, self.cells.len() as u64);
        for (&(q, r), &stress) in &self.cells {
            hash_i32(hash, q);
            hash_i32(hash, r);
            hash_u32(hash, stress);
        }
    }
}

pub(super) fn validate_saved_stress(cells: &[StressCell]) -> Result<(), String> {
    let mut previous = None;
    for cell in cells {
        if cell.stress == 0
            || cell.stress > STRESS_LIMIT
            || cell.q.abs_diff(0) > 100_000
            || cell.r.abs_diff(0) > 100_000
        {
            return Err("save contains invalid geomorphic stress".into());
        }
        let key = (cell.q, cell.r);
        if previous.is_some_and(|value| value >= key) {
            return Err("save contains duplicate or unordered geomorphic stress".into());
        }
        previous = Some(key);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct Bend {
    bank: (i32, i32),
    deposit: (i32, i32),
    load: u32,
    resistance: u32,
    protected: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BedChange {
    erode: (i32, i32),
    deposit: (i32, i32),
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
pub(super) struct EpochReport {
    pub chunks: usize,
    pub cells: usize,
    pub edges: usize,
    pub bends: usize,
    pub stressed_banks: usize,
    pub changes: usize,
    pub truncated: bool,
}

fn bounded_chunks(chunks: &BTreeSet<(i32, i32)>, start: (i32, i32)) -> Vec<(i32, i32)> {
    chunks
        .range(start..)
        .chain(chunks.range(..start))
        .take(CHUNK_BUDGET)
        .copied()
        .collect()
}

#[derive(Clone, Copy)]
struct CombinedBend {
    load: u32,
    resistance: u32,
    deposit: (i32, i32),
    protected: bool,
}

fn resolve(stress: &mut BankStress, bends: &[Bend]) -> Vec<BedChange> {
    let mut combined: BTreeMap<(i32, i32), CombinedBend> = BTreeMap::new();
    for bend in bends {
        combined
            .entry(bend.bank)
            .and_modify(|entry| {
                entry.load = entry.load.saturating_add(bend.load);
                entry.resistance = entry.resistance.max(bend.resistance);
                entry.deposit = entry.deposit.min(bend.deposit);
                entry.protected |= bend.protected;
            })
            .or_insert(CombinedBend {
                load: bend.load,
                resistance: bend.resistance,
                deposit: bend.deposit,
                protected: bend.protected,
            });
    }

    let mut changes = Vec::new();
    for (bank, bend) in combined {
        if bend.protected || bend.resistance >= PROTECTED_RESISTANCE {
            // Building a revetment clears the pending failure rather than banking it until the
            // player removes the protection.
            stress.set(bank, 0);
            continue;
        }
        let threshold = STRESS_QUANTUM.saturating_mul(bend.resistance.max(1));
        let next = stress.get(bank).saturating_add(bend.load).min(STRESS_LIMIT);
        if next < threshold || changes.len() >= CHANGE_BUDGET {
            stress.set(bank, next);
            continue;
        }
        stress.set(bank, next - threshold);
        changes.push(BedChange {
            erode: bank,
            deposit: bend.deposit,
        });
    }
    changes
}

fn turn(incoming: u8, outgoing: u8) -> Option<(u8, u8, u32)> {
    let clockwise = (outgoing + 6 - incoming) % 6;
    match clockwise {
        1 | 2 => Some(((incoming + 5) % 6, (incoming + 1) % 6, u32::from(clockwise))),
        4 | 5 => Some((
            (incoming + 1) % 6,
            (incoming + 5) % 6,
            u32::from(6 - clockwise),
        )),
        // Straight reaches do no work; a generated U-turn would violate terra's drainage order.
        0 | 3 => None,
        _ => unreachable!("hex direction delta is always in 0..6"),
    }
}

impl Core {
    fn geomorphic_surveyed(&self, cell: (i32, i32)) -> bool {
        let size = self.scenario.chunk_size;
        self.generated_chunks
            .contains(&(floor_div(cell.0, size), floor_div(cell.1, size)))
    }

    fn geomorphic_resistance(
        &self,
        channel: (i32, i32),
        bank: (i32, i32),
        bank_direction: u8,
    ) -> (u32, bool) {
        let substrate: u32 = match self.generated_ground_at(bank.0, bank.1).substrate {
            Substrate::Sand => 1,
            Substrate::Meadow => 2,
            Substrate::Soil => 4,
            Substrate::Rock => 16,
        };
        let surface = self
            .surface_definition(self.surface_at(bank.0, bank.1))
            .map_or(0, |definition| u32::from(definition.erosion_resistance));
        let vegetation = self
            .field_at(bank.0, bank.1)
            .filter(|_| self.deposit_quantity(bank) > 0)
            .and_then(|resource| self.item_definition(resource.item_id))
            .filter(|item| item.regrowth_ticks.is_some())
            .map_or(0, |item| u32::from(item.erosion_resistance));
        let retaining =
            u32::from(self.boundary_erosion_resistance(channel.0, channel.1, bank_direction));
        let occupied = self.entity_at(bank.0, bank.1).is_some();
        let resistance = substrate
            .saturating_add(surface)
            .saturating_add(vegetation)
            .saturating_add(retaining);
        (
            resistance,
            occupied || surface >= PROTECTED_RESISTANCE || retaining >= PROTECTED_RESISTANCE,
        )
    }

    fn geomorphic_bends(&self, epoch: u64) -> (Vec<Bend>, usize, usize, usize, bool) {
        let size = self.scenario.chunk_size;
        let mut bends = Vec::new();
        let mut chunks = 0usize;
        let mut cells = 0usize;
        let mut edges = 0usize;
        let epoch_low = epoch as u32;
        let epoch_high = (epoch >> 32) as u32;
        let start = (
            coordinate_hash(self.seed ^ 0x454d_424b, epoch_low as i32, epoch_high as i32) as i32,
            coordinate_hash(self.seed ^ 0x5249_5645, epoch_high as i32, epoch_low as i32) as i32,
        );
        let selected = bounded_chunks(&self.generated_chunks, start);
        let mut truncated = self.generated_chunks.len() > selected.len();

        'chunks: for (chunk_q, chunk_r) in selected {
            chunks += 1;
            for cell in hexes_in_chunk(chunk_q, chunk_r, size) {
                if cells >= CELL_BUDGET {
                    truncated = true;
                    break 'chunks;
                }
                cells += 1;
                let generated = self.generated_ground_at(cell.0, cell.1);
                let discharge = generated.hydrology.discharge_class;
                if discharge == 0 || self.water_depth_at(cell.0, cell.1) == 0 {
                    continue;
                }
                let Some(downstream) = self.ground_spine.downstream_at(cell.0, cell.1) else {
                    continue;
                };
                if !self.geomorphic_surveyed(downstream) {
                    continue;
                }
                edges += 1;
                if edges > EDGE_BUDGET {
                    truncated = true;
                    break 'chunks;
                }
                let outgoing = DIRECTIONS
                    .iter()
                    .position(|&(dq, dr)| (cell.0 + dq, cell.1 + dr) == downstream)
                    .expect("terra downstream is one hex away")
                    as u8;

                let mut upstream: Option<(u8, u8)> = None;
                for (direction, &(dq, dr)) in DIRECTIONS.iter().enumerate() {
                    let candidate = (cell.0 + dq, cell.1 + dr);
                    if !self.geomorphic_surveyed(candidate)
                        || self.water_depth_at(candidate.0, candidate.1) == 0
                        || self.ground_spine.downstream_at(candidate.0, candidate.1) != Some(cell)
                    {
                        continue;
                    }
                    let class = self
                        .generated_ground_at(candidate.0, candidate.1)
                        .hydrology
                        .discharge_class;
                    if class > 0 && upstream.is_none_or(|(_, current)| class > current) {
                        upstream = Some((direction as u8, class));
                    }
                }
                let Some((upstream_direction, upstream_class)) = upstream else {
                    continue;
                };
                let incoming = (upstream_direction + 3) % 6;
                let Some((bank_direction, deposit_direction, curvature)) = turn(incoming, outgoing)
                else {
                    continue;
                };
                let (bank_dq, bank_dr) = DIRECTIONS[bank_direction as usize];
                let (deposit_dq, deposit_dr) = DIRECTIONS[deposit_direction as usize];
                let bank = (cell.0 + bank_dq, cell.1 + bank_dr);
                let deposit = (cell.0 + deposit_dq, cell.1 + deposit_dr);
                if bank == deposit
                    || !self.geomorphic_surveyed(bank)
                    || !self.geomorphic_surveyed(deposit)
                {
                    continue;
                }
                let (resistance, bank_protected) =
                    self.geomorphic_resistance(cell, bank, bank_direction);
                let (_, deposit_protected) =
                    self.geomorphic_resistance(cell, deposit, deposit_direction);
                bends.push(Bend {
                    bank,
                    deposit,
                    load: u32::from(discharge.max(upstream_class)).saturating_mul(curvature),
                    resistance,
                    protected: bank_protected || deposit_protected,
                });
            }
        }
        (bends, chunks, cells, edges.min(EDGE_BUDGET), truncated)
    }

    fn apply_bed_change(&mut self, cell: (i32, i32), delta: i32) -> bool {
        let mut ground = self.ground.get(&cell).cloned().unwrap_or(GroundCell {
            q: cell.0,
            r: cell.1,
            surface: 0,
            elevation: 0,
            erosion: 0,
            paid: Vec::new(),
        });
        let next =
            (i32::from(ground.erosion) + delta).clamp(-EROSION_LIMIT_QUANTA, EROSION_LIMIT_QUANTA);
        if next == i32::from(ground.erosion) {
            return false;
        }
        ground.erosion = i16::try_from(next).expect("erosion is clamped to i16");
        if ground.is_untouched() {
            self.ground.remove(&cell);
        } else {
            self.ground.insert(cell, ground);
        }
        true
    }

    pub(super) fn run_geomorphic_epoch(&mut self) -> EpochReport {
        if !self.ground_is_physical() {
            return EpochReport::default();
        }
        let epoch = self.tick / EPOCH_TICKS;
        let (bends, chunks, cells, edges, truncated) = self.geomorphic_bends(epoch);
        let changes = resolve(&mut self.bank_stress, &bends);
        let mut seeds = BTreeSet::new();
        for change in &changes {
            if self.apply_bed_change(change.erode, -1) {
                seeds.insert(change.erode);
            }
            if self.apply_bed_change(change.deposit, 1) {
                seeds.insert(change.deposit);
            }
        }
        if !seeds.is_empty() {
            // An old transaction captured the bed before this epoch. Replaying it afterwards would
            // erase natural change that was never part of the player's edit, so the session-local
            // undo stack ends at the same discontinuity the visible ground does.
            self.ground_undo.clear();
            *self.ground_hash_cache.borrow_mut() = None;
            self.dirty.ground = true;
            self.settle_water(&seeds.into_iter().collect::<Vec<_>>());
            self.replan_walk();
            self.events.push(format!(
                "The river moved {} bank quantum{}",
                changes.len(),
                if changes.len() == 1 { "" } else { "s" }
            ));
        }
        EpochReport {
            chunks,
            cells,
            edges,
            bends: bends.len(),
            stressed_banks: self.bank_stress.len(),
            changes: changes.len(),
            truncated,
        }
    }

    pub(super) fn advance_geomorphology(&mut self) {
        if self.tick > 0 && self.tick % EPOCH_TICKS == 0 {
            self.run_geomorphic_epoch();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerated_bend_erodes_outside_and_deposits_inside() {
        let mut stress = BankStress::new();
        let bend = Bend {
            bank: (0, 1),
            deposit: (1, 0),
            load: 8,
            resistance: 1,
            protected: false,
        };
        for _ in 0..3 {
            assert!(resolve(&mut stress, &[bend]).is_empty());
        }
        assert_eq!(
            resolve(&mut stress, &[bend]),
            vec![BedChange {
                erode: (0, 1),
                deposit: (1, 0)
            }]
        );
        assert!(stress.is_empty());
    }

    #[test]
    fn straight_dry_and_protected_reaches_do_no_work() {
        assert_eq!(turn(0, 0), None, "straight reach");
        let mut stress = BankStress::new();
        let protected = Bend {
            bank: (0, 1),
            deposit: (1, 0),
            load: u32::MAX,
            resistance: PROTECTED_RESISTANCE,
            protected: true,
        };
        assert!(resolve(&mut stress, &[protected]).is_empty());
        assert!(stress.is_empty());
        // A dry reach never produces a Bend, so the resolver's empty input is its exact work set.
        assert!(resolve(&mut stress, &[]).is_empty());
    }

    #[test]
    fn simultaneous_banks_are_resolved_in_coordinate_order_and_bounded() {
        let mut stress = BankStress::new();
        let bends: Vec<_> = (0..(CHANGE_BUDGET + 3))
            .rev()
            .map(|q| Bend {
                bank: (q as i32, 0),
                deposit: (q as i32, 1),
                load: STRESS_QUANTUM,
                resistance: 1,
                protected: false,
            })
            .collect();
        let changes = resolve(&mut stress, &bends);
        assert_eq!(changes.len(), CHANGE_BUDGET);
        assert!(changes.windows(2).all(|pair| pair[0].erode < pair[1].erode));
        assert_eq!(stress.len(), 3);
    }

    #[test]
    fn shipped_resistance_data_protects_paving_and_retaining_walls() {
        let definitions: DefinitionsInput =
            serde_json::from_str(include_str!("../../src/data/definitions.json")).unwrap();
        assert!(definitions
            .surfaces
            .iter()
            .all(|surface| surface.erosion_resistance == u16::MAX));
        assert!(definitions
            .boundaries
            .iter()
            .filter(|boundary| boundary.family == BoundaryFamily::Wall)
            .all(|boundary| boundary.erosion_resistance == u16::MAX));
        let wood = definitions
            .items
            .iter()
            .find(|item| item.key == "wood")
            .unwrap();
        assert!(wood.regrowth_ticks.is_some());
        assert!(wood.erosion_resistance > 0 && wood.erosion_resistance < u16::MAX);
    }

    #[test]
    fn surveyed_chunk_window_wraps_in_order_and_is_bounded() {
        let chunks: BTreeSet<_> = (0..(CHUNK_BUDGET as i32 + 3)).map(|q| (q, -q)).collect();
        let selected = bounded_chunks(&chunks, (CHUNK_BUDGET as i32 + 1, i32::MIN));
        assert_eq!(selected.len(), CHUNK_BUDGET);
        assert_eq!(
            selected[0],
            (CHUNK_BUDGET as i32 + 1, -(CHUNK_BUDGET as i32 + 1))
        );
        assert_eq!(
            selected[1],
            (CHUNK_BUDGET as i32 + 2, -(CHUNK_BUDGET as i32 + 2))
        );
        assert_eq!(selected[2], (0, 0));
    }
}
