use super::*;

#[test]
fn placement_and_drag_build_exactly_what_the_rules_allow_and_undo_takes_it_back() {
    let mut core = legacy_band_game("new-game");
    core.player.inventory.insert(1, 100);
    core.player.inventory.insert(3, 100);
    core.player.inventory.insert(24, 100);
    assert!(core.place(2, 0, 2, 0, None).unwrap_err().contains("locked"));
    core.researched.extend([1, 2, 3, 4]);
    assert!(core
        .place(2, 1, 2, 0, None)
        .unwrap_err()
        .contains("environment"));
    assert!(core
        .place(20, 20, 2, 0, None)
        .unwrap_err()
        .contains("range"));
    core.player.inventory.clear();
    assert!(core
        .place(2, 0, 2, 0, None)
        .unwrap_err()
        .contains("Transport kit"));
    core.player.inventory.insert(11, 8);
    core.player.inventory.insert(19, 7);
    // Extractor wants plate, a gear and timber; this hand is holding the first two. Naming the
    // missing item is the message; "construction cost is not available" did not say which.
    assert!(core.place(3, 0, 1, 0, None).unwrap_err().contains("Timber"));
    core.player.inventory.clear();
    core.player.inventory.insert(24, 3);
    core.place(2, 0, 2, 0, None).unwrap();
    assert!(core
        .place(2, 0, 2, 0, None)
        .unwrap_err()
        .contains("occupied"));
    // Occupied foundation plus the reserved growth hex both have to be constructible, so the
    // empty-deposit case is asked of a pad that is inland of the water that refused (2, 1).
    let deposit = core.place(4, -3, 1, 0, None).unwrap_err();
    assert!(deposit.contains("deposit"), "{deposit}");
    set_player_hex(&mut core, 100, 100);
    core.player.inventory.insert(24, 2);
    let checksum_before_preview = core.checksum();
    assert!(core.placement_legality(101, 100, 2, 0, None, true).is_ok());
    assert_eq!(core.checksum(), checksum_before_preview);
    assert!(core
        .placement_legality(100, 100, 2, 0, None, true)
        .unwrap_err()
        .contains("player"));

    // The six corner vectors are one rotational family, not six hand-written special cases.
    let corners = &TRANSPORT_DIRECTIONS[usize::from(NORTH)..];
    for index in 0..corners.len() {
        let (q, r) = corners[index];
        assert_eq!(corners[(index + 1) % corners.len()], (-r, q + r));
    }
    // The six edges keep their indices, which is what makes every saved orientation, every
    // fixture, and every existing drag mean the same thing after the table grew.
    assert_eq!(TRANSPORT_DIRECTIONS[..DIRECTIONS.len()], DIRECTIONS);
    // Adjacency stays six. A boiler must never reach two rows.
    assert_eq!(DIRECTIONS.len(), 6);

    // Every corner heading resolves symmetrically, and no target in a wide lattice window gives
    // two headings the same full two-row close. The resolver still carries an explicit tie-break.
    use OrientationAxis::{Corner, Edge};
    for &(dq, dr) in &TRANSPORT_DIRECTIONS[usize::from(NORTH)..] {
        assert_eq!(
            line_between((0, 0), (dq * 3, dr * 3), Corner),
            vec![(0, 0), (dq, dr), (dq * 2, dr * 2), (dq * 3, dr * 3)]
        );
    }
    for q in -64..=64 {
        for r in -64..=64 {
            let remaining = axial_distance((0, 0), (q, r));
            let candidates = TRANSPORT_DIRECTIONS[usize::from(NORTH)..]
                .iter()
                .filter(|&&(dq, dr)| axial_distance((dq, dr), (q, r)) == remaining - 2)
                .count();
            assert!(candidates <= 1, "corner drag tie at {q},{r}");
        }
    }
    // Bounded like every other drag.
    assert_eq!(
        line_between((0, 0), (900, -1800), Corner).len(),
        MAX_LINE_CELLS
    );
    // And the property that keeps every existing test meaningful: the edge axis is the old
    // resolver, untouched.
    for &to in &[(3, 0), (4, 1), (5, 3), (0, -6), (-3, 2)] {
        assert_eq!(line_between((0, 0), to, Edge), hex_line((0, 0), to));
    }

    // A drag resolves one turn and stays bounded.
    // A straight run along a hex axis.
    assert_eq!(
        hex_line((0, 0), (3, 0)),
        vec![(0, 0), (1, 0), (2, 0), (3, 0)]
    );
    // An off-axis run turns exactly once rather than staircasing, so a belt line between two
    // endpoints carries the fewest direction changes it can.
    assert_eq!(
        hex_line((2, 0), (4, 1)),
        vec![(2, 0), (3, 0), (4, 0), (4, 1)]
    );
    let turns = hex_line((0, 0), (5, 3))
        .windows(2)
        .filter_map(|pair| step_direction(pair[0], pair[1]))
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|step| step[0] != step[1])
        .count();
    assert_eq!(turns, 1);
    // Both endpoints are always included, and a single-cell drag is a single placement.
    assert_eq!(hex_line((-3, 2), (-3, 2)), vec![(-3, 2)]);
    // One command can only ever expand into a bounded run.
    assert_eq!(hex_line((0, 0), (900, 0)).len(), MAX_LINE_CELLS);
    assert_eq!(step_direction((0, 0), (0, 1)), Some(1));
    assert_eq!(step_direction((0, 0), (4, 4)), None);

    // One drag builds exactly what the equivalent placements build.
    // The path and per-cell headings `a_drag_resolves_one_turn_and_stays_bounded` pins, written
    // out so this test does not re-derive them from the code it is checking.
    let equivalent = [((2, 0), 0u8), ((3, 0), 0), ((4, 0), 1), ((4, 1), 1)];

    let mut dragged = game("new-game");
    dragged.researched.extend([1, 2, 3, 4]);
    dragged.player.inventory.insert(24, 100);
    dragged.place_line((2, 0), (4, 1), 2, 0, None).unwrap();

    let mut individual = game("new-game");
    individual.researched.extend([1, 2, 3, 4]);
    individual.player.inventory.insert(24, 100);
    for ((q, r), orientation) in equivalent {
        individual.place(q, r, 2, orientation, None).unwrap();
    }

    // Same world, same blueprint, same materials spent: a drag is exactly its placements.
    assert_eq!(dragged.checksum(), individual.checksum());
    assert_eq!(dragged.entities.len(), individual.entities.len());
    // The drag routed the run itself — every belt points at its successor and the last one
    // keeps the run's heading — so the player never oriented a segment by hand.
    let headings: Vec<u8> = dragged
        .entities
        .iter()
        .filter(|entity| !entity.placed.scenario_owned)
        .map(|entity| entity.placed.orientation)
        .collect();
    assert_eq!(headings, vec![0, 0, 1, 1]);
    // One drag reports one result, not one per cell.
    assert_eq!(dragged.events.last().unwrap(), "Placed 4 × Belt");

    // A drag builds what it legally can and reports why it stopped.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    // Enough for two of the four cells the drag covers.
    core.player.inventory.insert(24, 2);
    core.place_line((2, 0), (4, 1), 2, 0, None).unwrap();
    assert_eq!(
        core.entities
            .iter()
            .filter(|entity| !entity.placed.scenario_owned)
            .count(),
        2
    );
    assert_eq!(core.player.inventory.get(&24).copied().unwrap_or(0), 0);
    // Running out of materials part-way is reported, and what was affordable still stands.
    assert!(core
        .events
        .iter()
        .any(|event| event.contains("Transport kit")));

    // A drag that can place nothing at all fails as the single placement would have.
    let mut empty = game("new-game");
    empty.researched.extend([1, 2, 3, 4]);
    assert!(empty
        .place_line((2, 0), (4, 1), 2, 0, None)
        .unwrap_err()
        .contains("Transport kit"));
    assert!(empty
        .entities
        .iter()
        .all(|entity| entity.placed.scenario_owned));

    // A drag preview is what the drag builds.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    // Materials for two of the four cells, so the preview has to show the run stopping.
    core.player.inventory.insert(24, 2);

    let preview = core.line_preview((2, 0), (4, 1), 2, 0, None);
    assert_eq!(preview.len(), 4);
    let promised: Vec<(i32, i32, u8)> = preview
        .iter()
        .filter(|cell| cell.legal)
        .map(|cell| (cell.q, cell.r, cell.orientation))
        .collect();
    assert_eq!(promised.len(), 2);
    // The preview spends materials as it walks, so it marks the exact cell the run stops at
    // rather than implying the whole line is affordable.
    assert!(!preview[2].legal && !preview[3].legal);

    core.place_line((2, 0), (4, 1), 2, 0, None).unwrap();
    let built: Vec<(i32, i32, u8)> = core
        .entities
        .iter()
        .filter(|entity| !entity.placed.scenario_owned)
        .map(|entity| (entity.placed.q, entity.placed.r, entity.placed.orientation))
        .collect();
    assert_eq!(built, promised);

    // Removal previews the same way: only cells actually holding something removable.
    let erasable = core.erase_line_preview((2, 0), (4, 1));
    assert_eq!(
        erasable
            .iter()
            .filter(|cell| cell.legal)
            .map(|cell| (cell.q, cell.r))
            .collect::<Vec<_>>(),
        vec![(2, 0), (3, 0)]
    );

    // A belt drag routes around an occupied hex.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    stock_for(&mut core, 4, 1);
    core.player.inventory.insert(24, 100);
    core.place(3, 0, 4, 0, None).unwrap();

    let preview = core.line_preview((2, 0), (4, 0), 2, 0, None);
    assert_eq!(preview.first().map(|cell| (cell.q, cell.r)), Some((2, 0)));
    assert_eq!(preview.last().map(|cell| (cell.q, cell.r)), Some((4, 0)));
    assert!(preview.iter().all(|cell| cell.legal));
    assert!(preview.iter().all(|cell| (cell.q, cell.r) != (3, 0)));
    assert!(preview.len() > 3, "the obstacle requires a shortest detour");

    let promised: Vec<(i32, i32, u8)> = preview
        .iter()
        .map(|cell| (cell.q, cell.r, cell.orientation))
        .collect();
    core.place_line((2, 0), (4, 0), 2, 0, None).unwrap();
    let built: Vec<(i32, i32, u8)> = core
        .entities
        .iter()
        .filter(|entity| entity.kind == BuildingKind::Belt)
        .map(|entity| (entity.placed.q, entity.placed.r, entity.placed.orientation))
        .collect();
    assert_eq!(built, promised);

    let mut creative = game("new-game");
    creative.creative = true;
    creative.researched.extend([1, 2, 3, 4]);
    creative.place(3, 0, 4, 0, None).unwrap();
    assert!(creative
        .line_preview((2, 0), (4, 0), 2, 0, None)
        .iter()
        .all(|cell| cell.legal));

    // One drag removes the run it covers.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    core.player.inventory.insert(24, 100);
    core.place_line((2, 0), (4, 1), 2, 0, None).unwrap();
    let spent = *core.player.inventory.get(&24).unwrap();
    core.erase_line((2, 0), (4, 1)).unwrap();
    assert!(core
        .entities
        .iter()
        .all(|entity| entity.placed.scenario_owned));
    // Removal refunds through the ordinary erase path, so a built-then-removed run is free.
    assert_eq!(core.player.inventory.get(&24), Some(&(spent + 4)));
    assert_eq!(core.events.last().unwrap(), "Recovered 4 buildings");
    // A drag across empty ground reports the same refusal a single erase would.
    assert!(core
        .erase_line((2, 0), (4, 1))
        .unwrap_err()
        .contains("no building"));

    // Undo takes back the last construction through the erase path.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3, 4]);
    core.player.inventory.insert(24, 100);
    // Building opens the world around what you build, and that opening is not a construction:
    // no undo takes back ground you have already seen. So survey the far end of the drag
    // before the baseline is taken, or this test would be measuring the survey rather than
    // the undo. Under the old chunk-ring survey this happened to be unnecessary — `(4, 1)`
    // shares a chunk with `(2, 0)` — which made the omission invisible rather than correct.
    let (far_x, far_y) = axial_world(4, 1);
    core.ensure_neighborhood(far_x, far_y);
    let before = core.checksum();

    core.place(2, 0, 2, 0, None).unwrap();
    core.undo().unwrap();
    // Undo is exactly an erase of what was just built, so the world returns to where it was.
    assert_eq!(core.checksum(), before);
    assert_eq!(core.events.last().unwrap(), "Undid the last construction");

    // It unwinds a drag one construction at a time, most recent first.
    core.place_line((2, 0), (4, 1), 2, 0, None).unwrap();
    for _ in 0..4 {
        core.undo().unwrap();
    }
    assert_eq!(core.checksum(), before);
    assert!(core.undo().unwrap_err().contains("nothing to undo"));

    // A construction already removed by hand is skipped rather than undoing something else.
    core.place(2, 0, 2, 0, None).unwrap();
    core.place(3, 0, 2, 0, None).unwrap();
    core.erase(3, 0).unwrap();
    core.undo().unwrap();
    assert!(core
        .entities
        .iter()
        .all(|entity| entity.placed.scenario_owned));

    // Undo history is session state: a save carries none of it, so a restored game has nothing
    // to take back and cannot erase across a load boundary.
    core.place(2, 0, 2, 0, None).unwrap();
    let (definitions, technologies, scenarios) = catalogs();
    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert!(restored.undo_stack.is_empty());
    assert_eq!(restored.checksum(), core.checksum());
}

/// Creative is one switch with three consequences: everything is known, nothing is charged, and
/// nothing is handed back. Each is checked against the ordinary path rather than a creative-only
/// one, because the whole value of a creative test bed is that it builds the same factory.
#[test]
fn creative_unlocks_grants_resizes_and_survives_a_save() {
    let mut core = legacy_band_game("new-game");
    // A locked building with an empty pack: refused for both reasons before the switch.
    let locked = core.place(2, 0, 2, 0, None).unwrap_err();
    assert!(locked.contains("locked by research"));
    core.set_creative(true);

    let every_technology: BTreeSet<TechnologyId> = core
        .technologies
        .technologies
        .iter()
        .map(|technology| technology.id)
        .collect();
    assert_eq!(core.researched, every_technology);

    assert!(core.player.inventory.is_empty());
    core.place(2, 0, 2, 0, None).unwrap();
    assert!(
        core.player.inventory.is_empty(),
        "creative construction must not reach into the pack"
    );

    // And recovers no construction cost, so a full pack can never refuse an erase. The belt's
    // in-transit cargo still spills into the world instead of being destroyed.
    let index = core.entity_at(2, 0).unwrap();
    core.entities[index].cargo = Some(Cargo {
        item_id: 3,
        quantity: 1,
    });
    core.grant(1, core.player_room_for(1)).unwrap();
    assert_eq!(
        core.slots_used(&core.player.inventory),
        core.player.carry_slots
    );
    let full = core.player.inventory.clone();
    core.erase(2, 0).unwrap();
    assert_eq!(core.player.inventory, full);
    assert_eq!(core.ground_items[0].item_id, 3);
    assert_eq!(core.ground_items[0].quantity, 1);

    // Placement's other rules are untouched: creative is free, not lawless.
    assert!(core
        .place(2, 1, 2, 0, None)
        .unwrap_err()
        .contains("environment"));

    // Leaving creative restores the prices and the refunds. What the settlement learned stays
    // learned, because a technology is knowledge rather than a purchase.
    let mut core = game("new-game");
    core.set_creative(true);
    core.set_creative(false);
    assert_eq!(core.researched.len(), core.technologies.technologies.len());
    assert!(core.place(2, 0, 2, 0, None).unwrap_err().contains("need"));
    assert!(core.grant(1, 1).unwrap_err().contains("creative"));
    assert!(core.discard(Some(1), 1).unwrap_err().contains("creative"));
    assert!(core.set_carry_slots(40).unwrap_err().contains("creative"));

    // Granting is a route into the pack like any other, so it obeys the one carrying rule: what
    // fits arrives, what does not is not invented, and an empty grant says so rather than lying.
    let mut core = game("new-game");
    core.set_creative(true);
    let stack = core.stack_size(1);
    let slots = core.player.carry_slots;

    core.grant(1, 5).unwrap();
    assert_eq!(core.player.inventory.get(&1), Some(&5));
    // Asking for far more than the pack holds tops it up to exactly full rather than refusing.
    core.grant(1, u32::MAX).unwrap();
    assert_eq!(core.player.inventory.get(&1), Some(&(stack * slots)));
    assert_eq!(core.slots_used(&core.player.inventory), slots);
    assert!(core.grant(1, 1).unwrap_err().contains("no room"));
    assert!(core.grant(9_999, 1).unwrap_err().contains("unknown item"));

    // Zero means the whole stack; a named quantity takes that much and no more.
    core.discard(Some(1), 3).unwrap();
    assert_eq!(core.player.inventory.get(&1), Some(&(stack * slots - 3)));
    // A part-emptied stack still occupies its slot, so nothing else fits until a whole one goes.
    assert!(core.grant(3, 1).unwrap_err().contains("no room"));
    core.discard(Some(1), stack).unwrap();
    core.grant(3, 4).unwrap();
    core.discard(Some(1), 0).unwrap();
    assert_eq!(core.player.inventory.get(&1), None);
    assert_eq!(core.player.inventory.get(&3), Some(&4));
    // Clearing the pack is one command, not one per stack against a batch that holds eight.
    core.discard(None, 0).unwrap();
    assert!(core.player.inventory.is_empty());
    assert!(core.discard(None, 0).unwrap_err().contains("nothing"));

    // The pack may be widened, within bounds, and never so far down that carried stock is stranded.
    let mut core = game("new-game");
    let scenario_slots = core.player.carry_slots;
    core.set_creative(true);
    let earned_slots = core.player.carry_slots;
    assert!(earned_slots > scenario_slots);

    core.set_carry_slots(MAX_CARRY_SLOTS).unwrap();
    assert_eq!(core.player.carry_slots, MAX_CARRY_SLOTS);
    assert!(core
        .set_carry_slots(MAX_CARRY_SLOTS + 1)
        .unwrap_err()
        .contains("out of range"));
    assert!(core
        .set_carry_slots(earned_slots - 1)
        .unwrap_err()
        .contains("out of range"));

    // Narrowing under what is already carried is refused rather than dropping the difference.
    // One item per slot, one more than the researched pack holds.
    for item_id in 1..=(earned_slots as ItemId + 1) {
        core.grant(item_id, 1).unwrap();
    }
    assert!(core
        .set_carry_slots(earned_slots)
        .unwrap_err()
        .contains("too much carried"));
    core.discard(None, 0).unwrap();
    core.set_carry_slots(earned_slots).unwrap();

    // Both halves of creative are run state now, so both survive a save and both are hashed. A file
    // with either edited out no longer describes the run it came from.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    core.set_creative(true);
    core.set_carry_slots(64).unwrap();
    core.place(2, 0, 2, 0, None).unwrap();

    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert!(restored.creative);
    assert_eq!(restored.player.carry_slots, 64);
    assert_eq!(restored.checksum(), core.checksum());

    // Neither is a free field: the checksum is what makes them run state rather than a note.
    let priced = save.replace("\"creative\":true", "\"creative\":false");
    assert!(
        Core::from_save(&definitions, &technologies, &scenarios, &priced)
            .err()
            .unwrap()
            .contains("checksum")
    );
    let narrowed = save.replace("\"carry_slots\":64", "\"carry_slots\":63");
    assert!(
        Core::from_save(&definitions, &technologies, &scenarios, &narrowed)
            .err()
            .unwrap()
            .contains("checksum")
    );
    // And the range check still refuses a pack outside what any run may have.
    let absurd = save.replace("\"carry_slots\":64", "\"carry_slots\":9999");
    assert!(
        Core::from_save(&definitions, &technologies, &scenarios, &absurd)
            .err()
            .unwrap()
            .contains("invalid player or research state")
    );
}

#[test]
fn erasing_refunds_spills_and_never_leaves_an_uncompilable_graph() {
    let mut core = game("new-game");
    core.researched.insert(1);
    core.player.inventory.insert(24, 2);
    core.place(2, 0, 2, 0, None).unwrap();
    let index = core
        .entities
        .iter()
        .position(|entity| entity.placed.q == 2)
        .unwrap();
    core.entities[index].cargo = Some(Cargo {
        item_id: 3,
        quantity: 1,
    });
    core.erase(2, 0).unwrap();
    assert_eq!(core.player.inventory.get(&24), Some(&2));
    assert_eq!(core.player.inventory.get(&3), None);
    assert_eq!(
        core.ground_items,
        vec![GroundItem {
            id: 1,
            q: 2,
            r: 0,
            item_id: 3,
            quantity: 1,
            despawn_tick: GROUND_ITEM_LIFETIME_TICKS,
        }]
    );
    assert!(core.erase(0, 0).unwrap_err().contains("protected"));

    // A belt may not be built into something that can never take an item, and no such edge is
    // compiled if one arises anyway.
    //
    // The old game answered this at delivery time, which meant it never answered it at all: the
    // line looked connected, compiled an edge, and quietly backed up. The static question gets its
    // own predicate so the answer cannot change with a recipe or a contract the way `accepts_item`
    // can, construction refuses by name and by hex, and only transport is held to its heading —
    // a machine that happens to face a pole is still a perfectly good machine.
    let mut core = game("new-game");
    core.set_creative(true);
    set_player_hex(&mut core, 1, 3);
    core.place(0, 3, 12, 0, None).unwrap();

    // Heading 4 is due north on the routing table, so from (0, 4) it points straight at the
    // pole. Pointed anywhere else the same belt on the same hex is fine.
    assert!(core.placement_legality(0, 4, 2, 0, None, false).is_ok());
    let refused = core
        .placement_legality(0, 4, 2, 4, None, false)
        .unwrap_err();
    assert!(refused.contains("Pole"), "names the building: {refused}");
    assert!(refused.contains("0, 3"), "names the hex: {refused}");
    assert!(
        refused.contains("never takes items"),
        "names the reason: {refused}"
    );
    let preview = core.line_preview((0, 5), (0, 4), 2, 4, None);
    let tip = preview
        .iter()
        .find(|cell| cell.q == 0 && cell.r == 4)
        .unwrap();
    assert!(!tip.legal);
    assert!(tip.reason.as_ref().unwrap().contains("Pole at 0, 3"));
    assert!(
        core.placement_legality(0, 4, 4, 4, None, false).is_ok(),
        "a container facing the same pole is not transport and is not refused"
    );

    // Nothing about the runtime question moved: a pole was never a delivery target and still
    // is not, asked the way the tick asks it.
    let pole = core.entity_at(0, 3).unwrap();
    assert!(!core.accepts_item(pole, 3));

    // And an edge into such a target is not compiled even when one arises anyway — building
    // the pole second, where no placement rule could have refused it.
    let mut core = empty_world("new-game");
    add_test_belt(&mut core, 0, 0, 0);
    add_test_entity(&mut core, 1, 0, 12, 0);
    core.compile_graph();
    assert!(
        core.graph[0].is_empty(),
        "the belt shows no downstream rather than a connection that never delivers"
    );

    // Demolishing a building with something in it no longer stops at a full pack.
    //
    // What fits comes back, what does not falls at the site on the ordinary ground-item clock, and
    // the two together are exactly what the building held — that split is the conservation law.
    // Refusing instead was the worse trade: a full pack and a full building had no order of
    // operations that emptied either, so the building the player wanted gone simply stayed. The
    // host warns first and says the ground items are on a timer, so the loss is a decision.
    let mut core = game("new-game");
    core.researched.extend([1, 4, 12]);
    set_player_hex(&mut core, 1, 3);
    core.player.inventory.insert(16, 3);
    core.place(0, 3, 4, 0, None).unwrap();

    // Three stacks inside, and room in the pack for exactly one of them.
    let stack = core.stack_size(3);
    let index = core.entity_at(0, 3).unwrap();
    core.entities[index].inventory.insert(3, stack * 3);
    core.player.inventory.clear();
    core.player.carry_slots = 1;

    core.erase(0, 3)
        .expect("a full pack no longer blocks a demolition");
    assert_eq!(core.player.inventory.get(&3), Some(&stack));
    assert_eq!(
        core.player.inventory.get(&16),
        None,
        "the construction cost had no slot left to come back into"
    );
    assert_eq!(
        core.ground_items
            .iter()
            .map(|item| (item.item_id, item.quantity, item.despawn_tick))
            .collect::<Vec<_>>(),
        vec![
            (3, stack * 2, GROUND_ITEM_LIFETIME_TICKS),
            (16, 3, GROUND_ITEM_LIFETIME_TICKS),
        ],
        "the remainder falls at the site, on the clock the confirmation states"
    );
    assert!(
        core.events
            .iter()
            .any(|event| event.contains("would not fit your pack")),
        "and the player is told, not left to notice"
    );

    // Temporarily blocked targets still compile and allow belts.
    for definition_id in [3, 4] {
        let mut core = game("new-game");
        core.set_creative(true);
        // Clear of the composer's three hexes, which reach south and east of their anchor.
        set_player_hex(&mut core, 2, 2);
        core.place(0, 3, definition_id, 0, (definition_id == 3).then_some(1))
            .unwrap();
        let target = core.entity_at(0, 3).unwrap();
        if definition_id == 4 {
            core.entities[target].inventory.insert(3, 60);
        } else {
            core.entities[target].placed.recipe_id = None;
        }
        // The belt comes in from the north-west, on ground neither footprint stands on.
        assert!(core.placement_legality(0, 2, 2, 1, None, false).is_ok());
        core.place(0, 2, 2, 1, None).unwrap();
        let belt = core.entity_at(0, 2).unwrap();
        assert_eq!(core.graph[belt].primary(), Some(target));
    }

    // Demolition overflow round trips and can be collected.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    core.set_creative(true);
    core.place(2, 0, 4, 0, None).unwrap();
    let container = core.entity_at(2, 0).unwrap();
    core.entities[container].inventory.insert(3, 60);
    core.player.inventory.clear();
    core.player
        .inventory
        .insert(1, core.player.carry_slots * core.stack_size(1));
    core.set_creative(false);
    core.erase(2, 0).unwrap();
    let save = core.save_string().unwrap();
    let mut restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.checksum(), core.checksum());
    assert_eq!(restored.ground_items, core.ground_items);
    restored.player.inventory.clear();
    set_player_hex(&mut restored, 2, 0);
    restored.tick += 30;
    restored.player.move_x = 1;
    restored.collect_ground_items();
    assert!(restored.ground_items.is_empty());
    assert_eq!(restored.player.inventory.get(&3), Some(&60));
    assert_eq!(restored.player.inventory.get(&16), Some(&3));
}

#[test]
fn footprints_occupy_turn_reserve_and_upgrade_as_one_building() {
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3]);
    stock_for(&mut core, 3, 1);
    core.place(-3, 1, 3, 0, Some(1)).unwrap();
    let composer = core
        .snapshot()
        .buildings
        .into_iter()
        .find(|entity| entity.definition_id == 3)
        .unwrap();
    assert_eq!(
        composer.footprint,
        vec![
            Coordinate { q: -3, r: 1 },
            Coordinate { q: -2, r: 1 },
            Coordinate { q: -3, r: 2 }
        ]
    );
    assert!(core
        .place(-2, 1, 2, 0, None)
        .unwrap_err()
        .contains("footprint"));
    core.erase(-3, 2).unwrap();
    assert!(core.entity_at(-3, 1).is_none());

    // A one-hex build reach still reaches a two-cell machine from the far lobe, even when the
    // command names the anchor. Reach is the Minkowski sum of the footprint with the range disc,
    // not a disc around one of its tiles.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 3]);
    stock_for(&mut core, 3, 1);
    core.place(-3, 1, 3, 0, Some(1)).unwrap();
    // One hex of world-unit reach: beside the far cell, out of range of the anchor alone.
    core.player.build_range = HEX_X as u32;
    set_player_hex(&mut core, -3, 3);
    assert!(core.entity_at(-3, 1).is_some());
    core.erase(-3, 1).unwrap();
    assert!(core.entity_at(-3, 1).is_none());
    assert!(core.entity_at(-3, 2).is_none());
}

/// A belt at a vertex heading routes two rows, and the hexes it spans stay free. This is the
/// whole answer to north-south transport: a direction-table row, resolved by the ray-cast the
/// graph compiler already was, with no sub-hex occupancy anywhere.
#[test]
fn rotation_and_the_two_row_reach_are_priced_gated_and_angular() {
    let mut core = game("new-game");
    core.researched.extend([1, 4, 11]);
    stock_for(&mut core, 4, 1);
    core.player.inventory.insert(24, 40);

    // A belt at (0, 3) facing north reaches (1, 1) — the same world column, two rows up.
    set_player_hex(&mut core, 1, 2);
    core.place(1, 1, 4, 0, None).unwrap();
    set_player_hex(&mut core, 1, 3);
    core.place(0, 3, 2, NORTH, None).unwrap();

    let belt = core.entity_at(0, 3).unwrap();
    let container = core.entity_at(1, 1).unwrap();
    assert_eq!(
        core.graph[belt],
        Links::single(Some(container)),
        "a north-facing belt must bind to what sits two rows above it"
    );
    // The seam it spans is two ordinary hexes, and neither is occupied by anything.
    assert_eq!(core.entity_at(0, 2), None);
    assert_eq!(core.entity_at(1, 2), None);
    // So they stay buildable, and the belt never claims them for collision either.
    assert!(core.placement_legality(0, 2, 2, 0, None, true).is_ok());
    assert!(!core.building_definition(2).unwrap().blocks_movement);
    // It occupies exactly one hex.
    assert_eq!(core.entity_footprint(&core.entities[belt]).len(), 1);

    // Rotation on the any axis walks all twelve headings once each, in angular order.
    //
    // The point of a single belt definition is that `R` nudges a heading by 30°, not that it
    // cycles a table. So this checks the *world vectors*, not the indices: consecutive headings
    // turn one twelfth of a circle clockwise, and twelve presses return to where they started.
    let mut core = game("new-game");
    core.researched.extend([1, 11]);
    core.player.inventory.insert(24, 40);
    set_player_hex(&mut core, 1, 3);
    core.place(0, 3, 2, 0, None).unwrap();

    let heading = |core: &Core| {
        core.entities[core.entity_at(0, 3).unwrap()]
            .placed
            .orientation
    };
    // Pointy-top axial, at unit size: a hex at (q, r) sits at `x = √3·(q + r/2)`, `y = 1.5·r`,
    // with `y` running south. The world angle of a heading is the angle of the vector it moves
    // along, growing clockwise from due east.
    let angle = |orientation: u8| {
        let (dq, dr) = TRANSPORT_DIRECTIONS[usize::from(orientation)];
        let (dq, dr) = (f64::from(dq), f64::from(dr));
        (1.5 * dr).atan2(3f64.sqrt() * (dq + dr / 2.0))
    };

    let mut seen = vec![heading(&core)];
    for _ in 0..11 {
        core.rotate(0, 3, false).unwrap();
        let now = heading(&core);
        let step = (angle(now) - angle(*seen.last().unwrap())).rem_euclid(std::f64::consts::TAU);
        assert!(
            (step - std::f64::consts::TAU / 12.0).abs() < 1e-9,
            "one press turned {step} radians, not 30°"
        );
        seen.push(now);
    }
    seen.sort_unstable();
    assert_eq!(seen, (0..12).collect::<Vec<u8>>(), "every heading, once");

    core.rotate(0, 3, false).unwrap();
    assert_eq!(heading(&core), 0, "twelve presses return to the start");
    core.rotate(0, 3, true).unwrap();
    assert_eq!(
        heading(&core),
        7,
        "and reverse rotation is the inverse press: 30° back from due east"
    );

    // Rotation offers a heading on the same terms `place` does: researched, and paid for.
    //
    // A belt bought at an edge heading and turned onto a vertex one would otherwise be the two-row
    // reach at the price of the short step — the exact dominance `corner_construction_cost` exists
    // to prevent — and `R` pressed before the research would hand it over for nothing at all.
    let mut core = game("new-game");
    core.researched.insert(1);
    core.player.inventory.insert(24, 8);
    set_player_hex(&mut core, 1, 3);
    core.place(0, 3, 2, 0, None).unwrap();

    let heading = |core: &Core| {
        core.entities[core.entity_at(0, 3).unwrap()]
            .placed
            .orientation
    };
    let kits = |core: &Core| core.player.inventory.get(&24).copied().unwrap_or(0);
    let paid = kits(&core);

    // Unresearched, `R` walks the six edges and steps straight over the vertex headings between
    // them, so the reach is not something a key the player already has can reach.
    for expected in 1..=5u8 {
        core.rotate(0, 3, false).unwrap();
        assert_eq!(heading(&core), expected);
    }
    core.rotate(0, 3, false).unwrap();
    assert_eq!(heading(&core), 0, "six presses close the edge ring");
    assert_eq!(kits(&core), paid, "and none of them cost anything");

    core.researched.insert(11);
    core.rotate(0, 3, false).unwrap();
    assert_eq!(
        heading(&core),
        NORTH + 2,
        "researched, the vertex heading is the very next one"
    );
    assert_eq!(
        kits(&core),
        paid - 1,
        "and turning onto it is charged the difference"
    );
    core.rotate(0, 3, true).unwrap();
    assert_eq!(heading(&core), 0);
    assert_eq!(
        kits(&core),
        paid,
        "turning back off it returns that difference"
    );

    // The difference is a real price, so a pack that cannot cover it is refused — and the belt
    // is left facing where it was rather than turned half way onto a heading nobody paid for.
    core.player.inventory.remove(&24);
    assert!(core.rotate(0, 3, false).unwrap_err().contains("need"));
    assert_eq!(heading(&core), 0);

    // Orientation is an axis the definition owns, and on the any axis that axis prices and gates
    // itself. The two-row reach costs what it covers and waits behind its own research, which is
    // what lets a belt and a riser be one building without the reach being free.
    let mut core = game("new-game");
    core.researched.extend([1]);
    core.player.inventory.insert(24, 40);
    set_player_hex(&mut core, 1, 3);

    // The belt's own unlock is done, so what refuses a vertex heading is the corner gate alone.
    assert!(core.placement_legality(0, 3, 2, 0, None, true).is_ok());
    assert!(core
        .placement_legality(0, 3, 2, NORTH, None, true)
        .unwrap_err()
        .contains("locked"));
    core.researched.insert(11);
    assert!(core.placement_legality(0, 3, 2, NORTH, None, true).is_ok());

    // An edge-only definition still refuses the vertex headings outright, by range.
    assert!(core
        .placement_legality(0, 3, 4, NORTH, None, true)
        .unwrap_err()
        .contains("oriented in 0..6"));

    // And the price is a data row, not a mechanism: the two-row heading simply costs more.
    let belt = core.building_definition(2).unwrap();
    let edge = belt.cost_at(0).to_vec();
    let corner = belt.cost_at(NORTH).to_vec();
    assert_ne!(edge, corner, "the reach a corner buys is not free");
    assert_eq!(
        corner.iter().map(|cost| cost.quantity).sum::<u32>(),
        edge.iter().map(|cost| cost.quantity).sum::<u32>() * 2,
        "a corner belt costs twice the belt, the way the riser's own row used to say"
    );

    // No definition needs a multi-cell corner footprint yet, so that untested combination is
    // still refused at load — for anything that may face a corner, not only for corner-only.
    let (mut definitions, _, _) = catalogs();
    let index = definitions
        .buildings
        .iter()
        .position(|building| building.id == 2)
        .unwrap();
    definitions.buildings[index]
        .footprint
        .push(Coordinate { q: 1, r: 0 });
    assert!(validate_definitions(&definitions)
        .unwrap_err()
        .contains("two-row period"));

    // And an any-axis definition that gates none of its headings is refused too, which is what
    // keeps the reach a research step rather than a property of the first belt of the game.
    let (mut definitions, _, _) = catalogs();
    let index = definitions
        .buildings
        .iter()
        .position(|building| building.id == 2)
        .unwrap();
    definitions.buildings[index].corner_technology_id = None;
    assert!(validate_definitions(&definitions)
        .unwrap_err()
        .contains("gates none of them"));
}
