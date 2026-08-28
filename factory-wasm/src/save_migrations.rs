//! Ordered save-envelope migration boundary.
//!
//! Released migrations belong here, one version step at a time, before the typed envelope is
//! validated. There is deliberately no guessed migration for historical formats: refusing a save
//! is safer than silently manufacturing state.

use serde_json::Value;
use std::borrow::Cow;

pub(super) fn migrate<'a>(json: &'a str, target_version: u16) -> Result<Cow<'a, str>, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|error| format!("malformed HXF1 save: {error}"))?;
    let version = value
        .get("save_version")
        .and_then(Value::as_u64)
        .and_then(|version| u16::try_from(version).ok())
        .ok_or("save has no valid save_version")?;

    if version == target_version {
        return Ok(Cow::Borrowed(json));
    }
    if version > target_version {
        return Err(format!(
            "save version {version} is newer than supported version {target_version}"
        ));
    }

    // Add released migrations as explicit adjacent steps. An unknown historical shape must never
    // be treated as if it were merely an older spelling of the current one.
    let mut value = value;
    let mut version = version;
    if version == 14 {
        walk_goal_14_to_15(&mut value);
        version = 15;
    }
    if version == 15 {
        compartment_storage_15_to_16(&mut value);
        version = 16;
    }
    if version == 16 {
        player_capabilities_16_to_17(&mut value);
        version = 17;
    }
    if version == 17 && target_version >= 18 {
        primitive_workshops_17_to_18(&mut value);
        version = 18;
    }
    if version == 18 && target_version >= 19 {
        transport_kits_18_to_19(&mut value);
        version = 19;
    }
    if version == 19 && target_version >= 20 {
        essential_bills_19_to_20(&mut value);
        version = 20;
    }
    if version == 20 && target_version >= 21 {
        research_foundations_20_to_21(&mut value);
        version = 21;
    }
    if version == 21 && target_version >= 22 {
        research_branches_21_to_22(&mut value);
        version = 22;
    }
    if version == 22 && target_version >= 23 {
        foundation_commissions_22_to_23(&mut value);
        version = 23;
    }
    if version == 23 && target_version >= 24 {
        industrial_bills_23_to_24(&mut value);
        version = 24;
    }
    if version == 24 && target_version >= 25 {
        mechanical_components_24_to_25(&mut value);
        version = 25;
    }
    if version == 25 && target_version >= 26 {
        power_and_tier_bills_25_to_26(&mut value);
        version = 26;
    }
    if version == 26 && target_version >= 27 {
        practical_projects_26_to_27(&mut value);
        version = 27;
    }

    if version == target_version {
        return Ok(Cow::Owned(serde_json::to_string(&value).map_err(
            |error| format!("migrated save could not be written: {error}"),
        )?));
    }
    Err(format!(
        "no migration path from save version {version} to {target_version}"
    ))
}

/// Practical projects make the hub's demand finite, and that moves one saved field.
///
/// Progress used to live inside a posted slot. It now belongs to the project, because a project
/// pays once: passing on a row whose reward can never be re-earned must not also destroy what was
/// already handed over. So each slot's `delivered` is lifted into `request_delivered`, keyed by the
/// project it was always progress against, and the slots keep only which row they hold.
///
/// A row the old file had already been paid for cannot carry progress forward — completion consumes
/// the bill — so any count standing against a filled project is dropped rather than restored. Zero
/// counts are dropped too: an absent key and a key holding nothing are the same run, and only one
/// of them can be the checksummed spelling of it.
///
/// Insight, research, stock and the board's contents are untouched. What the player has learned and
/// what the hub is asking for are exactly what they were; only the repeat income behind the board
/// is gone, and no saved field records that.
fn practical_projects_26_to_27(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.insert("save_version".into(), Value::from(27));
    if object.get("definition_version") == Some(&Value::from(21)) {
        object.insert("definition_version".into(), Value::from(22));
    }
    let Some(state) = object.get_mut("state").and_then(Value::as_object_mut) else {
        return;
    };
    let paid: Vec<String> = state
        .get("request_fills")
        .and_then(Value::as_object)
        .map(|fills| {
            fills
                .iter()
                .filter(|(_, count)| count.as_u64().unwrap_or(0) > 0)
                .map(|(id, _)| id.clone())
                .collect()
        })
        .unwrap_or_default();
    let mut delivered = serde_json::Map::new();
    if let Some(requests) = state.get_mut("requests").and_then(Value::as_array_mut) {
        for slot in requests.iter_mut() {
            let Some(slot) = slot.as_object_mut() else {
                continue;
            };
            let count = slot
                .remove("delivered")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            let Some(id) = slot.get("request_id").and_then(Value::as_u64) else {
                continue;
            };
            let key = id.to_string();
            if count > 0 && !paid.contains(&key) {
                delivered.insert(key, Value::from(count));
            }
        }
    }
    state.insert("request_delivered".into(), Value::Object(delivered));
}

/// Power and tier bills change only catalog prices, as at every price boundary since the transport
/// kits. Deep extractor and deep container stop asking for raw ore and the hydro generator stops
/// sharing the boiler's bill; no recipe, yield, work rate or research price moves with them.
///
/// State, placed entities, machine contents, insight and the checksum are untouched. A station
/// placed under the old bill refunds the new one when erased, which is exactly what rebuilding it
/// now costs, so the boundary conserves a line rather than paying a premium on it. None of the
/// parts these bills name has a recipe back to raw ore, so the revaluation cannot mint material.
fn power_and_tier_bills_25_to_26(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("save_version".into(), Value::from(26));
        if object.get("definition_version") == Some(&Value::from(20)) {
            object.insert("definition_version".into(), Value::from(21));
        }
    }
}

/// Keep stock, reserved jobs, contributions and the original checksum untouched. The component
/// output and duration are unchanged, so an already-paid job finishes exactly once. After checksum
/// verification, the loader closes the smaller commission if existing contributions satisfy it.
/// New demo layouts apply only to new games; a saved blueprint is never replaced by a template.
fn mechanical_components_24_to_25(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("save_version".into(), Value::from(25));
        if object.get("definition_version") == Some(&Value::from(19)) {
            object.insert("definition_version".into(), Value::from(20));
        }
        if object.get("scenario_version") == Some(&Value::from(6)) {
            object.insert("scenario_version".into(), Value::from(7));
        }
    }
}

/// Industrial bills change only catalog prices. State, active jobs and checksums stay intact.
/// Existing stations receive the current rebuild bill when erased, as at the essential-bills
/// boundary. This is a one-time revaluation, bounded by placed stations: rebuilding spends the
/// entire refund, and none of these parts can be converted back to raw ore.
fn industrial_bills_23_to_24(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("save_version".into(), Value::from(24));
        if object.get("definition_version") == Some(&Value::from(18)) {
            object.insert("definition_version".into(), Value::from(19));
        }
    }
}

/// Foundation commissions: technology catalog 11 makes the four starter automation nodes
/// grant-only. A factory that already finished Prove the line receives those four IDs if they
/// were missing; insight is not refunded or charged. Stage-zero factories are unchanged.
fn foundation_commissions_22_to_23(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.insert("save_version".into(), Value::from(23));
    if object.get("technology_version") == Some(&Value::from(10)) {
        object.insert("technology_version".into(), Value::from(11));
    }
    if object.get("scenario_version") == Some(&Value::from(5)) {
        object.insert("scenario_version".into(), Value::from(6));
    }
    let Some(state) = object.get_mut("state").and_then(Value::as_object_mut) else {
        return;
    };
    let stage = state
        .get("contract_stage")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if stage < 1 {
        return;
    }
    let mut researched: std::collections::BTreeSet<u64> = state
        .get("researched")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .collect();
    researched.extend([1, 2, 4, 8]);
    state.insert(
        "researched".into(),
        Value::Array(researched.into_iter().map(Value::from).collect()),
    );
}

/// Independent entry points remove three prerequisite edges without granting or revoking
/// research. Existing stock, insight, jobs and researched IDs survive byte-for-byte in state.
fn research_branches_21_to_22(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("save_version".into(), Value::from(22));
        if object.get("technology_version") == Some(&Value::from(9)) {
            object.insert("technology_version".into(), Value::from(10));
        }
    }
}

/// Branches and stages annotate the same technologies. Availability is derived from existing
/// insight and research, so no saved field or checksum changes and nothing is granted on load.
fn research_foundations_20_to_21(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("save_version".into(), Value::from(21));
        if object.get("technology_version") == Some(&Value::from(8)) {
            object.insert("technology_version".into(), Value::from(9));
        }
    }
}

/// Essential bills: one new item, one new drawing recipe, and the first extractor, composer,
/// container, generator and pole billed in manufactured parts instead of raw ore. Like the kit
/// step before it this is a price revision, not a state change: no saved field is added, removed
/// or reinterpreted, so stock, jobs, research, insight, entity identity and checksum survive.
///
/// What it does change is what those already-placed buildings hand back, and here the boundary
/// moves the *other* way from the kit one. `erase_refund` quotes the current bill, so an extractor
/// bought for four ore now returns two plates, a gear and two timber — more raw value than it
/// cost. That is a one-time revaluation of buildings that already exist, not a loop: the refund is
/// still exactly what rebuilding costs, so no repeated dismantle can profit, and no recipe turns a
/// plate, gear, timber or wire back into ore, so the windfall cannot be run through the tree twice.
fn essential_bills_19_to_20(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("save_version".into(), Value::from(20));
        if object.get("definition_version") == Some(&Value::from(17)) {
            object.insert("definition_version".into(), Value::from(18));
        }
    }
}

/// Transport kits: one new item, one new batch recipe, and a belt family bought with kits instead
/// of raw ore. No saved field is added, removed, or reinterpreted, so state and checksum survive
/// untouched and this step only advances the envelope's definition number.
///
/// The one thing that does change for an existing factory is what its already-placed belts hand
/// back. `erase_refund` quotes the current bill, so a legacy belt now returns the kit that would
/// rebuild it rather than the ore that bought it. That is deliberate and conserving: dismantling
/// and replacing a line is still free, the refund buys nothing but transport, and no recipe turns
/// a kit back into ore, so the boundary cannot be farmed for raw material.
fn transport_kits_18_to_19(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("save_version".into(), Value::from(19));
        if object.get("definition_version") == Some(&Value::from(16)) {
            object.insert("definition_version".into(), Value::from(17));
        }
    }
}

/// Two additive, initially unbuilt stations. Existing recipes, bills, entity state and checksum
/// are unchanged, including reserved jobs. No stock or insight is granted by this migration.
fn primitive_workshops_17_to_18(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("save_version".into(), Value::from(18));
        if object.get("definition_version") == Some(&Value::from(15)) {
            object.insert("definition_version".into(), Value::from(16));
        }
    }
}

/// Technology catalog 8 only adds two unresearched capability rows. A catalog-7 save therefore
/// already has the exact player values catalog 8 derives from its researched set; advancing the
/// envelope numbers preserves both state and checksum without inventing either breakthrough.
fn player_capabilities_16_to_17(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("save_version".into(), Value::from(17));
        if object.get("technology_version") == Some(&Value::from(7)) {
            object.insert("technology_version".into(), Value::from(8));
        }
    }
}

/// Compartment storage adds only default-empty maps and an empty cursor hand. Existing `inventory`
/// and `cargo` fields stay where they are so their checksum is unchanged; the version-16 runtime
/// classifies that legacy stock when it is read and drains it through the new compartments. The
/// accompanying definition revision only adds bounded source buffers, so it advances with this
/// migration instead of making every version-15 factory incompatible.
fn compartment_storage_15_to_16(value: &mut Value) {
    if let Some(player) = value
        .get_mut("state")
        .and_then(|state| state.get_mut("player"))
        .and_then(Value::as_object_mut)
    {
        player.insert("hand".into(), Value::Null);
    }
    if let Some(object) = value.as_object_mut() {
        object.insert("save_version".into(), Value::from(16));
        if object.get("definition_version") == Some(&Value::from(14)) {
            object.insert("definition_version".into(), Value::from(15));
        }
    }
}

/// Click-to-walk: the player carries the hex an autonomous walk is headed for.
///
/// A file written before walking to a click existed describes a player who is not walking, which is
/// a real state and not a missing one, so the goal is written as an explicit `null` rather than left
/// out for a defaulting deserializer to invent. `Core::checksum` hashes an absent goal as nothing,
/// so the migrated file still checksums to exactly what it did when it was written.
fn walk_goal_14_to_15(value: &mut Value) {
    if let Some(player) = value
        .get_mut("state")
        .and_then(|state| state.get_mut("player"))
        .and_then(Value::as_object_mut)
    {
        player.insert("walk_goal".into(), Value::Null);
    }
    if let Some(object) = value.as_object_mut() {
        object.insert("save_version".into(), Value::from(15));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_envelopes_pass_through_without_reserialization() {
        let json = r#"{ "save_version": 17, "sentinel": [3, 2, 1] }"#;
        let migrated = migrate(json, 17).expect("current save");
        assert!(matches!(migrated, Cow::Borrowed(_)));
        assert_eq!(migrated, json);
    }

    #[test]
    fn unknown_older_and_newer_versions_fail_at_the_boundary() {
        assert_eq!(
            migrate(r#"{"save_version":13}"#, 17).unwrap_err(),
            "no migration path from save version 13 to 17"
        );
        assert_eq!(
            migrate(r#"{"save_version":18}"#, 17).unwrap_err(),
            "save version 18 is newer than supported version 17"
        );
    }

    #[test]
    fn version_fourteen_gains_an_explicit_absent_walk_goal() {
        let json = r#"{"save_version":14,"state":{"player":{"x":7,"carry_slots":4}}}"#;
        let migrated = migrate(json, 17).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 17);
        assert_eq!(value["state"]["player"]["walk_goal"], Value::Null);
        // Nothing else about the player moves: the step adds a field, it does not rewrite a save.
        assert_eq!(value["state"]["player"]["x"], 7);
        assert_eq!(value["state"]["player"]["carry_slots"], 4);
    }

    #[test]
    fn a_fourteen_envelope_without_a_player_still_reaches_fifteen() {
        // The step must not depend on a shape it did not verify. A file the typed envelope will
        // reject for other reasons should reach that rejection, not be turned away here as
        // unmigratable.
        let migrated = migrate(r#"{"save_version":14}"#, 17).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 17);
    }

    #[test]
    fn version_fifteen_gains_an_explicit_empty_hand_without_moving_machine_stock() {
        let json = r#"{"save_version":15,"definition_version":14,"state":{"player":{"inventory":{"3":4}},"entities":[{"inventory":{"5":12},"cargo":{"item_id":4,"quantity":1}}]}}"#;
        let migrated = migrate(json, 17).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 17);
        assert_eq!(value["definition_version"], 15);
        assert_eq!(value["state"]["player"]["hand"], Value::Null);
        assert_eq!(value["state"]["entities"][0]["inventory"]["5"], 12);
        assert_eq!(value["state"]["entities"][0]["cargo"]["item_id"], 4);
    }

    #[test]
    fn version_eighteen_advances_the_definition_catalog_without_touching_stock_or_belts() {
        let json = r#"{"save_version":18,"definition_version":16,"technology_version":8,"state":{"player":{"inventory":{"1":9}},"entities":[{"definition_id":2,"orientation":0}]}}"#;
        let migrated = migrate(json, 19).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 19);
        assert_eq!(value["definition_version"], 17);
        // The step is a price revision, not a state rewrite: the ore in the pack stays ore, and a
        // placed belt keeps its own identity rather than being restocked with kits.
        assert_eq!(value["state"]["player"]["inventory"]["1"], 9);
        assert_eq!(value["state"]["entities"][0]["definition_id"], 2);
        assert_eq!(value["technology_version"], 8);
    }

    #[test]
    fn a_version_fifteen_file_reaches_twenty_through_every_definition_step() {
        let json = r#"{"save_version":15,"definition_version":14,"technology_version":7,"state":{"player":{}}}"#;
        let migrated = migrate(json, 20).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 20);
        assert_eq!(value["definition_version"], 18);
        assert_eq!(value["technology_version"], 8);
    }

    #[test]
    fn version_twenty_five_reprices_power_and_tier_bills_without_touching_state() {
        let json = r#"{"save_version":25,"definition_version":20,"technology_version":11,"scenario_version":7,"state":{"insight":9,"player":{"inventory":{"1":7}},"entities":[{"definition_id":19,"orientation":0,"inventory":{"1":5}}]}}"#;
        let migrated = migrate(json, 26).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 26);
        assert_eq!(value["definition_version"], 21);
        // A price revision, not a state rewrite: the ore in the pack stays ore, and the placed deep
        // extractor keeps its identity and everything it was holding.
        assert_eq!(value["state"]["player"]["inventory"]["1"], 7);
        assert_eq!(value["state"]["entities"][0]["definition_id"], 19);
        assert_eq!(value["state"]["entities"][0]["inventory"]["1"], 5);
        assert_eq!(value["state"]["insight"], 9);
        // Neither research nor the scenario moves at this boundary.
        assert_eq!(value["technology_version"], 11);
        assert_eq!(value["scenario_version"], 7);
    }

    /// Progress made against a project survives the move off the board slot — except where the
    /// project has already been paid for, which under the old rules was an ordinary state and under
    /// the new ones would be a project owing a second reward.
    #[test]
    fn version_twenty_six_lifts_delivered_progress_onto_the_project() {
        let json = r#"{"save_version":26,"definition_version":21,"technology_version":11,"scenario_version":7,"state":{"insight":40,"request_fills":{"1":1},"requests":[{"request_id":1,"delivered":4},{"request_id":5,"delivered":7},{"request_id":9,"delivered":0}]}}"#;
        let migrated = migrate(json, 27).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 27);
        assert_eq!(value["definition_version"], 22);
        // The part-filled row keeps its seven; the player handed those over and the hub will not
        // ask for them twice.
        assert_eq!(value["state"]["request_delivered"]["5"], 7);
        // Project 1 was already filled and paid, so its four are a leftover of a row that was
        // reposted under repeatable demand. Carrying them would credit a retired project.
        assert!(value["state"]["request_delivered"].get("1").is_none());
        // An untouched row writes no entry rather than a zero, so the map stays the set of debts.
        assert!(value["state"]["request_delivered"].get("9").is_none());
        // The slots keep their identity and lose only the count they no longer own.
        assert_eq!(value["state"]["requests"][1]["request_id"], 5);
        assert!(value["state"]["requests"][1].get("delivered").is_none());
        assert_eq!(value["state"]["insight"], 40);
        assert_eq!(value["technology_version"], 11);
        assert_eq!(value["scenario_version"], 7);
    }

    #[test]
    fn version_twenty_five_leaves_an_unexpected_definition_version_alone() {
        let json = r#"{"save_version":25,"definition_version":18,"state":{}}"#;
        let migrated = migrate(json, 26).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 26);
        // Only the definition version this step was written against advances. Anything else is a
        // shape this migration never saw, and guessing at it is what the module refuses to do.
        assert_eq!(value["definition_version"], 18);
    }

    #[test]
    fn version_twenty_two_grants_foundation_automation_after_the_opening_commission() {
        let json = r#"{"save_version":22,"technology_version":10,"scenario_version":5,"state":{"contract_stage":1,"researched":[3],"insight":12}}"#;
        let migrated = migrate(json, 23).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 23);
        assert_eq!(value["technology_version"], 11);
        assert_eq!(value["scenario_version"], 6);
        assert_eq!(value["state"]["insight"], 12);
        assert_eq!(
            value["state"]["researched"],
            serde_json::json!([1, 2, 3, 4, 8])
        );
    }

    #[test]
    fn version_twenty_two_leaves_a_stage_zero_factory_unresearched() {
        let json = r#"{"save_version":22,"technology_version":10,"scenario_version":5,"state":{"contract_stage":0,"researched":[],"insight":4}}"#;
        let migrated = migrate(json, 23).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 23);
        assert_eq!(value["state"]["researched"], serde_json::json!([]));
        assert_eq!(value["state"]["insight"], 4);
    }

    #[test]
    fn version_nineteen_reprices_the_stations_without_touching_stock_or_machines() {
        let json = r#"{"save_version":19,"definition_version":17,"technology_version":8,"state":{"player":{"inventory":{"1":9}},"entities":[{"definition_id":1,"orientation":0,"inventory":{"1":4}}]}}"#;
        let migrated = migrate(json, 20).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 20);
        assert_eq!(value["definition_version"], 18);
        // A price revision, not a state rewrite: the ore in the pack stays ore, and the placed
        // extractor keeps its identity and everything it was holding.
        assert_eq!(value["state"]["player"]["inventory"]["1"], 9);
        assert_eq!(value["state"]["entities"][0]["definition_id"], 1);
        assert_eq!(value["state"]["entities"][0]["inventory"]["1"], 4);
        assert_eq!(value["technology_version"], 8);
    }

    #[test]
    fn version_sixteen_advances_the_capability_catalog_without_granting_research() {
        let json = r#"{"save_version":16,"technology_version":7,"state":{"researched":[1,4],"player":{"carry_slots":8,"build_range":8870}}}"#;
        let migrated = migrate(json, 17).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 17);
        assert_eq!(value["technology_version"], 8);
        assert_eq!(value["state"]["researched"], serde_json::json!([1, 4]));
        assert_eq!(value["state"]["player"]["carry_slots"], 8);
        assert_eq!(value["state"]["player"]["build_range"], 8870);
    }
}
