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

    if version == target_version {
        return Ok(Cow::Owned(serde_json::to_string(&value).map_err(
            |error| format!("migrated save could not be written: {error}"),
        )?));
    }
    Err(format!(
        "no migration path from save version {version} to {target_version}"
    ))
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
        let json = r#"{ "save_version": 15, "sentinel": [3, 2, 1] }"#;
        let migrated = migrate(json, 15).expect("current save");
        assert!(matches!(migrated, Cow::Borrowed(_)));
        assert_eq!(migrated, json);
    }

    #[test]
    fn unknown_older_and_newer_versions_fail_at_the_boundary() {
        assert_eq!(
            migrate(r#"{"save_version":13}"#, 15).unwrap_err(),
            "no migration path from save version 13 to 15"
        );
        assert_eq!(
            migrate(r#"{"save_version":16}"#, 15).unwrap_err(),
            "save version 16 is newer than supported version 15"
        );
    }

    #[test]
    fn version_fourteen_gains_an_explicit_absent_walk_goal() {
        let json = r#"{"save_version":14,"state":{"player":{"x":7,"carry_slots":4}}}"#;
        let migrated = migrate(json, 15).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 15);
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
        let migrated = migrate(r#"{"save_version":14}"#, 15).expect("migrated save");
        let value: Value = serde_json::from_str(&migrated).expect("migrated json");
        assert_eq!(value["save_version"], 15);
    }
}
