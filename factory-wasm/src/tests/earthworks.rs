use super::*;

/// Every cell within `radius` steps of the anchor, as definition-relative offsets.
fn disc_offsets(radius: i32) -> Vec<(i32, i32)> {
    let mut cells = Vec::new();
    for q in -radius..=radius {
        for r in -radius..=radius {
            if axial_distance((0, 0), (q, r)) <= radius {
                cells.push((q, r));
            }
        }
    }
    cells
}

/// Give a live definition a footprint the shipped catalogue does not contain.
///
/// Phase 8 reauthors thirty buildings into multi-cell plants; the machinery that has to hold
/// them is being proved here first, against shapes no `definitions.json` carries yet. Editing
/// the catalogue in the Core is what `a_footprint_needs_ground_no_steeper_than_a_walk_can_climb`
/// already does, and it keeps the shipped file the subject of its own tests.
fn set_test_footprint(core: &mut Core, definition_id: DefinitionId, cells: &[(i32, i32)]) {
    core.definitions
        .buildings
        .iter_mut()
        .find(|building| building.id == definition_id)
        .expect("a building to reshape")
        .footprint = cells.iter().map(|&(q, r)| Coordinate { q, r }).collect();
}

fn set_test_envelope(core: &mut Core, definition_id: DefinitionId, cells: &[(i32, i32)]) {
    core.definitions
        .buildings
        .iter_mut()
        .find(|building| building.id == definition_id)
        .expect("a building to reshape")
        .service_envelope = cells.iter().map(|&(q, r)| Coordinate { q, r }).collect();
}

fn set_test_clearance(core: &mut Core, definition_id: DefinitionId, cells: &[(i32, i32)]) {
    core.definitions
        .buildings
        .iter_mut()
        .find(|building| building.id == definition_id)
        .expect("a building to reshape")
        .overhead_clearance = cells.iter().map(|&(q, r)| Coordinate { q, r }).collect();
}

fn set_test_foundation(core: &mut Core, definition_id: DefinitionId, class: FoundationClass) {
    core.definitions
        .buildings
        .iter_mut()
        .find(|building| building.id == definition_id)
        .expect("a building to reshape")
        .foundation_class = class;

    // The two-ring hexagon is the largest shape a definition may claim, and standing one is not a
    // special case: all nineteen cells enter the occupancy index, the snapshot publishes all
    // nineteen, and an erase aimed at the rim takes the whole building.
    let mut core = ground_world();
    core.researched.extend([1, 2, 3]);
    stock_for(&mut core, 3, 1);
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 0, 6);
    let cells = disc_offsets(2);
    assert_eq!(cells.len(), MAX_FOOTPRINT_CELLS);
    set_test_footprint(&mut core, 3, &cells);

    core.place(-4, 0, 3, 0, Some(1)).unwrap();
    let index = core.entity_at(-4, 0).expect("the plant stands");
    for &(q, r) in &cells {
        assert_eq!(
            core.entity_at(-4 + q, r),
            Some(index),
            "cell ({q}, {r}) belongs to the plant"
        );
    }
    let published = core
        .snapshot()
        .buildings
        .into_iter()
        .find(|entity| entity.definition_id == 3)
        .expect("the plant is published");
    assert_eq!(published.footprint.len(), MAX_FOOTPRINT_CELLS);

    // The rim, not the anchor: an erase names a hex the player can see, and every hex the
    // building covers is that building.
    core.erase(-6, 0).unwrap();
    assert!(cells
        .iter()
        .all(|&(q, r)| core.entity_at(-4 + q, r).is_none()));

    // A multi-cell footprint turns with its heading. Rotation by whole sixths is the only turn
    // this lattice has, which is why the validator keeps the twelve-heading transport axis
    // single-cell: a thirty-degree turn is not a symmetry of the grid and could not land a second
    // cell on a hex at all.
    let mut core = ground_world();
    core.researched.extend([1, 2, 3]);
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 0, 6);
    let offsets = [(0, 0), (1, 0), (2, 0), (0, 1)];
    set_test_footprint(&mut core, 3, &offsets);

    let mut shapes: BTreeSet<Vec<(i32, i32)>> = BTreeSet::new();
    for orientation in 0..6u8 {
        stock_for(&mut core, 3, 1);
        core.place(-4, 0, 3, orientation, Some(1)).unwrap();
        let index = core.entity_at(-4, 0).expect("the plant stands");
        let standing: Vec<(i32, i32)> = core
            .entity_footprint(&core.entities[index])
            .into_iter()
            .map(|cell| (cell.q, cell.r))
            .collect();
        let expected: Vec<(i32, i32)> = offsets
            .iter()
            .map(|&(q, r)| {
                let turned = rotate_coordinate(Coordinate { q, r }, orientation);
                (-4 + turned.q, turned.r)
            })
            .collect();
        assert_eq!(standing, expected, "heading {orientation}");
        let mut sorted = standing;
        sorted.sort();
        shapes.insert(sorted);
        core.erase(-4, 0).unwrap();
    }
    assert_eq!(shapes.len(), 6, "six headings, six distinct shapes");

    // The ceiling and the contiguity rule are properties of the catalogue, so they are checked
    // where a definition file is read rather than where a building is placed.
    let shaped = |cells: Vec<(i32, i32)>| {
        let mut definitions: DefinitionsInput = serde_json::from_str(DEFINITIONS).unwrap();
        definitions
            .buildings
            .iter_mut()
            .find(|building| building.id == 3)
            .expect("the composer")
            .footprint = cells
            .into_iter()
            .map(|(q, r)| Coordinate { q, r })
            .collect();
        definitions
    };

    assert_eq!(validate_definitions(&shaped(disc_offsets(2))), Ok(()));

    let mut oversized = disc_offsets(2);
    // Contiguous, and one cell past the ceiling: the shape is legal and only the size is not.
    oversized.push((3, 0));
    assert!(validate_definitions(&shaped(oversized))
        .unwrap_err()
        .contains("invalid footprint"));

    assert!(validate_definitions(&shaped(vec![(0, 0), (3, 0)]))
        .unwrap_err()
        .contains("disconnected pieces"));

    // A definition may not reserve a cell it occupies or disconnect.
    let mutate = |edit: fn(&mut BuildingDefinition)| {
        let mut definitions: DefinitionsInput = serde_json::from_str(DEFINITIONS).unwrap();
        let building = definitions
            .buildings
            .iter_mut()
            .find(|building| building.id == 4)
            .expect("the container");
        edit(building);
        definitions
    };

    assert_eq!(
        validate_definitions(&mutate(|building| {
            building.service_envelope = vec![Coordinate { q: 1, r: 0 }];
        })),
        Ok(())
    );
    assert!(validate_definitions(&mutate(|building| {
        building.service_envelope = vec![Coordinate { q: 0, r: 0 }];
    }))
    .unwrap_err()
    .contains("already occupies"));
    assert!(validate_definitions(&mutate(|building| {
        building.service_envelope = vec![Coordinate { q: 3, r: 0 }];
    }))
    .unwrap_err()
    .contains("disconnected pieces"));
    assert!(validate_definitions(&mutate(|building| {
        building.overhead_clearance = vec![Coordinate { q: 1, r: 0 }];
        building.service_envelope = vec![Coordinate { q: 1, r: 0 }];
    }))
    .unwrap_err()
    .contains("envelope and clearance"));

    // A taller tier may take more ground than the one it replaces, and it keeps every port it
    // had.
    //
    // That falls out of the growth rule rather than being enforced a second time. An output ray
    // binds to the first cell off the footprint, so the only growth that could take a port away
    // is growth into the very hex the ray binds at — and that hex is occupied by the thing being
    // fed, which is exactly what the check refuses. A building can gain an adjacency by getting
    // bigger; it cannot lose one.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 12]);
    for item_id in [1, 3, 6, 11, 16, 19, 20] {
        core.player.inventory.insert(item_id, 60);
    }
    core.player.carry_slots = 99;
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 3, 2);
    // A superset of the extractor's own two hexes, growing south onto free ground.
    set_test_footprint(&mut core, 19, &[(0, 0), (1, 0), (0, 1)]);

    stock_for(&mut core, 2, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    // The extractor stands on (3, 0) and (4, 0), so the hex its output ray binds at is the
    // first one past its own eastern cell.
    core.place(5, 0, 2, 0, None).unwrap();
    let extractor = core.entity_at(3, 0).expect("the extractor stands");
    let belt = core.entity_at(5, 0).expect("the belt stands");
    let fed = core.entities[belt].id;
    assert_eq!(
        core.graph[extractor].primary(),
        Some(belt),
        "the extractor feeds the belt in front of it"
    );

    core.upgrade(3, 0).unwrap();
    let extractor = core.entity_at(3, 0).expect("the deeper extractor stands");
    assert_eq!(core.entities[extractor].placed.definition_id, 19);
    assert_eq!(
        core.entity_at(3, 1),
        Some(extractor),
        "the taller tier took the free hex beside it"
    );
    let target = core.graph[extractor]
        .primary()
        .expect("it is still feeding something");
    assert_eq!(
        core.entities[target].id, fed,
        "and it is the same belt it was feeding"
    );

    // The growth check is one atomic question asked before anything is charged or written. A tier
    // that cannot fit leaves the building, the neighbour in its way and the player's pack exactly
    // as they were.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 12]);
    for item_id in [1, 3, 6, 11, 16, 19, 20] {
        core.player.inventory.insert(item_id, 60);
    }
    core.player.carry_slots = 99;
    reach(&mut core);
    set_player_hex(&mut core, 3, 2);
    // A superset of the extractor's own two hexes, growing east into the belt it feeds.
    set_test_footprint(&mut core, 19, &[(0, 0), (1, 0), (2, 0)]);

    stock_for(&mut core, 2, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    core.place(5, 0, 2, 0, None).unwrap();
    let extractor = core.entity_at(3, 0).expect("the extractor stands");
    let belt = core.entity_at(5, 0).expect("the belt stands");
    let before = core.player.inventory.clone();

    let refusal = core.upgrade(3, 0).unwrap_err();
    assert!(refusal.contains("needs more room"), "{refusal}");
    assert_eq!(core.entities[extractor].placed.definition_id, 1);
    assert_eq!(
        core.entity_at(5, 0),
        Some(belt),
        "the neighbour is untouched"
    );
    assert_eq!(
        core.player.inventory, before,
        "a refused upgrade is not charged"
    );

    // Ground the pair could not stand on together is the same refusal, asked of the whole
    // enlarged footprint rather than of the cell being grown onto.
    core.erase(5, 0).unwrap();
    core.set_creative(true);
    reach(&mut core);
    for _ in 0..MAX_GRADE_STEPS {
        core.edit_ground(&ground_edit(5, 0, GroundAction::Lower))
            .unwrap();
    }
    let refusal = core.upgrade(3, 0).unwrap_err();
    assert!(refusal.contains("level a pad"), "{refusal}");
    assert_eq!(core.entities[extractor].placed.definition_id, 1);

    // Occupied foundation, service envelope and overhead clearance are three different claims.
    //
    // Envelope is reserved empty ground: neighbours cannot occupy it, belts included, but the
    // player can walk through. Clearance is air: a belt may pass under a rotor, a machine may not.
    // Neither claim enters the occupancy index, so output rays still bind at the first occupied
    // cell off the hull.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    set_test_envelope(&mut core, 4, &[(1, 0)]);
    set_test_clearance(&mut core, 17, &[(1, 0)]);
    set_test_footprint(&mut core, 17, &[(0, 0)]);

    core.place(0, 0, 4, 0, None).unwrap();
    let container = core.entity_at(0, 0).expect("the crate stands");
    assert_eq!(core.entity_at(1, 0), None, "envelope is not occupancy");
    assert!(
        core.walkable_hex(1, 0),
        "the player can walk the reserved service hex"
    );
    let reserved = core.place(1, 0, 4, 0, None).unwrap_err();
    assert!(
        reserved.contains("reserved around the container"),
        "{reserved}"
    );
    let belt_on_envelope = core.place(1, 0, 2, 0, None).unwrap_err();
    assert!(
        belt_on_envelope.contains("reserved around the container"),
        "{belt_on_envelope}"
    );
    assert_eq!(core.entity_at(0, 0), Some(container));

    core.erase(0, 0).unwrap();
    core.place(3, 0, 17, 0, None).unwrap();
    let turbine = core.entity_at(3, 0).expect("the turbine stands");
    assert_eq!(core.entity_at(4, 0), None, "clearance is not occupancy");
    assert!(
        core.walkable_hex(4, 0),
        "the ground under a rotor stays open"
    );
    core.place(4, 0, 2, 0, None).unwrap();
    assert!(
        core.entity_at(4, 0).is_some(),
        "a belt may pass under the rotor"
    );
    core.erase(4, 0).unwrap();
    let machine = core.place(4, 0, 4, 0, None).unwrap_err();
    assert!(machine.contains("overhead clearance"), "{machine}");
    assert_eq!(core.entity_at(3, 0), Some(turbine));

    // An upgrade into a cell reserved at placement does not re-ask occupancy: the envelope held
    // it empty. Growing outside that envelope is still the atomic check.
    let mut core = game("new-game");
    core.researched.extend([1, 2, 12]);
    for item_id in [1, 3, 6, 11, 16, 19, 20] {
        core.player.inventory.insert(item_id, 60);
    }
    core.player.carry_slots = 99;
    core.player.build_range = 1 << 20;
    set_player_hex(&mut core, 3, 2);
    set_test_footprint(&mut core, 19, &[(0, 0), (1, 0), (0, 1)]);

    stock_for(&mut core, 2, 1);
    core.place(3, 0, 1, 0, None).unwrap();
    let extractor = core.entity_at(3, 0).expect("the extractor stands");
    assert_eq!(
        core.entity_at(3, 1),
        None,
        "the reserved growth hex is not occupied yet"
    );
    core.upgrade(3, 0).unwrap();
    assert_eq!(core.entities[extractor].placed.definition_id, 19);
    assert_eq!(
        core.entity_at(3, 1),
        Some(extractor),
        "the taller tier took the hex its envelope reserved"
    );
}

/// The whole surface contract in one pass: a preview costs nothing and moves nothing, a commit
/// spends exactly the declared bill, stripping the surface hands the same bill back, undo is
/// priced by the same arithmetic in reverse, and repainting what is already there is refused
/// rather than charged.
#[test]
fn ground_works_conserve_spoil_gate_routes_and_survive_a_save() {
    let mut core = ground_world();
    let gravel = item_id(&core, "gravel");
    core.player.inventory = BTreeMap::from([(gravel, 6)]);
    let initial = core.player.inventory.clone();
    let checksum = core.checksum();

    let edit = GroundEdit {
        to_q: 2,
        shape: GroundShape::Path,
        ..ground_edit(0, 0, GroundAction::Pave)
    };
    let preview = core.ground_preview(&edit);
    assert_eq!(preview.error, None);
    assert_eq!(preview.changes, 3);
    assert_eq!(
        preview.cost,
        vec![Ingredient {
            item_id: gravel,
            quantity: 3
        }]
    );
    assert_eq!(preview.cut, 0);
    assert_eq!(preview.fill, 0);
    assert_eq!(preview.covers, 0);
    assert_eq!(core.checksum(), checksum, "a preview changes nothing");
    assert_eq!(core.player.inventory, initial);

    core.edit_ground(&edit).unwrap();
    assert_eq!(core.ground.len(), 3);
    assert_eq!(core.player.inventory, BTreeMap::from([(gravel, 3)]));
    assert_eq!(core.surface_at(1, 0), 2);
    assert_eq!(core.movement_factor_at(1, 0), 120);
    assert_ne!(core.checksum(), checksum);

    // Repainting the same surface is a no-op, and a no-op costs nothing.
    let idle = core.ground_preview(&edit);
    assert_eq!(idle.changes, 0);
    assert!(idle.cost.is_empty());
    assert!(core.edit_ground(&edit).unwrap_err().contains("nothing"));
    assert_eq!(core.player.inventory, BTreeMap::from([(gravel, 3)]));

    let clear = GroundEdit {
        action: GroundAction::Clear,
        ..edit.clone()
    };
    assert_eq!(
        core.ground_preview(&clear).refund,
        vec![Ingredient {
            item_id: gravel,
            quantity: 3
        }]
    );
    core.edit_ground(&clear).unwrap();
    assert!(
        core.ground.is_empty(),
        "untreated ground leaves the overlay"
    );
    assert_eq!(core.player.inventory, initial);
    assert_eq!(core.checksum(), checksum, "the world came back exactly");

    // Undo re-lays what the clear took up, and buys it again at the same price.
    core.undo_ground().unwrap();
    assert_eq!(core.ground.len(), 3);
    assert_eq!(core.player.inventory, BTreeMap::from([(gravel, 3)]));
    core.undo_ground().unwrap();
    assert!(core.ground.is_empty());
    assert_eq!(core.player.inventory, initial);
    assert_eq!(core.checksum(), checksum);

    // Fill is dug, never conjured. This is the exploit check: raising ground with an empty ledger
    // is refused, the ledger conserves exactly one step per step in both directions, undo restores
    // the count the edit found rather than minting one, and neither the grade bound nor the ledger
    // can be walked past by repeating the edit.
    let mut core = ground_world();
    let checksum = core.checksum();
    assert_eq!(core.spoil, 0);

    let raise = ground_edit(0, 0, GroundAction::Raise);
    let refusal = core.ground_preview(&raise).error;
    assert!(
        refusal.as_deref().is_some_and(|m| m.contains("spoil")),
        "{refusal:?}"
    );
    assert!(core.edit_ground(&raise).is_err());
    assert_eq!(core.checksum(), checksum);

    let lower = ground_edit(3, 0, GroundAction::Lower);
    let step = core.grade_step_delta(1) as u64;
    let edits = u64::try_from(scale::EARTHWORK_LIMIT_QUANTA).unwrap() / step;
    for n in 1..=edits {
        core.edit_ground(&lower).unwrap();
        assert_eq!(core.spoil, step * n);
    }
    assert_eq!(
        core.ground_elevation_at(3, 0),
        -scale::EARTHWORK_LIMIT_QUANTA
    );
    assert!(core.edit_ground(&lower).unwrap_err().contains("full"));
    assert_eq!(
        core.spoil,
        u64::try_from(scale::EARTHWORK_LIMIT_QUANTA).unwrap()
    );

    // One step of fill spends exactly one step of spoil.
    core.edit_ground(&raise).unwrap();
    assert_eq!(
        core.spoil,
        u64::try_from(scale::EARTHWORK_LIMIT_QUANTA).unwrap() - step
    );
    assert_eq!(core.ground_elevation_at(0, 0), step as i32);
    core.undo_ground().unwrap();
    assert_eq!(
        core.spoil,
        u64::try_from(scale::EARTHWORK_LIMIT_QUANTA).unwrap(),
        "undoing fill returns the spoil it spent"
    );
    assert_eq!(core.ground_elevation_at(0, 0), 0);

    // Levelling evens onto the first cell of the selection and balances against the ledger.
    let level = GroundEdit {
        to_q: 3,
        shape: GroundShape::Path,
        action: GroundAction::Level,
        ..ground_edit(0, 0, GroundAction::Level)
    };
    let preview = core.ground_preview(&level);
    assert_eq!(preview.error, None);
    assert_eq!(
        preview.fill,
        u32::try_from(scale::EARTHWORK_LIMIT_QUANTA).unwrap(),
        "the pit is filled back to the first cell"
    );
    assert_eq!(preview.cut, 0);
    assert_eq!(preview.spoil, 0);
    core.edit_ground(&level).unwrap();
    assert_eq!(core.spoil, 0);
    assert!(core.ground.is_empty(), "level ground leaves the overlay");
    assert_eq!(core.checksum(), checksum, "the ledger balances to zero");

    // The selection modes, and the one property that makes an outline an outline: it is exactly
    // the hexes of its own filled shape that touch something outside it. Deriving the outline from
    // the fill rather than drawing it with geometry of its own is what makes it one hex thick at
    // every size, with no rounding rule that could disagree with the fill's.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    let cells = |edit: &GroundEdit| -> Vec<(i32, i32)> {
        let preview = core.ground_preview(edit);
        assert_eq!(preview.error, None);
        preview.cells.iter().map(|cell| (cell.q, cell.r)).collect()
    };

    // A circle is dragged from its centre out to a rim hex, so its radius is a distance the
    // player can count on the map rather than a number typed into a field.
    let disc = GroundEdit {
        to_q: 2,
        shape: GroundShape::Disc,
        ..ground_edit(0, 0, GroundAction::Pave)
    };
    let filled = cells(&disc);
    let rim = cells(&GroundEdit {
        shape: GroundShape::Ring,
        ..disc.clone()
    });
    assert_eq!(filled.len(), 19, "1 + 3n(n + 1) hexes at radius two");
    assert_eq!(rim.len(), 12, "6n hexes at radius two");
    assert!(rim.iter().all(|&cell| axial_distance((0, 0), cell) == 2));

    // A rectangle and its frame share both anchors, so a floor and the kerb round it are the
    // same drag with one button changed.
    let rect = GroundEdit {
        to_q: 3,
        to_r: 3,
        corner: 4,
        to_corner: 1,
        shape: GroundShape::Rect,
        ..ground_edit(0, 0, GroundAction::Pave)
    };
    let area = cells(&rect);
    let frame = cells(&GroundEdit {
        shape: GroundShape::Frame,
        ..rect.clone()
    });
    assert!(
        area.len() > frame.len(),
        "this rectangle has an interior to leave out: {} vs {}",
        area.len(),
        frame.len()
    );

    for (fill, outline) in [(filled, rim), (area, frame)] {
        let inside: BTreeSet<(i32, i32)> = fill.iter().copied().collect();
        let edge: BTreeSet<(i32, i32)> = fill
            .iter()
            .copied()
            .filter(|&(q, r)| {
                DIRECTIONS
                    .iter()
                    .any(|&(dq, dr)| !inside.contains(&(q + dq, r + dr)))
            })
            .collect();
        assert_eq!(outline.into_iter().collect::<BTreeSet<_>>(), edge);
    }

    // Both circular modes are bounded by arithmetic rather than by a scan, so an over-wide drag
    // is refused before a single hex is enumerated.
    let wide = core.ground_preview(&GroundEdit { to_q: 5, ..disc });
    assert!(wide.error.unwrap().contains("too wide"));
    assert!(wide.cells.is_empty());

    // What a selection has to do when it cannot be applied: stay on screen, and say which hex is
    // the problem. One obstacle used to erase the whole footprint it was standing in, which left
    // the player a refusal and no picture of what it was about.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    core.write_overlay(2, 0, WOOD, 9, 14);

    let lower = GroundEdit {
        to_q: 4,
        shape: GroundShape::Path,
        ..ground_edit(0, 0, GroundAction::Lower)
    };
    let preview = core.ground_preview(&lower);
    assert_eq!(preview.error, None, "one obstacle no longer refuses four");
    assert_eq!(preview.cells.len(), 5, "the footprint is drawn whole");
    assert_eq!(preview.blocked, 1);
    assert_eq!(preview.changes, 4);
    let stuck = preview
        .cells
        .iter()
        .find(|cell| (cell.q, cell.r) == (2, 0))
        .unwrap();
    assert!(stuck
        .blocked
        .as_deref()
        .is_some_and(|reason| reason.contains("deposit")));
    assert_eq!(stuck.change, 0, "a blocked hex moves no ground");
    core.edit_ground(&lower).unwrap();
    assert_eq!(core.spoil, 8);
    assert_eq!(core.ground_elevation_at(2, 0), 0, "the deposit sat still");

    // A refusal about the selection as a whole keeps its footprint too: that picture is how the
    // player works out where the spoil has to come from.
    let starved = core.ground_preview(&GroundEdit {
        steps: 3,
        ..GroundEdit {
            action: GroundAction::Raise,
            ..lower.clone()
        }
    });
    assert!(starved.error.unwrap().contains("spoil"));
    assert_eq!(starved.cells.len(), 5);

    // Depth is one number rather than three gestures, and a hex without room for the whole cut
    // takes what it has room for instead of refusing the pass. Prepare a cell two quanta shy
    // of the physical eight-metre limit so the final 1.5 m request exercises that clamp.
    for _ in 0..4 {
        core.edit_ground(&GroundEdit {
            steps: 3,
            ..ground_edit(3, 0, GroundAction::Lower)
        })
        .unwrap();
    }
    core.edit_ground(&GroundEdit {
        steps: 2,
        ..ground_edit(3, 0, GroundAction::Lower)
    })
    .unwrap();
    let deep = GroundEdit {
        steps: 3,
        ..ground_edit(3, 0, GroundAction::Lower)
    };
    assert_eq!(core.ground_preview(&deep).cut, 2, "clamped, not refused");
    core.edit_ground(&deep).unwrap();
    assert_eq!(
        core.ground_elevation_at(3, 0),
        -scale::EARTHWORK_LIMIT_QUANTA
    );
    assert!(core.edit_ground(&deep).unwrap_err().contains("full"));

    // Levelling names its datum. The same three hexes even onto the lowest, the highest, or the
    // one the drag started on, and the spoil ledger is what tells the three apart.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    // A stepped profile: 0, -0.5 m, -1.0 m across three hexes.
    core.edit_ground(&ground_edit(1, 0, GroundAction::Lower))
        .unwrap();
    core.edit_ground(&GroundEdit {
        steps: 2,
        ..ground_edit(2, 0, GroundAction::Lower)
    })
    .unwrap();
    assert_eq!(core.spoil, 6);

    let level = GroundEdit {
        to_q: 2,
        shape: GroundShape::Path,
        action: GroundAction::Level,
        ..ground_edit(0, 0, GroundAction::Level)
    };
    let lowest = core.ground_preview(&GroundEdit {
        reference: GroundReference::Lowest,
        ..level.clone()
    });
    assert_eq!(lowest.error, None);
    assert_eq!((lowest.cut, lowest.fill), (6, 0), "down to the deepest cut");
    assert_eq!(lowest.spoil, 12, "and the heap keeps what came out");

    let highest = core.ground_preview(&GroundEdit {
        reference: GroundReference::Highest,
        ..level.clone()
    });
    assert_eq!(
        (highest.cut, highest.fill),
        (0, 6),
        "up to the untouched hex"
    );
    assert_eq!(highest.spoil, 0, "which spends the heap instead");

    // The default is still the hex the drag started on, so an edit written before this control
    // existed means exactly what it meant.
    let first = core.ground_preview(&level);
    assert_eq!((first.cut, first.fill), (0, 6));

    core.edit_ground(&GroundEdit {
        reference: GroundReference::Lowest,
        ..level
    })
    .unwrap();
    for q in 0..=2 {
        assert_eq!(core.ground_elevation_at(q, 0), -4);
    }
    assert_eq!(core.spoil, 12);

    // Smooth is the intent-level tool: keep the first picked height and change only the harsh step
    // required to make the run walkable. The low end stays low instead of being needlessly levelled.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    let drop = GroundEdit {
        to_q: 2,
        shape: GroundShape::Path,
        steps: 2,
        ..ground_edit(1, 0, GroundAction::Lower)
    };
    core.edit_ground(&drop).unwrap();
    core.edit_ground(&drop).unwrap();
    let smooth = GroundEdit {
        q: 1,
        to_q: 1,
        datum: Some((0, 0)),
        action: GroundAction::Smooth,
        ..ground_edit(0, 0, GroundAction::Smooth)
    };
    let preview = core.ground_preview(&smooth);
    assert_eq!(preview.error, None);
    assert_eq!((preview.cut, preview.fill), (0, 4));
    assert_eq!(preview.changes, 1);
    core.edit_ground(&smooth).unwrap();
    assert_eq!(
        (0..=2)
            .map(|q| core.ground_elevation_at(q, 0))
            .collect::<Vec<_>>(),
        [0, -4, -8]
    );
    assert!(!(0..2).any(|q| core.grade_blocks((q, 0), (q + 1, 0))));

    // The route search prices travel time, so a longer prepared way beats a shorter raw one, and a
    // step nobody can climb stops the route and the body alike.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    set_player_hex(&mut core, 0, 0);

    // Untreated, the shortest way is the straight one.
    core.walk_to(5, 0).unwrap();
    assert_eq!(core.walk_path.len(), 5);
    assert!(core.walk_path.iter().all(|cell| cell.r == 0));

    // Concrete is a third faster, so five paved hexes and one raw one beat five raw ones.
    core.edit_ground(&GroundEdit {
        to_q: 4,
        to_r: 1,
        shape: GroundShape::Path,
        definition_id: 5,
        ..ground_edit(0, 1, GroundAction::Pave)
    })
    .unwrap();
    assert_eq!(core.movement_factor_at(2, 1), 130);
    assert_eq!(
        core.walk_step_cost((1, 1), 2, 1),
        WALK_STEP_COST * 100 / 130
    );
    core.walk_to(5, 0).unwrap();
    assert_eq!(core.walk_path.len(), 6, "the paved way is one hex longer");
    assert!(core.walk_path.iter().any(|cell| (cell.q, cell.r) == (3, 1)));

    // The player walks it at the speed the route was priced at.
    set_player_hex(&mut core, 2, 1);
    core.player.walk_goal = None;
    core.walk_path.clear();
    core.set_move_intent(1000, 0).unwrap();
    let start = core.player.x;
    core.advance_player_steps(1);
    // The paving factor the route priced, then whatever the mobility ladder is worth to this
    // player — the same two multiplications in the same order `player_step` does them in, because
    // the claim is that the step matches the price and not that either number is a particular one.
    let paved = PLAYER_SPEED * 130 / 100;
    assert_eq!(core.player.x - start, core.apply_move_speed(paved));

    // A wall taller than anyone can climb is a wall to the route and to the body.
    set_player_hex(&mut core, 0, 0);
    core.set_move_intent(0, 0).unwrap();
    core.edit_ground(&GroundEdit {
        steps: 3,
        ..ground_edit(-2, 0, GroundAction::Lower)
    })
    .unwrap();
    core.edit_ground(&GroundEdit {
        steps: 3,
        ..ground_edit(-1, 0, GroundAction::Raise)
    })
    .unwrap();
    assert_eq!(core.ground_elevation_at(-1, 0), 6);
    assert!(core.grade_blocks((0, 0), (-1, 0)));
    assert!(core.grade_blocks((-1, 0), (-2, 0)), "a wall is symmetric");
    assert!(core.walk_to(-1, 0).is_err());
    let (blocked_x, blocked_y) = axial_world(-1, 0);
    assert!(core.player_blocked(blocked_x, blocked_y));
    // Four quanta is still a walkable one-metre slope, not a wall.
    core.edit_ground(&ground_edit(-1, 0, GroundAction::Lower))
        .unwrap();
    assert!(!core.grade_blocks((0, 0), (-1, 0)));
    core.walk_to(-1, 0).unwrap();

    // Covering a deposit is deliberate, reversible and lossless. It is confirmed before it happens,
    // it suppresses hands, extractors, the published snapshot and regrowth without harvesting a
    // single unit, and stripping the surface hands back exactly what was sealed.
    let mut core = ground_world();
    core.write_overlay(2, 0, WOOD, 9, 14);
    core.rebuild_flora_regrowth();
    assert!(core.flora_regrowth.contains(&(2, 0)));
    core.player.inventory = BTreeMap::from([(item_id(&core, "gravel"), 4)]);

    let pave = ground_edit(2, 0, GroundAction::Pave);
    let warned = core.ground_preview(&pave);
    assert_eq!(warned.covers, 1);
    assert!(warned.error.unwrap().contains("Confirm covering"));
    assert!(core.edit_ground(&pave).is_err());

    let confirmed = GroundEdit {
        cover: true,
        ..pave
    };
    assert_eq!(core.ground_preview(&confirmed).error, None);
    core.edit_ground(&confirmed).unwrap();
    assert_eq!(core.field_at(2, 0), None, "a sealed deposit is unreachable");
    assert_eq!(core.deposit_quantity((2, 0)), 0);
    assert!(!core
        .resource_snapshots()
        .iter()
        .any(|row| (row.q, row.r) == (2, 0)));
    assert!(
        !core.flora_regrowth.contains(&(2, 0)),
        "sealing suppresses regrowth without harvesting"
    );
    // Nothing was taken: the overlay still holds every unit that was left.
    assert_eq!(core.tiles[&(2, 0)].resource.as_ref().unwrap().quantity, 9);
    core.advance_ticks(600);
    assert_eq!(core.tiles[&(2, 0)].resource.as_ref().unwrap().quantity, 9);

    core.edit_ground(&GroundEdit {
        action: GroundAction::Clear,
        ..confirmed
    })
    .unwrap();
    assert_eq!(core.deposit_quantity((2, 0)), 9, "the remainder comes back");
    assert!(core.flora_regrowth.contains(&(2, 0)));

    // Grading never moves a deposit, and an extractor at work is not paved over from under.
    assert!(core
        .ground_preview(&ground_edit(2, 0, GroundAction::Lower))
        .error
        .unwrap()
        .contains("deposit"));
    core.set_creative(true);
    reach(&mut core);
    core.write_overlay(3, 0, WOOD, 5, 5);
    assert_eq!(core.place(3, 0, 1, 0, None), Ok(()));
    core.compile_graph();
    assert!(core.field_covered_at((3, 0), (2, 0), core.extract_radius_of(1)));
    assert!(core
        .ground_preview(&confirmed)
        .error
        .unwrap()
        .contains("extractor"));

    // A footprint needs a pad flatter than the steepest slope a player may still walk.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    let container = core
        .definitions
        .buildings
        .iter_mut()
        .find(|d| d.id == 4)
        .unwrap();
    container.footprint = vec![Coordinate { q: 0, r: 0 }, Coordinate { q: 1, r: 0 }];
    assert_eq!(core.placement_legality(0, 0, 4, 0, None, true), Ok(()));

    for _ in 0..2 {
        core.edit_ground(&ground_edit(4, 0, GroundAction::Lower))
            .unwrap();
    }
    core.edit_ground(&ground_edit(1, 0, GroundAction::Raise))
        .unwrap();
    assert_eq!(core.ground_elevation_at(1, 0), scale::MAX_BUILD_STEP_QUANTA);
    assert_eq!(core.placement_legality(0, 0, 4, 0, None, true), Ok(()));

    // A one-metre slope is still walkable, but a foundation now needs the flatter pad contract.
    core.edit_ground(&ground_edit(1, 0, GroundAction::Raise))
        .unwrap();
    assert_eq!(core.ground_elevation_at(1, 0), scale::MAX_WALK_STEP_QUANTA);
    assert!(core
        .placement_legality(0, 0, 4, 0, None, true)
        .unwrap_err()
        .contains("level a pad"));

    // A span foundation may follow a slope a player can still walk; the pad class may not.
    set_test_foundation(&mut core, 4, FoundationClass::Span);
    assert_eq!(core.placement_legality(0, 0, 4, 0, None, true), Ok(()));
    set_test_foundation(&mut core, 4, FoundationClass::Pad);

    // Levelling the pair onto the first cell's grade is exactly what makes the site legal.
    core.edit_ground(&GroundEdit {
        to_q: 1,
        shape: GroundShape::Path,
        action: GroundAction::Level,
        ..ground_edit(0, 0, GroundAction::Level)
    })
    .unwrap();
    assert_eq!(core.ground_elevation_at(1, 0), 0);
    assert_eq!(core.placement_legality(0, 0, 4, 0, None, true), Ok(()));

    // Prepared ground survives a save, migrates forward from a file that never had any, refuses a
    // state the definitions cannot explain, and its dirty-tracked delta matches the full oracle.
    let mut core = ground_world();
    core.set_creative(true);
    reach(&mut core);
    core.edit_ground(&GroundEdit {
        to_q: 2,
        shape: GroundShape::Path,
        ..ground_edit(0, 0, GroundAction::Pave)
    })
    .unwrap();
    core.edit_ground(&ground_edit(0, 2, GroundAction::Lower))
        .unwrap();
    assert_eq!(core.spoil, 2);

    // Reach is a scenario property the loader checks against the catalogue rather than a
    // simulation result, so the borrowed test reach goes back before anything is written.
    core.player.build_range = core.earned_build_range();
    let save = core.save_string().unwrap();
    let (definitions, technologies, scenarios) = catalogs();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.ground, core.ground);
    assert_eq!(restored.spoil, core.spoil);
    assert_eq!(restored.checksum(), core.checksum());

    // The old one-square-metre ground cannot be reconstructed as physical drainage, even when
    // it happens to carry no prepared cells. The catalogue keeps the file exportable and the
    // native boundary refuses to pretend it can be resumed.
    let mut untouched = ground_world();
    untouched.player.build_range = untouched.earned_build_range();
    let plain = untouched.save_string().unwrap();
    let old = plain.replace(
        &format!("\"save_version\":{SAVE_VERSION}"),
        "\"save_version\":36",
    );
    let error = match Core::from_save(&definitions, &technologies, &scenarios, &old) {
        Ok(_) => panic!("legacy ground crossed the physical compatibility boundary"),
        Err(error) => error,
    };
    assert!(error.contains("export"), "{error}");

    let mut invalid = core.ground_snapshot();
    invalid[0].elevation = i16::try_from(scale::EARTHWORK_LIMIT_QUANTA + 1).unwrap();
    assert!(validate_saved_ground(&definitions, &invalid).is_err());
    let mut invalid = core.ground_snapshot();
    invalid[0].surface = 99;
    assert!(validate_saved_ground(&definitions, &invalid).is_err());
    let mut invalid = core.ground_snapshot();
    invalid.push(invalid[0].clone());
    assert!(validate_saved_ground(&definitions, &invalid).is_err());
    let mut invalid = core.ground_snapshot();
    invalid[0].paid = vec![Ingredient {
        item_id: 1,
        quantity: 1,
    }];
    invalid[0].surface = 0;
    assert!(
        validate_saved_ground(&definitions, &invalid).is_err(),
        "untreated ground cannot carry a paid bill"
    );

    // The digest is pure, and the cache is only ever an echo of it.
    assert_eq!(core.ground_state_hash(), core.uncached_ground_hash());

    // The dirty-tracked delta is what a full diff of two snapshots would have said, and nothing
    // is resent once the host has it.
    let mut factory = test_factory("new-game");
    factory.core = ground_world();
    factory.core.set_creative(true);
    reach(&mut factory.core);
    let mut previous = factory.core.snapshot();
    factory.build_delta();
    factory
        .core
        .edit_ground(&ground_edit(0, 0, GroundAction::Lower))
        .unwrap();
    assert_delta_matches_full_diff(&mut factory, &mut previous, "a cut");
    factory
        .core
        .edit_ground(&ground_edit(0, 1, GroundAction::Pave))
        .unwrap();
    assert_delta_matches_full_diff(&mut factory, &mut previous, "a paved cell");
    factory.core.undo_ground().unwrap();
    assert_delta_matches_full_diff(&mut factory, &mut previous, "an undo");
    let quiet = factory.build_delta();
    assert!(quiet.ground.is_none());
    assert!(quiet.spoil.is_none());
    assert!(quiet.water.is_none());
}

/// A flood is a sparse overlay, like a grade: the tile still carries the generated depth, and
/// the delta carries only the cells that left it. Returning to equilibrium sends the empty list
/// so the host drops the overlay rather than keeping the last flood it saw.
#[test]
fn a_disturbed_depth_is_what_the_delta_publishes() {
    let mut factory = test_factory("new-game");
    factory.core.set_creative(true);
    let (q, r) = {
        let size = factory.core.scenario.chunk_size;
        factory
            .core
            .generated_chunks
            .iter()
            .copied()
            .flat_map(|(chunk_q, chunk_r)| hexes_in_chunk(chunk_q, chunk_r, size))
            .find(|&(cell_q, cell_r)| factory.core.water_depth_at(cell_q, cell_r) == 0)
            .expect("the opening surveys dry ground")
    };
    factory.core.water.set(q, r, hydrology::WaterDelta::new(6));
    factory.core.settle_water(&[(q, r)]);
    let _ = factory.snapshot_json();
    let mut previous = factory.core.snapshot();
    assert!(
        !previous.water.is_empty(),
        "a flood is a departure the snapshot carries"
    );

    let seeds: Vec<(i32, i32)> = factory
        .core
        .water
        .cells()
        .iter()
        .map(|cell| (cell.q, cell.r))
        .collect();
    for &(cell_q, cell_r) in &seeds {
        factory
            .core
            .water
            .set(cell_q, cell_r, hydrology::WaterDelta::new(0));
    }
    factory.core.settle_water(&seeds);
    assert!(
        factory.core.water.is_empty(),
        "forgetting every departure is the equilibrium"
    );
    assert_delta_matches_full_diff(&mut factory, &mut previous, "draining the flood");
    let quiet = factory.build_delta();
    assert!(quiet.water.is_none());
}

/// The generated world is exactly as passable after this release as before it. Every pair of
/// walkable bands is within one climbable step, which is the whole reason `natural_elevation`
/// has the values it does, and it is asserted here rather than trusted.
#[test]
fn no_terrain_walls_itself_off_and_a_quarried_cliff_stops_being_a_wall() {
    let bands = [
        Terrain::DeepWater,
        Terrain::ShallowWater,
        Terrain::Shore,
        Terrain::Lowland,
        Terrain::Hills,
        Terrain::Highland,
        Terrain::Cliff,
    ];
    for &a in &bands {
        for &b in &bands {
            if a.blocks_movement() || b.blocks_movement() {
                continue;
            }
            assert!(
                (natural_elevation(a) - natural_elevation(b)).abs() <= MAX_WALK_STEP,
                "{a:?} and {b:?} would be walled off from each other"
            );
        }
    }
    // A run that has never touched the ground contributes nothing to the checksum, which is what
    // keeps a file written a release ago checksumming to the value it did then.
    let core = ground_world();
    assert!(core.ground.is_empty());
    assert_eq!(core.spoil, 0);
    assert_eq!(core.movement_factor_at(0, 0), UNTREATED_MOVEMENT);
    // The heuristic floor is the cheapest step the fastest legal surface can produce, so the
    // route search never overestimates and never returns a route that is not the cheapest.
    assert_eq!(
        MIN_WALK_STEP_COST,
        WALK_STEP_COST * UNTREATED_MOVEMENT / MAX_SURFACE_MOVEMENT
    );
    assert!(MIN_WALK_STEP_COST <= WALK_STEP_COST * UNTREATED_MOVEMENT / MAX_SURFACE_MOVEMENT);

    // The one wall the player may take apart, end to end.
    //
    // A cliff is impassable until somebody quarries it. Nothing may be laid on a face that is
    // still standing, one cut brings that face level with the highland beside it, and after the
    // cut the hex walks and builds like any other ground — with the rock that came out of it on
    // the spoil heap rather than gone. The band the generator drew never moves: the whole change
    // lives in the overlay, so a world nobody has dug is exactly as passable as it always was.
    let mut core = legacy_band_game("new-game");
    reach(&mut core);
    // The nearest cliff face outside the landing hub's own seven hexes.
    assert_eq!(core.terrain_at(2, -1), Terrain::Cliff);
    assert!(core.terrain_blocks_movement(2, -1));
    assert!(core.terrain_blocks_construction(2, -1));
    assert!(!core.walkable_hex(2, -1));
    assert_eq!(core.spoil, 0);

    let pave = core.ground_preview(&ground_edit(2, -1, GroundAction::Pave));
    assert!(
        pave.error
            .as_deref()
            .is_some_and(|error| error.contains("Cut this cliff down first")),
        "paving a standing cliff said {:?}",
        pave.error
    );

    core.edit_ground(&ground_edit(2, -1, GroundAction::Lower))
        .unwrap();
    assert!(core.cliff_quarried(2, -1));
    assert_eq!(core.terrain_at(2, -1), Terrain::Cliff);
    assert_eq!(
        core.ground_elevation_at(2, -1),
        natural_elevation(Terrain::Highland)
    );
    assert!(!core.terrain_blocks_movement(2, -1));
    assert!(!core.terrain_blocks_construction(2, -1));
    assert!(core.walkable_hex(2, -1));
    // Quarried rock leaves as spoil, on the same ledger every other cut pays into.
    assert_eq!(core.spoil, 1);

    // Undo is the edit run backwards, so the wall comes back and takes its spoil with it.
    core.undo_ground().unwrap();
    assert!(!core.cliff_quarried(2, -1));
    assert!(core.terrain_blocks_movement(2, -1));
    assert!(core.terrain_blocks_construction(2, -1));
    assert_eq!(core.spoil, 0);
    assert!(core.ground.is_empty());
}

#[test]
fn geomorphic_state_round_trips_and_keeps_the_next_epoch_deterministic() {
    let (definitions, technologies, scenarios) = catalogs();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|scenario| scenario.key == "new-game")
        .unwrap();
    let mut first = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
    first.ground.insert(
        (0, 0),
        GroundCell {
            q: 0,
            r: 0,
            surface: 0,
            elevation: 0,
            erosion: -1,
            paid: Vec::new(),
        },
    );
    first.bank_stress = geomorphology::BankStress::from_cells(&[geomorphology::StressCell {
        q: 2,
        r: -1,
        stress: 17,
    }]);
    let saved = first.save_string().unwrap();
    let mut restored = Core::from_save(&definitions, &technologies, &scenarios, &saved).unwrap();
    assert_eq!(restored.checksum(), first.checksum());
    assert_eq!(restored.ground, first.ground);
    assert_eq!(restored.bank_stress, first.bank_stress);

    let first_epoch = first.run_geomorphic_epoch();
    let restored_epoch = restored.run_geomorphic_epoch();
    assert_eq!(restored_epoch, first_epoch);
    assert_eq!(restored.checksum(), first.checksum());
}

#[test]
fn groundwork_lands_after_resolved_work_and_resumes_mid_job() {
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = bare_game("new-game");
    core.set_creative(true);
    reach(&mut core);
    set_player_hex(&mut core, 0, -5);
    let edit = GroundEdit {
        cover: true,
        ..ground_edit(0, -4, GroundAction::Lower)
    };
    let before = core.ground_elevation_at(0, -4);
    let preview = core.ground_preview(&edit);
    assert_eq!((preview.cut, preview.fill), (2, 0));
    assert_eq!(
        preview.work_steps,
        2 * GROUNDWORK_STEPS_PER_QUANTUM,
        "the native preview publishes the same resolved-volume clock the action uses"
    );

    core.begin_groundwork(edit.clone()).unwrap();
    let total = core.player.action_cooldown;
    assert_eq!(total, preview.work_steps);
    assert_eq!(core.pending_ground.as_ref(), Some(&edit));
    assert_eq!(
        core.ground_elevation_at(0, -4),
        before,
        "pressing the brush must not move the ground before the work is done"
    );
    assert!(
        core.begin_groundwork(ground_edit(1, -4, GroundAction::Lower))
            .is_err(),
        "one player cannot queue an unlimited field of instant cuts"
    );

    core.advance_player_steps(total / 2);
    core.player.build_range = core.earned_build_range();
    let save = core.save_string().unwrap();
    let mut resumed = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    let remaining = core.player.action_cooldown;
    assert_eq!(resumed.pending_ground.as_ref(), Some(&edit));
    resumed.advance_player_steps(remaining - 1);
    core.advance_player_steps(remaining - 1);
    assert_eq!(resumed.ground_elevation_at(0, -4), before);

    resumed.advance_player_steps(1);
    core.advance_player_steps(1);
    assert_eq!(resumed.ground_elevation_at(0, -4), before - 2);
    assert!(resumed.pending_ground.is_none());
    assert_eq!(
        resumed.checksum(),
        core.checksum(),
        "resumed groundwork and uninterrupted groundwork are the same run"
    );
}

#[test]
fn save_40_adopts_empty_geomorphology_without_changing_its_checksum() {
    let (definitions, technologies, scenarios) = catalogs();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|scenario| scenario.key == "new-game")
        .unwrap();
    let core = Core::new(&definitions, &technologies, scenario, None, None).unwrap();
    let current = core.save_string().unwrap();
    let save_40 = current
        .replacen("\"save_version\":45", "\"save_version\":40", 1)
        .replacen("\"definition_version\":30", "\"definition_version\":29", 1)
        .replacen("\"technology_version\":18", "\"technology_version\":16", 1)
        .replacen(",\"bank_stress\":[]", "", 1);
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save_40).unwrap();
    assert!(restored.bank_stress.is_empty());
    assert_eq!(restored.checksum(), core.checksum());
}
