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

    if version == target_version {
        return Ok(Cow::Owned(serde_json::to_string(&value).map_err(
            |error| format!("migrated save could not be written: {error}"),
        )?));
    }
    Err(format!(
        "no migration path from save version {version} to {target_version}"
    ))
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
