use super::*;

/// `tick` is the delta's own tick, and every lane entry is coded against it: an item that stepped
/// onto its belt two ticks ago travels as `2` rather than as a nine-digit absolute tick, which is
/// the difference between one byte and five on every item moving in the factory.
pub(super) fn write(writer: &mut Writer, entities: &[EntitySnapshot], tick: u64) {
    writer.uvarint(entities.len() as u64);
    let mut previous_id = 0u32;
    for entity in entities {
        // Ascending by stable id, which the host relies on to merge in one pass; the same ordering
        // makes the id itself cost the gap rather than the value.
        writer.uvarint(u64::from(entity.id - previous_id));
        previous_id = entity.id;
        writer.svarint(i64::from(entity.q));
        writer.svarint(i64::from(entity.r));
        writer.uvarint(u64::from(entity.definition_id));
        writer.u8(kind_code(entity.kind));
        writer.u8(entity.orientation);

        let mut flags = 0u32;
        if entity.recipe_id.is_some() {
            flags |= entity_flag::RECIPE_ID;
        }
        if entity.scenario_owned {
            flags |= entity_flag::SCENARIO_OWNED;
        }
        if entity.cargo.is_some() {
            flags |= entity_flag::CARGO;
        }
        if !entity.lane.is_empty() {
            flags |= entity_flag::LANE;
        }
        if entity.fuel_charge != 0 {
            flags |= entity_flag::FUEL_CHARGE;
        }
        if entity.fuel_required != 0 {
            flags |= entity_flag::FUEL_REQUIRED;
        }
        if entity.next_id.is_some() {
            flags |= entity_flag::NEXT_ID;
        }
        if !entity.branch_ids.is_empty() {
            flags |= entity_flag::BRANCH_IDS;
        }
        if !entity.input_inventory.is_empty() {
            flags |= entity_flag::INPUT_INVENTORY;
        }
        if !entity.fuel_inventory.is_empty() {
            flags |= entity_flag::FUEL_INVENTORY;
        }
        if !entity.output_inventory.is_empty() {
            flags |= entity_flag::OUTPUT_INVENTORY;
        }
        if !entity.output_routes.is_empty() {
            flags |= entity_flag::OUTPUT_ROUTES;
        }
        if entity.power_satisfied != 0 {
            flags |= entity_flag::POWER_SATISFIED;
        }
        if entity.power_demand != 0 {
            flags |= entity_flag::POWER_DEMAND;
        }
        if entity.power_charge != 0 {
            flags |= entity_flag::POWER_CHARGE;
        }
        if entity.power_capacity != 0 {
            flags |= entity_flag::POWER_CAPACITY;
        }
        if entity.water_source.is_some() {
            flags |= entity_flag::WATER_SOURCE;
        }
        writer.uvarint(u64::from(flags));

        if let Some(recipe_id) = entity.recipe_id {
            writer.uvarint(u64::from(recipe_id));
        }
        if let Some(cargo) = entity.cargo {
            writer.uvarint(u64::from(cargo.item_id));
            writer.uvarint(u64::from(cargo.quantity));
        }
        if flags & entity_flag::LANE != 0 {
            writer.uvarint(entity.lane.len() as u64);
            for item in &entity.lane {
                writer.uvarint(u64::from(item.cargo.item_id));
                writer.uvarint(u64::from(item.cargo.quantity));
                writer.uvarint(tick.saturating_sub(item.entered));
            }
        }
        writer.ingredients(&entity.inventory);
        if flags & entity_flag::INPUT_INVENTORY != 0 {
            writer.ingredients(&entity.input_inventory);
        }
        if flags & entity_flag::FUEL_INVENTORY != 0 {
            writer.ingredients(&entity.fuel_inventory);
        }
        if flags & entity_flag::OUTPUT_INVENTORY != 0 {
            writer.ingredients(&entity.output_inventory);
        }
        if flags & entity_flag::OUTPUT_ROUTES != 0 {
            writer.uvarint(entity.output_routes.len() as u64);
            for route in &entity.output_routes {
                writer.uvarint(u64::from(route.item_id));
                writer.svarint(i64::from(route.q - entity.q));
                writer.svarint(i64::from(route.r - entity.r));
                writer.u8(route.direction);
                writer.uvarint(u64::from(route.target_id.unwrap_or(0)));
            }
        }
        writer.uvarint(u64::from(entity.progress));
        writer.uvarint(u64::from(entity.progress_total));
        if entity.fuel_charge != 0 {
            writer.uvarint(u64::from(entity.fuel_charge));
        }
        if entity.fuel_required != 0 {
            writer.uvarint(u64::from(entity.fuel_required));
        }
        writer.u8(status_code(entity.status));
        if let Some(next_id) = entity.next_id {
            writer.uvarint(u64::from(next_id));
        }
        if !entity.branch_ids.is_empty() {
            writer.uvarint(entity.branch_ids.len() as u64);
            for branch_id in &entity.branch_ids {
                writer.uvarint(u64::from(*branch_id));
            }
        }
        if entity.power_satisfied != 0 {
            writer.uvarint(u64::from(entity.power_satisfied));
        }
        if entity.power_demand != 0 {
            writer.uvarint(u64::from(entity.power_demand));
        }
        if entity.power_charge != 0 {
            writer.uvarint(u64::from(entity.power_charge));
        }
        if entity.power_capacity != 0 {
            writer.uvarint(u64::from(entity.power_capacity));
        }
        if let Some(source) = entity.water_source {
            writer.svarint(i64::from(source.q - entity.q));
            writer.svarint(i64::from(source.r - entity.r));
            writer.uvarint(u64::from(source.available));
            writer.u8(source.discharge);
            writer.uvarint(u64::from(source.rate));
        }
        // Against the entity's own hex, so the single-cell footprint every belt and machine has
        // costs two bytes rather than two full coordinates.
        writer.uvarint(entity.footprint.len() as u64);
        for cell in &entity.footprint {
            writer.svarint(i64::from(cell.q) - i64::from(entity.q));
            writer.svarint(i64::from(cell.r) - i64::from(entity.r));
        }
    }
}
