//! What the shipped world actually hands the player, measured on the default preset.
//!
//! Every other section of the report reads the definitions alone. These rows read the generator,
//! which makes them the slow half and the half that answers a different question: not "does the
//! recipe tree bottom out in this material" but "can a player standing in the default world reach
//! any of it". They are grouped here so that distinction is visible in the file listing.

use super::*;

/// Which raw materials the economy bottoms out in, and whether the default world hands them over.
pub(super) fn access(economy: &Economy) -> Vec<MaterialAccess> {
    let params = preset_params(DEFAULT_PRESET_KEY).expect("the default preset is in the table");
    let seed = survey::default_seed();

    // Everything the recipe tree and the construction rows actually reach the ground through.
    let mut required: BTreeMap<ItemId, u32> = BTreeMap::new();
    let mut note = |economy: &Economy, ingredients: &[Ingredient]| {
        for (item_id, _) in economy.cost_of(ingredients).raw {
            *required.entry(item_id).or_insert(0) += 1;
        }
    };
    for recipe in &economy.definitions.recipes {
        note(economy, &recipe.inputs);
    }
    for building in &economy.definitions.buildings {
        if building.buildable {
            note(economy, &building.construction_cost);
        }
    }
    // Water is raw and is nobody's field: a pump makes it out of terrain. It reaches the tree
    // through concrete, and a boiler drinks it directly.
    required.entry(WATER_ITEM).or_insert(0);
    // Wildlife is raw and is nobody's field either, and unlike water it is not even in one place:
    // a carcass comes off an animal that walks, and cut feed comes off pasture that regrows. Asking
    // the generator for the nearest hex of either would measure scenery — the question this survey
    // exists to separate from "the world holds some" — so the fauna items leave the field survey
    // rather than answer it wrongly. Whether the herds themselves are reachable is a question about
    // pasture and water, which `Core::seed_herds` sites them by.
    for species in &economy.definitions.species {
        required.remove(&species.carcass_item);
        required.remove(&species.feed_item);
    }

    let spine = GroundSpine::physical(&params, seed, true);
    let fields = WorldFields::new(&params, seed, &spine);
    let guaranteed = guaranteed_patches(&fields, &spine);
    let mut rows = Vec::new();
    for (&item_id, &required_by) in &required {
        let promise = guaranteed.get(&item_id).copied();
        let (nearest, reachable) = if item_id == WATER_ITEM {
            nearest_water(&params, seed)
        } else {
            nearest_field(&fields, item_id, &spine)
        };
        rows.push(MaterialAccess {
            material: economy.item_key(item_id),
            guaranteed_walk: promise.map(|(walk, _)| walk),
            guaranteed_hexes: promise.map(|(_, hexes)| hexes).unwrap_or(0),
            nearest_generated: nearest,
            // A guaranteed patch is placed by the generator and clipped by the same member test as
            // every other, so unlike the old clearing cells it is reachable only if geography says
            // so — which is why the bootstrap pass measures the patch before it claims a cell.
            reachable,
            required_by,
        });
    }
    rows
}

/// Something can stand here, and what it stands on is not a cliff or a basin.
fn standable(params: &WorldParams, seed: u32, cell: (i32, i32), spine: &GroundSpine) -> bool {
    let _ = (params, seed);
    !spine.presentation_at(cell.0, cell.1).blocks_movement()
}

/// What the bootstrap pass promised, per material: the walk to the nearest hex of the guaranteed
/// patch, and how many hexes that patch holds.
fn guaranteed_patches(fields: &WorldFields, spine: &GroundSpine) -> BTreeMap<ItemId, (u32, u32)> {
    fields
        .guarantees(spine)
        .into_iter()
        .map(|(item_id, walk, hexes)| (item_id, (walk, hexes)))
        .collect()
}

fn nearest_field(
    fields: &WorldFields,
    item_id: ItemId,
    spine: &GroundSpine,
) -> (Option<u32>, bool) {
    let params = &fields.params;
    let seed = fields.seed;
    let mut nearest = None;
    let mut reachable = false;
    for cell in hexes_in_radius((0, 0), YIELD_RADIUS) {
        if axial_distance((0, 0), cell) <= LANDING_CLEAR_RADIUS {
            continue;
        }
        let Some(field) = fields.field_at(cell.0, cell.1, true, spine) else {
            continue;
        };
        if field.item_id != item_id {
            continue;
        }
        let distance = axial_distance((0, 0), cell) as u32;
        nearest = Some(nearest.map_or(distance, |value: u32| value.min(distance)));
        // Stone sits on cliffs. What makes it a material rather than scenery is that a hex beside
        // the cliff covers it, at the one reach every extractor and the player's own hand share.
        reachable = reachable
            || hexes_in_radius(cell, EXTRACT_RADIUS)
                .into_iter()
                .any(|neighbour| standable(params, seed, neighbour, spine));
    }
    (nearest, reachable)
}

fn nearest_water(params: &WorldParams, seed: u32) -> (Option<u32>, bool) {
    let spine = GroundSpine::physical(params, seed, true);
    let mut nearest = None;
    let mut reachable = false;
    for cell in hexes_in_radius((0, 0), YIELD_RADIUS) {
        if !spine.wet_at(cell.0, cell.1) {
            continue;
        }
        let distance = axial_distance((0, 0), cell) as u32;
        nearest = Some(nearest.map_or(distance, |value: u32| value.min(distance)));
        // A pump stands beside a basin, never in it.
        reachable = reachable
            || hexes_in_radius(cell, PUMP_RADIUS)
                .into_iter()
                .any(|neighbour| standable(params, seed, neighbour, &spine));
    }
    (nearest, reachable)
}

/// What a site is worth, per preset, measured on generated geography.
///
/// The landing clearing is excluded for the same reason the survey excludes it: it is a promise
/// rather than a landscape, and it is reported on its own in `reference`.
pub(super) fn extraction(economy: &Economy) -> Vec<SiteYield> {
    let seed = survey::default_seed();
    let mut rows = Vec::new();
    for preset in world_presets() {
        let spine = GroundSpine::physical(&preset.params, seed, true);
        let world = WorldFields::new(&preset.params, seed, &spine);
        let mut fields: BTreeMap<(i32, i32), ResourceState> = BTreeMap::new();
        for cell in hexes_in_radius((0, 0), YIELD_RADIUS) {
            if axial_distance((0, 0), cell) <= LANDING_CLEAR_RADIUS {
                continue;
            }
            if let Some(field) = world.field_at(cell.0, cell.1, true, &spine) {
                fields.insert(cell, field);
            }
        }
        // sites, total cell quantity, then per reach: total yield and same-material yield.
        let mut totals: BTreeMap<ItemId, (u64, u64, Vec<u64>, Vec<u64>)> = BTreeMap::new();
        for (&cell, field) in &fields {
            let entry = totals.entry(field.item_id).or_insert_with(|| {
                (
                    0,
                    0,
                    vec![0; YIELD_REACHES.len()],
                    vec![0; YIELD_REACHES.len()],
                )
            });
            entry.0 += 1;
            entry.1 += u64::from(field.quantity);
            for (slot, &reach) in YIELD_REACHES.iter().enumerate() {
                for covered in hexes_in_radius(cell, reach) {
                    let Some(other) = fields.get(&covered) else {
                        continue;
                    };
                    entry.2[slot] += u64::from(other.quantity);
                    if other.item_id == field.item_id {
                        entry.3[slot] += u64::from(other.quantity);
                    }
                }
            }
        }
        for (item_id, (sites, quantity, yields, same)) in totals {
            let mean = |total: u64| (total / sites.max(1)) as u32;
            rows.push(SiteYield {
                preset: preset.key.to_string(),
                material: economy.item_key(item_id),
                sites: sites as u32,
                mean_cell_quantity: mean(quantity),
                mean_site_yield: yields.into_iter().map(mean).collect(),
                mean_same_material: same.into_iter().map(mean).collect(),
            });
        }
    }
    rows
}
