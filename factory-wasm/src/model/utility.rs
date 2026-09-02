fn inventory_total(inventory: &BTreeMap<ItemId, u32>) -> u32 {
    inventory.values().sum()
}

fn has_ingredients(inventory: &BTreeMap<ItemId, u32>, ingredients: &[Ingredient]) -> bool {
    ingredients.iter().all(|ingredient| {
        inventory.get(&ingredient.item_id).copied().unwrap_or(0) >= ingredient.quantity
    })
}

fn subtract_item(inventory: &mut BTreeMap<ItemId, u32>, item_id: ItemId, quantity: u32) {
    let stored = inventory
        .get_mut(&item_id)
        .expect("validated inventory item exists");
    *stored -= quantity;
    if *stored == 0 {
        inventory.remove(&item_id);
    }
}

fn add_ingredients(inventory: &mut BTreeMap<ItemId, u32>, ingredients: &[Ingredient]) {
    for ingredient in ingredients {
        *inventory.entry(ingredient.item_id).or_default() += ingredient.quantity;
    }
}

fn add_inventory(target: &mut BTreeMap<ItemId, u32>, source: &BTreeMap<ItemId, u32>) {
    for (&item, &quantity) in source {
        *target.entry(item).or_default() += quantity;
    }
}

fn expand_components(affected: &mut BTreeSet<u32>, adjacency: &BTreeMap<u32, BTreeSet<u32>>) {
    let mut pending: Vec<u32> = affected.iter().copied().collect();
    while let Some(id) = pending.pop() {
        let Some(neighbors) = adjacency.get(&id) else {
            continue;
        };
        for &neighbor in neighbors {
            if affected.insert(neighbor) {
                pending.push(neighbor);
            }
        }
    }
}

fn hash_inventory(hash: &mut u32, inventory: &BTreeMap<ItemId, u32>) {
    for (&item, &quantity) in inventory {
        hash_u32(hash, u32::from(item));
        hash_u32(hash, quantity);
    }
    hash_u32(hash, u32::MAX);
}

/// Every field of a parameter set, in declared order. A world's identity is its seed *and* its
/// parameters, so a checksum that hashed only the seed would call two different worlds the same
/// one — including the rule table, whose row order is itself a generation input.
fn hash_world_params(hash: &mut u32, params: &WorldParams) {
    for value in [
        params.elevation_coarse_cell,
        params.elevation_fine_cell,
        params.elevation_coarse_weight,
        params.moisture_cell,
        params.richness_cell,
        params.water_level,
        params.shore_level,
        params.hills_level,
        params.highland_level,
        params.cliff_step,
        params.deep_water_moisture,
        params.site_cell,
        params.site_jitter,
        params.river_cell,
        params.river_width,
        params.river_max_elevation,
        params.ocean_level,
    ] {
        hash_i32(hash, value);
    }
    for rule in &params.site_rules {
        hash_u32(hash, rule.terrain as u32);
        hash_u32(hash, u32::from(rule.item_id));
        hash_u32(hash, rule.weight);
        hash_u32(hash, rule.radius_min);
        hash_u32(hash, rule.radius_max);
        hash_i32(hash, rule.site_min);
        hash_u32(hash, rule.yield_core);
        hash_u32(hash, rule.yield_rim);
        hash_u32(hash, rule.yield_jitter);
        for &band in &rule.member {
            hash_u32(hash, band as u32);
        }
        hash_u32(hash, u32::MAX);
        hash_u32(hash, rule.member_water_within);
        hash_u32(hash, u32::from(rule.center_ocean));
        hash_u32(hash, u32::from(rule.center_shore));
    }
    hash_u32(hash, u32::MAX);
}

fn hash_bytes(hash: &mut u32, bytes: &[u8]) {
    for &byte in bytes {
        *hash ^= u32::from(byte);
        *hash = hash.wrapping_mul(0x01000193);
    }
}

fn hash_u32(hash: &mut u32, value: u32) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_i32(hash: &mut u32, value: i32) {
    hash_u32(hash, value as u32);
}

fn hash_u64(hash: &mut u32, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}
