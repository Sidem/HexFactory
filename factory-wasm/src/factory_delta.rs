//! factory_delta — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Factory {
    /// The next delta, built from the core's dirty marks against the baseline the host holds.
    ///
    /// With no baseline — the first delta, and every reset, new game, and load — the host has no
    /// state to patch, so it gets a complete replacement. Otherwise only marked entries are
    /// materialized at all, and only those that genuinely differ from the baseline travel.
    pub(crate) fn build_delta(&mut self) -> SnapshotDelta {
        let base_revision = self.snapshot_revision;
        let revision = base_revision.saturating_add(1);
        self.snapshot_revision = revision;

        if self.baseline.is_none() {
            let snapshot = self.core.snapshot();
            self.baseline = Some(SnapshotBaseline::from_snapshot(&snapshot));
            self.core.dirty = SnapshotDirty::default();
            return SnapshotDelta::full(base_revision, revision, &snapshot);
        }

        let core = &mut self.core;
        let baseline = self.baseline.as_mut().expect("baseline exists");
        let mut dirty = std::mem::take(&mut core.dirty);
        let marked_entities = drain_marks(&mut dirty.entities);
        let marked_resources = drain_marks(&mut dirty.resources);
        let marked_terrain = drain_marks(&mut dirty.terrain);
        let mut marked_habitats = drain_marks(&mut dirty.habitats);
        for &(chunk_q, chunk_r) in &marked_terrain {
            marked_habitats.extend(hexes_in_chunk(chunk_q, chunk_r, core.scenario.chunk_size));
        }
        marked_habitats.sort_unstable();
        marked_habitats.dedup();

        let mut removed: Vec<u32> = Vec::new();
        for id in &dirty.removed {
            if baseline.buildings.remove(id).is_some() {
                removed.push(*id);
            }
        }
        let mut changed: Vec<EntitySnapshot> = Vec::new();
        for id in marked_entities {
            // Ids are monotonic, so an erased id never returns and needs no rebuild.
            if !dirty.removed.is_empty() && dirty.removed.contains(&id) {
                continue;
            }
            let Some(index) = core.index_of_entity(id) else {
                continue;
            };
            // A mark only says an entry may have moved. Comparing against what the host already
            // holds is what keeps a conservative mark from becoming wasted payload.
            let entity = core.entity_snapshot(index);
            if baseline.buildings.get(&id) != Some(&entity) {
                baseline.buildings.insert(id, entity.clone());
                changed.push(entity);
            }
        }
        // Both lists are in ascending id order, so the host merges them in one linear pass.
        let buildings = (!changed.is_empty() || !removed.is_empty()).then_some(BuildingsDelta {
            replace: false,
            changed,
            removed,
        });

        let resources = if dirty.resources_replace {
            Some(ResourcesDelta {
                replace: true,
                changed: core.resource_snapshots(),
            })
        } else {
            let changed: Vec<ResourceSnapshot> = marked_resources
                .into_iter()
                .filter_map(|key| core.resource_snapshot(key))
                .collect();
            (!changed.is_empty()).then_some(ResourcesDelta {
                replace: false,
                changed,
            })
        };

        let terrain = {
            let changed: Vec<TileSnapshot> = marked_terrain
                .into_iter()
                .flat_map(|(chunk_q, chunk_r)| core.chunk_terrain_snapshots(chunk_q, chunk_r))
                .collect();
            (!changed.is_empty()).then_some(TerrainDelta {
                replace: false,
                changed,
            })
        };

        let habitats = {
            let mut changed = Vec::new();
            for (q, r) in marked_habitats {
                let cell = core.habitat_snapshot(q, r);
                if cell.capacity == 0 {
                    if let Some(old) = baseline.habitats.remove(&(q, r)) {
                        changed.push(HabitatSnapshot {
                            capacity: 0,
                            discharge: 0,
                            ..old
                        });
                    }
                } else if baseline.habitats.get(&(q, r)) != Some(&cell) {
                    baseline.habitats.insert((q, r), cell);
                    changed.push(cell);
                }
            }
            (!changed.is_empty()).then_some(HabitatsDelta {
                replace: false,
                changed,
            })
        };

        let ground_items = if dirty.ground_items || baseline.ground_items != core.ground_items {
            baseline.ground_items = core.ground_items.clone();
            Some(core.ground_items.clone())
        } else {
            None
        };

        let research_availability = if baseline.insight != core.insight
            || !baseline
                .researched
                .iter()
                .copied()
                .eq(core.researched.iter().copied())
        {
            take_changed(
                &mut baseline.research_availability,
                core.research_availability_snapshot(),
            )
        } else {
            None
        };

        SnapshotDelta {
            base_revision,
            revision,
            research_availability,
            skills: if baseline.skills.state != core.skills
                || baseline.player.state.carry_slots != core.player.carry_slots
                || baseline.player.state.build_range != core.player.build_range
            {
                take_changed(&mut baseline.skills, core.skills_snapshot())
            } else {
                None
            },
            tick: core.tick,
            checksum: core.checksum(),
            belt_transit_ticks: BELT_TRANSIT_TICKS as u32,
            scenario: take_changed(&mut baseline.scenario, core.scenario.key.clone()),
            scenario_name: take_changed(&mut baseline.scenario_name, core.scenario.name.clone()),
            world_version: take_changed_copy(&mut baseline.world_version, WORLD_GENERATOR_VERSION),
            seed: take_changed_copy(&mut baseline.seed, core.seed),
            delivered: take_changed_copy(&mut baseline.delivered, core.delivered),
            delivered_by_item: take_changed(
                &mut baseline.delivered_by_item,
                core.delivered_by_item_snapshot(),
            ),
            insight: take_changed_copy(&mut baseline.insight, core.insight),
            victory: take_changed_copy(&mut baseline.victory, core.victory),
            contract: take_changed(&mut baseline.contract, core.contract_snapshot()),
            requests: take_changed(&mut baseline.requests, core.request_snapshots()),
            player: take_changed(&mut baseline.player, core.player_snapshot()),
            researched: take_changed(
                &mut baseline.researched,
                core.researched.iter().copied().collect(),
            ),
            chunks: dirty
                .chunks
                .then(|| take_changed(&mut baseline.chunks, core.chunk_snapshots()))
                .flatten(),
            // Terrain is never retained for comparison: `generate_chunk` is the only path that can
            // add a tile, nothing ever changes or removes one, and the mark names the chunks it
            // added. The surveyed-chunk set is ordered, and so are the marks, so the tiles travel in
            // the same order a full snapshot would have listed them in.
            terrain,
            habitats,
            resources,
            buildings,
            ground_items,
            boundaries: dirty
                .boundaries
                .then(|| take_changed(&mut baseline.boundaries, core.boundary_snapshot()))
                .flatten(),
            ground: dirty
                .ground
                .then(|| take_changed(&mut baseline.ground, core.ground_snapshot()))
                .flatten(),
            // Spoil is a single number and the tray shows it on every preview, so it is compared
            // rather than marked: the comparison is cheaper than the mark would be.
            spoil: (baseline.spoil != core.spoil).then(|| {
                baseline.spoil = core.spoil;
                core.spoil
            }),
            water: dirty
                .water
                .then(|| take_changed(&mut baseline.water, core.water.cells()))
                .flatten(),
            events: take_changed(&mut baseline.events, core.events.clone()),
        }
    }
}
