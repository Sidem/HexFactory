//! Eaten cover is a sparse stock deficit, separate from ongoing waste pressure.
use super::*;

impl Core {
    pub(crate) fn grass_limit(&self, q: i32, r: i32) -> u16 {
        if self.water_depth_at(q, r) > 0
            || self.surface_at(q, r) != 0
            || self.runtime.occupied.contains_key(&(q, r))
        {
            return 0;
        }
        match self.terrain_at(q, r) {
            Terrain::Lowland => 600,
            Terrain::Hills => 300,
            // Both sandy bands, because fertility is the test and the band is not: a beach the
            // player has cut a canal to is watered ground, and a bench the river has left dry is
            // not. The split between them is what the player sees, not what grows.
            Terrain::Riverbank | Terrain::Shore if self.fertile_riverbank_at(q, r).is_some() => 240,
            _ => 0,
        }
    }

    pub(crate) fn grass_stock(&self, q: i32, r: i32) -> u16 {
        self.grass_limit(q, r)
            .saturating_sub(self.grazed.get(&(q, r)).copied().unwrap_or(0))
    }

    pub(crate) fn graze_grass(&mut self, q: i32, r: i32, amount: u16) -> u16 {
        let eaten = self.grass_stock(q, r).min(amount);
        if eaten > 0 {
            *self.grazed.entry((q, r)).or_default() += eaten;
            self.dirty.habitats.push((q, r));
        }
        eaten
    }

    /// Only disturbed grass participates; untouched area has no regrowth work.
    pub(crate) fn regrow_grass(&mut self) {
        if !self.tick.is_multiple_of(100) {
            return;
        }
        let keys: Vec<_> = self.grazed.keys().copied().collect();
        for (q, r) in keys {
            let pressure = self.fouled.get(&(q, r)).copied().unwrap_or(0);
            let old = self.grazed[&(q, r)];
            let new = if pressure > 0 {
                old.saturating_add(pressure.min(30)).min(600)
            } else {
                old.saturating_sub(if self.grass_limit(q, r) >= 600 { 12 } else { 6 })
            };
            if new == 0 {
                self.grazed.remove(&(q, r));
            } else {
                self.grazed.insert((q, r), new);
            }
            if old != new {
                self.dirty.habitats.push((q, r));
            }
        }
    }

    /// Recomputed only when waste changes, never as a per-herd capacity query.
    pub(crate) fn refresh_fouling(&mut self, q: i32, r: i32) {
        for cell in
            std::iter::once((q, r)).chain(DIRECTIONS.iter().map(|&(dq, dr)| (q + dq, r + dr)))
        {
            let pressure = self
                .ground_items
                .iter()
                .filter(|item| axial_distance(cell, (item.q, item.r)) <= 1)
                .map(|item| {
                    self.item_definition(item.item_id).map_or(0, |d| {
                        item.quantity.saturating_mul(u32::from(d.habitat_damage))
                    })
                })
                .fold(0u32, u32::saturating_add)
                .min(1000) as u16;
            if pressure == 0 {
                self.fouled.remove(&cell);
            } else {
                self.fouled.insert(cell, pressure);
                self.grazed.entry(cell).or_insert(0);
            }
            self.dirty.habitats.push(cell);
        }
    }

    pub(crate) fn rebuild_fouling(&mut self) {
        self.fouled.clear();
        let cells: Vec<_> = self.ground_items.iter().map(|i| (i.q, i.r)).collect();
        for (q, r) in cells {
            self.refresh_fouling(q, r);
        }
    }
}
