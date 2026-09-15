use super::decode::{kind_of, status_of, Reader};
use super::*;
pub(super) fn read_entities(reader: &mut Reader, tick: u64) -> Vec<EntitySnapshot> {
    let count = reader.count();
    let mut id = 0u32;
    let mut entities = Vec::with_capacity(count);
    for _ in 0..count {
        id += reader.uvarint() as u32;
        let q = reader.svarint() as i32;
        let r = reader.svarint() as i32;
        let definition_id = reader.uvarint() as DefinitionId;
        let kind = kind_of(reader.u8());
        let orientation = reader.u8();
        let flags = reader.uvarint() as u32;
        let recipe_id = (flags & entity_flag::RECIPE_ID != 0).then(|| reader.uvarint() as RecipeId);
        let cargo = (flags & entity_flag::CARGO != 0).then(|| Cargo {
            item_id: reader.uvarint() as ItemId,
            quantity: reader.uvarint() as u32,
        });
        let lane = if flags & entity_flag::LANE != 0 {
            let count = reader.count();
            (0..count)
                .map(|_| {
                    let item_id = reader.uvarint() as ItemId;
                    let quantity = reader.uvarint() as u32;
                    let elapsed = reader.uvarint();
                    LaneItem {
                        cargo: Cargo { item_id, quantity },
                        entered: tick.saturating_sub(elapsed),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        let inventory = reader.ingredients();
        let input_inventory = if flags & entity_flag::INPUT_INVENTORY != 0 {
            reader.ingredients()
        } else {
            Vec::new()
        };
        let fuel_inventory = if flags & entity_flag::FUEL_INVENTORY != 0 {
            reader.ingredients()
        } else {
            Vec::new()
        };
        let output_inventory = if flags & entity_flag::OUTPUT_INVENTORY != 0 {
            reader.ingredients()
        } else {
            Vec::new()
        };
        let output_routes = if flags & entity_flag::OUTPUT_ROUTES != 0 {
            let count = reader.count();
            (0..count)
                .map(|_| {
                    let item_id = reader.uvarint() as ItemId;
                    let route_q = q + reader.svarint() as i32;
                    let route_r = r + reader.svarint() as i32;
                    let direction = reader.u8();
                    let target_id = match reader.uvarint() as u32 {
                        0 => None,
                        id => Some(id),
                    };
                    OutputRouteSnapshot {
                        item_id,
                        q: route_q,
                        r: route_r,
                        direction,
                        target_id,
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        let progress = reader.uvarint() as u32;
        let progress_total = reader.uvarint() as u32;
        let fuel_charge = if flags & entity_flag::FUEL_CHARGE != 0 {
            reader.uvarint() as u32
        } else {
            0
        };
        let fuel_required = if flags & entity_flag::FUEL_REQUIRED != 0 {
            reader.uvarint() as u32
        } else {
            0
        };
        let status = status_of(reader.u8());
        let next_id = (flags & entity_flag::NEXT_ID != 0).then(|| reader.uvarint() as u32);
        let branch_ids = if flags & entity_flag::BRANCH_IDS != 0 {
            let count = reader.uvarint() as usize;
            (0..count).map(|_| reader.uvarint() as u32).collect()
        } else {
            Vec::new()
        };
        let power_satisfied = if flags & entity_flag::POWER_SATISFIED != 0 {
            reader.uvarint() as u32
        } else {
            0
        };
        let power_demand = if flags & entity_flag::POWER_DEMAND != 0 {
            reader.uvarint() as u32
        } else {
            0
        };
        let power_charge = if flags & entity_flag::POWER_CHARGE != 0 {
            reader.uvarint() as u32
        } else {
            0
        };
        let power_capacity = if flags & entity_flag::POWER_CAPACITY != 0 {
            reader.uvarint() as u32
        } else {
            0
        };
        let water_source =
            (flags & entity_flag::WATER_SOURCE != 0).then(|| crate::WaterSourceSnapshot {
                q: q + reader.svarint() as i32,
                r: r + reader.svarint() as i32,
                available: reader.uvarint() as u32,
                discharge: reader.u8(),
                rate: reader.uvarint() as u32,
            });
        let extraction_source = (flags & entity_flag::EXTRACTION_SOURCE != 0).then(|| {
            crate::ExtractionSourceSnapshot {
                q: q + reader.svarint() as i32,
                r: r + reader.svarint() as i32,
                item_id: reader.uvarint() as ItemId,
            }
        });
        let extraction_output =
            (flags & entity_flag::EXTRACTION_OUTPUT != 0).then(|| crate::OutputRoute {
                q: q + reader.svarint() as i32,
                r: r + reader.svarint() as i32,
                direction: reader.u8(),
            });
        let cells = reader.count();
        let footprint = (0..cells)
            .map(|_| Coordinate {
                q: q + reader.svarint() as i32,
                r: r + reader.svarint() as i32,
            })
            .collect();
        entities.push(EntitySnapshot {
            id,
            q,
            r,
            definition_id,
            kind,
            orientation,
            recipe_id,
            scenario_owned: flags & entity_flag::SCENARIO_OWNED != 0,
            cargo,
            lane,
            inventory,
            input_inventory,
            fuel_inventory,
            output_inventory,
            output_routes,
            water_source,
            extraction_source,
            extraction_output,
            progress,
            progress_total,
            fuel_charge,
            fuel_required,
            power_satisfied,
            power_demand,
            power_charge,
            power_capacity,
            status,
            next_id,
            branch_ids,
            footprint,
        });
    }
    entities
}
