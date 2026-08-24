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
    Err(format!(
        "no migration path from save version {version} to {target_version}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_envelopes_pass_through_without_reserialization() {
        let json = r#"{ "save_version": 14, "sentinel": [3, 2, 1] }"#;
        let migrated = migrate(json, 14).expect("current save");
        assert!(matches!(migrated, Cow::Borrowed(_)));
        assert_eq!(migrated, json);
    }

    #[test]
    fn unknown_older_and_newer_versions_fail_at_the_boundary() {
        assert_eq!(
            migrate(r#"{"save_version":13}"#, 14).unwrap_err(),
            "no migration path from save version 13 to 14"
        );
        assert_eq!(
            migrate(r#"{"save_version":15}"#, 14).unwrap_err(),
            "save version 15 is newer than supported version 14"
        );
    }
}
