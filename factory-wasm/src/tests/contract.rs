use super::*;

#[test]
fn the_founding_contract_advances_stage_by_stage_and_carries_its_surplus() {
    let mut core = game("new-game");
    core.power_unmetered = false;
    set_player_hex(&mut core, 1, 0);
    // Research is funded by filling what the hub posted, one board row at a time. The opening
    // three are ore, stone, and wood, and each is worth ten insight.
    for (item, quantity) in [(1, 10), (6, 10), (9, 10)] {
        core.player.inventory.insert(item, quantity);
        core.deposit_inventory().unwrap();
    }
    assert_eq!(core.insight, 30);
    grant_foundations(&mut core);
    core.research(3).unwrap();
    stock_for(&mut core, 1, 1);
    stock_for(&mut core, 3, 1);
    stock_for(&mut core, 12, 2);
    stock_for(&mut core, 13, 1);
    core.player.inventory.insert(5, 16);
    core.player.inventory.insert(24, 8);
    set_player_hex(&mut core, 4, 2);
    // The same westward line, laid out around what each machine now stands on. The hub covers
    // every hex within one of the origin, so the line starts three further east and the
    // composer hands into the hub's eastern rim, which is what closes the stage. An extractor
    // is placed on its deposit rather than beside it, so the ore is written under the anchor
    // it moved to: this is a test about stages, not about where a generator puts iron.
    core.write_overlay(6, 0, 1, 2, 48);
    core.place(6, 0, 1, 3, None).unwrap();
    core.place(4, 0, 2, 3, None).unwrap();
    core.place(3, 0, 3, 3, Some(1)).unwrap();
    let composer = core.entity_at(3, 0).unwrap();
    core.entities[composer]
        .input_inventory
        .extend([(11, 1), (19, 1)]);
    set_player_hex(&mut core, 5, 2);
    let pole = try_place_near(&mut core, (6, 0), 12);
    let burner = try_place_near(&mut core, pole, 13);
    try_place_near(&mut core, (3, 0), 12);
    let _ = burner;
    if let Some(burner) = core
        .entities
        .iter_mut()
        .find(|entity| entity.kind == BuildingKind::Generator)
    {
        burner.inventory.insert(5, 16);
    }
    core.tick_many(500);
    // The running line closes the first stage, and closing it is deliberately not the end of
    // the contract: the hub has grown once, and free play has not been declared yet.
    assert_eq!(core.contract_stage, 1);
    assert!(!core.victory);
    assert_eq!(core.contract_snapshot().stage_key, "foundry");
    // The foundry module, delivered by hand. What this pins is the stage machinery, not a
    // second smelting line: the bill is two items from two chains, and both have to arrive.
    set_player_hex(&mut core, 0, -1);
    core.player.inventory.insert(11, 16);
    core.deposit_inventory().unwrap();
    assert_eq!(core.contract_stage, 1, "half a bill is not a stage");
    assert!(!core.victory);
    core.player.inventory.insert(14, 20);
    core.deposit_inventory().unwrap();
    assert_eq!(core.contract_stage, 2);
    assert!(core.victory);
    // Nothing is left to ask for, and the requirement list says so rather than repeating the
    // last bill at full.
    assert!(core.contract_snapshot().requirements.is_empty());
    assert!(core.contract_snapshot().complete);
    let checksum = core.checksum();
    core.tick_many(1);
    assert!(core.victory);
    assert_ne!(core.checksum(), checksum);

    // A stage consumes its bill and carries the surplus to the next one.
    let mut core = game("new-game");
    set_player_hex(&mut core, 0, -1);
    // Everything the whole contract asks for, in one delivery, plus one component too many.
    // The hub takes a later stage's materials as well as the current one's, which is the
    // surplus rule: a line automated early is credited when the stage that wants it arrives.
    core.player.inventory.insert(2, 2);
    core.player.inventory.insert(11, 16);
    core.player.inventory.insert(14, 20);
    core.deposit_inventory().unwrap();
    for id in [1, 2, 4, 8] {
        assert!(
            core.researched.contains(&id),
            "closing the opening commission grants {id}"
        );
    }
    // Both stages close in the same delivery, which is the reason the advance loops rather
    // than closing one stage per arriving item.
    assert_eq!(core.contract_stage, 2);
    assert!(core.victory);
    // Each stage consumed exactly its own bill, and the second component was never taken at
    // all: the hub accepts what it asked for and leaves the rest in the pack.
    assert_eq!(core.contract_contributed.get(&2), Some(&0));
    assert_eq!(core.contract_contributed.get(&11), Some(&0));
    assert_eq!(core.contract_contributed.get(&14), Some(&0));
    assert_eq!(core.player.inventory.get(&2), Some(&1));
    // A finished contract does not close the hub. The board is still posting, filling a row is
    // still what pays, and no stage index runs off the end of the list.
    let insight = core.insight;
    core.player.inventory.insert(1, 10);
    core.deposit_inventory().unwrap();
    assert!(core.insight > insight);
    assert_eq!(core.contract_stage, 2);
}

/// The price is posted, and it is paid on completion — never before, and never for anything the
/// hub did not ask for.
#[test]
fn the_board_posts_pays_passes_and_saves_what_the_player_could_make() {
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    let board = |core: &Core| -> Vec<String> {
        core.request_snapshots()
            .iter()
            .filter(|request| request.state == ProjectState::Posted)
            .map(|request| request.key.clone())
            .collect()
    };
    assert_eq!(board(&core), ["ore-assay", "cliff-stone", "cordwood"]);
    // Half a request is worth nothing. This is the whole difference from the currency it
    // replaced, where five ore was five insight and the player never saw the rate.
    core.player.inventory.insert(1, 5);
    core.deposit_inventory().unwrap();
    assert_eq!(core.insight, 0);
    assert_eq!(core.request_snapshots()[0].delivered, 5);
    core.player.inventory.insert(1, 5);
    core.deposit_inventory().unwrap();
    assert_eq!(core.insight, 10);
    // The slot that was filled holds the next row, in its own place: the board does not
    // shuffle, and it does not repost the row that was just paid for while others are unseen.
    assert_eq!(board(&core), ["clay-survey", "cliff-stone", "cordwood"]);
    assert_eq!(core.request_rounds.get(&1), Some(&1));
    assert_eq!(core.request_fills.get(&1), Some(&1));
    // And the row it paid for is retired, not merely off the board: the catalogue still
    // carries it so the player can see the work is done.
    let paid = core
        .request_snapshots()
        .into_iter()
        .find(|request| request.key == "ore-assay")
        .expect("a filled project stays in the catalogue");
    assert_eq!(paid.state, ProjectState::Complete);
    assert_eq!(paid.delivered, 0, "a retired project holds no progress");

    // Passing a row costs it a place in the queue, not its first-fill bonus. Skip used to share
    // `request_rounds` with payment, which would have turned "I have not found this yet" into
    // two insight for ten gathers.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.skip_request(0).unwrap();
    assert_eq!(core.request_rounds.get(&1), Some(&1));
    assert!(core.request_fills.get(&1).is_none());
    core.requests[0] = RequestState { request_id: 1 };
    let before = core.insight;
    core.player.inventory.insert(1, 10);
    core.deposit_inventory().unwrap();
    assert_eq!(
        core.insight - before,
        10,
        "a skipped row still pays its first fill"
    );
    assert_eq!(core.request_fills.get(&1), Some(&1));

    // A filled project is finished, and finished is for good. Delivering its item again is
    // ordinary freight into the hub, not a second payment.
    //
    // This is the shape the catalogue used to have inverted. A raw row paid ten once and two for
    // ever after, so the board was a tap: slow, dull, and unbounded, which meant no amount of
    // research could ever actually be priced. Demand is a bill now.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.player.inventory.insert(1, 10);
    core.deposit_inventory().unwrap();
    assert_eq!(core.insight, 10);
    assert_eq!(core.request_fills.get(&1), Some(&1));
    // Force ore-assay back into a slot by hand — nothing in the game can do this — and the
    // hub still refuses to buy what it has already commissioned.
    core.requests[0] = RequestState { request_id: 1 };
    core.player.inventory.insert(1, 10);
    core.deposit_inventory().unwrap();
    assert_eq!(core.insight, 10, "a second delivery buys nothing");
    assert_eq!(core.request_fills.get(&1), Some(&1));

    // Passing a part-filled project keeps what was handed over. Progress belongs to the project,
    // not to the slot it happened to be posted in.
    //
    // Under repeatable demand a skip that dropped the count cost a few minutes. Under a finite
    // catalogue it would destroy goods whose reward can never be earned again, so the count moved
    // off the board and onto the project itself.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.player.inventory.insert(1, 6);
    core.deposit_inventory().unwrap();
    assert_eq!(core.insight, 0);
    assert_eq!(core.project_delivered(1), 6);
    core.skip_request(0).unwrap();
    assert_eq!(core.project_delivered(1), 6, "the skip kept the ore");
    // Post it again and the remaining four finish it at the full price.
    core.post_request(project_id(&core, "ore-assay")).unwrap();
    core.player.inventory.insert(1, 4);
    core.deposit_inventory().unwrap();
    assert_eq!(core.insight, 10);
    assert_eq!(core.project_delivered(1), 0);

    // Posting is the player's choice, and the catalogue is the whole board. A finite bill has to
    // be browsable or a row that funds nothing else could hide behind two that do.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    // Commit ore to the first slot, so an untouched slot is the cheapest thing to displace.
    let committed = project_id(&core, "ore-assay");
    let wanted = project_id(&core, "clay-survey");
    core.player.inventory.insert(1, 4);
    core.deposit_inventory().unwrap();
    assert_eq!(core.project_delivered(committed), 4);
    core.post_request(wanted).unwrap();
    let posted: Vec<_> = core.requests.iter().map(|slot| slot.request_id).collect();
    assert!(posted.contains(&wanted), "clay-survey took a slot");
    assert!(
        posted.contains(&committed),
        "the part-filled row was displaced ahead of an untouched one, got {posted:?}"
    );
    assert_eq!(
        core.post_request(wanted),
        Err("Clay survey is already on the board".to_owned())
    );
    assert!(core.post_request(9999).is_err());

    // Part-delivered goods survive a save. They are the one thing in the request system a player
    // cannot re-earn, so losing them across a reload would be losing the work outright.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    let ore = project_id(&core, "ore-assay");
    core.player.inventory.insert(1, 6);
    core.deposit_inventory().unwrap();
    core.skip_request(0).unwrap();
    assert!(!posted_board(&core).contains(&"ore-assay".to_owned()));
    let (definitions, technologies, scenarios) = catalogs();
    let resumed = Core::from_save(
        &definitions,
        &technologies,
        &scenarios,
        &core.save_string().unwrap(),
    )
    .unwrap();
    assert_eq!(resumed.project_delivered(ore), 6);
    assert_eq!(core.checksum(), resumed.checksum());
    // And the checksum notices: progress is saved state, so a file that lost it is a different
    // game rather than the same game rounded.
    let with = core.checksum();
    core.request_delivered.remove(&ore);
    assert_ne!(with, core.checksum());

    // The board closes when the hub has nothing left to ask for. A finite catalogue that quietly
    // reposted its last row for ever would be the tap again, wearing a bill's clothes.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    for request in &core.definitions.requests {
        core.request_fills.insert(request.id, 1);
    }
    core.requests.clear();
    core.refill_requests();
    assert!(core.requests.is_empty(), "nothing is left to post");
    assert!(core
        .request_snapshots()
        .iter()
        .all(|request| request.state == ProjectState::Complete));

    // The hub takes what it asked for and leaves the rest in the pack — by hand and by belt, at one
    // predicate, so a line cannot void cargo the key would have refused.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.player.inventory.insert(3, 6);
    assert!(core
        .deposit_inventory()
        .unwrap_err()
        .contains("not asking for anything"));
    assert_eq!(core.player.inventory.get(&3), Some(&6));
    let hub = core
        .entities
        .iter()
        .position(|entity| entity.kind == BuildingKind::Hub)
        .expect("the landing hub");
    assert!(!core.can_accept(
        hub,
        Cargo {
            item_id: 3,
            quantity: 1
        }
    ));
    assert!(core.can_accept(
        hub,
        Cargo {
            item_id: 1,
            quantity: 1
        }
    ));
    // Ten ore is the whole standing order, so the eleventh has nowhere to go either.
    core.player.inventory.insert(1, 10);
    core.deposit_inventory().unwrap();
    assert!(!core.can_accept(
        hub,
        Cargo {
            item_id: 1,
            quantity: 1
        }
    ));

    // The board is drawn from the rules, so it can never post something the rules refuse.
    let mut core = game("new-game");
    assert!(core.item_reachable(1, 0), "ore is in the ground");
    assert!(
        !core.item_reachable(11, 0),
        "a plate needs a smelter nobody may build yet"
    );
    assert!(
        !core.item_reachable(10, 0),
        "water needs a pump, and water is nobody's field"
    );
    assert!(
        !core.item_reachable(CRYSTAL, 0),
        "signal crystal is machine only until an extractor is unlocked"
    );
    // Passing every slot repeatedly walks the whole eligible list. Nothing that needs a machine
    // may appear in it, however far up the catalogue that row stands.
    for _ in 0..12 {
        for slot in 0..REQUEST_SLOTS {
            let item = core.request_snapshots()[slot].item_id;
            assert!(
                core.item_reachable(item, 0),
                "the board posted item {item}, which cannot be produced yet"
            );
            core.skip_request(slot).unwrap();
        }
    }
    core.insight = 100;
    grant_foundations(&mut core);
    core.research(5).unwrap();
    assert!(core.item_reachable(11, 0), "the smelter unlocks the plate");
    assert!(
        core.item_reachable(CRYSTAL, 0),
        "an extractor unlocks the crystal field"
    );

    // Passing a row costs it a place in the queue, and costs the player whatever they had already
    // put against it. It is a decision, not a free reroll.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.player.inventory.insert(1, 5);
    core.deposit_inventory().unwrap();
    assert_eq!(core.request_snapshots()[0].delivered, 5);
    core.skip_request(0).unwrap();
    assert_eq!(core.request_snapshots()[0].key, "clay-survey");
    assert_eq!(core.request_snapshots()[0].delivered, 0);
    assert_eq!(core.insight, 0);
    assert!(core.skip_request(9).unwrap_err().contains("no request"));

    // Once a smelter is unlocked, a free slot is reserved for the deepest reachable row rather
    // than the next unseen ore assay. The other two slots still cycle, and nothing unmakeable is
    // posted — reservation walks the same `item_reachable` predicate the rest of the board does.
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.insight = 100;
    grant_foundations(&mut core);
    core.research(5).unwrap();
    assert!(core.item_reachable(11, 0));
    let before: Vec<String> = posted_board(&core);
    assert!(
        before.iter().all(|key| {
            let item = core
                .definitions
                .requests
                .iter()
                .find(|request| request.key == *key)
                .map(|request| request.item_id)
                .unwrap();
            core.item_depth(item) == 0
        }),
        "the opening board is raw, got {before:?}"
    );
    core.player.inventory.insert(1, 10);
    core.deposit_inventory().unwrap();
    let after: Vec<String> = posted_board(&core);
    let depths: Vec<u32> = after
        .iter()
        .map(|key| {
            let item = core
                .definitions
                .requests
                .iter()
                .find(|request| request.key == *key)
                .map(|request| request.item_id)
                .unwrap();
            core.item_depth(item)
        })
        .collect();
    assert!(
        depths.iter().any(|&depth| depth > 0),
        "the freed slot should post the deepest reachable row, got {after:?} at {depths:?}"
    );
    for request in core
        .request_snapshots()
        .iter()
        .filter(|request| request.state == ProjectState::Posted)
    {
        assert!(
            core.item_reachable(request.item_id, 0),
            "reserved slot posted item {}, which cannot be produced",
            request.item_id
        );
    }
}

#[test]
fn the_hub_takes_delivery_from_every_footprint_cell_and_saves_its_board() {
    let mut core = game("new-game");
    // Give player iron ore (id 1) and wood (id 8). Both are standing requests in new game.
    core.player.inventory.insert(1, 10);
    core.player.inventory.insert(8, 10);
    set_player_hex(&mut core, 0, 1);
    // Deliver only iron ore
    core.deposit_item(Some(1)).unwrap();
    // Iron ore was delivered, wood remains in pack
    assert_eq!(core.player.inventory.get(&1), None);
    assert_eq!(core.player.inventory.get(&8), Some(&10));

    // A delivery is in range of the landing hub when the player stands beside *any* cell it
    // occupies. The hub is seven hexes; measuring from the anchor alone made the far lobes
    // decorative — you could stand next to them and still be told to walk closer.
    let mut core = game("new-game");
    let hub = core
        .entities
        .iter()
        .find(|entity| entity.kind == BuildingKind::Hub)
        .expect("the landing hub");
    assert_eq!(
        core.entity_footprint(hub),
        vec![
            Coordinate { q: 0, r: 0 },
            Coordinate { q: 1, r: 0 },
            Coordinate { q: 0, r: 1 },
            Coordinate { q: -1, r: 1 },
            Coordinate { q: -1, r: 0 },
            Coordinate { q: 0, r: -1 },
            Coordinate { q: 1, r: -1 },
        ]
    );

    // Beside the south-east lobe, two hexes from the origin. The old origin-circle refused this.
    core.player.inventory.insert(1, 1);
    set_player_hex(&mut core, 0, 2);
    core.deposit_item(Some(1)).unwrap();
    assert_eq!(core.player.inventory.get(&1), None);

    // Beside the south-west lobe.
    core.player.inventory.insert(1, 1);
    set_player_hex(&mut core, -2, 2);
    core.deposit_item(Some(1)).unwrap();
    assert_eq!(core.player.inventory.get(&1), None);

    // Three hexes past the south-east lobe is past a two-hex reach from every occupied cell.
    core.player.inventory.insert(1, 1);
    set_player_hex(&mut core, 0, 4);
    assert!(core
        .deposit_item(Some(1))
        .unwrap_err()
        .contains("beside the landing hub"));

    // A board is saved state, restored rather than redrawn.
    let (definitions, technologies, scenarios) = catalogs();
    let mut core = game("new-game");
    set_player_hex(&mut core, 1, 0);
    core.player.inventory.insert(1, 10);
    core.player.inventory.insert(6, 4);
    core.deposit_inventory().unwrap();
    let before = core.request_snapshots();
    let save = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &save).unwrap();
    assert_eq!(restored.request_snapshots(), before);
    assert_eq!(restored.request_rounds, core.request_rounds);
    assert_eq!(restored.request_fills, core.request_fills);
    assert_eq!(restored.insight, 10);
    // A row this build does not ship would survive the file and then be drawn as a request
    // nobody can read, so the loader refuses it before the checksum ever gets the chance.
    let forged = save.replace("\"request_id\":4", "\"request_id\":9999");
    assert_ne!(forged, save);
    let refusal = Core::from_save(&definitions, &technologies, &scenarios, &forged)
        .err()
        .expect("a forged board is refused");
    assert!(refusal.contains("unknown hub request"), "{refusal}");
}
