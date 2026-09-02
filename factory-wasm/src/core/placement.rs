//! placement — extracted from lib.rs by scripts/rust-split.mjs.
//! Methods moved verbatim; add the imports the compiler asks for.

use super::*;

impl Core {
    pub(crate) fn footprint_for(&self, placed: PlacedBuilding, orientation: u8) -> Vec<Coordinate> {
        self.building_definition(placed.definition_id)
            .map(|definition| {
                Self::oriented_cells(&definition.footprint, placed.q, placed.r, orientation)
            })
            .unwrap_or_else(|| {
                vec![Coordinate {
                    q: placed.q,
                    r: placed.r,
                }]
            })
    }

    pub(crate) fn envelope_for(&self, placed: PlacedBuilding, orientation: u8) -> Vec<Coordinate> {
        self.building_definition(placed.definition_id)
            .map(|definition| {
                Self::oriented_cells(
                    &definition.service_envelope,
                    placed.q,
                    placed.r,
                    orientation,
                )
            })
            .unwrap_or_default()
    }

    pub(crate) fn clearance_for(&self, placed: PlacedBuilding, orientation: u8) -> Vec<Coordinate> {
        self.building_definition(placed.definition_id)
            .map(|definition| {
                Self::oriented_cells(
                    &definition.overhead_clearance,
                    placed.q,
                    placed.r,
                    orientation,
                )
            })
            .unwrap_or_default()
    }

    /// Rotate authored offsets onto a heading and translate them to a world anchor.
    ///
    /// No definition needs a multi-cell corner-heading footprint yet, and the validator keeps
    /// that axis single-cell (envelope and clearance included). A single `(0, 0)` cell is
    /// invariant under rotation, so leaving a corner heading unrotated is exact.
    pub(crate) fn oriented_cells(
        offsets: &[Coordinate],
        q: i32,
        r: i32,
        orientation: u8,
    ) -> Vec<Coordinate> {
        offsets
            .iter()
            .map(|offset| {
                let offset = match orientation {
                    NORTH.. => *offset,
                    turns => rotate_coordinate(*offset, turns),
                };
                Coordinate {
                    q: q + offset.q,
                    r: r + offset.r,
                }
            })
            .collect()
    }

    pub(crate) fn entity_footprint(&self, entity: &Entity) -> Vec<Coordinate> {
        self.footprint_for(entity.placed, entity.placed.orientation)
    }

    pub(crate) fn entity_envelope(&self, entity: &Entity) -> Vec<Coordinate> {
        self.envelope_for(entity.placed, entity.placed.orientation)
    }

    pub(crate) fn entity_clearance(&self, entity: &Entity) -> Vec<Coordinate> {
        self.clearance_for(entity.placed, entity.placed.orientation)
    }

    /// True when this kind may share a cell with someone else's overhead clearance.
    ///
    /// A rotor reserves air, not the ground: belts, poles and bridge decks can pass under it.
    /// Machines cannot.
    pub(crate) fn is_low_infrastructure(kind: BuildingKind) -> bool {
        matches!(
            kind,
            BuildingKind::Belt | BuildingKind::Pole | BuildingKind::Bridge
        )
    }

    pub(crate) fn pad_step_limit(&self, class: FoundationClass) -> i32 {
        match class {
            FoundationClass::Pad => self.build_step_limit(),
            FoundationClass::Span => self.walk_step_limit(),
            FoundationClass::Retaining => self.grade_limit(),
        }
    }

    /// Squared world-unit distance from the player to a hex centre.
    pub(crate) fn player_range_to_hex(&self, q: i32, r: i32) -> i64 {
        let (x, y) = axial_world(q, r);
        squared_distance(self.player.x, self.player.y, x, y)
    }

    pub(crate) fn within_world_range(&self, q: i32, r: i32, range: u32) -> bool {
        self.player_range_to_hex(q, r) <= i64::from(range).pow(2)
    }

    /// True when the player is within `range` world units of any cell this building occupies.
    ///
    /// Access is a disc around the whole footprint, not around the anchor tile: standing beside a
    /// three-cell hub's far lobe is standing beside the hub.
    pub(crate) fn within_world_range_of_entity(&self, index: usize, range: u32) -> bool {
        let limit = i64::from(range).pow(2);
        self.entity_footprint(&self.entities[index])
            .iter()
            .any(|cell| self.player_range_to_hex(cell.q, cell.r) <= limit)
    }

    /// True when the player stands within `radius` hex steps of any cell this building occupies.
    pub(crate) fn within_hex_range_of_entity(&self, index: usize, radius: i32) -> bool {
        let player = world_to_axial(self.player.x, self.player.y);
        self.entity_footprint(&self.entities[index])
            .iter()
            .any(|cell| axial_distance(player, (cell.q, cell.r)) <= radius)
    }

    /// Build-range for a named hex: the building that occupies it, measured from its whole
    /// footprint, or the hex itself when nothing stands there.
    pub(crate) fn within_build_range_of_target(&self, q: i32, r: i32) -> bool {
        match self.entity_at(q, r) {
            Some(index) => self.within_world_range_of_entity(index, self.player.build_range),
            None => self.within_world_range(q, r, self.player.build_range),
        }
    }
}
