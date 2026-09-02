use super::*;

/// Every status spelling the host can render. The wire carries the index, so a reordering here
/// is a wire break; the fixture is what makes that break visible in both languages at once.
const WIRE_STATUSES: [(EntityStatus, &str); 18] = [
    (EntityStatus::OutputBlocked, "output blocked"),
    (EntityStatus::DepositDepleted, "deposit depleted"),
    (EntityStatus::Extracting, "extracting"),
    (EntityStatus::NoWaterInReach, "no water in reach"),
    (EntityStatus::Pumping, "pumping"),
    (EntityStatus::Composing, "composing"),
    (EntityStatus::OutOfFuel, "out of fuel"),
    (EntityStatus::WaitingForInputs, "waiting for inputs"),
    (EntityStatus::Buffered, "buffered"),
    (EntityStatus::Carrying, "carrying"),
    (EntityStatus::Receiving, "receiving"),
    (EntityStatus::LandingHub, "landing hub"),
    (EntityStatus::Idle, "idle"),
    (EntityStatus::NoPower, "no power"),
    (EntityStatus::Generating, "generating"),
    (EntityStatus::Brownout, "brownout"),
    (EntityStatus::NoBoiler, "no boiler"),
    (EntityStatus::SwitchedOff, "switched off"),
];

const WIRE_KINDS: [(BuildingKind, &str); 11] = [
    (BuildingKind::Extractor, "extractor"),
    (BuildingKind::Belt, "belt"),
    (BuildingKind::Composer, "composer"),
    (BuildingKind::Container, "container"),
    (BuildingKind::Consumer, "consumer"),
    (BuildingKind::Hub, "hub"),
    (BuildingKind::Pump, "pump"),
    (BuildingKind::Pole, "pole"),
    (BuildingKind::Generator, "generator"),
    (BuildingKind::Boiler, "boiler"),
    (BuildingKind::Bridge, "bridge"),
];

const WIRE_TERRAIN: [(Terrain, &str); 7] = [
    (Terrain::DeepWater, "deep_water"),
    (Terrain::ShallowWater, "shallow_water"),
    (Terrain::Shore, "shore"),
    (Terrain::Lowland, "lowland"),
    (Terrain::Hills, "hills"),
    (Terrain::Highland, "highland"),
    (Terrain::Cliff, "cliff"),
];

#[test]
fn entity_status_spellings_are_what_the_host_renders() {
    // The enum exists so the wire can carry a byte, but what reaches the player is still the
    // string. Renaming a variant is allowed; changing its spelling changes the game's text.
    for (status, spelling) in WIRE_STATUSES {
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            serde_json::Value::String(spelling.to_owned()),
            "status spelling changed"
        );
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Deltas chosen to walk the whole surface of the encoding rather than to look like a frame:
/// an empty group mask, every scalar group at once, both patch kinds carrying entries, and the
/// replace form with nothing in it.
fn wire_fixture_cases() -> Vec<(&'static str, SnapshotDelta)> {
    // A closure rather than a value: every case below fills a different handful of groups in
    // and leaves the rest absent, and `..` moves what it spreads from.
    let empty = || SnapshotDelta {
        boundaries: None,
        ground: None,
        spoil: None,
        water: None,
        base_revision: 0,
        revision: 1,
        tick: 0,
        checksum: 0,
        belt_transit_ticks: BELT_TRANSIT_TICKS as u32,
        scenario: None,
        scenario_name: None,
        world_version: None,
        seed: None,
        delivered: None,
        delivered_by_item: None,
        insight: None,
        victory: None,
        contract: None,
        requests: None,
        player: None,
        researched: None,
        research_availability: None,
        skills: None,
        chunks: None,
        terrain: None,
        resources: None,
        buildings: None,
        ground_items: None,
        events: None,
    };

    // A frame that changed nothing but the clock. The mask is zero and the body is empty, which
    // is the case a quiet factory spends most of its frames in.
    let quiet = SnapshotDelta {
        base_revision: 41,
        revision: 42,
        tick: 1_000_000,
        checksum: 0xdead_beef,
        ..empty()
    };

    // Every scalar group, with negative coordinates and multi-byte varints, so a decoder that
    // reads a field in the wrong order or forgets to zigzag cannot pass.
    let scalars = SnapshotDelta {
        base_revision: 2,
        revision: 3,
        tick: 300,
        checksum: 7,
        scenario: Some("new-game".to_owned()),
        scenario_name: Some("New game".to_owned()),
        world_version: Some(5),
        seed: Some(4_294_967_295),
        // Exactly 2^53 - 1. The invariant is that nothing wider than that travels as a number,
        // and the host still receives these as JavaScript numbers, so the boundary itself is
        // the largest value worth pinning — a fixture above it would pin rounding, not the
        // encoding.
        delivered: Some(9_007_199_254_740_991),
        delivered_by_item: Some(vec![
            Ingredient64 {
                item_id: 1,
                quantity: 1_000_000_000_000,
            },
            Ingredient64 {
                item_id: 300,
                quantity: 0,
            },
        ]),
        insight: Some(64_000),
        victory: Some(true),
        // A multi-line bill with one line over-delivered and one untouched, so a decoder that
        // loses the count, swaps `delivered` and `required`, or reads the trailing flag before
        // the list cannot pass.
        contract: Some(ContractSnapshot {
            key: "founding".to_owned(),
            name: "Founding contract".to_owned(),
            stage: 1,
            stages: 2,
            stage_key: "foundry".to_owned(),
            stage_name: "Raise the foundry module".to_owned(),
            stage_brief: "Plate and brick, from two landscapes.".to_owned(),
            requirements: vec![
                ContractRequirement {
                    item_id: 11,
                    delivered: 16,
                    required: 16,
                },
                ContractRequirement {
                    item_id: 14,
                    delivered: 0,
                    required: 20,
                },
            ],
            complete: false,
        }),
        // A board with one row part-filled and one untouched, so a decoder that loses the
        // count, swaps `delivered` and `required`, or reads the price before the numbers cannot
        // pass. The brief carries the multi-byte case the events list carries too. Two
        // different states, either side of the numbers, so a decoder that drops the state byte
        // or reads it at the wrong offset fails here rather than in the panel.
        requests: Some(vec![
            RequestSnapshot {
                key: "plate-stock".to_owned(),
                name: "Plate stock".to_owned(),
                brief: "Smelted iron — not ore.".to_owned(),
                item_id: 11,
                delivered: 3,
                required: 8,
                insight: 22,
                state: ProjectState::Posted,
            },
            RequestSnapshot {
                key: "cliff-stone".to_owned(),
                name: "Cliff stone".to_owned(),
                brief: "Cut stone for the apron.".to_owned(),
                item_id: 6,
                delivered: 0,
                required: 10,
                insight: 10,
                state: ProjectState::Complete,
            },
        ]),
        player: Some(PlayerSnapshot {
            state: PlayerState {
                x: -123_456,
                y: 654_321,
                facing_x: -1000,
                facing_y: 866,
                move_x: 0,
                move_y: -1,
                inventory: BTreeMap::from([(1, 40), (3, 20), (65_535, 1)]),
                hand: Some(Cargo {
                    item_id: 5,
                    quantity: 3,
                }),
                action_cooldown: 5,
                build_range: 4096,
                carry_slots: 12,
                walk_goal: Some(Coordinate { q: -70, r: 12 }),
            },
            carry_stacks: vec![
                Ingredient {
                    item_id: 1,
                    quantity: 40,
                },
                Ingredient {
                    item_id: 3,
                    quantity: 20,
                },
            ],
            radius: 580,
            action_cooldown_total: 6,
            extract_radius: 1,
            creative: true,
            // A route that steps in every direction the delta coding has to carry, ending on
            // the goal above, so the fixture pins the chain rather than a straight line.
            walk_path: vec![
                Coordinate { q: -74, r: 14 },
                Coordinate { q: -73, r: 14 },
                Coordinate { q: -73, r: 13 },
                Coordinate { q: -72, r: 13 },
                Coordinate { q: -72, r: 12 },
                Coordinate { q: -71, r: 12 },
                Coordinate { q: -70, r: 12 },
            ],
        }),
        researched: Some(vec![1, 2, 3, 4]),
        research_availability: Some(vec![
            ResearchAvailability {
                technology_id: 1,
                complete: true,
                insight_shortfall: 0,
                missing_prerequisites: vec![],
            },
            ResearchAvailability {
                technology_id: 300,
                complete: false,
                insight_shortfall: 70_000,
                missing_prerequisites: vec![5, 256],
            },
        ]),
        skills: Some(SkillsSnapshot {
            state: SkillsState {
                points: 2,
                purchased: BTreeSet::from([1]),
                granted: BTreeSet::from([300]),
                completed: BTreeSet::from([2, 400]),
                sandbox: true,
            },
            availability: vec![SkillAvailability {
                skill_id: 301,
                complete: false,
                points_shortfall: 128,
                current_value: 6,
                resulting_value: 10,
                missing_prerequisites: vec![300],
            }],
        }),
        chunks: Some(vec![
            ChunkSnapshot {
                chunk_q: 0,
                chunk_r: 0,
                entity_count: 3,
                x: -8192,
                y: -8192,
                span: 16_384,
            },
            ChunkSnapshot {
                chunk_q: -2,
                chunk_r: 1,
                entity_count: 0,
                x: -40_960,
                y: 8192,
                span: 16_384,
            },
        ]),
        events: Some(vec![
            "Gathered Iron ore".to_owned(),
            // Multi-byte UTF-8, because the string length is written in bytes and a decoder
            // that reads it as characters would desynchronise the rest of the buffer.
            "Delivered 3 × Steel — objective met".to_owned(),
        ]),
        ground_items: Some(vec![
            GroundItem {
                id: 1,
                q: -2,
                r: 5,
                item_id: 11,
                quantity: 4,
                despawn_tick: 900,
            },
            GroundItem {
                id: 2,
                q: 10,
                r: -3,
                item_id: 6,
                quantity: 1,
                despawn_tick: 600,
            },
        ]),
        ..empty()
    };

    // Both patches carrying entries: a bare belt beside a machine with every option set, a
    // removal list, a deposit patch over negative coordinates, and terrain.
    let patches = SnapshotDelta {
        base_revision: 10,
        revision: 11,
        tick: 512,
        checksum: 0x0102_0304,
        // A patch rather than a replace, a summit beside a flooded basin, and a height that
        // steps down by more than a byte of zigzag between the two: the pair pins the height
        // delta coding, a signed absolute bed, standing water and a drainage class at once.
        terrain: Some(TerrainDelta {
            replace: false,
            changed: vec![
                TileSnapshot {
                    q: -3,
                    r: -4,
                    x: -8_870,
                    y: -6_144,
                    radius: 1024,
                    terrain: Terrain::Cliff,
                    height: 4_212,
                    substrate: Substrate::Rock,
                    water_depth: 0,
                    discharge: 0,
                },
                TileSnapshot {
                    q: -2,
                    r: -4,
                    x: -7_096,
                    y: -6_144,
                    radius: 1024,
                    terrain: Terrain::DeepWater,
                    height: -37,
                    substrate: Substrate::Sand,
                    water_depth: 41,
                    discharge: 7,
                },
            ],
        }),
        resources: Some(ResourcesDelta {
            replace: false,
            changed: vec![
                ResourceSnapshot {
                    q: -32,
                    r: 0,
                    x: -56_768,
                    y: 0,
                    radius: 1024,
                    item_id: 1,
                    quantity: 0,
                    initial_quantity: 48,
                },
                ResourceSnapshot {
                    q: -32,
                    r: 3,
                    x: -54_107,
                    y: 4_608,
                    radius: 1024,
                    item_id: 2,
                    quantity: 17,
                    initial_quantity: 60,
                },
            ],
        }),
        buildings: Some(BuildingsDelta {
            replace: false,
            changed: vec![
                EntitySnapshot {
                    id: 7,
                    q: 2,
                    r: 0,
                    definition_id: 2,
                    kind: BuildingKind::Belt,
                    orientation: 3,
                    recipe_id: None,
                    scenario_owned: false,
                    cargo: None,
                    lane: Vec::new(),
                    inventory: Vec::new(),
                    input_inventory: Vec::new(),
                    fuel_inventory: Vec::new(),
                    output_inventory: Vec::new(),
                    output_routes: Vec::new(),
                    water_source: None,
                    progress: 0,
                    progress_total: 0,
                    fuel_charge: 0,
                    fuel_required: 0,
                    power_satisfied: 0,
                    power_demand: 0,
                    // A belt sets no high flag, so its flag field is still the one byte it was
                    // before the field became a uvarint. That is the whole point of the change
                    // and this entity is what pins it.
                    power_charge: 0,
                    power_capacity: 0,
                    status: EntityStatus::Idle,
                    next_id: None,
                    // No outputs at all, which is the empty branch list — the case every
                    // entity that is not a splitter encodes.
                    branch_ids: Vec::new(),
                    footprint: vec![Coordinate { q: 2, r: 0 }],
                },
                // A belt mid-run: one item finished crossing and waiting at the exit, three
                // more strung out behind it, and the last of those stepped on so long ago that
                // its elapsed count needs a second byte — the jammed lane the cadence exists
                // to make visible. Its lane flag is the highest entity bit there is, so this
                // is also the widest flag field the encoder writes.
                EntitySnapshot {
                    id: 12,
                    q: 3,
                    r: 0,
                    definition_id: 2,
                    kind: BuildingKind::Belt,
                    orientation: 3,
                    recipe_id: None,
                    scenario_owned: false,
                    cargo: Some(Cargo {
                        item_id: 1,
                        quantity: 1,
                    }),
                    lane: vec![
                        LaneItem {
                            cargo: Cargo {
                                item_id: 1,
                                quantity: 1,
                            },
                            entered: 300,
                        },
                        LaneItem {
                            cargo: Cargo {
                                item_id: 4,
                                quantity: 2,
                            },
                            entered: 495,
                        },
                        LaneItem {
                            cargo: Cargo {
                                item_id: 1,
                                quantity: 1,
                            },
                            entered: 512,
                        },
                    ],
                    inventory: Vec::new(),
                    input_inventory: Vec::new(),
                    fuel_inventory: Vec::new(),
                    output_inventory: Vec::new(),
                    output_routes: Vec::new(),
                    water_source: None,
                    progress: 0,
                    progress_total: 0,
                    fuel_charge: 0,
                    fuel_required: 0,
                    power_satisfied: 0,
                    power_demand: 0,
                    power_charge: 0,
                    power_capacity: 0,
                    status: EntityStatus::OutputBlocked,
                    next_id: Some(4_294_967_295),
                    branch_ids: Vec::new(),
                    footprint: vec![Coordinate { q: 3, r: 0 }],
                },
                EntitySnapshot {
                    id: 4_294_967_295,
                    q: -1,
                    r: 6,
                    definition_id: 3,
                    kind: BuildingKind::Composer,
                    orientation: 5,
                    recipe_id: Some(11),
                    scenario_owned: true,
                    cargo: Some(Cargo {
                        item_id: 4,
                        quantity: 2,
                    }),
                    lane: Vec::new(),
                    inventory: vec![
                        Ingredient {
                            item_id: 1,
                            quantity: 6,
                        },
                        Ingredient {
                            item_id: 5,
                            quantity: 300,
                        },
                    ],
                    input_inventory: vec![Ingredient {
                        item_id: 2,
                        quantity: 12,
                    }],
                    fuel_inventory: vec![Ingredient {
                        item_id: 5,
                        quantity: 7,
                    }],
                    output_inventory: vec![Ingredient {
                        item_id: 4,
                        quantity: 9,
                    }],
                    output_routes: vec![OutputRouteSnapshot {
                        item_id: 4,
                        q: -1,
                        r: 6,
                        direction: 5,
                        target_id: Some(7),
                    }],
                    // Synthetic every-field case: pins signed source offsets and the finite /
                    // replenishing rate payload without adding another entity to the fixture.
                    water_source: Some(WaterSourceSnapshot {
                        q: -3,
                        r: 8,
                        available: 12,
                        discharge: 3,
                        rate: 3,
                    }),
                    progress: 17,
                    progress_total: 40,
                    fuel_charge: 250,
                    fuel_required: 100,
                    power_satisfied: 8,
                    power_demand: 12,
                    // Both high bits set, so this entity's flag field is two bytes and the
                    // fixture carries a decoder that has to widen past the old fixed byte.
                    power_charge: 96,
                    power_capacity: 360,
                    status: EntityStatus::Composing,
                    next_id: Some(9),
                    // A full branch list, carrying both a small id and the largest one a u32
                    // holds, so the decoder is pinned at both ends of the range it must widen
                    // across. This entity is the fixture's every-field-at-its-limit case.
                    branch_ids: vec![4, 4_294_967_295],
                    // A multi-cell footprint, coded against the entity's own hex.
                    footprint: vec![
                        Coordinate { q: -1, r: 6 },
                        Coordinate { q: 0, r: 6 },
                        Coordinate { q: -1, r: 7 },
                    ],
                },
            ],
            removed: vec![1, 2, 900],
        }),
        ..empty()
    };

    // The full-replace form both patches take on the first frame, a reset, a new game, and a
    // load — here with nothing in it, so the replace flag is what is being read rather than
    // the entries after it.
    let replace = SnapshotDelta {
        boundaries: Some(Vec::new()),
        base_revision: 0,
        revision: 1,
        tick: 0,
        checksum: 1,
        resources: Some(ResourcesDelta {
            replace: true,
            changed: Vec::new(),
        }),
        buildings: Some(BuildingsDelta {
            replace: true,
            changed: Vec::new(),
            removed: Vec::new(),
        }),
        events: Some(Vec::new()),
        ..empty()
    };

    let boundaries = SnapshotDelta {
        boundaries: Some(vec![Boundary {
            segment: Segment {
                q: -4,
                r: 7,
                chord: 2,
            },
            definition_id: 2,
            open: true,
            paid: vec![Ingredient {
                item_id: 15,
                quantity: 2,
            }],
        }]),
        ..empty()
    };
    // Prepared ground carries a signed elevation beside an unsigned surface id, and the two are
    // encoded differently. A cut cell and a paved cell in the same case is what pins that: swap
    // the two readers and the cut hex comes back as a huge surface id rather than as an error.
    let ground = SnapshotDelta {
        ground: Some(vec![
            GroundCell {
                q: 2,
                r: -3,
                surface: 4,
                elevation: 0,
                erosion: 1,
                paid: vec![Ingredient {
                    item_id: 15,
                    quantity: 1,
                }],
            },
            GroundCell {
                q: -1,
                r: 0,
                surface: 0,
                elevation: -2,
                erosion: 0,
                paid: Vec::new(),
            },
        ]),
        spoil: Some(6),
        ..empty()
    };
    // Departure is signed, like a cut's elevation: a flooded cell and a drained one in the same
    // case is what pins the reader. Swap it for an unsigned varint and the drained hex comes
    // back as a huge positive depth rather than as an error.
    let water = SnapshotDelta {
        water: Some(vec![
            hydrology::WaterCell {
                q: 2,
                r: -3,
                departure: 6,
            },
            hydrology::WaterCell {
                q: -1,
                r: 0,
                departure: -4,
            },
        ]),
        ..empty()
    };

    vec![
        ("boundaries with paid recovery", boundaries),
        ("prepared ground and spoil", ground),
        ("disturbed water", water),
        ("a quiet frame", quiet),
        ("every scalar group", scalars),
        ("both patches with entries", patches),
        ("the empty full replace", replace),
    ]
}

/// The one artifact both languages are pinned to, in the same role
/// `fixtures/hex-directions.json` plays for the direction table.
///
/// Rust asserts it encodes these deltas to exactly these bytes and serializes them to exactly
/// this JSON. `tests/snapshotWire.test.ts` asserts the shipped TypeScript decoder turns those
/// same bytes back into that same JSON. Together they say the binary path delivers what the
/// JSON path delivered, which is the whole claim of the encoding.
///
/// Regenerate with `UPDATE_WIRE_FIXTURE=1 cargo test wire_fixture` and read the diff: a change
/// here is a wire break, and the decoder on the other side has to move with it.
#[test]
fn the_cross_language_fixtures_pin_the_format_and_the_economy() {
    let cases: Vec<serde_json::Value> = wire_fixture_cases()
        .into_iter()
        .map(|(name, delta)| {
            serde_json::json!({
                "name": name,
                "bytes": hex_encode(&wire::encode_delta(&delta)),
                "delta": serde_json::to_value(&delta).unwrap(),
            })
        })
        .collect();
    let generated = serde_json::json!({
        "magic": std::str::from_utf8(&wire::WIRE_MAGIC).unwrap(),
        "version": wire::WIRE_VERSION,
        "kinds": WIRE_KINDS.map(|(_, name)| name).to_vec(),
        "terrain": WIRE_TERRAIN.map(|(_, name)| name).to_vec(),
        "statuses": WIRE_STATUSES.map(|(_, name)| name).to_vec(),
        "cases": cases,
    });

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/snapshot-delta-wire.json");
    if std::env::var("UPDATE_WIRE_FIXTURE").is_ok() {
        let mut text = serde_json::to_string_pretty(&generated).unwrap();
        text.push('\n');
        std::fs::write(&path, text).unwrap();
    }
    let recorded: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).expect(
        "fixtures/snapshot-delta-wire.json exists — regenerate with UPDATE_WIRE_FIXTURE=1",
    ))
    .unwrap();
    assert_eq!(
        generated, recorded,
        "the wire format moved; the TypeScript decoder has to move with it"
    );

    // The economy's own fixture, in the role `fixtures/hex-directions.json` plays for the
    // direction table and `fixtures/snapshot-delta-wire.json` plays for the wire.
    //
    // Balance was the one system here with no representation: the costs were data, but every
    // figure that decides whether the data works — items per minute, what a generator carries,
    // what a building costs once its inputs are expanded to raw materials — existed nowhere and
    // was checked by nothing. This is that file. Rust computes it from the shipped catalogues and
    // `tests/balance.test.ts` recomputes the cost trees in TypeScript against the same
    // `definitions.json`, so the recorded numbers are pinned by two independent expansions rather
    // than by one implementation agreeing with its own output.
    //
    // Regenerate with `UPDATE_BALANCE_FIXTURE=1 cargo test balance_fixture`, then
    // `npx prettier --write fixtures/balance.json` because serde and prettier disagree about
    // short arrays, and read the diff: a change here is a change to what the game plays like, and
    // it should be one somebody meant. The comparison is over parsed JSON, so the formatting pass
    // cannot change what the test asserts.
    let report = balance::compute();
    let generated = serde_json::to_value(&report).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/balance.json");
    if std::env::var("UPDATE_BALANCE_FIXTURE").is_ok() {
        // The report, not the `Value` built from it: serde orders a `Value`'s keys
        // alphabetically and the struct in declaration order, and only the second keeps a
        // regenerated fixture diffable against the one it replaces.
        let mut text = serde_json::to_string_pretty(&report).unwrap();
        text.push('\n');
        std::fs::write(&path, text).unwrap();
    }
    let recorded: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .expect("fixtures/balance.json exists — regenerate with UPDATE_BALANCE_FIXTURE=1"),
    )
    .unwrap();
    assert_eq!(
        generated, recorded,
        "the economy moved; say so in the plan and regenerate the fixture"
    );
}
