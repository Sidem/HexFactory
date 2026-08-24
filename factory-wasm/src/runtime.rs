//! Derived iteration and topology indexes for the hot path.
//!
//! These indexes are rebuilt when the blueprint graph changes. They are never saved, hashed, or
//! checksummed: they only retain deterministic orders and reverse edges that the tick previously
//! rediscovered and sorted every time it ran.

use super::{BuildingKind, Entity, Links};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct RuntimeIndex {
    /// Every entity in stable-id arbitration order.
    pub(super) entity_order: Vec<usize>,
    /// Entities that have at least one compiled output edge.
    pub(super) transport_order: Vec<usize>,
    /// Only entities with native per-tick machine work, in stable-id order.
    pub(super) machine_order: Vec<usize>,
    /// Whether an entity definition arbitrates its incoming edges as a merger.
    pub(super) mergers: Vec<bool>,
    /// Compiled incoming transport edges. Feeders retain stable-id order.
    pub(super) feeders: Vec<Vec<usize>>,
    /// Targets with at least one feeder and merger arbitration, in stable-id order.
    pub(super) merger_targets: Vec<usize>,
    /// Occupied footprint cells, maintained with the compiled topology.
    pub(super) occupied: BTreeMap<(i32, i32), usize>,
    /// Entities attached to a power network, filled after power compilation.
    pub(super) power_order: Vec<usize>,
    /// Reused transfer scratch. Indexes correspond to the current entity vector.
    pub(super) claimed: Vec<bool>,
    pub(super) delivered: Vec<bool>,
}

impl RuntimeIndex {
    pub(super) fn rebuild(
        &mut self,
        entities: &[Entity],
        graph: &[Links],
        mergers: Vec<bool>,
        occupied: BTreeMap<(i32, i32), usize>,
    ) {
        let mut entity_order: Vec<usize> = (0..entities.len()).collect();
        entity_order.sort_by_key(|&index| entities[index].id);

        self.machine_order = entity_order
            .iter()
            .copied()
            .filter(|&index| {
                matches!(
                    entities[index].kind,
                    BuildingKind::Extractor | BuildingKind::Composer | BuildingKind::Pump
                )
            })
            .collect();
        self.transport_order = entity_order
            .iter()
            .copied()
            .filter(|&index| !graph[index].is_empty())
            .collect();

        self.feeders = vec![Vec::new(); entities.len()];
        for &source in &entity_order {
            for target in graph[source].iter() {
                if target != source {
                    self.feeders[target].push(source);
                }
            }
        }
        self.merger_targets = entity_order
            .iter()
            .copied()
            .filter(|&target| mergers[target] && !self.feeders[target].is_empty())
            .collect();
        self.entity_order = entity_order;
        self.mergers = mergers;
        self.occupied = occupied;
        self.claimed.resize(entities.len(), false);
        self.delivered.resize(entities.len(), false);
        self.clear_transfer_scratch();
    }

    pub(super) fn rebuild_power(&mut self, power_of: &[Option<u32>]) {
        self.power_order = self
            .entity_order
            .iter()
            .copied()
            .filter(|&index| power_of.get(index).copied().flatten().is_some())
            .collect();
    }

    pub(super) fn clear_transfer_scratch(&mut self) {
        self.claimed.fill(false);
        self.delivered.fill(false);
    }
}
