use super::*;

/// The stated curve, and the proof that stating it is not the same as describing it.
///
/// Two rules, both claims about the data rather than about taste. A tier costs strictly more
/// than the tier it upgrades from, and a machine costs no less than a machine of the same kind
/// whose technology it is unlocked behind. The negative case is the point: price a cutter in one stone and the curve breaks, because a cutter
/// two technologies past a smelter costs less than the smelter.
#[test]
fn the_economy_holds_at_every_step_of_the_curve() {
    let report = balance::compute();
    assert!(!report.curve.is_empty());
    for step in &report.curve {
        assert!(
            step.holds,
            "{} ({}) follows {} ({}) by {} and does not cost more",
            step.building,
            step.effort_milli,
            step.follows,
            step.follows_effort_milli,
            step.relation
        );
    }

    let mut broken: DefinitionsInput = serde_json::from_str(DEFINITIONS).unwrap();
    let technologies: TechnologiesInput = serde_json::from_str(TECHNOLOGIES).unwrap();
    let cutter = broken
        .buildings
        .iter_mut()
        .find(|building| building.key == "cutter")
        .expect("the cutter is in the catalogue");
    cutter.construction_cost = vec![Ingredient {
        item_id: STONE,
        quantity: 1,
    }];
    let broken = balance::compute_from(broken, technologies);
    assert!(
        broken.curve.iter().any(|step| !step.holds),
        "a cheaper-than-its-predecessor building has to fail the curve"
    );

    // The two rates a player compares without being told they are comparing them: their own
    // hands, and the first machine that replaces them.
    //
    // These are measured against the same wall clock, and the order between them is a design
    // decision that has now been made twice in opposite directions. Through v0.16 the hand ran at
    // 300 items a minute against an extractor's 120, so the first automation in the game was two
    // and a half times slower than doing it yourself, which read as a punishment. v0.17 made them
    // equal and v0.23 pinned the hand as never faster.
    //
    // This inverts that on purpose. A tier-one extractor is *half* the hand on the same material
    // and the deep extractor is what draws level. The trade is no longer speed — it is that the
    // machine works while the player is somewhere else, so automation becomes a question of how
    // many you can afford to run and to power rather than of raw rate. The reason the old rule
    // existed still holds and is still guarded: what must never happen is an upgrade that leaves
    // a player slower than their own hands with no way up.
    let report = balance::compute();
    let rate_for = |building: &str, item: &str| -> u64 {
        report
            .machines
            .iter()
            .find(|machine| {
                machine.building == building && machine.output_item.as_deref() == Some(item)
            })
            .map(|machine| machine.per_minute_milli)
            .unwrap_or(0)
    };
    assert!(
        !report.reference.hand_gathers.is_empty(),
        "the hand still takes something"
    );
    for gather in &report.reference.hand_gathers {
        let hand = u64::from(gather.items_per_minute) * 1000;
        let tier_one = rate_for("extractor", &gather.item);
        let deep = rate_for("extractor-ii", &gather.item);
        // Half, within what a whole number of ticks allows: sand and clay want 13.33 ticks and
        // are given 13, which is the only material pair the ladder does not hit exactly.
        assert!(
            tier_one * 100 > hand * 45 && tier_one * 100 < hand * 55,
            "{} tier one at {tier_one} is not half the hand at {hand}",
            gather.item
        );
        // The way up has to exist, or this is the v0.16 punishment again with extra steps.
        assert!(
            deep * 100 > hand * 94,
            "{} deep extractor at {deep} does not reach the hand at {hand}",
            gather.item
        );
    }
    // Crystal is the one material with an extraction rate and no hand rate, and it is the
    // slowest thing dug: twice the ore it shares the highland with.
    assert_eq!(
        rate_for("extractor", "crystal") * 2,
        rate_for("extractor", "ore"),
        "crystal takes twice as long as iron"
    );
    let wood = report
        .reference
        .hand_gathers
        .iter()
        .find(|gather| gather.item == "wood")
        .expect("wood is the fastest hand");
    assert!(
        report
            .reference
            .hand_gathers
            .iter()
            .any(|gather| gather.item == "ore" && gather.items_per_minute < wood.items_per_minute),
        "hard rock has to be slower than wood"
    );
    assert!(
        !report
            .reference
            .hand_gathers
            .iter()
            .any(|gather| gather.item == "crystal"),
        "signal crystal is machine only"
    );
    // Both work the same seven cells: reach is what an upgrade buys, never what a hand grows.
    assert_eq!(report.reference.cells_in_reach.first(), Some(&7));

    // A fuel recipe that hands back more energy than it was given is a perpetual motion machine.
    //
    // Charcoal was exactly that: two wood at two energy each into one charcoal at eight, for a
    // kiln that needs no fuel of its own. Wood regrows, so the char recipe was an unbounded free
    // power source sitting one technology into the tree. Real pyrolysis burns part of the charge
    // to cook the rest and keeps a quarter to a half of the wood's energy — a kiln is bought for
    // **density**, four times the energy in one belt slot, and never for energy itself.
    //
    // So the band is two-sided. Above 1000 the world makes energy from nothing; below 250 the
    // kiln burns more than the worst real one and nobody would run it. Fuel is a property of the
    // item, so this is the one place the round trip can be checked at all — nothing in a recipe
    // row knows what its inputs burn for.
    let report = balance::compute();
    let converted: Vec<_> = report
        .fuel
        .iter()
        .filter(|entry| entry.recipe.is_some())
        .collect();
    assert!(!converted.is_empty(), "some fuel is crafted");
    for entry in converted {
        let gain = entry.gain_milli.unwrap_or(0);
        assert!(
            gain <= 1000,
            "{} returns {} energy for {} — a kiln is not a power source",
            entry.item,
            entry.output_energy,
            entry.input_energy
        );
        assert!(
            gain >= 250,
            "{} returns {} energy for {} — worse than the worst kiln anyone has built",
            entry.item,
            entry.output_energy,
            entry.input_energy
        );
    }

    // Processing has to pay for itself, and the request board is where it is paid.
    //
    // A request that pays no better per gather than a raw one is a request nobody would ever build
    // a machine for: the smelter costs research, construction, power, and fuel, and the hub would
    // be offering the same rate for two ore as for the plate they became. So every row whose item
    // comes out of a recipe has to beat every row whose item comes out of the ground — measured
    // through the whole tree, fuel included, which is the only comparison that is not a guess.
    let report = balance::compute();
    let raw: Vec<_> = report
        .requests
        .iter()
        .filter(|request| request.machine_ticks == 0)
        .collect();
    let processed: Vec<_> = report
        .requests
        .iter()
        .filter(|request| request.machine_ticks > 0)
        .collect();
    assert!(raw.len() >= 7, "the eight opening materials, less water");
    assert!(processed.len() >= 10, "a ladder, not one processed row");
    let best_raw = raw
        .iter()
        .map(|request| request.insight_per_gather_milli)
        .max()
        .expect("a raw request");
    for request in processed {
        assert!(
            request.insight_per_gather_milli > best_raw,
            "{} pays {} insight per thousand gathers and the best raw row pays {} — nobody \
                 would build the machine",
            request.request,
            request.insight_per_gather_milli,
            best_raw
        );
    }

    // The finite catalogue pays for the whole purchasable tree, with room to spare.
    //
    // This is the safeguard that makes finite demand shippable at all. Once a project retires
    // there is no tap to fall back on, so if the catalogue ever priced below the tree a player
    // could deliver every commission the hub will ever make and still be locked out of research
    // with nothing left to sell. The surplus is deliberate: it has to be possible to spend on the
    // wrong branch first and still finish, or the "choice" of what to research is a quiz.
    let report = balance::compute();
    let budget = &report.budget;
    assert!(
        budget.project_insight >= budget.research_cost,
        "{} projects pay {} against {} of research",
        budget.projects,
        budget.project_insight,
        budget.research_cost
    );
    assert!(
        budget.surplus_ratio_milli >= 1250,
        "the catalogue pays {} for {} of research — {}x, and a wrong purchase order strands \
             the player",
        budget.project_insight,
        budget.research_cost,
        budget.surplus_ratio_milli as f64 / 1000.0
    );
    // The grants are not bought, and counting them as cost would flatter the ratio above.
    assert!(
        !budget.granted_technologies.is_empty(),
        "the contract stages still hand technologies over"
    );

    // Every raw project the hub will ever post, added up, is less than the technology tree.
    //
    // Raw rows are the bootstrap and nothing more. Because each pays exactly once, this sum *is*
    // the entire lifetime income of hand-gathering, so the assertion is now a hard floor rather
    // than a rate comparison: a player who never builds a machine cannot finish the tree, and the
    // processed rows are the only way across.
    let report = balance::compute();
    let tree: u32 = {
        let technologies: TechnologiesInput = serde_json::from_str(TECHNOLOGIES).unwrap();
        technologies
            .technologies
            .iter()
            .map(|technology| technology.cost)
            .sum()
    };
    let raw_cycle: u32 = report
        .requests
        .iter()
        .filter(|request| request.machine_ticks == 0)
        .map(|request| request.insight)
        .sum();
    assert!(
        raw_cycle < tree,
        "one cycle of raw requests pays {raw_cycle} and the tree costs {tree}"
    );
    assert!(tree >= 113, "the tree grew, it must not have shrunk");

    // Every material the economy bottoms out in can actually be had, from the site the game
    // starts you on, under the preset it starts you in.
    //
    // Two separate questions, and the second is the one that bites: stone is generated on cliffs
    // that nothing can stand on, so "the world holds some" and "you can reach some" are different
    // claims and only the second one makes it a material rather than scenery.
    let report = balance::compute();
    assert!(report.access.len() >= 9, "eight fields and water");
    for material in &report.access {
        if material.material == "sand" || material.material == "crystal" {
            // Sand is the regional ocean and crystal is the reason to leave. Neither is
            // guaranteed, and a 96-hex sample of a 512-hex landform often never reaches them.
            continue;
        }
        assert!(
            material.reachable,
            "{} is required by {} rows and nothing can reach any of it",
            material.material, material.required_by
        );
        assert!(
            material.nearest_generated.is_some(),
            "{} is required by {} rows and the default world generates none",
            material.material,
            material.required_by
        );
        // A guaranteed material is guaranteed as a *patch*, not as a cell: the clearing holds
        // nothing now, so a promise that could be kept with one hex would be no promise.
        assert!(
            material.guaranteed_walk.is_none() || material.guaranteed_hexes >= WORKABLE_PATCH_HEXES,
            "{} is guaranteed as {} hexes, which no extractor can fill from",
            material.material,
            material.guaranteed_hexes
        );
    }
    // Water is nobody's field: a pump makes it out of terrain, so it is the one raw material
    // the opening does not guarantee and it still has to be within reach of the landing site.
    let water = report
        .access
        .iter()
        .find(|material| material.material == "water")
        .expect("water is a raw material");
    assert_eq!(water.guaranteed_walk, None);
    let first_pump_ceiling = BOOTSTRAP_GUARANTEES
        .iter()
        .find(|&&(item_id, _, _)| item_id == CLAY)
        .map(|&(_, _, ceiling)| ceiling as u32)
        .unwrap();
    assert!(water.nearest_generated.unwrap_or(u32::MAX) <= first_pump_ceiling);

    // The founding contract has to be a founding *project*, and that is a claim about its bill.
    //
    // Three components prove one chain out of one landscape, which was the whole of v0.13's
    // objective and is deliberately only the first stage now. What the milestone asserts is that
    // the project the hub actually builds cannot be paid for out of that chain: it needs more than
    // one raw material, it costs strictly more, and — like every powered machine in this game — it
    // cannot be run at all without an On-site Power branch nothing else forces.
    let report = balance::compute();
    let founding: Vec<_> = report
        .contracts
        .iter()
        .filter(|stage| stage.scenario == "new-game")
        .collect();
    assert!(
        founding.len() >= 2,
        "a single-stage contract is the old objective wearing a new name"
    );
    let first = founding.first().expect("a first stage");
    let last = founding.last().expect("a last stage");
    assert!(
        last.raw_materials >= 2,
        "the founding project bottoms out in {} raw material(s); a project that needs one \
             landscape is a longer version of the opening",
        last.raw_materials
    );
    assert!(
        last.opening.gather_total > first.opening.gather_total,
        "the project must cost more than the beat that proves the line"
    );
    for stage in &founding {
        let needs_power = stage.opening.buildings.iter().any(|key| {
            let (definitions, _, _) = catalogs();
            definitions
                .buildings
                .iter()
                .any(|building| building.key == *key && building.power_draw.unwrap_or(0) > 0)
        });
        assert!(
            !needs_power
                || stage
                    .opening
                    .technologies
                    .iter()
                    .any(|key| key == "on-site-power"),
            "{} is payable without power, so the guidance may lead somewhere the rules refuse",
            stage.stage
        );
    }
    assert!(first.opening.technologies.is_empty());
    assert!(first.opening.player_work_ticks > 0);

    // An opening that needs a machine the rules will not run is not an opening.
    //
    // `power_progress` returns zero off a network, so a plan naming a smelter and no generator is
    // a plan for a factory that stands still. This is the same defect the scripted next action
    // had, asserted here against the numbers rather than against the sentence.
    let report = balance::compute();
    let definitions: DefinitionsInput =
        serde_json::from_str(include_str!("../../../src/data/definitions.json")).unwrap();
    let building = |key: &str| {
        definitions
            .buildings
            .iter()
            .find(|building| building.key == key)
            .expect("opening names a shipped building")
    };
    let openings = report
        .openings
        .iter()
        .chain(report.contracts.iter().map(|stage| &stage.opening));
    for opening in openings {
        let draws = opening
            .buildings
            .iter()
            .any(|key| building(key).power_draw.unwrap_or(0) > 0);
        if !draws {
            continue;
        }
        assert!(
            opening
                .buildings
                .iter()
                .any(|key| building(key).power_output.unwrap_or(0) > 0),
            "{} draws power and generates none",
            opening.name
        );
    }

    // A generator whose upkeep eats its own output is not a generator.
    //
    // A boiler drinks one water every tick it runs and a turbine is dead without one beside it,
    // so the pumps are part of the plant whether or not the definition file says so. Through
    // v0.16 the pump made one water every six ticks, which is six pumps drawing 24 of the
    // turbine's 48 before a single machine ran — leaving the mid-game workhorse behind a hydro
    // generator that cost exactly the same and needed neither fuel nor plumbing.
    let report = balance::compute();
    for plant in &report.power {
        assert!(
            plant.net_output > 0,
            "{} produces {} and spends {} keeping itself fed",
            plant.building,
            plant.output,
            plant.upkeep_draw
        );
    }
    // The one that burns fuel and drinks water is the one that carries the most, or the cost
    // of running it buys nothing.
    let best_free = report
        .power
        .iter()
        .filter(|plant| plant.fuel_energy_per_tick == 0)
        .map(|plant| plant.net_output)
        .max()
        .unwrap_or(0);
    let steam = report
        .power
        .iter()
        .find(|plant| plant.source == "turbine")
        .expect("steam is in the catalogue");
    assert!(
        steam.net_output > best_free,
        "steam nets {} against {} for a generator that needs nothing",
        steam.net_output,
        best_free
    );

    // Research is funded out of distinct projects, each counted once at its posted price.
    //
    // A funding line used to name one request and grind it, so the harness quoted a length that
    // no longer exists: the hub buys a given bill exactly once. An opening's cost is now a
    // shopping list — whole projects, cheapest per item first, until the insight is raised — and
    // this recomputes that list from the catalogue rather than trusting the field.
    let report = balance::compute();
    let (definitions, _, _) = catalogs();
    let mut multi = 0;
    let openings = report
        .openings
        .iter()
        .chain(report.contracts.iter().map(|stage| &stage.opening));
    for opening in openings {
        if opening.insight == 0 {
            assert_eq!(
                opening.insight_items, 0,
                "{} pays for nothing",
                opening.name
            );
            assert!(opening.insight_projects.is_empty());
            continue;
        }
        let named: Vec<_> = opening
            .insight_projects
            .iter()
            .map(|key| {
                definitions
                    .requests
                    .iter()
                    .find(|request| &request.key == key)
                    .expect("a funding line names a standing project")
            })
            .collect();
        let unique: BTreeSet<_> = opening.insight_projects.iter().collect();
        assert_eq!(
            unique.len(),
            opening.insight_projects.len(),
            "{} funds itself by delivering the same project twice",
            opening.name
        );
        let raised: u32 = named.iter().map(|request| request.insight).sum();
        assert!(
            raised >= opening.insight,
            "{} raises {} against a bill of {}",
            opening.name,
            raised,
            opening.insight
        );
        // Whole projects, so the list must not carry a row it did not need to reach the bill.
        let without = raised - named.last().expect("a funded line names one").insight;
        assert!(
            without < opening.insight,
            "{} lists a project it does not need",
            opening.name
        );
        assert_eq!(
            opening.insight_items,
            named.iter().map(|request| request.quantity).sum::<u32>(),
            "{} funds {} insight off {:?}",
            opening.name,
            opening.insight,
            opening.insight_projects
        );
        if named.len() > 1 {
            multi += 1;
        }
    }
    assert!(
        multi > 0,
        "no opening needs a second project, so the finite count is not being measured"
    );

    // A technology the hub grants for finishing a commission is not free, and an opening that needs
    // one has to deliver the commission first.
    //
    // Four technologies cannot be bought at any price: the contract stage hands them over. Pricing
    // them at their `insight` cost of zero told the harness the early stations were unlocked from a
    // standing start, which skipped the delivery every real opening makes before it places one.
    // The resolver now folds the owed stage's bill into the opening it blocks — and a stage does
    // not commission itself, or nothing would ever resolve.
    let report = balance::compute();
    let (_, technologies, _) = catalogs();
    let granted: BTreeSet<&str> = technologies
        .technologies
        .iter()
        .filter_map(|technology| match &technology.grant {
            TechnologyGrant::ContractStage { key, .. } => Some(key.as_str()),
            TechnologyGrant::Purchase => None,
        })
        .collect();
    assert!(
        !granted.is_empty(),
        "the founding commission grants technologies, or this measures nothing"
    );
    for stage in &report.contracts {
        assert!(
            !stage.opening.commissions.contains(&stage.stage),
            "{} commissions itself",
            stage.stage
        );
    }
    let openings = report
        .openings
        .iter()
        .chain(report.contracts.iter().map(|stage| &stage.opening));
    let mut owed = 0;
    for opening in openings {
        for key in &opening.commissions {
            assert!(
                granted.contains(key.as_str()),
                "{} waits on {key}, which grants nothing",
                opening.name
            );
            owed += 1;
        }
    }
    assert!(
        owed > 0,
        "no opening waits on a commission, so the grant is still being priced at zero"
    );
}
