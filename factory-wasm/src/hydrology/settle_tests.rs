
    /// A hand-built patch of ground. Anything outside `surveyed` panics on a bed or depth read, so
    /// "the solver never looks past the frontier" is checked by every test in this module at once.
    struct TestField {
        beds: BTreeMap<(i32, i32), i32>,
        equilibrium: BTreeMap<(i32, i32), i32>,
        ocean: BTreeSet<(i32, i32)>,
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
                default_bed: bed,
                surveyed: None,
            }
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

