use serde_json::Error as JsonError;

use crate::domain::Profile;
use crate::error::{CoreError, CoreResult};

/// Parse a profile from the canonical JSON schema.
pub fn parse_profile_json(input: &str) -> CoreResult<Profile> {
    let profile: Profile = serde_json::from_str(input).map_err(map_json_error)?;
    validate_profile(profile)
}

/// Parse a profile, auto-detecting JSON versus the plain-text format.
pub fn parse_profile(input: &str) -> CoreResult<Profile> {
    let trimmed = input.trim_start();
    if trimmed.starts_with('{') {
        parse_profile_json(input)
    } else {
        super::profile_text::parse_profile_text(input)
    }
}

fn validate_profile(profile: Profile) -> CoreResult<Profile> {
    if profile.name.trim().is_empty() {
        return Err(CoreError::validation("profile.name must not be empty"));
    }
    Ok(profile)
}

fn map_json_error(error: JsonError) -> CoreError {
    CoreError::parse(format!("invalid profile JSON: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_profile() {
        let json = r#"{"name":"Ada Lovelace"}"#;
        let profile = parse_profile_json(json).unwrap();
        assert_eq!(profile.name, "Ada Lovelace");
        assert!(profile.roles.is_empty());
    }

    #[test]
    fn rejects_empty_name() {
        let json = r#"{"name":"   "}"#;
        let err = parse_profile_json(json).unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }

    #[test]
    fn rejects_malformed_json() {
        let err = parse_profile_json("{not json").unwrap_err();
        assert!(matches!(err, CoreError::Parse(_)));
    }

    #[test]
    fn autodetects_json() {
        let profile = parse_profile(r#"{"name":"Grace Hopper"}"#).unwrap();
        assert_eq!(profile.name, "Grace Hopper");
    }
}
