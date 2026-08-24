use crate::config::Result;
use extism_pdk::warn;
use nd_pdk::{host::config, lyrics::Error};
use std::num::IntErrorKind;

pub fn get_string(key: &str) -> Result<Option<String>> {
    Ok(get_raw_string(key)?.filter(|s| !s.trim().is_empty()))
}

/// Like [`get_string`], but keeps blank values.
pub fn get_raw_string(key: &str) -> Result<Option<String>> {
    config::get(key).map_err(|e| Error::new(e.to_string()))
}

pub fn get_bool(key: &str, default: bool) -> Result<bool> {
    Ok(get_parsed(key, "boolean", parse_bool)?.unwrap_or(default))
}

pub fn get_i64(key: &str, default: i64) -> Result<i64> {
    Ok(get_parsed(key, "integer", parse_i64)?.unwrap_or(default))
}

pub fn get_f64(key: &str, default: f64) -> Result<f64> {
    Ok(get_parsed(key, "number", parse_f64)?.unwrap_or(default))
}

pub fn get_optional_i32(key: &str) -> Result<Option<i32>> {
    get_parsed(key, "32-bit integer", parse_i32)
}

fn get_parsed<T>(
    key: &str,
    kind: &str,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Result<Option<T>> {
    let Some(raw) = get_string(key)? else {
        return Ok(None);
    };

    match parse(&raw) {
        Some(value) => Ok(Some(value)),
        None => {
            warn!("{key} value '{raw}' is not a valid {kind}, ignoring it");
            Ok(None)
        }
    }
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn parse_i64(raw: &str) -> Option<i64> {
    match raw.trim().parse::<i64>() {
        Ok(value) => Some(value),
        Err(e) => match e.kind() {
            IntErrorKind::PosOverflow => Some(i64::MAX),
            IntErrorKind::NegOverflow => Some(i64::MIN),
            _ => None,
        },
    }
}

fn parse_i32(raw: &str) -> Option<i32> {
    i32::try_from(parse_i64(raw)?).ok()
}

fn parse_f64(raw: &str) -> Option<f64> {
    let value = raw.trim().parse::<f64>().ok()?;
    (!value.is_nan()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn check_bool(raw: &str, expected: Option<bool>) {
        assert_eq!(parse_bool(raw), expected, "boolean from {raw:?}");
    }

    #[track_caller]
    fn check_i64(raw: &str, expected: Option<i64>) {
        assert_eq!(parse_i64(raw), expected, "integer from {raw:?}");
    }

    #[track_caller]
    fn check_i32(raw: &str, expected: Option<i32>) {
        assert_eq!(parse_i32(raw), expected, "32-bit integer from {raw:?}");
    }

    #[track_caller]
    fn check_f64(raw: &str, expected: Option<f64>) {
        assert_eq!(parse_f64(raw), expected, "number from {raw:?}");
    }

    #[test]
    fn parse_bool_accepts_valid_values() {
        for (raw, expected) in [
            ("true", true),
            (" True ", true),
            ("1", true),
            ("false", false),
            ("FALSE", false),
            ("0", false),
        ] {
            check_bool(raw, Some(expected));
        }
    }

    #[test]
    fn parse_bool_rejects_invalid_values() {
        for raw in ["yes", "no", "abc", "", "  "] {
            check_bool(raw, None);
        }
    }

    #[test]
    fn parse_i64_accepts_valid_values() {
        for (raw, expected) in [("336", 336), (" 24 ", 24), ("\t-5\n", -5)] {
            check_i64(raw, Some(expected));
        }

        check_i64(&i64::MAX.to_string(), Some(i64::MAX));
        check_i64(&i64::MIN.to_string(), Some(i64::MIN));
    }

    #[test]
    fn parse_i64_saturates_on_overflow() {
        check_i64("10000000000000000000000000", Some(i64::MAX));
        check_i64("-10000000000000000000000000", Some(i64::MIN));
    }

    #[test]
    fn parse_i32_accepts_valid_values() {
        for (raw, expected) in [(" 42 ", 42), ("-7", -7)] {
            check_i32(raw, Some(expected));
        }

        check_i32(&i32::MAX.to_string(), Some(i32::MAX));
        check_i32(&i32::MIN.to_string(), Some(i32::MIN));
    }

    #[test]
    fn parse_i32_rejects_values_outside_range() {
        for raw in [
            (i32::MAX as i64 + 1).to_string(),
            (i32::MIN as i64 - 1).to_string(),
            "10000000000000000000000000".to_string(),
        ] {
            check_i32(&raw, None);
        }

        check_i32(&i32::MAX.to_string(), Some(i32::MAX));
        check_i32(&i32::MIN.to_string(), Some(i32::MIN));
    }

    #[test]
    fn parse_integer_rejects_invalid_values() {
        for raw in ["abc", "", "  ", "12x", "1.5"] {
            check_i64(raw, None);
            check_i32(raw, None);
        }
    }

    #[test]
    fn parse_f64_accepts_finite_values() {
        for (raw, expected) in [("3", 3.0), (" 2.5 ", 2.5), ("-0.5", -0.5)] {
            check_f64(raw, Some(expected));
        }
    }

    #[test]
    fn parse_f64_handles_overflow() {
        check_f64("1e400", Some(f64::INFINITY));
        check_f64("-1e400", Some(f64::NEG_INFINITY));
    }

    #[test]
    fn parse_f64_rejects_invalid_values() {
        for raw in ["nan", "NaN", "abc", "", "  "] {
            check_f64(raw, None);
        }
    }
}
