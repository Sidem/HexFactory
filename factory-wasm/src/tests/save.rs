use super::*;

/// Every save written before the physical world is refused, and refused with a way out.
///
/// Nine migration tests used to stand here, one per rung of the old ladder. The scale break
/// retired all of them at once: a file from any of those versions is turned away at the
/// envelope now, so each of those tests had become this one assertion followed by unreachable
/// legacy code. This is what is left, and it is the whole claim.
#[test]
fn a_pre_physical_save_is_refused_with_an_export_offered() {
    assert_pre_physical_save_is_refused();
}

#[test]
fn a_save_resumes_and_replays_in_a_deterministic_order() {
    let (definitions, technologies, scenarios) = catalogs();
    let mut uninterrupted = game("factory-demo");
    // Metered on both sides, which is the shipped rule and the only way this test is honest.
    // `power_unmetered` is a harness hook that no save carries, so a resumed core always comes
    // back metered; leaving the running one unmetered compared two different games. It passed
    // until v0.19 only because a fully supplied grid used to make the two paths agree by
    // arithmetic — with banked energy they no longer do, and the resume is exactly what should
    // catch that.
    uninterrupted.power_unmetered = false;
    uninterrupted.tick_many(120);
    let save = uninterrupted.save_string().unwrap();
    assert!(save.starts_with(SAVE_PREFIX));
    let mut resumed = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    uninterrupted.tick_many(180);
    resumed.tick_many(180);
    assert_eq!(uninterrupted.checksum(), resumed.checksum());
    assert_eq!(uninterrupted.delivered, resumed.delivered);
    assert!(Core::from_save(&definitions, &technologies, &scenarios, "bad").is_err());
    // Written against the live version rather than a literal, so bumping a version is a
    // one-line change in one place and this test keeps testing the rejection it names.
    let incompatible = save.replacen(
        &format!("\"definition_version\":{}", definitions.version),
        "\"definition_version\":999",
        1,
    );
    assert!(Core::from_save(&definitions, &technologies, &scenarios, &incompatible).is_err());
    // Version 16 is the previous envelope. Technology catalog 7 has neither capability row, so
    // an empty fresh run can be spelled exactly as that release did and must migrate to the
    // same checksum without being granted research.
    let previous_source = game("new-game").save_string().unwrap();
    let previous_envelope = previous_source
        .replacen("\"technology_version\":8", "\"technology_version\":7", 1)
        .replacen("\"definition_version\":16", "\"definition_version\":15", 1)
        .replacen(
            &format!("\"save_version\":{SAVE_VERSION}"),
            "\"save_version\":16",
            1,
        );
    assert_refused_as_legacy_scale(Core::from_save(
        &definitions,
        &technologies,
        &scenarios,
        &previous_envelope,
    ));
    let baseline =
        Core::from_save(&definitions, &technologies, &scenarios, &previous_source).unwrap();
    assert_eq!(baseline.player.walk_goal, None);
    // Everything older still is. There is no migration for it, and reading one as a newer
    // spelling of the same thing is exactly what the boundary refuses to do.
    let unmigratable = save.replacen(
        &format!("\"save_version\":{SAVE_VERSION}"),
        "\"save_version\":13",
        1,
    );
    assert!(
        Core::from_save(&definitions, &technologies, &scenarios, &unmigratable).is_err(),
        "a version-13 save must be refused rather than read with six-direction orientations"
    );
    // v0.16 takes the generator to 6 because `WorldParams` entered the envelope and the
    // checksum. A version-5 envelope names no parameters at all, so it cannot be read as the
    // default set — it is rejected.
    let old_world = save.replacen(
        &format!("\"world_generator_version\":{WORLD_GENERATOR_VERSION}"),
        &format!(
            "\"world_generator_version\":{}",
            WORLD_GENERATOR_VERSION - 1
        ),
        1,
    );
    assert!(Core::from_save(&definitions, &technologies, &scenarios, &old_world).is_err());
    // The parameters are checksummed, so editing them in a saved file is caught as tampering
    // rather than quietly regenerating a different world under the same overlay.
    let edited_params = save.replacen("\"water_level\":18000", "\"water_level\":19000", 1);
    assert_ne!(edited_params, save, "the save carries its world parameters");
    assert!(Core::from_save(&definitions, &technologies, &scenarios, &edited_params).is_err());

    // Reset replay and scenario insertion order are deterministic.
    let (definitions, technologies, scenarios) = catalogs();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|value| value.key == "factory-demo")
        .unwrap();
    let mut reversed = scenario.clone();
    reversed.buildings.reverse();
    let mut a = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
    let mut b = Core::new(&definitions, &technologies, &reversed, None, None).unwrap();
    a.tick_many(300);
    b.tick_many(300);
    assert_eq!(a.checksum(), b.checksum());
    let expected = a.checksum();
    let mut replay = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
    replay.tick_many(300);
    assert_eq!(replay.checksum(), expected);
}

#[test]
fn deltas_send_only_what_changed_and_match_a_full_snapshot_diff() {
    let mut core = game("new-game");
    let previous = core.snapshot();
    core.tick_many(1);
    let current = core.snapshot();
    let delta = SnapshotDelta::between(7, 8, &previous, &current);
    assert_eq!(delta.base_revision, 7);
    assert_eq!(delta.revision, 8);
    assert_eq!(delta.tick, 1);
    assert!(delta.terrain.is_none());
    assert!(delta.resources.is_none());
    assert!(delta.buildings.is_none());
    assert!(delta.events.is_some());
    let json = serde_json::to_string(&delta).unwrap();
    assert!(!json.contains("\"terrain\""));
    assert!(!json.contains("\"resources\""));
    assert!(!json.contains("\"buildings\""));

    // Generated chunk bounds report the surveyed world area.
    let mut core = game("new-game");
    let snapshot = core.snapshot();
    let size = core.scenario.chunk_size;
    assert!(!snapshot.chunks.is_empty());
    for chunk in &snapshot.chunks {
        let (x, y, span) = chunk_world_bounds(chunk.chunk_q, chunk.chunk_r, size);
        assert_eq!(chunk.x, x);
        assert_eq!(chunk.y, y);
        assert_eq!(chunk.span, span);
    }
    let contains = |chunk: &ChunkSnapshot, x: i32, y: i32| {
        (chunk.x..chunk.x + chunk.span).contains(&x) && (chunk.y..chunk.y + chunk.span).contains(&y)
    };
    // The player always stands inside surveyed world.
    assert!(snapshot
        .chunks
        .iter()
        .any(|chunk| contains(chunk, core.player.x, core.player.y)));
    // Distant world stays unreported, which is what the host renders as fog.
    let (far_q, far_r) = (size * 4, size * 4);
    let (far_x, far_y) = axial_world(far_q, far_r);
    assert!(!snapshot
        .chunks
        .iter()
        .any(|chunk| contains(chunk, far_x, far_y)));

    // Travelling there surveys it, so the fogged area shrinks as the player explores.
    core.ensure_neighborhood(far_x, far_y);
    let explored = core.snapshot();
    assert!(explored.chunks.len() > snapshot.chunks.len());
    assert!(explored
        .chunks
        .iter()
        .any(|chunk| contains(chunk, far_x, far_y)));

    // Buildings delta sends only the entities that changed.
    let mut core = game("new-game");
    core.researched.insert(2);
    stock_for(&mut core, 1, 1);
    set_player_hex(&mut core, 3, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    add_test_belt(&mut core, 4, 1, 0);
    core.compile_graph();

    // One tick advances only the extractor's progress; the hub and the belt are untouched.
    let previous = core.snapshot();
    core.tick_many(1);
    let current = core.snapshot();
    let patch = buildings_delta(&previous.buildings, &current.buildings).unwrap();
    assert!(!patch.replace);
    assert!(patch.removed.is_empty());
    assert_eq!(
        patch
            .changed
            .iter()
            .map(|entity| entity.id)
            .collect::<Vec<_>>(),
        vec![2]
    );
    assert!(current.buildings.len() > patch.changed.len());
    let json = serde_json::to_string(&SnapshotDelta::between(0, 1, &previous, &current)).unwrap();
    assert!(json.len() < serde_json::to_string(&current.buildings).unwrap().len());

    // Erasing reports the id instead of resending every surviving entity.
    let previous = current;
    core.erase(3, 0).unwrap();
    let current = core.snapshot();
    let patch = buildings_delta(&previous.buildings, &current.buildings).unwrap();
    assert_eq!(patch.removed, vec![2]);
    assert!(patch.changed.is_empty());

    // A full delta stays a complete replacement, so a host with no prior state is correct.
    let full = SnapshotDelta::full(0, 1, &current).buildings.unwrap();
    assert!(full.replace);
    assert_eq!(full.changed, current.buildings);

    // The shipped delta is built from marks made where state is mutated, not by diffing two
    // complete snapshots, so a missed mark would silently strand the host on stale state. This
    // pins the builder against the full diff it replaces, step by step, across every path that
    // touches a snapshot group: quiet frames, ticks, gathering to depletion, hub delivery,
    // research, placement, rotation, erasure, and travel into unsurveyed world.
    let mut factory = test_factory("new-game");
    // Setup pokes happen before the baseline is taken, so the checked run only exercises real
    // native paths. Shrinking a guaranteed deposit lets the run reach depletion. The starting
    // pack stays inside the carrying rule, or the gathering steps below would be refused.
    factory.core.player.inventory.insert(1, 40);
    factory.core.player.inventory.insert(2, 3);
    factory.core.player.inventory.insert(3, 20);
    factory.core.player.inventory.insert(6, 8);
    set_player_hex(&mut factory.core, 4, -2);
    factory.core.write_overlay(4, -2, 1, 2, 36);
    // The clearing generates nothing since v0.21, so the deposit the extractor further down
    // stands on is written here rather than found. Same reasoning as `TEST_FIELD`: this is a
    // test about which marks a delta carries, not about where a generator puts iron.
    factory.core.write_overlay(6, 0, 1, 48, 48);
    let surveyed_at_start = factory.core.generated_chunks.len();

    // Establish the baseline exactly as the worker does on its first frame.
    let _ = factory.snapshot_json();
    let mut previous = factory.core.snapshot();
    let mut check = |factory: &mut Factory, step: &str| {
        assert_delta_matches_full_diff(factory, &mut previous, step);
    };

    factory.core.advance("[]", 0, 0).unwrap();
    check(&mut factory, "an empty frame");
    factory.core.advance(IDLE, 1, 1).unwrap();
    check(&mut factory, "one idle tick");

    // Gathering, through the frame the deposit runs dry and one rejected attempt after it.
    // The cooldown between attempts is paid in player steps, because that is the clock the
    // player's own actions run on — the factory ticks here only exercise the tick paths.
    for round in 0..3 {
        factory
            .core
            .advance(r#"[{"type":"gather"}]"#, 2, 60)
            .unwrap();
        check(&mut factory, &format!("gather attempt {round}"));
    }
    assert_eq!(factory.core.deposit_quantity((4, -2)), 0);

    // Delivery and research: insight, delivered totals, the objective, and unlocks.
    set_player_hex(&mut factory.core, 1, 0);
    check(&mut factory, "walking to the landing hub");
    factory
        .core
        .advance(r#"[{"type":"deposit"}]"#, 1, 0)
        .unwrap();
    check(&mut factory, "delivering inventory to the hub");
    // Prove the line grants the four starter technologies; Composition is still an insight
    // purchase. Insight is compared against the baseline rather than marked, so a direct
    // change is exactly what the host would see from any native path that moves it.
    assert_eq!(factory.core.researched.len(), 4);
    factory.core.insight += 8;
    check(&mut factory, "funding the research");
    factory
        .core
        .advance(r#"[{"type":"research","technology_id":3}]"#, 1, 0)
        .unwrap();
    check(&mut factory, "researching composition");
    assert_eq!(factory.core.researched.len(), 5);

    // Player state is compared against the baseline rather than marked, so restocking directly
    // is exactly what the host would see from any native path that changes inventory.
    // Kept inside the carrying rule, so the erase further down still has somewhere to refund to.
    stock_for(&mut factory.core, 1, 1);
    stock_for(&mut factory.core, 3, 1);
    factory.core.player.inventory.insert(24, 8);
    check(&mut factory, "restocking the player");

    // Construction: inserted entities, recompiled transport, and per-chunk entity counts.
    // The build site stands off the hub's seven hexes: the composer's three would otherwise
    // reach into them. The line still runs west and the composer still hands into the hub's
    // eastern rim; only the empty ground between the machines moved.
    set_player_hex(&mut factory.core, 4, 2);
    check(&mut factory, "walking to the build site");
    factory.core.place(6, 0, 1, 3, None).unwrap();
    check(&mut factory, "placing an extractor");
    factory.core.place(4, 0, 2, 3, None).unwrap();
    check(&mut factory, "placing a belt");
    factory.core.place(3, 0, 3, 3, Some(1)).unwrap();
    check(&mut factory, "placing a composer");

    // The factory running: machine progress, cargo transfer, hub deliveries, and victory.
    for round in 0..8 {
        factory.core.advance(IDLE, 20, 0).unwrap();
        check(&mut factory, &format!("running the factory, round {round}"));
    }
    assert!(factory.core.delivered > 0, "the scripted run must produce");

    // Edits against a live blueprint, including orientations that split and rejoin components.
    for turn in 0..6 {
        factory.core.rotate(4, 0, false).unwrap();
        check(&mut factory, &format!("rotating a belt, turn {turn}"));
    }
    factory.core.erase(4, 0).unwrap();
    check(&mut factory, "erasing a belt");
    factory.core.advance(IDLE, 5, 0).unwrap();
    check(&mut factory, "ticking with the belt gone");
    factory.core.place(4, 0, 2, 3, None).unwrap();
    check(&mut factory, "replacing the belt");

    // Cutting flora and letting it grow back. Regrowth is the one thing that changes a deposit
    // without an extractor or a player touching it that frame, so it has to mark what it moved.
    set_player_hex(&mut factory.core, -3, 1);
    check(&mut factory, "walking to the flora");
    factory
        .core
        .advance(r#"[{"type":"gather"}]"#, 1, GATHER_COOLDOWN_STEPS)
        .unwrap();
    check(&mut factory, "cutting flora");
    let regrowth = factory
        .core
        .item_definition(WOOD)
        .unwrap()
        .regrowth_ticks
        .expect("wood regrows");
    factory.core.advance(IDLE, regrowth, 0).unwrap();
    check(&mut factory, "flora growing back");
    assert!(
        factory.core.flora_regrowth.is_empty(),
        "the cut cell must have grown back inside its own cadence"
    );

    // Travel into unsurveyed world: terrain, deposits, chunk bounds, and every extractor's
    // resolved deposit reference at once. The neighborhood generator is the same one walking
    // uses; a far hex is used so derived water or cliffs cannot stall the survey.
    for (label, (q, r)) in [("east", (24, 0)), ("south", (24, 16))] {
        set_player_hex(&mut factory.core, q, r);
        factory.core.advance(IDLE, 1, 1).unwrap();
        check(
            &mut factory,
            &format!("travelling {label} into unsurveyed world"),
        );
    }
    factory.core.advance(IDLE, 1, 1).unwrap();
    check(&mut factory, "standing still again");
    assert!(
        factory.core.generated_chunks.len() > surveyed_at_start,
        "the scripted run must survey new world"
    );

    // A load replaces the core the baseline described, so the host is sent a complete
    // replacement rather than a patch against state that no longer exists.
    let save = factory.core.save_string().unwrap();
    factory.load_string(&save).unwrap();
    let delta = factory.build_delta();
    assert!(
        delta
            .buildings
            .expect("full delta carries buildings")
            .replace
    );
    assert!(
        delta
            .resources
            .expect("full delta carries resources")
            .replace
    );
    assert!(delta.terrain.is_some());
    assert!(delta.chunks.is_some());
    assert!(delta.player.is_some());

    // World generation invalidates resolved deposit references, so it must invalidate the entity
    // snapshots derived from them in the same breath. Today's deposit radii are smaller than the
    // tile spacing, so a generated deposit does not in fact reach an existing extractor and the
    // scripted equivalence run cannot observe this — which is exactly why the coupling is pinned
    // here directly rather than left to depend on that geometry holding.
    let mut core = game("new-game");
    core.researched.insert(2);
    stock_for(&mut core, 1, 1);
    set_player_hex(&mut core, 3, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    let index = core.entity_at(3, 0).unwrap();
    core.extractor_deposit(index);
    assert_eq!(core.deposit_links.len(), 1);

    core.dirty = SnapshotDirty::default();
    core.generate_chunk(-9, 7);

    assert!(core.deposit_links.is_empty(), "references are re-resolved");
    let marked: Vec<u32> = core.entities.iter().map(|entity| entity.id).collect();
    assert_eq!(
        drain_marks(&mut core.dirty.entities),
        marked,
        "every entity snapshot derived from a deposit is suspect too"
    );
    assert!(core.dirty.chunks, "the surveyed chunk set grew");

    // An extractor's reported status is resolved through its cached deposit reference instead of
    // a scan over every generated tile. The two must agree exactly, including after the deposit
    // under it runs dry.
    let mut core = game("new-game");
    core.researched.extend([1, 2]);
    stock_for(&mut core, 1, 1);
    set_player_hex(&mut core, 3, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    let index = core.entity_at(3, 0).unwrap();

    let scanned = |core: &Core| {
        let (x, y) = axial_world(core.entities[index].placed.q, core.entities[index].placed.r);
        core.resource_at_world(x, y)
            .map(|key| core.deposit_quantity(key))
            .unwrap_or(0)
            > 0
    };

    for _ in 0..3 {
        let expected = scanned(&core);
        assert_eq!(core.extractor_deposit(index).is_some(), expected);
        assert_eq!(
            core.status_of(index, expected, true, true, false),
            core.entity_snapshot(index).status
        );
        core.tick_many(20);
    }

    // Draining the field must flip both the scan and the cached reference together.
    core.write_overlay(3, 0, 1, 0, 48);
    assert!(!scanned(&core));
    assert!(core.extractor_deposit(index).is_none());
    core.entities[index].cargo = None;
    assert_eq!(
        core.entity_snapshot(index).status,
        EntityStatus::DepositDepleted
    );

    // Combined advance preserves command events through native ticks.
    let mut core = game("new-game");
    core.player.inventory.insert(1, 8);
    core.player.inventory.insert(3, 4);
    set_player_hex(&mut core, 1, 0);
    core.advance(r#"[{"type":"deposit"}]"#, 1, 0).unwrap();
    assert_eq!(core.tick, 1);
    // Eight ore, because the opening board asks for ore and nobody has asked for crystal yet.
    assert!(core
        .events
        .iter()
        .any(|event| event.contains("Delivered 8 to the landing hub")));
    assert_eq!(core.player.inventory.get(&3), Some(&4));

    // Malformed technology graphs and locked forged commands are rejected.
    let (definitions, mut technologies, scenarios) = catalogs();
    technologies.technologies[1].prerequisites = vec![3];
    assert!(validate_technologies(&definitions, &technologies).is_err());
    let mut core = game("new-game");
    core.player.inventory.insert(1, 100);
    core.apply_commands(r#"[{"type":"place","q":2,"r":0,"definition_id":2,"orientation":0}]"#)
        .unwrap();
    assert!(core.entities.iter().all(|entity| entity.placed.q != 2));
    assert!(core.events[0].contains("locked"));
    assert!(validate_scenarios(&definitions, &catalogs().1, &scenarios).is_ok());

    // Progression registries reject missing duplicate and unknown references.
    let (definitions, technologies, _) = catalogs();
    for change in 0..9 {
        let mut invalid = technologies.clone();
        match change {
            0 => invalid.branches.clear(),
            1 => invalid.stages.push(invalid.stages[0].clone()),
            2 => invalid.branches[0].key = "Bad key".into(),
            3 => invalid.stages[0].name = " ".into(),
            4 => invalid.technologies[0].branch = "missing".into(),
            5 => invalid.technologies[0].stage = "missing".into(),
            6 => invalid.technologies[1].key = invalid.technologies[0].key.clone(),
            7 => invalid.technologies[1].prerequisites = vec![1, 1],
            _ => invalid.branches = vec![invalid.branches[0].clone(); 65],
        }
        assert!(
            validate_technologies(&definitions, &invalid).is_err(),
            "case {change}"
        );
    }
}
