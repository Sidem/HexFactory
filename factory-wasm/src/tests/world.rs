use super::*;

#[test]
fn native_and_host_agree_on_directions_passability_heights_and_hexes() {
    let fixture: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../../../fixtures/hex-directions.json")).unwrap();
    let actual: Vec<(i32, i32)> = fixture
        .iter()
        .map(|entry| {
            (
                entry["q"].as_i64().unwrap() as i32,
                entry["r"].as_i64().unwrap() as i32,
            )
        })
        .collect();
    assert_eq!(actual, TRANSPORT_DIRECTIONS);

    // Which bands the player cannot stand on is native's rule, and since v0.12.3 the renderer
    // draws that category before it draws the material — so the host holds a copy of the rule and
    // a copy is a thing that drifts. This is the `fixtures/hex-directions.json` idiom applied to
    // it: Rust asserts the file against the predicates, `tests/host.test.ts` asserts it against
    // `src/core/terrain.ts`, and neither side may move without the other.
    #[derive(Deserialize)]
    struct PassabilityEntry {
        terrain: Terrain,
        passable: bool,
        buildable: bool,
    }
    #[derive(Deserialize)]
    struct PhysicalEntry {
        substrate: String,
        slope: i32,
        water_depth: i32,
        passable: bool,
        buildable: bool,
    }
    #[derive(Deserialize)]
    struct PassabilityFixture {
        bands: Vec<PassabilityEntry>,
        physical: Vec<PhysicalEntry>,
    }

    const BANDS: [Terrain; 7] = [
        Terrain::DeepWater,
        Terrain::ShallowWater,
        Terrain::Shore,
        Terrain::Lowland,
        Terrain::Hills,
        Terrain::Highland,
        Terrain::Cliff,
    ];
    // A band added to the enum makes this match non-exhaustive, which is what sends whoever
    // added it to `BANDS` above and to the fixture beside it.
    for band in BANDS {
        match band {
            Terrain::DeepWater
            | Terrain::ShallowWater
            | Terrain::Shore
            | Terrain::Lowland
            | Terrain::Hills
            | Terrain::Highland
            | Terrain::Cliff => {}
        }
    }

    let fixture: PassabilityFixture =
        serde_json::from_str(include_str!("../../../fixtures/terrain-passability.json")).unwrap();
    assert_eq!(
        fixture.bands.len(),
        BANDS.len(),
        "a band has no fixture entry"
    );
    for (index, (entry, band)) in fixture.bands.iter().zip(BANDS).enumerate() {
        assert_eq!(entry.terrain, band, "fixture is in declaration order");
        // `world_preview_bytes` sends a band as its position in this list and nothing else, so
        // the row a host reads a preview byte through is pinned to the cast that wrote it.
        assert_eq!(band as u8, index as u8, "{band:?} moved in the declaration");
        assert_eq!(
            entry.passable,
            !band.blocks_movement(),
            "{band:?} passability disagrees with the fixture"
        );
        assert_eq!(
            entry.buildable,
            !band.blocks_construction(),
            "{band:?} buildability disagrees with the fixture"
        );
        let finished = FinishedGround {
            generated: GeneratedGround::from_legacy_band(band),
            earthwork: GroundDelta::default(),
            erosion: GroundDelta::default(),
            surface: 0,
        };
        assert_eq!(
            entry.passable,
            !finished.blocks_movement(),
            "{band:?} changed while passing through the ground spine"
        );
        assert_eq!(
            entry.buildable,
            !finished.blocks_construction(),
            "{band:?} changed while passing through the ground spine"
        );
    }
    assert!(
        fixture.physical.iter().any(|entry| entry.water_depth > 0)
            && fixture.physical.iter().any(|entry| entry.slope > 0),
        "physical cases must exercise both halves of access"
    );
    for entry in fixture.physical {
        // Substrate is deliberately present even though it does not block today. Adding a
        // resistance rule later must move this exhaustive match and both fixture readers.
        match entry.substrate.as_str() {
            "soil" | "sand" | "meadow" | "rock" => {}
            other => panic!("unknown substrate {other}"),
        }
        assert_eq!(
            entry.passable,
            entry.slope <= scale::MAX_WALK_STEP_QUANTA
                && entry.water_depth < scale::WADE_LIMIT_QUANTA
        );
        assert_eq!(
            entry.buildable,
            entry.slope <= scale::MAX_BUILD_STEP_QUANTA && entry.water_depth == 0
        );
    }

    // The wire's `height` is an integer in whatever unit the active ground source counts in, and
    // the renderer has to turn it into a scene height. That conversion is a copy of a native fact,
    // and a copy is a thing that drifts — so it goes through the `fixtures/hex-directions.json`
    // idiom rather than through a constant somebody remembers to change.
    //
    // `height_unit` is the one that matters at the compatibility boundary. Production still builds
    // `GroundSpine::legacy`, whose height is a presentation band step; when the physical source
    // activates the same field becomes a 0.25 m quantum, the number stays an integer, and nothing
    // in the payload announces that the world got seventeen times taller. This test is what makes
    // that switch reach `src/rendering/sceneScale.ts` in the same commit.
    #[derive(Deserialize)]
    struct SceneScale {
        height_unit: String,
        height_quantum_mm: i32,
        cell_circumradius_mm: i32,
        max_walk_step: i32,
        relief_min: i32,
        relief_max: i32,
    }

    let fixture: SceneScale =
        serde_json::from_str(include_str!("../../../fixtures/scene-scale.json")).unwrap();
    assert_eq!(fixture.height_quantum_mm, scale::HEIGHT_QUANTUM_MM);
    assert_eq!(fixture.cell_circumradius_mm, scale::CELL_CIRCUMRADIUS_MM);
    // Read out of a real Core rather than declared here, so the fixture answers to the source
    // production constructs and not to a second opinion about which one that is.
    let physical = test_factory("new-game").core.ground_is_physical();
    assert_eq!(
        fixture.height_unit,
        if physical { "quantum" } else { "legacy_step" },
        "the fixture names a height unit the shipped ground source does not publish"
    );
    assert_eq!(
        fixture.max_walk_step,
        if physical {
            scale::MAX_WALK_STEP_QUANTA
        } else {
            MAX_WALK_STEP
        },
        "the renderer's cliff threshold is not the step the player can climb"
    );
    // The full reach of finished ground: the generated bed's own range, opened at both ends by
    // everything the player is allowed to dig or pile on top of it. The camera brackets its
    // depth range with this, so a summit that native can generate is a summit the camera can
    // still draw and still pick.
    let (relief_min, relief_max) = if physical {
        (
            scale::BED_MIN_QUANTA - scale::EARTHWORK_LIMIT_QUANTA,
            scale::BED_MAX_QUANTA + scale::EARTHWORK_LIMIT_QUANTA,
        )
    } else {
        let steps = i32::from(MAX_GRADE_STEPS);
        (
            ground_spine::legacy_band_elevation(Terrain::DeepWater) - steps,
            ground_spine::legacy_band_elevation(Terrain::Cliff) + steps,
        )
    };
    assert_eq!(fixture.relief_min, relief_min);
    assert_eq!(fixture.relief_max, relief_max);

    // A preview pixel is turned into a world point and the point into the hex holding it. The
    // round trip is what makes that a picture of the map rather than of a sheared rhombus, and it
    // has to hold on both sides of the origin — truncating division is exactly the bug that would
    // pass the northern half and shear the southern one.
    for q in -40..=40 {
        for r in -40..=40 {
            let (x, y) = axial_world(q, r);
            assert_eq!(
                hex_at_world(i64::from(x), i64::from(y)),
                (q, r),
                "centre of hex {q},{r}"
            );
        }
    }
}

/// The preview exists so a player can see a parameter set before playing it, which is only
/// worth anything if it is the set that gets played. These are the properties that make it one
/// picture of one world rather than a second generator that happens to look similar.
#[test]
fn world_preview_rasters_the_world_the_run_would_generate() {
    let factory = test_factory("new-game");
    let params = factory.core.world_params.clone();
    let seed = factory.core.seed;
    let json = serde_json::to_string(&params).unwrap();
    let (width, height) = (64u32, 48u32);
    let cells = factory
        .preview_cells(&json, seed, width, height, 512)
        .expect("a shipped parameter set rasters");
    assert_eq!(cells.len(), (width * height) as usize);
    assert!(
        cells.iter().all(|&band| band <= Terrain::Cliff as u8),
        "a preview byte is a band index"
    );
    // The window is centred on the landing site, so the middle pixel is the clearing that
    // `terrain_at` forces there. That pins the centring and the encoding together.
    let centre = (height / 2 * width + width / 2) as usize;
    assert_eq!(cells[centre], Terrain::Lowland as u8);

    // The picture is of these parameters and not of a cached world: raising the sea to just
    // under the shore cut has to flood ground that was dry.
    let water = |cells: &[u8]| {
        cells
            .iter()
            .filter(|&&band| {
                band == Terrain::DeepWater as u8 || band == Terrain::ShallowWater as u8
            })
            .count()
    };
    let flooded = WorldParams {
        water_level: params.shore_level - 1,
        ..params.clone()
    };
    let risen = factory
        .preview_cells(
            &serde_json::to_string(&flooded).unwrap(),
            seed,
            width,
            height,
            512,
        )
        .unwrap();
    assert!(water(&risen) > water(&cells), "a risen sea floods nothing");

    // A set `Core::new` would refuse is refused here too, rather than drawn or divided by. A
    // slider mid-drag is the caller this is for.
    let broken = WorldParams {
        site_cell: 0,
        ..params.clone()
    };
    assert!(factory
        .preview_cells(
            &serde_json::to_string(&broken).unwrap(),
            seed,
            width,
            height,
            512
        )
        .is_err());

    // Deposits are reported as lattice centres rather than sampled, so what pins them is the
    // lattice: `site_cell` is how far apart sites stand, and a window of fixed size holds fewer
    // of them when they stand further apart.
    let factory = test_factory("new-game");
    let params = factory.core.world_params.clone();
    let seed = factory.core.seed;
    let read = |params: &WorldParams, across: u32| -> PreviewSites {
        factory
            .preview_sites(
                &serde_json::to_string(params).unwrap(),
                seed,
                64,
                48,
                across,
            )
            .expect("a shipped parameter set reports sites")
    };

    let shipped = read(&params, 64);
    assert!(!shipped.sites.is_empty(), "a shipped world holds deposits");
    assert_eq!(shipped.total as usize, shipped.sites.len());
    assert!(!shipped.dense);
    // `Core::new` built this world, so its opening is met — a preview claiming otherwise would
    // be warning about a world that starts fine.
    assert!(shipped.unmet.is_empty());

    let sparse = read(
        &WorldParams {
            site_cell: params.site_cell * 2,
            ..params.clone()
        },
        64,
    );
    assert!(
        sparse.total < shipped.total,
        "doubling the lattice left the window as crowded"
    );

    // Wide enough to hold more deposits than are worth drawing: the count still travels, the
    // list does not, and `dense` is what tells the two apart from a world with no deposits.
    let wide = read(&params, MAX_PREVIEW_SPAN);
    assert!(wide.dense);
    assert!(wide.sites.is_empty());
    assert!(wide.unmet.is_empty(), "the bootstrap verdict still travels");
}

#[test]
fn a_world_that_opens_is_diagnosed_repaired_and_free_of_legacy_band_cuts() {
    let factory = test_factory("new-game");
    let params = factory.core.world_params.clone();
    assert!(
        bootstraps(&params, 7),
        "the shipped parameters have to open, or the rest of this proves nothing"
    );
    let (needs, repair) = factory.preview_diagnosis(&params, 7, &[]);
    // Not merely empty: nothing was searched. A repair ladder run over a world nobody is
    // stuck in would be two dozen bootstrap passes behind every slider drag.
    assert!(needs.is_empty());
    assert!(repair.is_none());

    // Physical opening outcrops do not depend on legacy band cuts.
    let factory = test_factory("new-game");
    let params = drowned_params(&factory.core.world_params);
    let spine = GroundSpine::physical(&params, 7, true);
    let (_, unmet) = bootstrap_sites(&params, 7, &spine);
    assert!(
        unmet.is_empty(),
        "physical outcrops must not inherit absent legacy bands: {unmet:?}"
    );
    let (needs, repair) = factory.preview_diagnosis(&params, 7, &[]);
    assert!(needs.is_empty());
    assert!(repair.is_none());

    // Physical opening outcrops survive legacy band controls.
    let factory = test_factory("new-game");
    let base = factory.core.world_params.clone();
    let params = drowned_params(&base);
    let spine = GroundSpine::physical(&params, 7, true);
    let (_, unmet) = bootstrap_sites(&params, 7, &spine);
    assert!(unmet.is_empty(), "physical opening lost {unmet:?}");

    // A sparse site lattice is repaired by a verified change.
    let factory = test_factory("new-game");
    let params = WorldParams {
        site_cell: 128,
        ..factory.core.world_params.clone()
    };
    let spine = GroundSpine::physical(&params, 7, true);
    let (_, unmet) = bootstrap_sites(&params, 7, &spine);
    let unmet: Vec<ItemId> = unmet.iter().map(|&(item_id, _)| item_id).collect();
    assert!(!unmet.is_empty());
    let (_, repair) = factory.preview_diagnosis(&params, 7, &unmet);
    let repair = repair.expect("a sparse lattice has a verified way out");
    let mut fixed = params.clone();
    for change in &repair.changes {
        assert_eq!(read_world_scalar(&params, change.field), Some(change.from));
        write_world_scalar(&mut fixed, change.field, change.to);
    }
    assert!(fixed.validate(&factory.definitions).is_ok());
    assert!(bootstraps(&fixed, repair.seed.unwrap_or(7)));
}

#[test]
fn chunk_generation_is_seeded_cached_and_invertible() {
    let mut a = game("new-game");
    let mut b = game("new-game");
    a.generate_chunk(8, -4);
    a.generate_chunk(-6, 3);
    b.generate_chunk(-6, 3);
    b.generate_chunk(8, -4);
    assert_eq!(a.checksum(), b.checksum());
    assert_eq!(coordinate_hash(1213486160, 81, -33), 166_969_415);
    assert_ne!(
        coordinate_hash(1213486160, 81, -33),
        coordinate_hash(1213486161, 81, -33)
    );
    // The site lattice is a cache, and a cache is exactly where order-dependence gets into a
    // generator: `a` walked one chunk first and `b` the other, so their lattices were filled
    // in different orders. Every cell they both hold has to agree, and the cached answer has
    // to be the uncached one — the two halves of "derived state, and derived from what".
    for (&cell, &site) in a.fields.sites.borrow().iter() {
        assert_eq!(site, a.fields.site_uncached(cell, &a.ground_spine));
        if let Some(&other) = b.fields.sites.borrow().get(&cell) {
            assert_eq!(site, other);
        }
    }

    // The cache pays for the site model and must not change it. `field_at` is asked over a disc
    // wide enough to cross many lattice cells, warm and cold, and the two must never disagree.
    let params = preset_params("continental").unwrap();
    let seed = survey::default_seed();
    let spine = GroundSpine::physical(&params, seed, true);
    let warm = WorldFields::new(&params, seed, &spine);
    for (q, r) in hexes_in_radius((14, -9), 24) {
        let cold = WorldFields::new(&params, seed, &spine);
        assert_eq!(
            warm.field_at(q, r, true, &spine),
            cold.field_at(q, r, true, &spine),
            "the cache changed the world at {q},{r}"
        );
        let cell = (
            floor_div(q, params.site_cell),
            floor_div(r, params.site_cell),
        );
        assert_eq!(warm.site_at(cell, &spine), warm.site_uncached(cell, &spine));
    }
    // And the cheap water test the fast path opens with agrees with the band decision it
    // skips, clearing included. If it ever did not, `field_at` would drop deposits silently.
    for (q, r) in hexes_in_radius((0, 0), 40) {
        assert_eq!(
            is_water_at(&params, seed, q, r),
            terrain_at(&params, seed, q, r, true).is_water(),
            "the cheap water test disagrees at {q},{r}"
        );
    }

    // World to axial inverts axial world and rounds to the nearest hex.
    for q in -12..=12 {
        for r in -12..=12 {
            let (x, y) = axial_world(q, r);
            assert_eq!(world_to_axial(x, y), (q, r));
        }
    }
    let (x, y) = axial_world(3, -2);
    assert_eq!(world_to_axial(x + 200, y - 150), (3, -2));
}

#[test]
fn materials_are_generated_where_geography_says_and_harvested_within_a_radius() {
    // The one test that must see an untouched world: the claim is that an unmined field costs
    // nothing stored, and a fixture that pre-writes eight tiles would answer it in advance.
    let mut core = bare_game("new-game");
    assert!(core.ground_is_physical());
    assert!(!core
        .generated_ground_at(0, 0)
        .hydrology
        .depth_quanta
        .is_positive());
    assert!(!core.terrain_blocks_movement(0, 0));
    // The clearing holds no field at all now: the eight hardcoded cells it used to carry were
    // a sample platter, and the opening is placed by the generator outside it.
    for cell in hexes_in_radius((0, 0), LANDING_CLEAR_RADIUS) {
        assert_eq!(core.field_at(cell.0, cell.1), None);
    }
    let cell = *core
        .fields
        .bootstrap
        .values()
        .map(|site| site.center)
        .min()
        .as_ref()
        .expect("a new world guarantees an opening");
    let quantity = core
        .field_at(cell.0, cell.1)
        .expect("a site centre")
        .quantity;
    assert!(quantity > 0);
    assert_eq!(core.deposit_quantity(cell), quantity);
    // Unmined field is derived: the overlay is empty until something is taken, but the
    // snapshot still reports the cell so the host can draw it.
    assert!(core.tiles.is_empty());
    core.ensure_neighborhood(axial_world(cell.0, cell.1).0, axial_world(cell.0, cell.1).1);
    assert!(core
        .resource_snapshots()
        .iter()
        .any(|resource| resource.q == cell.0
            && resource.r == cell.1
            && resource.quantity == quantity));
    let before = core.checksum();
    set_player_hex(&mut core, cell.0, cell.1);
    core.gather().unwrap();
    cooldown(&mut core);
    assert_eq!(core.deposit_quantity(cell), quantity - 1);
    assert_eq!(
        core.tiles[&cell].resource.as_ref().unwrap().quantity,
        quantity - 1
    );
    assert_ne!(core.checksum(), before);

    // An extractor harvests every field cell inside its radius.
    let mut core = game("new-game");
    core.researched.insert(2);
    stock_for(&mut core, 1, 1);
    set_player_hex(&mut core, 3, 1);
    // Two ore cells one step apart, written into the overlay because the clearing generates
    // none: this is a test about which cell inside a reach is drawn from first, and standing
    // it on geography would make it a test about geography.
    core.write_overlay(3, 0, 1, 48, 48);
    core.write_overlay(4, 0, 1, 3, 3);
    core.place(3, 0, 1, 0, None).unwrap();
    let index = core.entity_at(3, 0).unwrap();
    let candidates = core.deposit_candidates(3, 0, EXTRACT_RADIUS);
    assert_eq!(candidates[0], (3, 0));
    assert!(candidates.contains(&(4, 0)));
    assert_eq!(core.extractor_deposit(index), Some((3, 0)));
    core.write_overlay(3, 0, 1, 0, 48);
    assert_eq!(core.extractor_deposit(index), Some((4, 0)));

    // Geography is still the material map. A deposit is a site rather than a per-hex decision now,
    // so what a band holds is the set of rules that may *reach* into it — the member table — and
    // this asserts that set exactly, band by band.
    // Real relief: the subject here *is* the generated ground, so `game`'s level opening
    // would leave nothing to measure.
    let core = field_game("new-game");
    assert!(core.ground_is_physical());

    let mut seen: BTreeMap<Terrain, BTreeSet<ItemId>> = BTreeMap::new();
    let mut land = 0u32;
    let mut fields = 0u32;
    for q in -80..80 {
        for r in -80..80 {
            // The clearing is deliberately not geography, so it is not evidence about which
            // band holds what.
            if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
                continue;
            }
            let terrain = core.terrain_at(q, r);
            if !terrain.is_water() {
                land += 1;
            }
            if let Some(field) = core.fields.field_at(q, r, true, &core.ground_spine) {
                fields += 1;
                seen.entry(terrain).or_default().insert(field.item_id);
            }
        }
    }
    // A field is a place. Barren ground has to be the common case, or the landscape is a
    // carpet and a site is stumbled over rather than chosen. The floor keeps a weight change
    // from emptying a band by accident.
    assert!(land > 0);
    assert!(
        fields * 100 < land * 22,
        "fields too dense: {fields} of {land} land hexes"
    );
    assert!(
        fields * 100 > land * 3,
        "fields too sparse: {fields} of {land} land hexes"
    );
    // Physical ground no longer promises that every old presentation band occurs in one local
    // sample. What remains authoritative here is that every authored raw material appears on
    // dry ground and water itself is pumped rather than mined.
    let generated: BTreeSet<ItemId> = seen.values().flatten().copied().collect();
    for item_id in [
        IRON_ORE, COPPER_ORE, COAL, STONE, SAND, CLAY, WOOD, LIMESTONE, CRUDE_OIL,
    ] {
        assert!(
            generated.contains(&item_id),
            "sample generated no item {item_id}"
        );
    }
    // Crystal is deliberately remote and rare; the opening sample is allowed not to contain it.
    // Water is pumped, not mined, which is why a basin can never be emptied. `validate` refuses
    // a rule that names a water band, and this is that refusal seen from the world.
    assert!(!seen.contains_key(&Terrain::DeepWater));
    assert!(!seen.contains_key(&Terrain::ShallowWater));

    // Sandy-looking tiles are the shore band. Clay may still sit on them, but sand has to be
    // what a player walking a beach finds first — not a regional ocean they never reach.
    // Real relief: a shore is a fact about generated ground. See `field_game`.
    let core = field_game("new-game");
    let mut shore = 0u32;
    let mut sand = 0u32;
    let mut clay = 0u32;
    for q in -160..160 {
        for r in -160..160 {
            if axial_distance((0, 0), (q, r)) <= LANDING_CLEAR_RADIUS {
                continue;
            }
            if core.terrain_at(q, r) != Terrain::Shore {
                continue;
            }
            shore += 1;
            let Some(field) = core.fields.field_at(q, r, true, &core.ground_spine) else {
                continue;
            };
            match field.item_id {
                SAND => sand += 1,
                CLAY => clay += 1,
                _ => {}
            }
        }
    }
    assert!(
        shore > 40,
        "the sample has to hold a real shore, saw {shore} shore hexes"
    );
    assert!(
        sand > 0,
        "sandy tiles held no sand at all ({clay} clay on {shore} shore hexes)"
    );
    assert!(
        sand >= clay,
        "sand should be the common field on shore, saw {sand} sand vs {clay} clay on {shore} \
             shore hexes"
    );
}

/// The seed is no longer the only thing a world can differ by. Two parameter sets on the same
/// seed have to be different *landforms*, not the same landform with the cuts moved.
#[test]
fn two_parameter_sets_on_one_seed_are_different_landforms() {
    let seed = survey::default_seed();
    let continental = preset_params("continental").unwrap();
    let basin = preset_params("basin").unwrap();
    // The landing disc is an opening, not a landform: both presets fade toward the same
    // local blend there. The claim is about the world beyond it.
    let inner = landing_radius(&continental).max(landing_radius(&basin)) + 8;
    let outer = inner + 48;
    let mut differing = 0u32;
    let mut hexes = 0u32;
    for q in -outer..outer {
        for r in -outer..outer {
            let distance = axial_distance((0, 0), (q, r));
            if distance <= inner || distance > outer {
                continue;
            }
            hexes += 1;
            if terrain_at(&continental, seed, q, r, true) != terrain_at(&basin, seed, q, r, true) {
                differing += 1;
            }
        }
    }
    assert!(
        differing * 100 > hexes * 60,
        "only {differing} of {hexes} hexes differ between two parameter sets"
    );

    // And the sliders that used to decide what a world looked like no longer reach the landform
    // at all. Feature scale and the four band levels described a world cut out of noise by
    // thresholds; the physical world is a surface with a height, and moving a threshold under it
    // moves nothing. Every hex within a radius of 24 answers exactly the same either way.
    let altered = WorldParams {
        elevation_coarse_cell: 4,
        water_level: 50_000,
        shore_level: 52_000,
        hills_level: 58_000,
        highland_level: 62_000,
        ..continental.clone()
    };
    let first = GroundSpine::physical(&continental, seed, true);
    let second = GroundSpine::physical(&altered, seed, true);
    for (q, r) in hexes_in_radius((0, 0), 24) {
        assert_eq!(
            first.generated_at(q, r),
            second.generated_at(q, r),
            "legacy band/scale sliders leaked into the physical landform at {q},{r}"
        );
    }
}
