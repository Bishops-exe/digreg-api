use serde::{Deserialize, Deserializer};

/// Deserialize an integer the API uses as a boolean: `0` → `false`, any other
/// value → `true`.
///
/// Some fields are a strict `0`/`1` flag, others (e.g. `cancelled` on grade
/// entries) are documented as "int `0`/non-`0`" — see `docs/API.md`. Treating
/// everything non-zero as `true` covers both.
pub fn parse_int_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(u32::deserialize(deserializer)? != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    #[serde(transparent)]
    struct Flag(#[serde(deserialize_with = "parse_int_bool")] bool);

    fn parse(json: &str) -> Flag {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn zero_is_false() {
        assert!(!parse("0").0);
    }

    #[test]
    fn any_non_zero_integer_is_true() {
        assert!(parse("1").0);
        assert!(parse("5").0);
    }

    #[test]
    fn a_json_boolean_is_rejected() {
        assert!(serde_json::from_str::<Flag>("true").is_err());
    }
}