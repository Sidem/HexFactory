use super::*;

/// What the opening promises, asserted rather than assumed.
///
/// The eight hardcoded clearing cells are gone, so the guarantee is now something the
/// generator has to *find*: a patch of each material, in its window, big enough to stand an
/// extractor in. Every preset generates the field materials somewhere in the sample, and the
/// seven guaranteed ones land where they were promised — which is what makes the first hour
/// playable rather than just survivable.
///
/// Sand and crystal are deliberately not guaranteed. Sand goes where the ocean gate says a
/// coast is, and crystal is the reason to leave.
#[test]
fn every_preset_opens_a_workable_world_on_any_seed() {
    let (definitions, _, _) = catalogs();
    for preset in world_presets() {
        let params = preset.params.clone();
        params
            .validate(&definitions)
            .unwrap_or_else(|error| panic!("preset {} is invalid: {error}", preset.key));
        let report = survey::run(
            preset.key,
            &params,
            survey::default_seed(),
            survey::DEFAULT_RADIUS,
        );
        assert!(
            (10..=60).contains(&report.fertile_riverbank.per_mille_land),
            "preset {}: fertile riverbank is {} per mille of land",
            preset.key,
            report.fertile_riverbank.per_mille_land
        );
        assert!(
            report
                .fertile_riverbank
                .nearest
                .is_some_and(|distance| distance <= 40),
            "preset {}: nearest fertile riverbank is {:?}",
            preset.key,
            report.fertile_riverbank.nearest
        );
        assert!(
            report.fertile_riverbank.capacity > 0,
            "preset {} has no fertile-riverbank capacity",
            preset.key
        );
        for material in &report.materials {
            let nearest = match material.nearest {
                Some(value) => value,
                None if (material.item_id == SAND || material.item_id == CRYSTAL)
                    && report.radius < survey::landscape_radius(params.elevation_coarse_cell) =>
                {
                    // Sand sits on the regional ocean; crystal is the reason to leave. A
                    // 96-hex opening sample of a 512-hex landform often never reaches either,
                    // and that is the world working.
                    continue;
                }
                None => panic!(
                    "preset {} generates no {} anywhere in a {}-hex sample",
                    preset.key, material.name, report.hexes
                ),
            };
            let ceiling = if material.item_id == CRYSTAL || material.item_id == SAND {
                survey::DEFAULT_RADIUS as u32
            } else {
                40 + BOOTSTRAP_WIDEN_CAP as u32
            };
            assert!(
                nearest <= ceiling,
                "preset {}: nearest {} is {nearest} hexes from the landing site",
                preset.key,
                material.name
            );
        }
        for (row, &(item_id, _, ceiling)) in report.bootstrap.iter().zip(&BOOTSTRAP_GUARANTEES) {
            assert_eq!(row.item_id, item_id);
            let walk = row.edge.unwrap_or_else(|| {
                panic!(
                    "preset {} cannot place its guaranteed {}",
                    preset.key, row.name
                )
            });
            // The ceiling is the window's, plus whatever widening the seed needed. The floor
            // is what keeps a guaranteed disc out of the clearing and is never widened.
            assert!(
                walk > LANDING_CLEAR_RADIUS as u32
                    && walk <= (ceiling + BOOTSTRAP_WIDEN_CAP) as u32,
                "preset {}: guaranteed {} is {walk} hexes out",
                preset.key,
                row.name
            );
            assert!(
                row.hexes >= WORKABLE_PATCH_HEXES,
                "preset {}: guaranteed {} is {} hexes, which no extractor can fill from",
                preset.key,
                row.name,
                row.hexes
            );
        }
        // Barren ground stays the common case under every preset, or a site is stumbled over
        // rather than chosen. This is the v0.15 density floor and ceiling, per preset.
        let fields: u32 = report.materials.iter().map(|entry| entry.cells).sum();
        assert!(
            fields * 100 < report.land_hexes * 22 && fields * 100 > report.land_hexes * 3,
            "preset {}: {fields} fields on {} land hexes",
            preset.key,
            report.land_hexes
        );
    }

    // The patch fill is a second pass over the same cells the material counts walked, and every
    // mean, the purity share, and the workable-patch distance are all divided out of its totals.
    // A fill that lost a hex, followed a neighbour of another material, or visited one twice would
    // move all of them at once and none of them visibly, so the accounting is asserted directly
    // rather than inferred from a figure looking plausible.
    //
    // This is the measurement Landforms and Fields v0.21 is tuned against. It has to be trusted
    // before the generator moves, which is why it lands in the same commit as the before figures
    // and ahead of any generation rule.
    let seed = survey::default_seed();
    for preset in world_presets() {
        let report = survey::run(preset.key, &preset.params, seed, 48);
        let mut counted = 0u32;
        let mut pure = 0u32;
        for (material, patch) in report.materials.iter().zip(&report.patches) {
            assert_eq!(
                material.item_id, patch.item_id,
                "preset {}: the two material tables are in different orders",
                preset.key
            );
            assert_eq!(
                patch.hexes, material.cells,
                "preset {}: the {} fill visited {} hexes against {} counted cells",
                preset.key, material.name, patch.hexes, material.cells
            );
            assert_eq!(
                patch.patches == 0,
                patch.hexes == 0,
                "preset {}: {} has {} patches over {} hexes",
                preset.key,
                material.name,
                patch.patches,
                patch.hexes
            );
            assert!(
                patch.largest_patch <= patch.hexes && patch.truncated_patches <= patch.patches,
                "preset {}: {} reports a largest patch of {} and {} truncated of {} over {} \
                     hexes",
                preset.key,
                material.name,
                patch.largest_patch,
                patch.truncated_patches,
                patch.patches,
                patch.hexes
            );
            // A workable patch is at least seven hexes, so claiming one means the largest
            // patch is at least that big, and no patch can start nearer than the nearest cell.
            match patch.nearest_workable_patch {
                Some(distance) => {
                    assert!(
                        patch.largest_patch >= 7,
                        "preset {}: {} claims a workable patch with a largest patch of {}",
                        preset.key,
                        material.name,
                        patch.largest_patch
                    );
                    assert!(
                        distance >= material.nearest.expect("a patch implies a cell"),
                        "preset {}: {} puts a workable patch at {distance}, nearer than its \
                             nearest cell",
                        preset.key,
                        material.name
                    );
                }
                None => assert!(
                    patch.largest_patch < 7,
                    "preset {}: {} has a {}-hex patch and reports none workable",
                    preset.key,
                    material.name,
                    patch.largest_patch
                ),
            }
            counted += patch.hexes;
            pure += patch.purity_per_mille * patch.hexes / 1000;
        }
        assert!(
            counted > 0,
            "preset {} generates nothing at all",
            preset.key
        );
        // The whole-sample purity is the same count divided by the same denominator, so it has
        // to agree with the per-material shares to within their rounding.
        let overall = report.purity_per_mille * counted / 1000;
        assert!(
            overall.abs_diff(pure) <= report.patches.len() as u32,
            "preset {}: whole-sample purity implies {overall} pure hexes against {pure} from \
                 the material rows",
            preset.key
        );
    }

    // **The number this milestone exists for.**
    //
    // A deposit used to be decided per hex from independent noise channels, so along every
    // iron/coal boundary the two alternated hex by hex and an extractor covered both and cleanly
    // worked neither. Purity is the share of resource hexes whose radius-1 disc holds exactly one
    // material, and the measured before figures were `continental` 532, `archipelago` 474,
    // `highlands` 662, `basin` 631 — every preset failing, the wettest failing hardest.
    //
    // It is asserted at 950 rather than at whatever the presets happen to reach, because the
    // point is the model and not the tuning: a rule table that could not clear this bar would
    // mean the lattice had stopped being the thing that decides what a patch is made of.
    let seed = survey::default_seed();
    for preset in world_presets() {
        let report = survey::run(preset.key, &preset.params, seed, survey::DEFAULT_RADIUS);
        assert!(
            report.purity_per_mille >= 950,
            "preset {}: purity is {} per mille",
            preset.key,
            report.purity_per_mille
        );
        // A patch worth automating, per material an extractor is stood on for its own sake.
        // Forests are the one that is measured in area rather than in throughput, so their
        // bar is the deep extractor's disc rather than the base one's.
        for (item_id, floor) in [
            (IRON_ORE, 19),
            (COAL, 19),
            (COPPER_ORE, 19),
            (STONE, 19),
            (WOOD, 61),
        ] {
            let patch = report
                .patches
                .iter()
                .find(|entry| entry.item_id == item_id)
                .expect("every generated item has a row");
            assert!(
                patch.largest_patch >= floor,
                "preset {}: the largest {} patch is {} hexes",
                preset.key,
                patch.name,
                patch.largest_patch
            );
        }
    }

    // The opening is a promise about every seed, not about the shipped one.
    //
    // A guarantee that only holds on the seed it was tuned against is not a guarantee, and the
    // bootstrap pass is the one part of generation that can fail outright — it widens a window in
    // fixed steps and then gives up, and `Core::new` refuses a world it gave up on. So the claim
    // is checked where it would break: every preset, ten seeds, including the presets whose bands
    // are scarce enough to make a window hard to fill.
    let (definitions, technologies, scenarios) = catalogs();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|value| value.key == "new-game")
        .unwrap();
    for preset in world_presets() {
        for step in 0..10u32 {
            let seed = survey::default_seed().wrapping_add(step.wrapping_mul(0x9E3779B1));
            let spine = GroundSpine::physical(&preset.params, seed, true);
            let fields = WorldFields::new(&preset.params, seed, &spine);
            assert!(
                fields.unmet.is_empty(),
                "preset {} on seed {seed} cannot place {:?}",
                preset.key,
                fields.unmet
            );
            let placed: BTreeMap<ItemId, (u32, u32)> = fields
                .guarantees(&spine)
                .into_iter()
                .map(|(item_id, walk, hexes)| (item_id, (walk, hexes)))
                .collect();
            for &(item_id, floor, _) in &BOOTSTRAP_GUARANTEES {
                let (walk, hexes) = placed[&item_id];
                // The floor is never widened: a guaranteed disc that reached inside the
                // clearing would put a deposit where field suppression deletes it.
                assert!(
                    walk >= floor as u32,
                    "preset {} on seed {seed}: item {item_id} is {walk} hexes out, inside its \
                         floor of {floor}",
                    preset.key
                );
                assert!(
                    hexes >= WORKABLE_PATCH_HEXES,
                    "preset {} on seed {seed}: item {item_id} is {hexes} hexes",
                    preset.key
                );
            }
            // Crystal is the reason to leave, so nothing may guarantee it.
            assert!(!placed.contains_key(&CRYSTAL));
            Core::new(
                &definitions,
                &technologies,
                scenario,
                Some(seed),
                Some(preset.params.clone()),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "preset {} on seed {seed} is unplayable: {error}",
                    preset.key
                )
            });
        }
    }

    // A large landform must not strand the player on the 7-hex clearing. The landing disc fades
    // toward the opening blend and lifts a sea-spawn origin, so the first two dozen hexes stay
    // mostly walkable on every seed of every preset.
    for preset in world_presets() {
        for step in 0..10u32 {
            let seed = survey::default_seed().wrapping_add(step.wrapping_mul(0x9E3779B1));
            let mut blocked = 0u32;
            let mut hexes = 0u32;
            for (q, r) in hexes_in_radius((0, 0), 24) {
                if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
                    continue;
                }
                hexes += 1;
                if terrain_at(&preset.params, seed, q, r, true).blocks_movement() {
                    blocked += 1;
                }
            }
            assert!(
                blocked * 100 < hexes * 40,
                "preset {} on seed {seed}: {blocked} of {hexes} hexes in the first 24 are \
                     impassable",
                preset.key
            );
        }
    }
}

/// A world's identity is its seed *and* its parameters, so a scalar the checksum does not read
/// is a scalar two different worlds can silently share. Every one of them is moved, one at a
/// time, and the hash has to move with it.
#[test]
fn world_parameters_are_checksummed_validated_and_restored_with_their_sites() {
    let base = preset_params("continental").unwrap();
    let hash_of = |params: &WorldParams| {
        let mut hash = 0x811c9dc5u32;
        hash_world_params(&mut hash, params);
        hash
    };
    let baseline = hash_of(&base);
    let mut moved: Vec<WorldParams> = Vec::new();
    for shift in [
        |p: &mut WorldParams| p.elevation_coarse_cell += 1,
        |p: &mut WorldParams| p.elevation_fine_cell += 1,
        |p: &mut WorldParams| p.elevation_coarse_weight += 1,
        |p: &mut WorldParams| p.moisture_cell += 1,
        |p: &mut WorldParams| p.richness_cell += 1,
        |p: &mut WorldParams| p.water_level += 1,
        |p: &mut WorldParams| p.shore_level += 1,
        |p: &mut WorldParams| p.hills_level += 1,
        |p: &mut WorldParams| p.highland_level += 1,
        |p: &mut WorldParams| p.cliff_step += 1,
        |p: &mut WorldParams| p.deep_water_moisture += 1,
        |p: &mut WorldParams| p.site_cell += 1,
        |p: &mut WorldParams| p.site_jitter += 1,
        |p: &mut WorldParams| p.river_cell += 1,
        |p: &mut WorldParams| p.river_width += 1,
        |p: &mut WorldParams| p.river_max_elevation += 1,
        |p: &mut WorldParams| p.ocean_level += 1,
        |p: &mut WorldParams| p.site_rules[0].weight += 1,
        |p: &mut WorldParams| p.site_rules[0].radius_min += 1,
        |p: &mut WorldParams| p.site_rules[0].radius_max += 1,
        |p: &mut WorldParams| p.site_rules[0].site_min += 1,
        |p: &mut WorldParams| p.site_rules[0].yield_core += 1,
        |p: &mut WorldParams| p.site_rules[0].yield_rim += 1,
        |p: &mut WorldParams| p.site_rules[0].yield_jitter += 1,
        |p: &mut WorldParams| p.site_rules[0].member_water_within += 1,
        |p: &mut WorldParams| p.site_rules[0].center_ocean = true,
        |p: &mut WorldParams| p.site_rules[0].center_shore = true,
        |p: &mut WorldParams| p.site_rules[0].member.push(Terrain::Cliff),
        |p: &mut WorldParams| p.site_rules[0].item_id = CRYSTAL,
        |p: &mut WorldParams| p.site_rules[0].terrain = Terrain::Shore,
    ] {
        let mut params = base.clone();
        shift(&mut params);
        assert_ne!(
            hash_of(&params),
            baseline,
            "a world parameter changed and the checksum did not"
        );
        moved.push(params);
    }
    // And no two of them collide, which is the failure a per-field test on its own cannot see.
    let mut hashes: Vec<u32> = moved.iter().map(hash_of).collect();
    let total = hashes.len();
    hashes.sort_unstable();
    hashes.dedup();
    assert_eq!(hashes.len(), total, "two parameter changes hash the same");

    // A site's yield falls from its core to its rim, which is what makes the middle of a field
    // worth aiming an extractor at rather than any hex of it being as good as any other.
    let params = preset_params("continental").unwrap();
    let seed = survey::default_seed();
    let spine = GroundSpine::physical(&params, seed, true);
    let fields = WorldFields::new(&params, seed, &spine);
    let mut compared = 0u32;
    let mut core_wins = 0u32;
    for cell in (-8..8).flat_map(|q| (-8..8).map(move |r| (q, r))) {
        let Some(site) = fields.site_at(cell, &spine) else {
            continue;
        };
        let rule = &params.site_rules[site.rule];
        if rule.yield_core == rule.yield_rim || site.radius < 2 {
            continue;
        }
        let Some(center) = fields.field_at(site.center.0, site.center.1, true, &spine) else {
            continue;
        };
        for rim in hexes_in_radius(site.center, site.radius)
            .into_iter()
            .filter(|&cell| axial_distance(site.center, cell) == site.radius)
        {
            let Some(edge) = fields.field_at(rim.0, rim.1, true, &spine) else {
                continue;
            };
            if edge.item_id != center.item_id {
                continue;
            }
            compared += 1;
            core_wins += u32::from(center.quantity > edge.quantity);
        }
    }
    assert!(compared > 20, "only {compared} core/rim pairs to compare");
    // Jitter is deliberately allowed to invert a single pair; a gradient it could hide would
    // be a gradient no player could read.
    assert!(
        core_wins * 100 > compared * 85,
        "the core beat the rim in only {core_wins} of {compared} pairs"
    );

    // A parameter set that is not a world at all is refused before one is built from it. What this
    // deliberately does not try to catch is a set that is a world but an unplayable one — that is
    // what the survey measures, and no validator can decide it.
    let (definitions, technologies, scenarios) = catalogs();
    let base = preset_params("continental").unwrap();
    // One valid row, so each case below differs from a world by exactly the thing it names.
    let one_rule = || SiteRule {
        terrain: Terrain::Hills,
        item_id: IRON_ORE,
        weight: 1,
        radius_min: 1,
        radius_max: 2,
        site_min: ANY,
        yield_core: 4,
        yield_rim: 2,
        yield_jitter: 1,
        member: Vec::new(),
        member_water_within: 0,
        center_ocean: false,
        center_shore: false,
    };
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|value| value.key == "new-game")
        .unwrap();
    let refused = [
        WorldParams {
            elevation_coarse_cell: 0,
            ..base.clone()
        },
        WorldParams {
            elevation_coarse_weight: 140,
            ..base.clone()
        },
        // Bands out of order do not make a band rare; they make it unreachable.
        WorldParams {
            hills_level: 10_000,
            ..base.clone()
        },
        WorldParams {
            site_rules: Vec::new(),
            ..base.clone()
        },
        WorldParams {
            site_rules: vec![SiteRule {
                item_id: 9999,
                ..one_rule()
            }],
            ..base.clone()
        },
        // Yield is `interpolated + hash % yield_jitter`, so a zero jitter is a division by zero.
        WorldParams {
            site_rules: vec![SiteRule {
                yield_jitter: 0,
                ..one_rule()
            }],
            ..base.clone()
        },
        // A radius of zero is a deposit that is not anywhere, and an inverted range would make
        // `radius_max - radius_min + 1` wrap.
        WorldParams {
            site_rules: vec![SiteRule {
                radius_min: 4,
                radius_max: 2,
                ..one_rule()
            }],
            ..base.clone()
        },
        WorldParams {
            site_rules: vec![SiteRule {
                radius_max: MAX_SITE_RADIUS + 1,
                ..one_rule()
            }],
            ..base.clone()
        },
        // A water band would make the cheap water test `field_at` opens with unsound, and a
        // deposit in a basin is nothing a pump or an extractor could reach anyway.
        WorldParams {
            site_rules: vec![SiteRule {
                member: vec![Terrain::Hills, Terrain::DeepWater],
                ..one_rule()
            }],
            ..base.clone()
        },
        // Every row weighted zero is a table that generates nothing at all.
        WorldParams {
            site_rules: vec![SiteRule {
                weight: 0,
                ..one_rule()
            }],
            ..base.clone()
        },
        WorldParams {
            site_jitter: MAX_SITE_JITTER + 1,
            ..base.clone()
        },
    ];
    for params in refused {
        assert!(
            Core::new(&definitions, &technologies, scenario, None, Some(params)).is_err(),
            "a parameter set that is not a world must be refused"
        );
    }
    assert!(preset_params("no-such-preset").is_none());

    // A world's parameters survive the round trip, and the world that comes back is the one that
    // was saved rather than the scenario's default.
    let (definitions, technologies, scenarios) = catalogs();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|value| value.key == "new-game")
        .unwrap();
    let basin = preset_params("basin").unwrap();
    let mut core = Core::new(
        &definitions,
        &technologies,
        scenario,
        None,
        Some(basin.clone()),
    )
    .unwrap();
    assert_ne!(core.world_params, default_world_params());
    core.tick_many(30);
    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.world_params, basin);
    assert_eq!(restored.checksum(), core.checksum());
    // The default-parameter core is the same scenario, the same seed, and a different world.
    let default = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
    assert_eq!(default.seed, core.seed);
    assert_ne!(
        default.checksum(),
        Core::new(&definitions, &technologies, scenario, None, Some(basin),)
            .unwrap()
            .checksum()
    );
}
