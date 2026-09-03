    const SEED: u32 = 0x5EED_A17E;

    /// A modest patch, so the invariant tests cover several provinces and a seam in each
    /// direction without making `cargo test` slow.
    fn patch() -> Vec<(i32, i32)> {
        let mut cells = Vec::new();
        for r in -PROVINCE_CELL..(2 * PROVINCE_CELL) {
            for q in -PROVINCE_CELL..(2 * PROVINCE_CELL) {
                cells.push((q, r));
            }
        }
        cells
    }

    /// The macro graph is a forest: every edge strictly decreases `(rank, pq, pr)`, so following
    /// outlets from anywhere terminates rather than looping. Both halves are asserted — the
    /// ordering on each edge, and the termination it is supposed to buy.
    #[test]
    fn province_outlets_strictly_descend_and_their_chains_terminate() {
        for pr in -6..6 {
            for pq in -6..6 {
                if let Outlet::Province { pq: nq, pr: nr } = province_outlet(SEED, pq, pr) {
                    let here = (province_rank(SEED, pq, pr), pq, pr);
                    let there = (province_rank(SEED, nq, nr), nq, nr);
                    assert!(
                        there < here,
                        "province ({pq},{pr}) drains to a higher outlet"
                    );
                }

                // Which is the same claim followed rather than checked one edge at a time: from
                // anywhere, the chain of outlets ends.
                let mut current = (pq, pr);
                let mut steps = 0;
                while let Outlet::Province { pq: nq, pr: nr } =
                    province_outlet(SEED, current.0, current.1)
                {
                    current = (nq, nr);
                    steps += 1;
                    assert!(
                        steps < 4_096,
                        "outlet chain from ({pq},{pr}) did not terminate"
                    );
                }
            }
        }
    }

    /// Both sides of a seam name the same pour point. Without this a channel would arrive at a
    /// different cell depending on which province was asked, and every province boundary in the
    /// world would show a tear.
    #[test]
    fn seam_pour_points_agree_from_both_sides() {
        for pr in -3..3 {
            for pq in -3..3 {
                for (dq, dr) in PROVINCE_FACES {
                    let neighbour = (pq + dq, pr + dr);
                    let (mine, theirs) = seam_pour(SEED, (pq, pr), neighbour);
                    let (their_side, my_side) = seam_pour(SEED, neighbour, (pq, pr));
                    assert_eq!(mine, my_side);
                    assert_eq!(theirs, their_side);
                    assert_eq!(province_of(mine.0, mine.1), (pq, pr));
                    assert_eq!(province_of(theirs.0, theirs.1), neighbour);
                    assert_eq!(axial_distance(mine, theirs), 1);
                }
            }
        }

        // The cells on either side of a seam are computed identically whether the province was
        // solved on its own or after its neighbours — the halo is an implementation detail, not a
        // place where results are approximate.
        let mut alone = Terra::new(SEED);
        let border = alone.province(0, 0);
        let mut surrounded = Terra::new(SEED);
        for pr in -1..=1 {
            for pq in -1..=1 {
                surrounded.province(pq, pr);
            }
        }
        let after = surrounded.province(0, 0);
        for r in 0..PROVINCE_CELL {
            for q in 0..PROVINCE_CELL {
                assert_eq!(border.head(q, r), after.head(q, r), "height at ({q},{r})");
                assert_eq!(border.flow(q, r), after.flow(q, r), "flow at ({q},{r})");
            }
        }
    }

    /// The claim caching is allowed to make, and the only one: the same answer either way — for a
    /// single cell against the uncached oracle, and for a whole patch walked in reverse. What
    /// caching is allowed to change is the cost, which is bounded here too.
    #[test]
    fn the_cache_changes_the_cost_and_nothing_else() {
        let mut terra = Terra::new(SEED);
        for (q, r) in [
            (0, 0),
            (-1, -1),
            (PROVINCE_CELL - 1, 0),
            (PROVINCE_CELL, 0),
            (0, PROVINCE_CELL - 1),
            (0, PROVINCE_CELL),
            (-PROVINCE_CELL, -PROVINCE_CELL),
            (3 * PROVINCE_CELL + 7, -2 * PROVINCE_CELL - 5),
        ] {
            assert_eq!(
                terra.head(q, r),
                Terra::head_uncached(SEED, q, r),
                "cached and uncached height disagree at ({q},{r})"
            );
        }

        // Query order cannot matter either: two caches walked in opposite directions must agree
        // cell for cell, height, flow and water alike.
        let cells = patch();
        let mut forward = Terra::new(SEED);
        let mut backward = Terra::new(SEED);
        let mut readings = Vec::with_capacity(cells.len());
        for &(q, r) in &cells {
            readings.push((forward.head(q, r), forward.flow(q, r), forward.water(q, r)));
        }
        for (index, &(q, r)) in cells.iter().enumerate().rev() {
            let expected = readings[index];
            assert_eq!(
                (
                    backward.head(q, r),
                    backward.flow(q, r),
                    backward.water(q, r)
                ),
                expected,
                "reverse query order changed ({q},{r})"
            );
        }

        // And solving one cell costs a bounded number of provinces: reading a whole province does
        // not pull in a continent.
        let mut single = Terra::new(SEED);
        single.head(0, 0);
        assert_eq!(single.provinces_solved(), 1);
        for r in 0..PROVINCE_CELL {
            for q in 0..PROVINCE_CELL {
                single.head(q, r);
            }
        }
        assert_eq!(single.provinces_solved(), 1);
    }

    /// The drainage invariants the brief names as acceptance, over a real patch of world.
    #[test]
    fn drainage_never_runs_uphill_and_never_cycles() {
        let mut terra = Terra::new(SEED);
        for (q, r) in patch() {
            let here = terra.head(q, r);
            if let Some((nq, nr)) = terra.downstream(q, r) {
                let there = terra.head(nq, nr);
                assert!(there <= here, "({q},{r}) at {here} flows uphill to {there}");
                let (here_mq, there_mq) = (terra.head_mq(q, r), terra.head_mq(nq, nr));
                assert!(
                    (there_mq, nq, nr) < (here_mq, q, r),
                    "({q},{r}) flows to an equal-or-greater key, which would admit a cycle"
                );
            }
        }

        // Followed rather than checked edge by edge: every path that is not a lake reaches a
        // declared outlet — the sea, a lake, or an honestly reported frontier basin. Nothing
        // wanders forever.
        for r in (-PROVINCE_CELL..(2 * PROVINCE_CELL)).step_by(7) {
            for q in (-PROVINCE_CELL..(2 * PROVINCE_CELL)).step_by(7) {
                let (mut cq, mut cr) = (q, r);
                let mut steps = 0u32;
                loop {
                    if terra.head(cq, cr) < SEA_LEVEL_QUANTA {
                        break;
                    }
                    match terra.flow(cq, cr) {
                        Flow::To(direction) => {
                            let (dq, dr) = DIRECTIONS[direction as usize];
                            cq += dq;
                            cr += dr;
                        }
                        Flow::Lake(_) | Flow::Frontier => break,
                    }
                    steps += 1;
                    assert!(steps < 20_000, "the path from ({q},{r}) never terminated");
                }
            }
        }

        // A lake is where a path stops, so the same walk has to find the retained water honest: a
        // lake reports the rim it spills over, and that rim stands at or above every cell it
        // covers. A lake surface below its own bed would be the model lying about water.
        let mut found = 0;
        for pr in -1..=1 {
            for pq in -1..=1 {
                let province = terra.province(pq, pr);
                for lake in province.lakes() {
                    assert!(lake.cells > 0);
                }
                let (origin_q, origin_r) = province_origin(pq, pr);
                for r in origin_r..(origin_r + PROVINCE_CELL) {
                    for q in origin_q..(origin_q + PROVINCE_CELL) {
                        if let Some(Flow::Lake(id)) = province.flow(q, r) {
                            let lake = province.lake(id);
                            let head = province.head_mq(q, r).expect("own cell");
                            assert!(
                                lake.spill_mq >= head,
                                "lake surface {} is below its bed {head} at ({q},{r})",
                                lake.spill_mq
                            );
                            found += 1;
                        }
                    }
                }
            }
        }
        // The prototype is worth nothing if it produces no basins at all; a landscape with no
        // closed depression anywhere has been smoothed until it stopped being terrain.
        assert!(found > 0, "no lake cells anywhere in nine provinces");
    }

    /// Springs sit above sea level, on damp ground, at the head of a channel and inside their own
    /// province.
    ///
    /// Deliberately not "every province has a spring". A spring needs a channel head, and a
    /// channel needs [`CHANNEL_CLASS_MIN`] — about five hectares of catchment — so a province gets
    /// roughly one. Nine provinces finding none is ordinary, which is why the sample is 49 and the
    /// assertion is about the predicate rather than the density.
    #[test]
    fn springs_are_wet_high_ground() {
        let (cq, cr) = highest_province(SEED);
        let mut found = 0;
        for pr in cr - 3..=cr + 3 {
            for pq in cq - 3..=cq + 3 {
                let spine = build_spine(SEED, pq, pr, &[]);
                for &(q, r) in &spine.springs {
                    assert_eq!(
                        province_of(q, r),
                        (pq, pr),
                        "a spring outside its own province"
                    );
                    // Altitude eases the threshold, but never past the cap, so this is the
                    // weakest moisture any spring can have at any height.
                    assert!(
                        moisture(SEED, q, r) > SPRING_MOISTURE - SPRING_ALTITUDE_CAP,
                        "a dry spring"
                    );
                    assert!(
                        continental_mq(SEED, q, r) > SEA_LEVEL_QUANTA,
                        "a spring below sea level"
                    );
                    // A spring is the top of a channel, so the cell it names has to be one.
                    let channel = spine
                        .channels
                        .get(&(q, r))
                        .expect("a spring off the channel");
                    assert!(channel.class >= CHANNEL_CLASS_MIN);
                    assert!(channel.wet, "a spring that starts no water");
                    found += 1;
                }
            }
        }
        assert!(found > 0, "no springs anywhere in forty-nine provinces");
    }

    /// The province with the most height in it, within a couple of continental wavelengths.
    ///
    /// The origin is not land. At [`SEED`] it sits about 300 m under water, which is the generator
    /// working — a world with a sea in it has to put some seeds in the sea. Tests that are about
    /// hills, springs and rivers have to say where the hills are rather than assuming the origin.
    fn highest_province(seed: u32) -> (i32, i32) {
        let reach = CONTINENT_PROVINCES * 2;
        let mut best = ((0, 0), i32::MIN);
        for pr in (-reach..=reach).step_by(2) {
            for pq in (-reach..=reach).step_by(2) {
                let (q, r) = province_origin(pq, pr);
                let height = continental_mq(seed, q, r);
                if height > best.1 {
                    best = ((pq, pr), height);
                }
            }
        }
        assert!(
            best.1 > SEA_LEVEL_QUANTA,
            "no land within two continental wavelengths of the origin"
        );
        best.0
    }

    /// Discharge classes are monotone in catchment and saturate rather than overflowing, and every
    /// width those classes buy stays inside the halo the solve computes — which is what makes a
    /// cell's height complete before anyone outside the province reads it.
    #[test]
    fn discharge_classes_are_monotone_and_no_valley_outgrows_the_halo() {
        let mut last = 0;
        for exponent in 0..40 {
            let class = discharge_class(1u64 << exponent);
            assert!(class >= last);
            last = class;
        }
        assert_eq!(discharge_class(u64::MAX), 7);
        assert_eq!(discharge_class(0), 0);

        for class in 0..=7u8 {
            assert!(valley_half_width(class) <= VALLEY_RADIUS);
            assert!(river_half_width(class) <= VALLEY_RADIUS);
            assert!(river_half_width(class) + river_bench_width(class) <= VALLEY_RADIUS);
        }
        assert_eq!(river_half_width(CHANNEL_CLASS_MIN) * 2 + 1, 3);
        assert_eq!(river_half_width(3) * 2 + 1, 5);
        assert_eq!(river_half_width(4) * 2 + 1, 7);
        assert_eq!(river_half_width(5) * 2 + 1, 9);
        assert_eq!(river_half_width(6) * 2 + 1, 11);
        assert_eq!(river_half_width(7) * 2 + 1, 13);
        assert_eq!(river_bench_width(CHANNEL_CLASS_MIN), 1);
        assert_eq!(river_bench_width(5), 2);
        assert!(HALO > VALLEY_RADIUS);

        // Width and depth are one design, not two tables that happen to sit near each other. The
        // bed climbs [`CHANNEL_CROSS_GRADE_MQ`] a cell from the thalweg out, so a class whose
        // half-width caught up with its depth would publish a dry lane inside its own river.
        for class in CHANNEL_CLASS_MIN..=7 {
            let margin_mq = (bed_depth(class) - river_half_width(class)) * MQ as i32;
            assert!(
                margin_mq >= 2 * MQ as i32,
                "class {class} leaves only {margin_mq} mq at its waterline"
            );
        }

        // The ladder has to spend its classes on catchments this generator produces. The largest
        // basin the upstream walk can count is the whole of its budget; a top class further away
        // than that is a class no river ever reaches.
        let countable =
            UPSTREAM_PROVINCE_BUDGET as u64 * (PROVINCE_CELL as u64 * PROVINCE_CELL as u64);
        assert_eq!(discharge_class(countable), 7);
        assert!(discharge_class(countable / 8) >= CHANNEL_CLASS_MIN + 2);
    }

    #[test]
    fn landing_shelves_are_deterministic_dry_and_buildable() {
        for seed in [SEED, 1_213_486_160, 0xA11C_E551] {
            let mut first = Terra::new(seed);
            let site = first.landing_site();
            assert!(
                first.provinces_solved() <= LANDING_PROVINCE_SOLVE_BUDGET,
                "landing search exceeded its province budget"
            );
            let mut reordered = Terra::new(seed);
            for &(q, r) in &[(8_000, -4_000), (-6_000, 3_000), (128, 128)] {
                reordered.head(q, r);
            }
            assert_eq!(site, reordered.landing_site());

            let pad = hexes_in_radius((site.q, site.r), LANDING_PAD_RADIUS);
            let clear = hexes_in_radius((site.q, site.r), LANDING_CLEAR_RADIUS);
            let heights: Vec<_> = pad.iter().map(|&(q, r)| first.head(q, r)).collect();
            assert!(
                heights.iter().max().unwrap() - heights.iter().min().unwrap()
                    <= crate::scale::MAX_BUILD_STEP_QUANTA,
                "seed {seed} chose an uneven opening pad at {},{}",
                site.q,
                site.r
            );
            assert!(
                clear.iter().all(|&(q, r)| !first.water(q, r).is_wet()),
                "seed {seed} chose a wet opening at {},{}",
                site.q,
                site.r
            );
            assert!(clear
                .iter()
                .all(|&(q, r)| DIRECTIONS.iter().all(|&(dq, dr)| {
                    (first.head(q, r) - first.head(q + dq, r + dr)).abs()
                        <= crate::scale::MAX_WALK_STEP_QUANTA
                })));
            assert!(
                site.bed_quanta <= LANDING_ALTITUDE_CEILING,
                "seed {seed} landed at {} m rather than on the coastal plain",
                f64::from(site.bed_quanta) * 0.25
            );
            // The opening stands back from the surf rather than on it. The beach has to be inside
            // the first pump's walk, and outside the clearing by enough that the coastal plain
            // holding the reaches that run to the ocean is what the opening is standing on.
            let beach = first
                .sea_distance((site.q, site.r), LANDING_BEACH_RADIUS)
                .unwrap_or_else(|| {
                    panic!(
                        "seed {seed} has no ocean beach within {LANDING_BEACH_RADIUS} cells of the landing"
                    )
                });
            assert!(
                (LANDING_BEACH_MIN..=LANDING_BEACH_RADIUS).contains(&beach),
                "seed {seed} landed {beach} cells from the sea"
            );
        }
    }

    /// A different seed is a different world; the same seed is the same world twice; and both are
    /// worlds the survey certifies and the rescale earned.
    #[test]
    fn seeds_separate_worlds_that_survey_clean_with_real_relief() {
        let mut first = Terra::new(SEED);
        let mut again = Terra::new(SEED);
        let mut other = Terra::new(SEED ^ 0x9999);
        let mut differences = 0;
        for r in 0..64 {
            for q in 0..64 {
                assert_eq!(first.head(q, r), again.head(q, r));
                if first.head(q, r) != other.head(q, r) {
                    differences += 1;
                }
            }
        }
        assert!(
            differences > 3_000,
            "two seeds produced nearly the same world"
        );

        // The relief in those worlds has to be worth the rescale: a world that is flat at
        // 25 m² per cell has not earned the compatibility break. The wide measurement is taken on
        // the bare height field rather than on a survey, because the claim is about the continental
        // wavelength — 33 km — and no sample small enough to solve inside a test can span one.
        let step = PROVINCE_CELL;
        let reach = 32; // 32 provinces each way: 68 km, two continental wavelengths.
        let mut low = i32::MAX;
        let mut high = i32::MIN;
        for r in -reach..=reach {
            for q in -reach..=reach {
                let height = base_mq(SEED, q * step, r * step);
                low = low.min(height);
                high = high.max(height);
            }
        }
        let range = (high - low) / MQ as i32;
        // 1,200 quanta is 300 m. Below that the rescale buys nothing a band enum could not fake.
        assert!(
            range > 1_200,
            "only {range} quanta of relief across 68 km of the height field"
        );
    }
