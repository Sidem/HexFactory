use super::*;

#[test]
fn movement_intent_aim_and_cadence_are_native() {
    let mut core = legacy_band_game("new-game");
    // Stay inside the landing clearing so derived water and cliffs cannot interrupt the walk.
    set_player_hex(&mut core, 0, 3);
    let start = (core.player.x, core.player.y);
    core.set_move_intent(707, -707).unwrap();
    core.advance_player_steps(3);
    let step = 707 * PLAYER_SPEED / 1000;
    assert_eq!(core.player.x, start.0 + 3 * step);
    assert_eq!(core.player.y, start.1 - 3 * step);
    assert_eq!((core.player.facing_x, core.player.facing_y), (707, -707));
    core.set_move_intent(0, 0).unwrap();
    core.advance_player_steps(3);
    assert_eq!(
        (core.player.x, core.player.y),
        (start.0 + 3 * step, start.1 - 3 * step)
    );
    assert!(core.set_move_intent(1001, 0).is_err());

    // A guaranteed landing cliff still blocks: stand just west of (1, -1) and walk east.
    let (cliff_x, cliff_y) = axial_world(1, -1);
    core.player.x = cliff_x - HEX_X / 2 - 20;
    core.player.y = cliff_y;
    let blocked_x = core.player.x;
    core.set_move_intent(1000, 0).unwrap();
    core.advance_player_steps(1);
    assert_eq!(core.player.x, blocked_x);
    assert_eq!(core.terrain_at(1, -1), Terrain::Cliff);

    // Shallows are a 5 m/s ford: walkable, not buildable, and the gait does not matter once
    // you are in the water. Deep water stays a wall.
    assert!(!Terrain::ShallowWater.blocks_movement());
    assert!(Terrain::ShallowWater.blocks_construction());
    assert!(Terrain::DeepWater.blocks_movement());
    assert!(Terrain::DeepWater.blocks_construction());

    let mut core = legacy_band_game("new-game");
    set_player_hex(&mut core, 2, 1);
    assert_eq!(core.terrain_at(2, 1), Terrain::ShallowWater);
    let start = (core.player.x, core.player.y);
    let ford = PLAYER_SPEED / 5;

    core.set_move_intent(1000, 0).unwrap();
    core.advance_player_steps(1);
    assert_eq!(core.player.x, start.0 + ford);

    core.player.x = start.0;
    core.set_move_intent(600, 0).unwrap();
    core.advance_player_steps(1);
    assert_eq!(
        core.player.x,
        start.0 + ford,
        "wading is 5 m/s at any gait, not 3/5 of it"
    );

    // Still not a building site: the player can stand in it, a pump cannot.
    set_player_hex(&mut core, 0, 3);
    core.researched.extend([1, 2, 5, 7]);
    core.player.inventory.insert(11, 20);
    core.player.inventory.insert(14, 20);
    assert!(core
        .place(2, 1, 11, 0, None)
        .unwrap_err()
        .contains("environment blocks construction"));

    // Facing became something the player aims rather than a side effect of walking, so the command
    // that sets it has to resolve as natively as the movement it sits beside: the host names a
    // world point and this turns it into the vector the checksum hashes.
    let mut core = game("new-game");
    set_player_hex(&mut core, 0, 3);
    let (x, y) = (core.player.x, core.player.y);

    core.set_aim(x + 5_000, y).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (1000, 0));
    core.set_aim(x, y - 5_000).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (0, -1000));

    // A diagonal resolves to a unit vector, not to whatever delta the host happened to send,
    // and pushing the same direction ten times further does not change the answer.
    core.set_aim(x - 3_000, y + 3_000).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (-707, 707));
    core.set_aim(x - 30_000, y + 30_000).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (-707, 707));

    // A cursor resting exactly on the player names no direction, so the last one stands.
    core.set_aim(x, y).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (-707, 707));
    assert!(core.set_aim(x + (MAX_AIM_DISTANCE as i32) + 1, y).is_err());

    // What an aim resolves to is ordinary player state: it is saved, and the save validator
    // that bounds facing accepts it, because native produced it rather than the host.
    let (definitions, technologies, scenarios) = catalogs();
    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(
        (restored.player.facing_x, restored.player.facing_y),
        (-707, 707)
    );

    // What keeps a pointer aiming and a touch layout facing the way it walks, with no stored
    // aiming mode for the save format and the checksum to carry: both commands write facing, and
    // whichever the host sent last in the batch is the one that stands.
    let mut core = game("new-game");
    set_player_hex(&mut core, 0, 3);
    let (x, y) = (core.player.x, core.player.y);
    let batch = format!(
        r#"[{{"type":"move_intent","x":1000,"y":0}},{{"type":"aim","x":{x},"y":{}}}]"#,
        y - 4_000
    );
    core.advance(&batch, 0, 0).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (0, -1000));

    // A frame with no aim in it — every frame of the touch layout — still faces the walk.
    core.advance(IDLE_MOVE_EAST, 0, 0).unwrap();
    assert_eq!((core.player.facing_x, core.player.facing_y), (1000, 0));

    // Integer square root is exact on squares and truncates between them.
    assert_eq!(integer_sqrt(0), 0);
    assert_eq!(integer_sqrt(-9), 0);
    for root in [1_i64, 2, 3, 1_000, 46_341, 3_037_000_499] {
        assert_eq!(integer_sqrt(root * root), root);
        assert_eq!(integer_sqrt(root * root - 1), root - 1);
    }

    // The player walks on its own cadence not the factorys.
    // The complaint this answers: the player stopped when the factory paused and crawled at a
    // low speed multiplier, because walking ran inside the simulation tick.
    let mut core = game("new-game");
    set_player_hex(&mut core, 0, 3);
    let start = (core.player.x, core.player.y);
    core.set_move_intent(1000, 0).unwrap();

    // A paused factory advances no ticks at all, and the player still walks.
    core.advance(IDLE_MOVE_EAST, 0, 10).unwrap();
    assert_eq!(core.tick, 0);
    assert_eq!(core.player.x, start.0 + 10 * PLAYER_SPEED);

    // Ticking the factory without spending player steps moves nothing.
    let held = core.player.x;
    core.advance("[]", 30, 0).unwrap();
    assert_eq!(core.tick, 30);
    assert_eq!(core.player.x, held);

    // The same step count always covers the same ground, whatever the factory is doing, so a
    // replay of the same commands and counts still reproduces the same position.
    let mut slow = game("new-game");
    let mut fast = game("new-game");
    for core in [&mut slow, &mut fast] {
        set_player_hex(core, 0, 3);
    }
    for _ in 0..4 {
        slow.advance(IDLE_MOVE_EAST, 1, 8).unwrap();
        fast.advance(IDLE_MOVE_EAST, 16, 8).unwrap();
    }
    assert_eq!(slow.player.x, fast.player.x);
    assert_eq!(slow.player.y, fast.player.y);
    assert_eq!(Factory::player_ticks_per_second(), PLAYER_TICKS_PER_SECOND);

    // A hexagon is 25 m², the walk is 15 m/s, the run is 25 m/s. Native stores one step size — the
    // run, at intent 1000 — and the host sends 600 for the walk, which is exactly 3/5 of full
    // intent. Neighbour spacing is still `HEX_X` world units, now read as 5.373 m.
    //
    // The gait ratio is the structural half and holds at any speed; the pinned constant is the
    // half that carries the decision. `PLAYER_SPEED` stayed at 275 across the rescale, so a hex
    // still takes about 0.36 s to cross at a walk and the metre figures moved instead.
    const WALK_INTENT: i32 = 600;
    let walk = WALK_INTENT * PLAYER_SPEED / 1000;
    assert_eq!(walk * 5, PLAYER_SPEED * 3);
    assert_eq!(PLAYER_SPEED, 275);

    // Metres a second, out of world units a step: 30 steps a second over `HEX_X` units of
    // 5.373 m. Integer throughout, and the run lands on 25 m/s to the metre.
    let run_mm_s =
        PLAYER_SPEED as i64 * PLAYER_TICKS_PER_SECOND as i64 * crate::scale::CELL_SPACING_MM as i64
            / HEX_X as i64;
    assert_eq!(run_mm_s / 1_000, 24);
    assert_eq!((run_mm_s + 500) / 1_000, 25);
    let walk_mm_s =
        walk as i64 * PLAYER_TICKS_PER_SECOND as i64 * crate::scale::CELL_SPACING_MM as i64
            / HEX_X as i64;
    assert_eq!((walk_mm_s + 500) / 1_000, 15);
}

#[test]
fn swimming_is_a_learned_deep_water_route_and_not_a_building_rule() {
    let mut core = field_game("new-game");
    let deep = (-512..=512)
        .flat_map(|q| (-512..=512).map(move |r| (q, r)))
        .find(|&(q, r)| {
            core.terrain_at(q, r) == Terrain::DeepWater
                && !core.runtime.occupied.contains_key(&(q, r))
        })
        .expect("the physical world has open deep water");

    assert!(core.terrain_blocks_movement(deep.0, deep.1));
    assert!(!core.walkable_hex(deep.0, deep.1));

    // Mobility follows surveying in the authored skill ladder and spends a real journey point.
    core.skills.purchased.insert(3);
    core.skills.points = 1;
    core.purchase_skill(4).unwrap();
    assert!(core.can_swim());
    assert!(core.walkable_hex(deep.0, deep.1));
    assert!(
        core.terrain_blocks_movement(deep.0, deep.1),
        "learning to swim must not make deep water a construction surface"
    );

    set_player_hex(&mut core, deep.0, deep.1);
    core.set_move_intent(1000, 0).unwrap();
    assert_eq!(core.player_step(), (PLAYER_SPEED / SWIM_SPEED_DIVISOR, 0));
    assert_eq!(core.walk_step_cost(deep, deep.0, deep.1), WALK_SWIM_COST);
}

/// The whole gesture, end to end: a click names a hex, native finds the way, and the player
/// walks it without another command being sent.
#[test]
fn a_click_routes_walks_and_replans_around_what_blocks_it() {
    let mut core = game("new-game");
    // Outside the hub's seven hexes: the hub blocks movement, so a player standing inside it
    // would be measuring collision rather than walking.
    set_player_hex(&mut core, 2, 0);
    core.walk_to(6, 0).unwrap();
    assert_eq!(core.player.walk_goal, Some(Coordinate { q: 6, r: 0 }));
    assert_route_is_walkable(&core, (2, 0), (6, 0));

    // No further input at all — the run below sends an empty batch every frame.
    for _ in 0..40 {
        core.advance("[]", 0, 5).unwrap();
        if core.player.walk_goal.is_none() {
            break;
        }
    }
    assert_eq!(world_to_axial(core.player.x, core.player.y), (6, 0));
    // Arrival ends the walk and drops the intent, so the player stops rather than drifting on.
    assert_eq!(core.player.walk_goal, None);
    assert!(core.walk_path.is_empty());
    assert_eq!((core.player.move_x, core.player.move_y), (0, 0));

    // The route goes round what blocks it. A wall the player built themselves is as real to the
    // search as a cliff is, because both answer the same `walkable_hex`.
    let mut core = legacy_band_game("new-game");
    set_player_hex(&mut core, 1, 0);
    let barrier = [(3, -1), (3, 0), (3, 1)];
    wall(&mut core, &barrier);

    core.walk_to(6, 0).unwrap();
    assert_route_is_walkable(&core, (1, 0), (6, 0));
    for cell in &core.walk_path {
        assert!(
            !barrier.contains(&(cell.q, cell.r)),
            "the route runs through the wall at {cell:?}"
        );
    }
    assert!(
        core.walk_path.len() > axial_distance((1, 0), (6, 0)) as usize,
        "a route round a wall cannot be as short as the straight line it replaces"
    );

    // Water is not scenery to a route. Shallows are walkable and are therefore never refused, but
    // `player_step` fords them at a fifth speed, so the search charges five and takes the long dry
    // way — which is the way the player would have taken, and five times faster.
    // The complaint this answers: the shortest route and the fastest route are not the same
    // route once water is on the map, and the shortest one wades.
    assert_eq!(
        WALK_SHALLOW_COST,
        WALK_STEP_COST * (PLAYER_SPEED / (PLAYER_SPEED / 5)) as u32,
        "the ford's price to the route is the fraction of speed the ford actually costs"
    );

    let mut core = legacy_band_game("new-game");
    set_player_hex(&mut core, 0, 2);
    assert_eq!(core.terrain_at(1, 2), Terrain::ShallowWater);
    assert_eq!(core.terrain_at(2, 2), Terrain::ShallowWater);

    core.walk_to(3, 2).unwrap();
    assert_route_is_walkable(&core, (0, 2), (3, 2));
    for cell in &core.walk_path {
        assert_ne!(
            core.terrain_at(cell.q, cell.r),
            Terrain::ShallowWater,
            "the route wades at {cell:?} when dry ground was cheaper"
        );
    }
    // Three hexes wading costs eleven; four hexes round the south of the water costs four. A
    // search that costed every hex the same would have returned the three, and the player would
    // have spent two of them crossing at 5 m/s.
    assert_eq!(core.walk_path.len(), 4);
    assert_eq!(axial_distance((0, 2), (3, 2)), 3);

    // Three refusals, each an event rather than a silent no-op: the player pointed at something and
    // is owed an answer about it.
    let mut core = legacy_band_game("new-game");
    set_player_hex(&mut core, 1, 0);
    let standing = (core.player.x, core.player.y);

    // Ground the player cannot stand on at all.
    assert_eq!(core.terrain_at(9, 0), Terrain::Cliff);
    assert!(core.walk_to(9, 0).unwrap_err().contains("No way through"));

    // Ground that is fine in itself and walled off from the world.
    wall(&mut core, &ring(4, 0));
    assert!(core.walkable_hex(4, 0));
    assert!(core.walk_to(4, 0).unwrap_err().contains("No way through"));

    // Further than a click is allowed to mean.
    assert!(core
        .walk_to(1 + MAX_WALK_DISTANCE + 1, 0)
        .unwrap_err()
        .contains("too far"));

    assert_eq!(core.player.walk_goal, None);
    assert_eq!((core.player.x, core.player.y), standing);
    assert_eq!((core.player.move_x, core.player.move_y), (0, 0));

    // Clicking your own feet cancels rather than searching, which is the useful reading of it and
    // the cheapest.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.walk_to(6, 0).unwrap();
    assert!(core.player.walk_goal.is_some());
    core.walk_to(1, 0).unwrap();
    assert_eq!(core.player.walk_goal, None);
    assert!(core.walk_path.is_empty());

    // The moment the player touches the movement keys they are driving. Both the key going down
    // and the key coming back up cancel, because both are the host saying the player is steering.
    for batch in [IDLE_MOVE_EAST, IDLE] {
        let mut core = game("new-game");
        set_player_hex(&mut core, 1, 0);
        core.walk_to(6, 0).unwrap();
        core.advance("[]", 0, 5).unwrap();
        assert!(
            core.player.walk_goal.is_some(),
            "{batch} should interrupt a walk in flight"
        );

        core.advance(batch, 0, 1).unwrap();
        assert_eq!(core.player.walk_goal, None);
        assert!(core.walk_path.is_empty());
    }

    // A wall raised across a live route is answered when it is raised, not when the player reaches
    // it — the drawn ribbon and the walk are the same path, so a stale one would be the host
    // promising a walk that cannot happen.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.walk_to(6, 0).unwrap();
    assert_eq!(
        core.walk_path.len(),
        5,
        "the clear route is the straight one"
    );

    core.advance("[]", 0, 3).unwrap();
    wall(&mut core, &[(3, -1), (3, 0), (3, 1)]);
    assert_eq!(core.player.walk_goal, Some(Coordinate { q: 6, r: 0 }));
    assert!(
        core.walk_path.len() > 5,
        "the route should have been rebuilt round the new wall"
    );

    // Now shut the destination off entirely. The goal stands until the next player step, which
    // is the one place a walk is allowed to end — and it says why.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.walk_to(4, 0).unwrap();
    wall(&mut core, &ring(4, 0));
    assert_eq!(core.player.walk_goal, Some(Coordinate { q: 4, r: 0 }));
    assert!(core.walk_path.is_empty());

    core.advance("[]", 0, 1).unwrap();
    assert_eq!(core.player.walk_goal, None);
    assert!(
        core.events.iter().any(|event| event.contains("blocked")),
        "a walk that cannot finish has to say so: {:?}",
        core.events
    );

    // Where the player is headed is state the run carries: it is hashed, it is saved, and it comes
    // back walking. The route is not saved — it is rebuilt against the world that loaded, which is
    // the only version of this that cannot come back describing a corridor that no longer exists.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    // Outside the hub's seven hexes, so the walk being saved is a walk rather than a collision.
    set_player_hex(&mut core, 2, 0);
    let idle = core.checksum();
    core.walk_to(6, 0).unwrap();
    assert_ne!(
        core.checksum(),
        idle,
        "a player walking somewhere is not the same run as one standing still"
    );

    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.player.walk_goal, Some(Coordinate { q: 6, r: 0 }));
    assert_eq!(restored.walk_path, core.walk_path);
    assert_eq!(restored.checksum(), core.checksum());

    // And it keeps going, which is the whole point of saving it.
    let mut resumed = restored;
    for _ in 0..40 {
        resumed.advance("[]", 0, 5).unwrap();
        if resumed.player.walk_goal.is_none() {
            break;
        }
    }
    assert_eq!(world_to_axial(resumed.player.x, resumed.player.y), (6, 0));

    // The search is simulation, so it answers the same way every time. Ties break on `(f, g, q, r)`
    // rather than on whatever order a heap happened to pop, which is what makes this true rather
    // than usually true.
    let batch = r#"[{"type":"walk_to","q":6,"r":0}]"#;
    let mut first = game("new-game");
    let mut second = game("new-game");
    for core in [&mut first, &mut second] {
        set_player_hex(core, 1, 0);
        wall(core, &[(3, -1), (3, 0), (3, 1)]);
        core.advance(batch, 0, 0).unwrap();
    }
    assert_eq!(first.walk_path, second.walk_path);
    for _ in 0..20 {
        first.advance("[]", 2, 5).unwrap();
        second.advance("[]", 2, 5).unwrap();
    }
    assert_eq!(first.checksum(), second.checksum());
    assert_eq!(
        (first.player.x, first.player.y),
        (second.player.x, second.player.y)
    );

    // Thinking about a route must not change the world. `terrain_at` is a pure function of the
    // parameters and the seed, and the search deliberately never calls `ensure_tile` — if it did,
    // considering a hex would survey it, and `generated_chunks` is a checksum input.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    let before = core.checksum();
    let chunks = core.generated_chunks.clone();

    let here = world_to_axial(core.player.x, core.player.y);
    for goal in [(6, 0), (0, 2), (9, 0), (1 + MAX_WALK_DISTANCE, 0)] {
        let _ = core.walk_route(here, goal);
    }
    assert_eq!(core.generated_chunks, chunks);
    assert_eq!(core.checksum(), before);
}

#[test]
fn gathering_is_bounded_by_reach_cooldown_and_what_the_hex_holds() {
    let mut core = game("new-game");
    set_player_hex(&mut core, 3, 0);
    let before = core.deposit_quantity((3, 0));
    for _ in 0..before {
        core.gather().unwrap();
        cooldown(&mut core);
    }
    assert_eq!(core.player.inventory.get(&1), Some(&before));
    assert_eq!(core.deposit_quantity((3, 0)), 0);
    assert!(core.gather().is_err());

    // A gather takes from the hex the player is standing on, wherever they stand inside it and
    // whichever way they face. The old target was pushed half a gather range along the facing and
    // then resolved to the nearest field cell, so stepping off-centre inside your own hex silently
    // moved the harvest to the neighbour ahead: the number under your feet stayed put while a
    // different hex counted down. Nothing on screen shows facing, so that was unattributable.
    for (facing_x, facing_y) in [(1000, 0), (-1000, 0), (500, 866), (-500, -866)] {
        for offset in [-880, -400, 0, 400, 880] {
            let mut core = game("new-game");
            set_player_hex(&mut core, 3, 0);
            // Field cells on both sides, so a target that drifts either way is visible.
            core.write_overlay(4, 0, 1, 20, 20);
            core.write_overlay(2, 0, 1, 20, 20);
            core.player.x += offset;
            core.player.facing_x = facing_x;
            core.player.facing_y = facing_y;
            core.gather().unwrap();
            cooldown(&mut core);
            assert_eq!(
                (
                    core.deposit_quantity((2, 0)),
                    core.deposit_quantity((3, 0)),
                    core.deposit_quantity((4, 0)),
                ),
                (20, 47, 20),
                "offset {offset} facing {facing_x},{facing_y} took from the wrong hex"
            );
        }
    }

    // Reach is exactly what an extractor on the same hex would cover, and it does not depend on
    // facing. Standing on the field takes from it; standing one step away still reaches it, which
    // is what lets a player work a field edge; two steps away is out of reach from every angle.
    for &(dq, dr) in &DIRECTIONS {
        for steps in 0..=2 {
            for facing in 0..6u8 {
                let mut core = game("new-game");
                let (x, y) = axial_world(3 + dq * steps, dr * steps);
                core.player.x = x;
                core.player.y = y;
                (core.player.facing_x, core.player.facing_y) = world_direction(facing);
                core.ensure_neighborhood(core.player.x, core.player.y);
                let reached = core.gather().is_ok();
                cooldown(&mut core);
                // One step out only reaches back if no nearer field cell outbids (3,0); the
                // rule is the shared candidate list, so ask it rather than restating it.
                let expected = core.resource_at_world(x, y) == Some((3, 0));
                assert_eq!(
                    reached && core.deposit_quantity((3, 0)) == 47,
                    expected,
                    "step {steps} along {dq},{dr} facing {facing}"
                );
                if steps == 2 {
                    assert_eq!(core.deposit_quantity((3, 0)), 48, "reach ran past one hex");
                }
            }
        }
    }

    // The cooldown between two gathers runs on the player's clock, not the factory's. It used to
    // be decremented once per simulation tick, so pausing froze it outright — one gather, then
    // "action cooling down" for as long as the factory stayed paused — and the harvest rate
    // otherwise rode the speed setting, six times faster at 60 tps than at 4.
    let mut core = game("new-game");
    set_player_hex(&mut core, 3, 0);
    core.gather().unwrap();
    assert!(core.gather().is_err(), "the cooldown has to hold at all");
    // The factory is paused for the whole of this: not one tick is advanced.
    let total = core.player.action_cooldown;
    assert!(total > 1, "iron ore is slower than a single step");
    core.advance_player_steps(total - 1);
    assert!(core.gather().is_err(), "cleared early");
    core.advance_player_steps(1);
    // The step that cleared the counter is the step the first swing landed on: one unit, paid
    // at the end of the work rather than at the start of it.
    assert_eq!(core.deposit_quantity((3, 0)), 47);
    core.gather().unwrap();
    cooldown(&mut core);
    assert_eq!(core.tick, 0);
    assert_eq!(core.deposit_quantity((3, 0)), 46);

    // And running the factory on its own no longer clears it.
    let mut core = game("new-game");
    set_player_hex(&mut core, 3, 0);
    core.gather().unwrap();
    core.tick_many(240);
    assert!(
        core.gather().is_err(),
        "factory time paid the player's debt"
    );

    // The first harvest of a session used to be free. The counter was a debt charged *after* an
    // instant take, so the button banked a unit the moment it went down and only then made the
    // player wait — the one gather in a run that cost nothing was the first one, and the ring drew
    // a wait for work that had already been paid out.
    //
    // It now measures the swing itself. Nothing moves until the work is spent, the deposit and the
    // pack change in the same step, and a swing the player walks out of reach of pays nothing —
    // harvesting is work over a hex, not a toll on the hex you were last standing beside.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    set_player_hex(&mut core, 3, 0);
    let initial = core.deposit_quantity((3, 0));

    core.gather().unwrap();
    let work = core.player.action_cooldown;
    assert!(work > 1, "iron ore is more than a single step of work");
    assert_eq!(
        (
            core.player.inventory.get(&IRON_ORE),
            core.deposit_quantity((3, 0))
        ),
        (None, initial),
        "the press alone moved something"
    );
    core.advance_player_steps(work - 1);
    assert_eq!(
        (
            core.player.inventory.get(&IRON_ORE),
            core.deposit_quantity((3, 0))
        ),
        (None, initial),
        "paid before the work was finished"
    );
    core.advance_player_steps(1);
    assert_eq!(
        (
            core.player.inventory.get(&IRON_ORE).copied(),
            core.deposit_quantity((3, 0))
        ),
        (Some(1), initial - 1),
        "the deposit and the pack move together, at the end"
    );

    // A swing carries across a save, because the counter that is running is saved and what it
    // is working on has to be saved with it.
    core.gather().unwrap();
    core.advance_player_steps(work / 2);
    let save = core.save_string().unwrap();
    let mut resumed = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    resumed.advance_player_steps(work);
    core.advance_player_steps(work);
    assert_eq!(
        (
            resumed.player.inventory.get(&IRON_ORE).copied(),
            resumed.deposit_quantity((3, 0))
        ),
        (Some(2), initial - 2),
        "a resumed swing has to land"
    );
    assert_eq!(
        resumed.checksum(),
        core.checksum(),
        "the resumed swing and the uninterrupted one are the same run"
    );

    // Walking out of reach cancels it. Reach is the same predicate the start asked, so a swing
    // can never land on a cell an extractor standing here could not work.
    core.gather().unwrap();
    core.advance_player_steps(work / 2);
    set_player_hex(&mut core, 9, 0);
    core.advance_player_steps(work);
    assert_eq!(
        (
            core.player.inventory.get(&IRON_ORE).copied(),
            core.deposit_quantity((3, 0))
        ),
        (Some(2), initial - 2),
        "a harvest the player walked away from still paid"
    );
}
