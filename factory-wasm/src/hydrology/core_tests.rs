    /// The three catalogues the shipped game loads, as `Core::new` and `Core::from_save` take them.
    fn catalogues() -> (DefinitionsInput, TechnologiesInput, ScenariosInput) {
        (
            serde_json::from_str(include_str!("../../../src/data/definitions.json")).unwrap(),
            serde_json::from_str(include_str!("../../../src/data/technologies.json")).unwrap(),
            serde_json::from_str(include_str!("../../../src/data/scenarios.json")).unwrap(),
        )
    }

    /// A real opening world on the physical source, surveyed around the landing shelf.
    fn physical_core() -> Core {
        let (definitions, technologies, scenarios) = catalogues();
        let core = Core::new(
            &definitions,
            &technologies,
            &scenarios.scenarios[0],
            None,
            None,
        )
        .unwrap();
        assert!(
            core.ground_is_physical(),
            "the opening is the physical world"
        );
        assert!(
            !core.generated_chunks.is_empty(),
            "the opening surveys the shelf it starts on"
        );
        core
    }

    /// A dry cell in a dry disc: no pool lip refills a pump draw.
    fn dry_cell(core: &Core) -> (i32, i32) {
        let size = core.scenario.chunk_size;
        let dry = |q: i32, r: i32| {
            let c = core.generated_ground_at(q, r);
            c.hydrology.depth_quanta == 0 && !c.presentation.is_water()
        };
        core.generated_chunks
            .iter()
            .flat_map(|&(cq, cr)| hexes_in_chunk(cq, cr, size))
            .find(|&(q, r)| dry(q, r) && DIRECTIONS.iter().all(|&(dq, dr)| dry(q + dq, r + dr)))
            .expect("the opening shelf is dry")
    }

    /// Survey outward from the origin until a surveyed cell holds inland deep water, and name it.
    fn survey_out_to_deep_water(core: &mut Core) -> Option<(i32, i32)> {
        let size = core.scenario.chunk_size;
        for ring in 0..=12 {
            for dq in -ring..=ring {
                for dr in (-ring).max(-dq - ring)..=ring.min(-dq + ring) {
                    core.generate_chunk(dq, dr);
                    let found = hexes_in_chunk(dq, dr, size).find(|&(q, r)| {
                        core.generated_ground_at(q, r).presentation == Terrain::DeepWater
                            && !core.ocean(q, r)
                    });
                    if found.is_some() {
                        return found;
                    }
                }
            }
        }
        None
    }

    /// Depth is the answer and the band is only a drawing. A meadow under water stops a walk its
    /// own band would allow, a drained deep-water cell is ground whatever the band draws, route
    /// cost and wading read the disturbed depth, and the solver reads the same finished bed the
    /// predicate does — the four ways that one claim can be got wrong.
    #[test]
    fn every_water_answer_comes_from_depth_rather_than_the_band() {
        let mut core = physical_core();
        let (q, r) = dry_cell(&core);
        assert!(!core.terrain_blocks_movement(q, r));
        assert!(!core.terrain_blocks_construction(q, r));

        // A ford: deep enough to refuse a foundation, shallow enough to wade.
        core.water.set(
            q,
            r,
            WaterDelta::new(i16::try_from(crate::scale::WADE_LIMIT_QUANTA - 1).unwrap()),
        );
        assert!(
            !core.terrain_blocks_movement(q, r),
            "water under the wade limit is a ford"
        );
        assert!(
            core.terrain_blocks_construction(q, r),
            "any standing water refuses a foundation"
        );

        core.water.set(
            q,
            r,
            WaterDelta::new(i16::try_from(crate::scale::WADE_LIMIT_QUANTA).unwrap()),
        );
        assert!(
            core.terrain_blocks_movement(q, r),
            "the band still says meadow; the predicate is the answer"
        );
        assert!(
            !core.generated_ground_at(q, r).presentation.is_water(),
            "the flood did not rewrite the generated band"
        );

        // Route cost reads the same depth: one quantum is a ford priced as one, and the wade limit
        // stops the route outright.
        core.water.set(q, r, WaterDelta::new(0));
        assert!(!core.shallow_water_at(q, r));
        core.water.set(q, r, WaterDelta::new(1));
        assert!(core.shallow_water_at(q, r), "a flooded meadow is a ford");
        let climb =
            (core.ground_elevation_at(q, r) - core.ground_elevation_at(q - 1, r)).max(0) as u32;
        assert_eq!(
            core.walk_step_cost((q - 1, r), q, r),
            WALK_SHALLOW_COST + climb * WALK_CLIMB_COST,
            "the water part of route cost is the ford cost"
        );
        core.water.set(
            q,
            r,
            WaterDelta::new(i16::try_from(crate::scale::WADE_LIMIT_QUANTA).unwrap()),
        );
        assert!(
            !core.walkable_hex(q, r),
            "deep disturbed water stops the route"
        );

        // And the solver reads the bed the predicate does — the ground the player finished, not the
        // generated bed underneath it.
        core.water.set(q, r, WaterDelta::new(6));
        assert_eq!(core.water_depth_at(q, r), 6);
        assert_eq!(
            core.water_surface_at(q, r),
            core.ground_elevation_at(q, r) + 6,
            "water stands on the ground the player finished, not on the generated bed"
        );
        assert_eq!(
            WaterField::bed_quanta(&core, q, r),
            core.ground_elevation_at(q, r),
            "the solver reads the same bed the predicate does"
        );
        core.water.set(q, r, WaterDelta::new(0));

        // The opening shelf is deliberately dry, so the surveyed rings hold no deep water at all.
        // Walk chunks outward until the generator offers an inland one — the landing site is a
        // translation of an unbounded source, so "there is water somewhere out there" is a property
        // of the generator rather than of this seed's luck.
        let (q, r) = survey_out_to_deep_water(&mut core)
            .expect("the physical generator puts inland deep water within reach of the opening");
        assert!(core.terrain_blocks_movement(q, r));
        let depth = core.water_depth_at(q, r);
        core.water
            .set(q, r, WaterDelta::new(i16::try_from(-depth).unwrap()));
        assert_eq!(core.water_depth_at(q, r), 0);
        assert!(
            !core.terrain_blocks_movement(q, r),
            "a drained cell is ground, whatever the band draws"
        );
    }

    /// A hydrology solve may never insert a gameplay chunk — not while settling, not through a
    /// player's bounded flood or drain command, and not when a survey resumes a departure that was
    /// waiting at the old frontier. Surveying is the player's decision; water arriving somewhere is
    /// not a reason to make it for them.
    #[test]
    fn no_solve_may_survey_a_chunk() {
        let mut core = physical_core();
        let (q, r) = dry_cell(&core);
        core.water.set(q, r, WaterDelta::new(40));
        let surveyed = core.generated_chunks.clone();
        let report = core.settle_water(&[(q, r)]);
        assert!(report.cells > 0);
        assert_eq!(
            core.generated_chunks, surveyed,
            "a hydrology solve may never insert a gameplay chunk"
        );

        // The player's own commands are bounded, reach the same solve, and survey nothing either.
        let mut core = physical_core();
        let (q, r) = dry_cell(&core);
        let surveyed = core.generated_chunks.clone();
        assert!(core
            .edit_water(q, r, WaterAction::Flood, WATER_COMMAND_LIMIT_QUANTA + 1)
            .unwrap_err()
            .contains("1..="));
        let report = core.edit_water(q, r, WaterAction::Flood, 3).unwrap();
        assert!(report.cells > 0);
        assert_eq!(core.generated_chunks, surveyed);
        assert!(core.dirty.water);

        core.creative = true;
        (core.player.x, core.player.y) = axial_world(q, r);
        core.apply_commands(&format!(
            r#"[{{"type":"water_edit","q":{q},"r":{r},"action":"flood","quanta":1}}]"#
        ))
        .unwrap();
        assert!(
            core.events
                .iter()
                .any(|event| event.starts_with("Water settled over")),
            "the JSON command reaches the bounded native edit"
        );

        // And a departure left waiting past the frontier resumes when the player finally surveys
        // there, opening that one chunk and no other.
        let mut core = physical_core();
        let size = core.scenario.chunk_size;
        let chunk = (20, -11);
        let target = (chunk.0 * size, chunk.1 * size);
        assert!(!core.generated_chunks.contains(&chunk));
        core.water.set(target.0, target.1, WaterDelta::new(3));
        core.dirty.water = false;
        let before = core.generated_chunks.len();
        core.generate_chunk(chunk.0, chunk.1);
        assert!(
            core.dirty.water,
            "survey ran the waiting departure through the solve"
        );
        assert_eq!(
            core.generated_chunks.len(),
            before + 1,
            "the resumed solve did not survey past its new frontier"
        );
    }

    #[test]
    fn a_finite_pump_draw_moves_depth_and_a_river_draw_obeys_its_rate() {
        let mut core = physical_core();
        let (q, r) = dry_cell(&core);
        core.water.set(q, r, WaterDelta::new(1));
        let finite = WaterSourceSnapshot {
            q,
            r,
            available: 1,
            discharge: 0,
            rate: 1,
        };
        assert!(core.draw_pump_source(finite));
        assert_eq!(core.water_depth_at(q, r), 0, "the finite cell ran dry");

        let river = WaterSourceSnapshot {
            q,
            r,
            available: 4,
            discharge: 1,
            rate: 1,
        };
        assert!(core.draw_pump_source(river));
        assert!(
            !core.draw_pump_source(river),
            "one discharge class grants one withdrawal in the tick"
        );
        core.water_draws.clear();
        assert!(
            core.draw_pump_source(river),
            "the source replenishes next tick"
        );
    }

    /// Everything the save file has to say about disturbed water: a departure is a checksum input,
    /// a world back at equilibrium hashes as one that never left it, the cells round trip through
    /// the envelope, a version-38 world resumes on the checksum it was written with, and the
    /// storage guard refuses what it must while staying wider than any legal dam.
    #[test]
    fn a_departure_is_saved_checksummed_restored_and_guarded() {
        let mut core = physical_core();
        let baseline = core.checksum();
        let (q, r) = dry_cell(&core);
        core.water.set(q, r, WaterDelta::new(5));
        assert_ne!(
            core.checksum(),
            baseline,
            "disturbed water is a checksum input"
        );
        core.water.set(q, r, WaterDelta::new(0));
        assert_eq!(
            core.checksum(),
            baseline,
            "a world back at its equilibrium hashes as one that never left it"
        );

        core.water.set(q, r, WaterDelta::new(5));
        let saved = core.save_string().expect("the world saves");
        let restored: SaveEnvelope = serde_json::from_str(
            saved
                .strip_prefix(SAVE_PREFIX)
                .expect("the save carries its prefix"),
        )
        .expect("the save parses");
        assert_eq!(restored.state.water, vec![WaterCell { q, r, departure: 5 }]);
        assert_eq!(
            DisturbedWater::from_cells(&restored.state.water),
            core.water
        );

        // The version-39 rung is a stamp and nothing else. A version-38 world could not make a
        // departure, and this version computes the same equilibrium from the same seed, so the file
        // resumes on the checksum it was written with rather than on a recomputed one.
        let core = physical_core();
        let saved = core.save_string().expect("the world saves");
        let old = saved.replace(
            &format!("\"save_version\":{SAVE_VERSION}"),
            "\"save_version\":38",
        );
        assert_ne!(old, saved, "the stamp was found and rewritten");

        let (definitions, technologies, scenarios) = catalogues();
        let restored = Core::from_save(&definitions, &technologies, &scenarios, &old)
            .expect("a version-38 world resumes");
        assert!(restored.water.is_empty(), "it had no departure to carry");
        assert_eq!(
            restored.checksum(),
            core.checksum(),
            "and it hashes exactly what it hashed before hydrology existed"
        );

        // The guard refuses a departure past its limit and a cell named twice, and the limit itself
        // is wider than anything the generator makes but narrower than the integer it guards — so a
        // legal dam can never reach it and the guard can never overflow.
        let past = [WaterCell {
            q: 0,
            r: 0,
            departure: i16::try_from(DEPARTURE_LIMIT_QUANTA + 1).unwrap(),
        }];
        assert!(validate_saved_water(&past).is_err());
        let twice = [
            WaterCell {
                q: 2,
                r: -1,
                departure: 3,
            },
            WaterCell {
                q: 2,
                r: -1,
                departure: -3,
            },
        ];
        assert!(validate_saved_water(&twice).is_err());
        assert!(validate_saved_water(&twice[..1]).is_ok());

        let relief = crate::scale::BED_MAX_QUANTA - crate::scale::BED_MIN_QUANTA;
        assert!(
            DEPARTURE_LIMIT_QUANTA > relief,
            "a legal dam must not be able to reach the storage guard"
        );
        assert!(
            i32::from(i16::MAX) > DEPARTURE_LIMIT_QUANTA,
            "the guard must fit the integer it guards"
        );
    }

    #[test]
    fn a_reversal_puts_back_what_a_solve_forgot_and_drops_what_it_invented() {
        let before = DisturbedWater::from_cells(&[
            WaterCell {
                q: 0,
                r: 0,
                departure: 3,
            },
            WaterCell {
                q: 1,
                r: 0,
                departure: -2,
            },
        ]);
        let after = DisturbedWater::from_cells(&[
            WaterCell {
                q: 1,
                r: 0,
                departure: -2,
            },
            WaterCell {
                q: 2,
                r: 0,
                departure: 5,
            },
        ]);
        assert_eq!(
            before.reversal_of(&after),
            vec![
                WaterCell {
                    q: 0,
                    r: 0,
                    departure: 3,
                },
                WaterCell {
                    q: 2,
                    r: 0,
                    departure: 0,
                },
            ],
            "a cell the solve left alone is not in the record"
        );
        let mut restored = after.clone();
        restored.apply(&before.reversal_of(&after));
        assert_eq!(restored, before, "and applying it is a true inverse");
    }

    /// One hex, lowered by `steps` grade steps, priced and committed the way a player's drag is.
    fn lower(q: i32, r: i32, steps: u8) -> GroundEdit {
        GroundEdit {
            q,
            r,
            to_q: q,
            to_r: r,
            corner: 0,
            to_corner: 0,
            shape: GroundShape::Cell,
            definition_id: 2,
            action: GroundAction::Lower,
            steps,
            reference: GroundReference::default(),
            cover: false,
        }
    }

    /// Whether this hex would take that cut whole — no obstacle, no deposit, no refusal.
    fn diggable(core: &Core, (q, r): (i32, i32), steps: u8) -> bool {
        let preview = core.ground_preview(&lower(q, r, steps));
        preview.error.is_none() && preview.blocked == 0 && preview.changes > 0
    }

    /// A surveyed, dry, diggable hex whose six neighbours are all surveyed, dry and diggable too, so
    /// a pond dug into one of them has a bank the test can compute rather than guess.
    fn pit_and_bank(core: &Core) -> ((i32, i32), (i32, i32)) {
        let size = core.scenario.chunk_size;
        let dry = |core: &Core, (q, r): (i32, i32)| {
            core.surveyed(q, r)
                && core.water_depth_at(q, r) == 0
                && !core.generated_ground_at(q, r).presentation.is_water()
        };
        core.generated_chunks
            .iter()
            .flat_map(|&(cq, cr)| hexes_in_chunk(cq, cr, size))
            .filter(|&c| dry(core, c) && diggable(core, c, 4))
            .find_map(|(q, r)| {
                let ring: Vec<(i32, i32)> = DIRECTIONS
                    .iter()
                    .map(|&(dq, dr)| (q + dq, r + dr))
                    .collect();
                // The bank has to be the rim itself, not merely a neighbour. The pond stands one
                // quantum under the lowest bed around it, so a cut into any higher neighbour can
                // leave that neighbour still above the water and the test would be asserting the
                // model failed to move water uphill. Picking the lowest neighbour makes "the
                // pond's surface is suddenly above it" true by construction, whatever relief the
                // generator lays down here.
                let rim = ring
                    .iter()
                    .map(|&(cq, cr)| core.ground_elevation_at(cq, cr))
                    .min()
                    .expect("a hex has neighbours");
                ring.iter()
                    .all(|&cell| dry(core, cell))
                    .then(|| {
                        ring.iter()
                            .copied()
                            .find(|&cell| {
                                core.ground_elevation_at(cell.0, cell.1) == rim
                                    && diggable(core, cell, 2)
                            })
                            .map(|bank| ((q, r), bank))
                    })
                    .flatten()
            })
            .expect("the opening shelf has an open pair of dry hexes")
    }

    /// Digging beside standing water floods the cut, and nothing had to ask it to. The earthwork
    /// moved the bed, and the bed is what the water stands on.
    #[test]
    fn a_cut_beside_a_pond_floods_and_the_undo_puts_the_water_back() {
        let mut core = physical_core();
        core.set_creative(true);
        let ((pit_q, pit_r), (bank_q, bank_r)) = pit_and_bank(&core);

        // A pit next door, and a pond in it standing exactly one quantum under the lowest bed around
        // it. One quantum is head the model will not move on, so this is a world already at rest.
        core.edit_ground(&lower(pit_q, pit_r, 4)).unwrap();
        let floor = core.ground_elevation_at(pit_q, pit_r);
        let rim = DIRECTIONS
            .iter()
            .map(|&(dq, dr)| core.ground_elevation_at(pit_q + dq, pit_r + dr))
            .min()
            .expect("a hex has neighbours");
        let depth = rim + 1 - floor;
        assert!(depth > 0, "the cut put the floor below its own rim");
        core.water
            .set(pit_q, pit_r, WaterDelta::new(i16::try_from(depth).unwrap()));
        core.settle_water(&[(pit_q, pit_r)]);
        assert_eq!(
            core.water_depth_at(pit_q, pit_r),
            depth,
            "a pond under its rim has nowhere to go"
        );
        let held: i32 = core.water.iter().map(|(_, d)| i32::from(d.get())).sum();
        assert_eq!(held, depth, "and the shelf around it is dry");
        assert_eq!(
            core.snapshot().water,
            core.water.cells(),
            "the snapshot is the departure set, not a second picture of it"
        );
        let checksum = core.checksum();

        // Now cut the bank. The pond's surface is suddenly above it, and the water finds the cut.
        core.events.clear();
        core.edit_ground(&lower(bank_q, bank_r, 2)).unwrap();
        assert!(
            core.water_depth_at(bank_q, bank_r) > 0,
            "the cut took water nobody handed it"
        );
        assert!(
            core.water_depth_at(pit_q, pit_r) < depth,
            "and the pond is what gave it up"
        );
        assert_eq!(
            core.water
                .iter()
                .map(|(_, d)| i32::from(d.get()))
                .sum::<i32>(),
            held,
            "the water was moved, not made"
        );
        assert_eq!(
            core.snapshot().water,
            core.water.cells(),
            "the flood the solve left is the flood the host is told about"
        );
        assert!(
            core.events
                .iter()
                .any(|event| event.contains("Water found the new grade")),
            "{:?}",
            core.events
        );

        // Undo restores the ground and the water that was standing on it, exactly. The water is put
        // back from the record rather than solved for again, so this is an identity and not a second
        // opinion that happens to agree.
        core.undo_ground().unwrap();
        assert_eq!(core.water_depth_at(bank_q, bank_r), 0);
        assert_eq!(core.water_depth_at(pit_q, pit_r), depth);
        assert_eq!(core.checksum(), checksum, "the world came back exactly");

        // The common case is the other half of the same rule, and it must stay free: a grade with no
        // water anywhere near it leaves no departure, says nothing about water, does not open the
        // world to look, and hashes as if hydrology were not here.
        let mut core = physical_core();
        core.set_creative(true);
        let ((q, r), _) = pit_and_bank(&core);
        let checksum = core.checksum();
        let chunks = core.generated_chunks.len();

        core.events.clear();
        core.edit_ground(&lower(q, r, 1)).unwrap();
        assert!(core.water.is_empty(), "dry ground disturbs no water");
        assert!(
            !core
                .events
                .iter()
                .any(|event| event.contains("Water found the new grade")),
            "{:?}",
            core.events
        );
        assert_eq!(
            core.generated_chunks.len(),
            chunks,
            "and the settle did not open the world to look"
        );

        core.undo_ground().unwrap();
        assert_eq!(core.checksum(), checksum);
    }
