use super::*;

#[test]
fn boundaries_are_canonical_atomic_conserving_and_block_what_crosses_them() {
    let mut core = empty_world("new-game");
    core.compile_graph();
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    let timber = core
        .definitions
        .items
        .iter()
        .find(|i| i.key == "timber")
        .unwrap()
        .id;
    let wire = core
        .definitions
        .items
        .iter()
        .find(|i| i.key == "iron-wire")
        .unwrap()
        .id;
    core.player.inventory = BTreeMap::from([(timber, 20), (wire, 2)]);
    let initial = core.player.inventory.clone();
    let edit = boundary_edit(0, 0);
    let checksum = core.checksum();
    let preview = core.boundary_preview(&edit);
    assert_eq!(preview.error, None);
    assert_eq!(
        preview.cost,
        vec![Ingredient {
            item_id: timber,
            quantity: 2
        }]
    );
    assert_eq!(core.checksum(), checksum);
    core.edit_boundaries(&edit).unwrap();
    assert_eq!(core.boundaries.len(), 1);
    assert_eq!(core.player.inventory[&timber], 18);
    assert_eq!(core.boundary_preview(&edit).changes, 0);
    let checksum = core.checksum();
    assert!(core.edit_boundaries(&edit).is_err());
    assert_eq!(checksum, core.checksum());
    let gate = BoundaryEdit {
        definition_id: 2,
        ..edit.clone()
    };
    assert_eq!(
        core.boundary_preview(&gate).cost,
        vec![Ingredient {
            item_id: wire,
            quantity: 1
        }]
    );
    core.edit_boundaries(&gate).unwrap();
    assert!(core.boundaries.values().next().unwrap().open);
    core.undo_boundary().unwrap();
    assert!(!core.boundaries.values().next().unwrap().open);
    core.undo_boundary().unwrap();
    assert!(core.boundaries.is_empty());
    assert_eq!(core.player.inventory, initial);
    core.set_creative(true);
    core.edit_boundaries(&edit).unwrap();
    core.set_creative(false);
    let before_remove = core.player.inventory.clone();
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Remove,
        ..edit
    })
    .unwrap();
    assert_eq!(core.player.inventory, before_remove);

    // Boundaries are canonical bounded atomic and reject unsafe sites.
    let mut core = empty_world("new-game");
    core.scenario.generated_environment = false;
    core.compile_graph();
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    core.set_creative(true);
    let edit = boundary_edit(0, 0);
    core.edit_boundaries(&edit).unwrap();
    // The same chord named from the neighbour that shares it is the same record, not a second.
    let reverse = edge_edit(1, 0, 3);
    assert_eq!(core.boundary_preview(&reverse).changes, 0);
    assert_eq!(core.boundaries.len(), 1);
    core.undo_boundary().unwrap();
    let yard = BoundaryEdit {
        shape: BoundaryShape::Yard,
        to_q: 1,
        to_corner: 3,
        ..edit.clone()
    };
    let preview = core.boundary_preview(&yard);
    assert_eq!(preview.error, None);
    let sides = preview.segments.len();
    assert!(sides >= 4);
    core.edit_boundaries(&yard).unwrap();
    assert_eq!(core.boundaries.len(), sides);
    core.undo_boundary().unwrap();
    let checksum = core.checksum();
    for invalid in [
        // A rectangle far past the segment budget is refused before anything is priced.
        BoundaryEdit {
            to_q: 99_999,
            ..yard.clone()
        },
        BoundaryEdit {
            q: i32::MIN,
            ..edit.clone()
        },
        BoundaryEdit {
            corner: 6,
            ..edit.clone()
        },
        BoundaryEdit {
            q: 99,
            r: 99,
            to_q: 99,
            to_r: 99,
            ..edit.clone()
        },
        // A rectangle with no extent is a point, which the yard shape cannot draw.
        BoundaryEdit {
            to_q: 0,
            to_corner: 1,
            ..yard.clone()
        },
    ] {
        assert!(core.boundary_preview(&invalid).error.is_some());
        assert!(core.edit_boundaries(&invalid).is_err());
        assert_eq!(core.checksum(), checksum);
    }
    core.set_creative(false);
    core.player.inventory.clear();
    let before = core.checksum();
    assert!(core.edit_boundaries(&yard).is_err());
    assert_eq!(core.checksum(), before);
    // Anchors stop one hex short of the limit, so canonicalizing a chord onto its neighbour can
    // never mint a record that save loading would reject.
    assert!(core
        .boundary_preview(&BoundaryEdit {
            q: -100_000,
            to_q: -100_000,
            ..edit.clone()
        })
        .error
        .unwrap()
        .contains("coordinate range"));
    core.set_creative(true);
    core.player.x = HEX_X / 2;
    assert!(core
        .boundary_preview(&edit)
        .error
        .unwrap()
        .contains("Step away"));

    // Boundaries block manual and click walks and gates replan routes.
    let mut core = empty_world("new-game");
    core.compile_graph();
    core.player.x = 0;
    core.player.y = 0;
    core.set_creative(true);
    let edit = boundary_edit(0, 0);
    core.walk_to(2, 0).unwrap();
    core.edit_boundaries(&edit).unwrap();
    assert_ne!(core.walk_path.first().map(|c| (c.q, c.r)), Some((1, 0)));
    core.set_move_intent(1000, 0).unwrap();
    core.advance_player_steps(30);
    assert!(core.player.x < HEX_X / 2);
    core.player.x = 0;
    core.player.y = 0;
    core.edit_boundaries(&BoundaryEdit {
        definition_id: 2,
        ..edit.clone()
    })
    .unwrap();
    core.walk_to(2, 0).unwrap();
    assert_eq!(core.walk_path.first().map(|c| (c.q, c.r)), Some((1, 0)));
    core.advance_player_steps(80);
    assert_eq!(world_to_axial(core.player.x, core.player.y), (2, 0));
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Close,
        ..edit.clone()
    })
    .unwrap();
    assert!(core.boundary_blocks_segment(axial_world(0, 0), axial_world(1, 0)));
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Open,
        ..edit
    })
    .unwrap();
    assert!(!core.boundary_blocks_segment(axial_world(0, 0), axial_world(1, 0)));

    // Boundaries protect transport and recompile future connections without losing cargo.
    let mut core = empty_world("new-game");
    core.compile_graph();
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    core.set_creative(true);
    let edit = boundary_edit(0, 0);
    core.edit_boundaries(&edit).unwrap();
    let a = add_test_entity(&mut core, 0, 0, 2, 0);
    let b = add_test_entity(&mut core, 1, 0, 2, 0);
    core.compile_graph();
    assert!(link_ids(&core, a).is_empty());
    let cargo = Cargo {
        item_id: 1,
        quantity: 1,
    };
    let a_index = index_of(&core, a);
    core.entities[a_index].cargo = Some(cargo);
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Remove,
        ..edit.clone()
    })
    .unwrap();
    assert_eq!(link_ids(&core, a), vec![b]);
    let graph = core.graph.clone();
    core.compile_graph();
    assert_eq!(graph, core.graph);
    assert_eq!(core.entities[index_of(&core, a)].cargo, Some(cargo));
    let checksum = core.checksum();
    assert!(core
        .edit_boundaries(&edit)
        .unwrap_err()
        .contains("transport"));
    assert_eq!(checksum, core.checksum());
    assert!(core.undo_boundary().unwrap_err().contains("transport"));

    // Boundaries save migrate validate and dirty deltas match the full oracle.
    let mut core = game("new-game");
    let old = core
        .save_string()
        .unwrap()
        .replace("\"save_version\":29", "\"save_version\":28")
        .replace("\"definition_version\":23", "\"definition_version\":22");
    let (definitions, technologies, scenarios) = catalogs();
    let migrated = Core::from_save(&definitions, &technologies, &scenarios, &old).unwrap();
    assert_eq!(migrated.checksum(), core.checksum());
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    core.set_creative(true);
    let edit = boundary_edit(-2, -2);
    core.edit_boundaries(&edit).unwrap();
    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.boundaries, core.boundaries);
    assert_eq!(restored.checksum(), core.checksum());
    assert!(restored.boundary_undo.is_empty());
    // Every boundary written before the vertex lattice spelled its chord `direction` and only
    // ever held the three shared edges, which are the same three chords under the same
    // identity. Old saves therefore load in place, byte for byte, with no state rewrite.
    let legacy = save
        .replace("\"save_version\":34", "\"save_version\":32")
        .replace("\"chord\":", "\"direction\":");
    assert!(legacy.contains("\"direction\":"));
    let loaded = Core::from_save(&definitions, &technologies, &scenarios, &legacy).unwrap();
    assert_eq!(loaded.boundaries, core.boundaries);
    assert_eq!(loaded.checksum(), core.checksum());
    let previous = core.snapshot();
    let baseline = SnapshotBaseline::from_snapshot(&previous);
    core.dirty = SnapshotDirty::default();
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Remove,
        ..edit
    })
    .unwrap();
    let current = core.snapshot();
    let mut factory = Factory {
        definitions,
        technologies,
        scenarios,
        core,
        snapshot_revision: 0,
        baseline: Some(baseline),
    };
    let delta = factory.build_delta();
    assert_eq!(delta, SnapshotDelta::between(0, 1, &previous, &current));
    assert_eq!(delta.boundaries, Some(Vec::new()));
    assert!(factory.build_delta().boundaries.is_none());

    // Boundaries cover all six sides vertices and keep the source digest exact.
    let mut core = empty_world("new-game");
    core.scenario.generated_environment = false;
    core.compile_graph();
    core.player.x = 0;
    core.player.y = 0;
    core.set_creative(true);
    for direction in 0..6 {
        let edit = edge_edit(0, 0, direction);
        core.edit_boundaries(&edit).unwrap();
        let (q, r) = DIRECTIONS[direction as usize];
        let other = axial_world(q, r);
        assert!(core.boundary_blocks_segment((0, 0), other));
        assert!(core.boundary_blocks_segment(other, (0, 0)));
        assert!(core.boundary_blocks_player(other.0 / 2, other.1 / 2));
        let reverse = edge_edit(q, r, (direction + 3) % 6);
        assert_eq!(core.boundary_preview(&reverse).changes, 0);
        assert_eq!(core.boundary_state_hash(), core.uncached_boundary_hash());
        assert_eq!(core.boundary_state_hash(), core.uncached_boundary_hash());
    }
    for (q, r) in TRANSPORT_DIRECTIONS {
        assert!(core.boundary_blocks_segment((0, 0), axial_world(q, r)));
    }
    assert!(core.walk_route((0, 0), (2, 0)).is_none());
    core.edit_boundaries(&BoundaryEdit {
        definition_id: 2,
        ..boundary_edit(0, 0)
    })
    .unwrap();
    assert!(core.walk_route((0, 0), (2, 0)).is_some());
    assert_eq!(core.boundary_state_hash(), core.uncached_boundary_hash());
    let hash = core.checksum();
    *core.boundary_hash_cache.borrow_mut() = None;
    assert_eq!(core.checksum(), hash);
    core.undo_boundary().unwrap();
    assert_eq!(core.boundary_state_hash(), core.uncached_boundary_hash());
    let mut invalid = core.boundary_snapshot();
    invalid.push(invalid[0].clone());
    assert!(validate_saved_boundaries(&core.definitions, &invalid).is_err());
    invalid = core.boundary_snapshot();
    invalid[0].paid = vec![Ingredient {
        item_id: 1,
        quantity: 1000,
    }];
    assert!(validate_saved_boundaries(&core.definitions, &invalid).is_err());
    invalid = core.boundary_snapshot();
    invalid[0].open = true;
    assert!(validate_saved_boundaries(&core.definitions, &invalid).is_err());
    let yard = BoundaryEdit {
        q: -2,
        r: -2,
        corner: 0,
        to_q: 0,
        to_r: 0,
        to_corner: 3,
        shape: BoundaryShape::Yard,
        ..boundary_edit(0, 0)
    };
    // The player is standing on the rectangle's own edge; step off it before walling it.
    core.player.x = 4 * HEX_X;
    let sides = core.boundary_preview(&yard);
    assert_eq!(sides.error, None);
    // A closed rectangle: every vertex it visits is entered once and left once.
    let mut visits: BTreeMap<(i32, i32), usize> = BTreeMap::new();
    for segment in &sides.segments {
        let (a, b) = segment.ends();
        *visits.entry(a).or_default() += 1;
        *visits.entry(b).or_default() += 1;
    }
    assert!(visits.values().all(|&n| n == 2));

    // The point of anchoring on vertices: a wall can hold one heading for a long run. Twelve
    // headings leave every lattice vertex, thirty degrees apart, and each has to draw exactly
    // straight for at least the twenty segments this phase is graded on.
    //
    // Only six of the twelve repeat one chord over and over — the honeycomb is not a lattice under
    // its own edges, so the other six alternate two chord lengths and are no less straight for it.
    // The test is collinearity, not sameness: every vertex the run touches lies on the ray, and
    // each one is further along it than the last.
    let mut core = empty_world("new-game");
    core.scenario.generated_environment = false;
    core.compile_graph();
    core.player.x = 40 * HEX_X;
    core.player.y = 0;
    core.set_creative(true);
    // Creative mode recomputes reach from earned skills; a twenty-segment run outruns it.
    core.player.build_range = 200 * HEX_X as u32;
    let start = (0, 0, 0u8);
    let origin = corner_world(start.0, start.1, start.2);
    let mut headings = BTreeSet::new();
    for corner in 0..6u8 {
        for hex in corner_hexes(start.0, start.1, start.2) {
            let Some(local) = (0..6u8).find(|&k| corner_world(hex.0, hex.1, k) == origin) else {
                continue;
            };
            if corner == local {
                continue;
            }
            let step = corner_world(hex.0, hex.1, corner);
            let (dx, dy) = (step.0 - origin.0, step.1 - origin.1);
            if !headings.insert((dx, dy)) {
                continue;
            }
            // Aim further and further along the ray, stopping at the first vertex on it whose
            // run is long enough to grade.
            let mut run = None;
            for reach in 1..=64 {
                let far = (origin.0 + dx * reach, origin.1 + dy * reach);
                let end = nearest_corner(far.0, far.1);
                if corner_world(end.0, end.1, end.2) != far {
                    continue;
                }
                let preview = core.boundary_preview(&line_edit(start, end));
                assert_eq!(preview.error, None, "heading {dx}, {dy} at {far:?}");
                if preview.segments.len() >= 20 {
                    run = Some((far, preview));
                    break;
                }
            }
            let (far, preview) = run.expect("twenty segments on this heading");
            let mut at = origin;
            for segment in &preview.segments {
                let (a, b) = segment.ends();
                assert!(a == at || b == at, "heading {dx}, {dy} broke at {at:?}");
                let next = if a == at { b } else { a };
                let (ax, ay) = (i64::from(next.0 - origin.0), i64::from(next.1 - origin.1));
                assert_eq!(
                    ax * i64::from(dy) - ay * i64::from(dx),
                    0,
                    "heading {dx}, {dy} left the line at {next:?}"
                );
                assert!(
                    i64::from(next.0 - at.0) * i64::from(dx)
                        + i64::from(next.1 - at.1) * i64::from(dy)
                        > 0
                );
                at = next;
            }
            assert_eq!(at, far);
        }
    }
    assert_eq!(headings.len(), 12);

    // Boundaries refuse full pack refunds and unfunded undo without changing state.
    let mut core = empty_world("new-game");
    core.compile_graph();
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    let timber = core
        .definitions
        .items
        .iter()
        .find(|i| i.key == "timber")
        .unwrap()
        .id;
    core.player.inventory = BTreeMap::from([(timber, 2)]);
    let edit = boundary_edit(0, 0);
    core.edit_boundaries(&edit).unwrap();
    core.player.inventory = BTreeMap::from([(
        IRON_ORE,
        core.stack_size(IRON_ORE) * core.player.carry_slots,
    )]);
    let remove = BoundaryEdit {
        action: BoundaryAction::Remove,
        ..edit
    };
    let checksum = core.checksum();
    assert!(core.edit_boundaries(&remove).unwrap_err().contains("room"));
    assert_eq!(core.checksum(), checksum);
    core.player.inventory.clear();
    core.edit_boundaries(&remove).unwrap();
    assert_eq!(core.player.inventory[&timber], 2);
    core.player.inventory.clear();
    let checksum = core.checksum();
    assert!(core.undo_boundary().unwrap_err().contains("materials"));
    assert_eq!(core.checksum(), checksum);
    core.player.inventory.insert(timber, 2);
    core.undo_boundary().unwrap();
    assert_eq!(core.boundaries.len(), 1);
    assert!(core.player.inventory.is_empty());

    // Boundaries protect multicell placement rotation and live gate crossings.
    let mut core = empty_world("new-game");
    core.scenario.generated_environment = false;
    core.compile_graph();
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    core.set_creative(true);
    let edit = BoundaryEdit {
        definition_id: 2,
        ..edge_edit(0, 0, 1)
    };
    core.edit_boundaries(&edit).unwrap();
    let container = core
        .definitions
        .buildings
        .iter_mut()
        .find(|d| d.id == 4)
        .unwrap();
    container.footprint = vec![Coordinate { q: 0, r: 0 }, Coordinate { q: 1, r: 0 }];
    assert!(core.placement_legality(0, 0, 4, 1, None, true).is_err());
    add_test_entity(&mut core, 0, 0, 4, 0);
    core.compile_graph();
    assert!(core.rotate(0, 0, false).unwrap_err().contains("boundary"));
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Remove,
        ..edit
    })
    .unwrap();
    core.rotate(0, 0, false).unwrap();
    let checksum = core.checksum();
    assert!(core.undo_boundary().unwrap_err().contains("building"));
    assert_eq!(core.checksum(), checksum);
    core.rotate(0, 0, true).unwrap();
    core.undo_boundary().unwrap();
    core.entities.clear();
    core.compile_graph();
    let a = add_test_entity(&mut core, 0, 0, 2, 1);
    let b = add_test_entity(&mut core, 0, 1, 2, 1);
    core.compile_graph();
    assert_eq!(link_ids(&core, a), vec![b]);
    let checksum = core.checksum();
    assert!(core
        .edit_boundaries(&BoundaryEdit {
            action: BoundaryAction::Close,
            ..edit
        })
        .unwrap_err()
        .contains("transport"));
    assert_eq!(core.checksum(), checksum);
}

#[test]
fn masonry_walls_need_fired_masonry_and_pay_cement() {
    let mut core = empty_world("new-game");
    core.compile_graph();
    core.player.x = -3 * HEX_X;
    core.player.y = 0;
    let brick = item_id(&core, "brick");
    let cement = item_id(&core, "cement");
    let timber = item_id(&core, "timber");
    core.player.inventory = BTreeMap::from([(brick, 12), (cement, 4), (timber, 8)]);
    let brick_wall = core
        .definitions
        .boundaries
        .iter()
        .find(|d| d.key == "brick-wall")
        .unwrap()
        .id;
    let timber_wall = core
        .definitions
        .boundaries
        .iter()
        .find(|d| d.key == "timber-wall")
        .unwrap()
        .id;
    let masonry = core
        .technologies
        .technologies
        .iter()
        .find(|t| t.key == "fired-masonry")
        .unwrap()
        .id;
    let edit = BoundaryEdit {
        definition_id: brick_wall,
        ..boundary_edit(0, 0)
    };
    assert!(core
        .boundary_preview(&edit)
        .error
        .as_deref()
        .unwrap()
        .contains("Fired Masonry"));
    core.edit_boundaries(&BoundaryEdit {
        definition_id: timber_wall,
        ..edit.clone()
    })
    .unwrap();
    assert_eq!(core.player.inventory[&timber], 4);
    core.edit_boundaries(&BoundaryEdit {
        action: BoundaryAction::Remove,
        ..edit.clone()
    })
    .unwrap();
    assert_eq!(core.player.inventory[&timber], 8);
    core.insight = 8;
    core.researched.extend([5, 7]);
    core.research(masonry).unwrap();
    core.edit_boundaries(&edit).unwrap();
    assert_eq!(core.player.inventory[&brick], 9);
    assert_eq!(core.player.inventory[&cement], 3);
    assert_eq!(
        core.boundaries.values().next().unwrap().definition_id,
        brick_wall
    );
}
