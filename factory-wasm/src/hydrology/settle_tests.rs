
    /// A hand-built patch of ground. Anything outside `surveyed` panics on a bed or depth read, so
    /// "the solver never looks past the frontier" is checked by every test in this module at once.
    struct TestField {
        beds: BTreeMap<(i32, i32), i32>,
        equilibrium: BTreeMap<(i32, i32), i32>,
        ocean: BTreeSet<(i32, i32)>,
        channel: BTreeSet<(i32, i32)>,
        default_bed: i32,
        /// Cells outside this set are unsurveyed. `None` surveys everything the map names.
        surveyed: Option<BTreeSet<(i32, i32)>>,
    }

    impl TestField {
        /// A flat pan of the given radius at height `bed`.
        fn flat(radius: i32, bed: i32) -> Self {
            let beds = hexes_in_radius((0, 0), radius)
                .into_iter()
                .map(|cell| (cell, bed))
                .collect();
            Self {
                beds,
                equilibrium: BTreeMap::new(),
                ocean: BTreeSet::new(),
                channel: BTreeSet::new(),
                default_bed: bed,
                surveyed: None,
            }
        }

        /// Water the generator keeps supplied: a reach of a river rather than a pool.
        fn river(mut self, q: i32, r: i32, depth: i32) -> Self {
            self.channel.insert((q, r));
            self.equilibrium.insert((q, r), depth);
            self.beds.insert((q, r), self.default_bed - depth);
            self
        }

        fn bed(mut self, q: i32, r: i32, height: i32) -> Self {
            self.beds.insert((q, r), height);
            self
        }

        fn water(mut self, q: i32, r: i32, depth: i32) -> Self {
            self.equilibrium.insert((q, r), depth);
            self
        }

        fn sea(mut self, q: i32, r: i32) -> Self {
            self.ocean.insert((q, r));
            self.beds.insert((q, r), crate::scale::SEA_LEVEL_QUANTA - 8);
            self.equilibrium.insert((q, r), 8);
            self
        }

        fn survey(mut self, cells: &[(i32, i32)]) -> Self {
            self.surveyed = Some(cells.iter().copied().collect());
            self
        }

        fn assert_surveyed(&self, q: i32, r: i32) {
            assert!(
                self.surveyed(q, r),
                "the solver read an unsurveyed cell at {q},{r}"
            );
        }

        fn total_depth(&self, water: &DisturbedWater) -> i32 {
            self.beds
                .keys()
                .filter(|cell| !self.ocean.contains(cell))
                .map(|&(q, r)| self.equilibrium_depth(q, r) + i32::from(water.delta_at(q, r).get()))
                .sum()
        }

        fn surface(&self, water: &DisturbedWater, q: i32, r: i32) -> i32 {
            self.bed_quanta(q, r)
                + self.equilibrium_depth(q, r)
                + i32::from(water.delta_at(q, r).get())
        }
    }

    impl WaterField for TestField {
        fn bed_quanta(&self, q: i32, r: i32) -> i32 {
            self.assert_surveyed(q, r);
            self.beds.get(&(q, r)).copied().unwrap_or(self.default_bed)
        }

        fn equilibrium_depth(&self, q: i32, r: i32) -> i32 {
            self.assert_surveyed(q, r);
            self.equilibrium.get(&(q, r)).copied().unwrap_or(0)
        }

        fn surveyed(&self, q: i32, r: i32) -> bool {
            match &self.surveyed {
                Some(cells) => cells.contains(&(q, r)),
                None => self.beds.contains_key(&(q, r)),
            }
        }

        fn ocean(&self, q: i32, r: i32) -> bool {
            self.ocean.contains(&(q, r))
        }

        fn channel(&self, q: i32, r: i32) -> bool {
            self.channel.contains(&(q, r))
        }
    }

    /// The model's whole statement about a single disturbance, in the order a player produces it:
    /// an untouched world stores nothing, a cut beside a pool levels with it, an odd volume rests
    /// one quantum apart, a real step drains a cell dry, and a reopened outlet empties a bowl down
    /// its channel without losing a quantum on the way.
    #[test]
    fn water_levels_with_what_is_dug_beside_it_and_stores_only_the_difference() {
        let field = TestField::flat(4, 100).water(0, 0, 3).bed(0, 0, 94);
        let mut water = DisturbedWater::new();
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled, "an undisturbed pool is already settled");
        assert_eq!(report.transfers, 0);
        assert!(
            water.is_empty(),
            "equilibrium is not a departure and must not be stored"
        );

        // A pool six quanta deep in a pit, and a neighbouring cell dug to the same floor.
        let field = TestField::flat(4, 100)
            .bed(0, 0, 94)
            .water(0, 0, 6)
            .bed(1, 0, 94);
        let mut water = DisturbedWater::new();
        let before = field.total_depth(&water);
        let report = settle(&field, &mut water, &[(1, 0)]);
        assert!(report.settled);
        assert_eq!(report.outflow_quanta, 0, "nothing reached a boundary");
        assert_eq!(
            field.total_depth(&water),
            before,
            "a transfer between two cells of the region conserves depth"
        );
        assert_eq!(
            field.surface(&water, 0, 0),
            field.surface(&water, 1, 0),
            "two cells on one floor level exactly at an even depth"
        );
        assert_eq!(water.delta_at(1, 0).get(), 3);
        assert_eq!(water.delta_at(0, 0).get(), -3);

        // An odd volume cannot split evenly, which is the residual this model states.
        let field = TestField::flat(4, 100)
            .bed(0, 0, 94)
            .water(0, 0, 7)
            .bed(1, 0, 94);
        let mut water = DisturbedWater::new();
        settle(&field, &mut water, &[(1, 0)]);
        let gap = field.surface(&water, 0, 0) - field.surface(&water, 1, 0);
        assert_eq!(gap.abs(), 1, "an odd volume rests one quantum apart");

        // One quantum standing on ground a quantum above its neighbour: head 2, so it leaves.
        let field = TestField::flat(4, 100)
            .bed(0, 0, 100)
            .water(0, 0, 1)
            .bed(1, 0, 99);
        let mut water = DisturbedWater::new();
        settle(&field, &mut water, &[(0, 0)]);
        assert_eq!(
            field.equilibrium_depth(0, 0) + i32::from(water.delta_at(0, 0).get()),
            0,
            "the cell drained"
        );
        assert_eq!(
            field.equilibrium_depth(1, 0) + i32::from(water.delta_at(1, 0).get()),
            1,
        );

        // A bowl at 94 holding twelve quanta, and a graded channel out of it to a far lower shelf.
        let mut field = TestField::flat(6, 100).bed(0, 0, 94).water(0, 0, 12);
        for step in 1..=5 {
            field = field.bed(step, 0, 94 - step * 2);
        }
        let mut water = DisturbedWater::new();
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled, "{report:?}");
        let held = field.equilibrium_depth(0, 0) + i32::from(water.delta_at(0, 0).get());
        assert_eq!(held, 0, "the cut drained down its reopened outlet");
        assert_eq!(
            field.total_depth(&water),
            12,
            "the water went down the channel rather than out of existence"
        );
    }

    /// The two boundaries a solve can reach, and what each of them is allowed to do with the water
    /// that arrives: the ocean absorbs it and never itself departs, and the surveyed frontier lets
    /// it leave while naming what left — without either reading past the frontier or mistaking a
    /// generated pool sitting on it for a leak.
    #[test]
    fn water_leaves_at_a_boundary_without_the_boundary_being_simulated() {
        let field = TestField::flat(4, 100)
            .bed(0, 0, 20)
            .water(0, 0, 40)
            .sea(1, 0);
        let mut water = DisturbedWater::new();
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled);
        assert!(report.outflow_quanta > 0, "water reached the sea");
        assert_eq!(
            water.delta_at(1, 0),
            WaterDelta::default(),
            "the ocean is a boundary condition and never departs"
        );

        // Everything but this single cell is unsurveyed, so any read beyond it panics in the field.
        let field = TestField::flat(2, 100).bed(0, 0, 100).survey(&[(0, 0)]);
        let mut water = DisturbedWater::new();
        // Nine quanta of departure standing on dry generated ground, with nowhere surveyed to go.
        water.set(0, 0, WaterDelta::new(9));
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled);
        assert_eq!(
            report.outflow_quanta, 8,
            "water above the generated equilibrium runs off the surveyed edge"
        );
        assert_eq!(
            report.frontier.values().copied().sum::<i32>(),
            8,
            "frontier outflow is named for later survey rather than discarded"
        );
        assert_eq!(
            water.delta_at(0, 0).get(),
            1,
            "the stated residual holds at a boundary too: the last quantum has no two-quantum head"
        );
        assert_eq!(field.surface(&water, 0, 0), 101);

        // And a generated pool that happens to sit on the frontier is not a leak: nothing departs
        // and nothing is stored.
        let field = TestField::flat(2, 100)
            .bed(0, 0, 94)
            .water(0, 0, 6)
            .survey(&[(0, 0)]);
        let mut water = DisturbedWater::new();
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled);
        assert_eq!(
            report.outflow_quanta, 0,
            "a generated pool at the frontier is not a leak"
        );
        assert!(water.is_empty());
    }

    /// What the saved departure set is, as distinct from the water: the solve's answer cannot
    /// depend on the order its seeds arrived in, and a cell that is flooded and drained back is
    /// forgotten entirely rather than stored as a zero — it has to hash as untouched, or every
    /// checksum would record work that left no trace on the world.
    #[test]
    fn the_departure_set_is_order_independent_and_forgets_what_returned() {
        let build = || {
            TestField::flat(5, 100)
                .bed(0, 0, 90)
                .water(0, 0, 10)
                .bed(1, 0, 92)
                .bed(-1, 0, 91)
                .bed(0, 1, 93)
        };
        let forward = {
            let mut water = DisturbedWater::new();
            settle(&build(), &mut water, &[(0, 0), (1, 0), (-1, 0), (0, 1)]);
            water
        };
        let reversed = {
            let mut water = DisturbedWater::new();
            settle(&build(), &mut water, &[(0, 1), (-1, 0), (1, 0), (0, 0)]);
            water
        };
        assert_eq!(forward, reversed, "the solve is order independent");

        let untouched = {
            let mut hash = 0x811c_9dc5u32;
            DisturbedWater::new().hash_into(&mut hash);
            hash
        };
        let mut water = DisturbedWater::new();
        water.set(3, -2, WaterDelta::new(7));
        assert_eq!(water.len(), 1);
        let flooded = {
            let mut hash = 0x811c_9dc5u32;
            water.hash_into(&mut hash);
            hash
        };
        assert_ne!(flooded, untouched);
        water.set(3, -2, WaterDelta::new(0));
        assert!(water.is_empty(), "a returned cell leaves the departure set");
        let drained = {
            let mut hash = 0x811c_9dc5u32;
            water.hash_into(&mut hash);
            hash
        };
        assert_eq!(
            drained, untouched,
            "flooding and draining back is not a saved difference"
        );
    }

    /// What one disturbance is allowed to cost. The active region grows with the water and stops
    /// where the water does — seven cells for dry ground and seven for a pit the pool cannot climb
    /// out of — and where a command is wider than the budget the region says so, walls the water in
    /// rather than losing it, and leaves the solve unfinished for rescheduling. The sweep budget is
    /// the other half of the cost, and the shape that stresses it worst — a rough-floored bowl with
    /// one low rim cell, where every cell has somewhere to push and only one leads out — has to
    /// finish inside it without clamping or going negative anywhere.
    #[test]
    fn a_disturbance_claims_only_what_the_water_covers_and_stops_at_its_budget() {
        let field = TestField::flat(12, 100);
        let mut water = DisturbedWater::new();
        let region = active_region(&field, &[(0, 0)]);
        assert_eq!(region.len(), 7, "the seed and its ring, and no further");
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled);
        assert_eq!(report.cells, 7, "dry ground is never claimed");
        assert_eq!(report.transfers, 0);
        assert!(water.is_empty());

        // Ten quanta in a pit, and a wide flat pan around it the water cannot climb onto.
        let field = TestField::flat(12, 100).bed(0, 0, 90).water(0, 0, 10);
        let mut water = DisturbedWater::new();
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled);
        assert_eq!(
            report.cells, 7,
            "growth follows the water, and this water goes nowhere"
        );

        // A flood command wider than the budget: the seeds alone exhaust it.
        let field = TestField::flat(64, 100);
        let seeds = hexes_in_radius((0, 0), 40);
        assert!(seeds.len() > ACTIVE_CELL_BUDGET);
        let region = active_region(&field, &seeds);
        assert!(
            region.truncated(),
            "a disturbance wider than the budget must report the truncation"
        );
        assert_eq!(region.len(), ACTIVE_CELL_BUDGET);

        let mut field = TestField::flat(64, 100);
        for cell in hexes_in_radius((0, 0), 8) {
            field = field.water(cell.0, cell.1, 40);
        }
        let mut water = DisturbedWater::new();
        let before = field.total_depth(&water);
        let report = settle(&field, &mut water, &hexes_in_radius((0, 0), 40));
        assert!(report.truncated);
        assert!(
            !report.settled,
            "a truncated region is unfinished and must be rescheduled"
        );
        assert_eq!(report.outflow_quanta, 0, "no boundary was reached");
        assert_eq!(
            field.total_depth(&water),
            before,
            "the budget is a wall, not a drain"
        );

        // A bowl with a rough floor and a single low rim cell: the worst shape this model meets,
        // because every cell has somewhere to push and only one of them leads out.
        let mut field = TestField::flat(10, 200);
        for (index, cell) in hexes_in_radius((0, 0), 6).into_iter().enumerate() {
            let jitter = (index % 5) as i32;
            field = field.bed(cell.0, cell.1, 150 + jitter).water(
                cell.0,
                cell.1,
                20 + (index % 7) as i32,
            );
        }
        let mut water = DisturbedWater::new();
        let seeds: Vec<_> = hexes_in_radius((0, 0), 6);
        let report = settle(&field, &mut water, &seeds);
        assert!(report.settled, "{report:?}");
        assert!(report.sweeps < SETTLE_SWEEP_BUDGET, "{report:?}");
        assert_eq!(report.clamped, 0);
        for (q, r) in hexes_in_radius((0, 0), 6) {
            let depth = field.equilibrium_depth(q, r) + i32::from(water.delta_at(q, r).get());
            assert!(
                depth >= 0,
                "a settled cell holds no negative depth at {q},{r}"
            );
        }
    }


    /// A canal is the whole reason a channel is a head rather than a reservoir.
    ///
    /// The same trench, cut beside the same four quanta of water, twice. Beside a pool the two split
    /// what the pool had and both end up lower, because that water is all the water there is. Beside
    /// a river the trench comes up to the river's own surface and the reach is still at its generated
    /// depth when the solve stops: what the trench took came from upstream, which is what lets a
    /// player water ground the river never reached without emptying the river to do it.
    #[test]
    fn a_trench_fills_from_a_channel_without_drawing_the_channel_down() {
        let pool = TestField::flat(4, 100)
            .water(0, 0, 4)
            .bed(0, 0, 96)
            .bed(1, 0, 96);
        let mut water = DisturbedWater::new();
        let report = settle(&pool, &mut water, &[(1, 0)]);
        assert!(report.settled, "{report:?}");
        assert_eq!(report.inflow_quanta, 0, "a pool is supplied by nothing");
        assert_eq!(pool.surface(&water, 0, 0), pool.surface(&water, 1, 0));
        assert!(
            pool.surface(&water, 0, 0) < 100,
            "the pool paid for the trench out of its own depth"
        );

        let river = TestField::flat(4, 100).river(0, 0, 4).bed(1, 0, 96);
        let mut water = DisturbedWater::new();
        let report = settle(&river, &mut water, &[(1, 0)]);
        assert!(report.settled, "{report:?}");
        assert!(report.inflow_quanta > 0, "the channel supplied the fill");
        assert_eq!(
            water.delta_at(0, 0),
            WaterDelta::new(0),
            "the reach is back at its generated depth and stores no departure"
        );
        assert_eq!(
            river.surface(&water, 1, 0),
            100,
            "and the trench fills exactly to the supplied river level"
        );

        // This is the smallest useful canal cut: its floor is only one quantum below the river
        // surface. A supplied head must cross that last quarter metre; the finite-pool residual
        // rule must not leave the canal looking dry merely because the difference is odd.
        let river = TestField::flat(4, 100).river(0, 0, 4).bed(1, 0, 99);
        let mut water = DisturbedWater::new();
        let report = settle(&river, &mut water, &[(1, 0)]);
        assert!(report.settled, "{report:?}");
        assert_eq!(water.delta_at(1, 0), WaterDelta::new(1));
        assert_eq!(river.surface(&water, 1, 0), 100);
    }

    /// The recharge is a restoration, never a spring. A reach nobody has touched is already full, so
    /// it adds nothing and the untouched world still settles without moving a quantum; and a reach
    /// somebody dug out is no longer the channel the generator drew, so it stops being supplied and
    /// holds what they left in it.
    #[test]
    fn a_channel_refills_only_to_its_own_depth_and_a_dug_reach_stops_refilling() {
        let field = TestField::flat(3, 100).river(0, 0, 4);
        let mut water = DisturbedWater::new();
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled, "{report:?}");
        assert_eq!(report.inflow_quanta, 0, "a full reach is supplied by nothing");
        assert_eq!(report.transfers, 0);
        assert!(water.is_empty(), "and stores no departure");

        // Drain it by hand. The reach is still on its generated bed, so upstream puts it back.
        water.set(0, 0, WaterDelta::new(-3));
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled, "{report:?}");
        assert_eq!(report.inflow_quanta, 3);
        assert!(water.is_empty(), "the reach is at its generated depth again");

        // The same drain on a reach whose bed was cut. Nothing supplies it, and it stays down.
        let dug = TestField::flat(3, 100).river(0, 0, 4);
        let dug = TestField {
            channel: BTreeSet::new(),
            ..dug
        };
        let mut water = DisturbedWater::new();
        water.set(0, 0, WaterDelta::new(-3));
        let report = settle(&dug, &mut water, &[(0, 0)]);
        assert!(report.settled, "{report:?}");
        assert_eq!(report.inflow_quanta, 0);
        assert_eq!(water.delta_at(0, 0), WaterDelta::new(-3));
    }

    /// The defect the first canal found: a river conveying itself.
    ///
    /// A reach standing two quanta above anything is a head like any other, so a supplied reach used
    /// to pour into the sea, over the frontier and into the next reach down — and then be put back,
    /// and pour again, for as many sweeps as the budget allowed. In play, one six-quanta trench left
    /// 992 quanta of water piled on three cells the player had never surveyed, and raised half the
    /// rivers on the map by a quantum on its way there. A head fills ground beside it and nothing
    /// else, so a river descending four steps into the sea moves nothing at all.
    #[test]
    fn a_reach_supplies_ground_and_never_the_sea_the_frontier_or_the_next_reach_down() {
        // Four reaches stepping downhill: the sea below the first, the unsurveyed edge past the
        // last, and each one two quanta or more above the next.
        let stepped = || {
            TestField::flat(3, 100)
                .river(0, 0, 4)
                .sea(0, -1)
                .river(1, 0, 4)
                .bed(1, 0, 92)
                .river(2, 0, 4)
                .bed(2, 0, 88)
                .river(3, 0, 4)
                .bed(3, 0, 84)
        };
        let field = stepped();
        let mut water = DisturbedWater::new();
        let report = settle(&field, &mut water, &[(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert!(report.settled, "{report:?}");
        assert_eq!(report.transfers, 0, "a river at rest is at rest");
        assert_eq!(report.inflow_quanta, 0);
        assert_eq!(report.outflow_quanta, 0, "nothing was supplied to be lost");
        assert!(report.frontier.is_empty(), "{:?}", report.frontier);
        assert!(water.is_empty(), "and the world stores nothing");

        // A trench beside the top reach, touching that reach and no other. It fills from upstream,
        // and the water it is filled with is all the water the solve moves.
        let dug = stepped().bed(-1, 1, 94);
        let mut water = DisturbedWater::new();
        let report = settle(&dug, &mut water, &[(-1, 1)]);
        assert!(report.settled, "{report:?}");
        assert!(report.inflow_quanta > 0, "the reach supplied the trench");
        assert_eq!(report.outflow_quanta, 0, "and nothing ran past the edges");
        assert!(report.frontier.is_empty(), "{:?}", report.frontier);
        assert_eq!(
            dug.surface(&water, -1, 1),
            100,
            "the trench stands at the river level it was cut from"
        );
        for reach in [(0, 0), (1, 0), (2, 0), (3, 0)] {
            assert_eq!(
                water.delta_at(reach.0, reach.1),
                WaterDelta::new(0),
                "reach {reach:?} is still at its generated depth"
            );
        }
    }
