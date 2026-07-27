use crate::config::Result;
use extism_pdk::warn;
use nd_pdk::{host::config, lyrics::Error};
use std::num::IntErrorKind;

pub fn get_string(key: &str) -> Result<Option<String>> {
    config::get(key)
        .map_err(|e| Error::new(e.to_string()))
        .map(|v| v.filter(|s| !s.trim().is_empty()))
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

    #[test]
    fn test_parse_bool_accepts_both_spellings() {
        assert_eq!(parse_bool(" True "), Some(true));
        assert_eq!(parse_bool("FALSE"), Some(false));
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("0"), Some(false));
    }

    #[test]
    fn test_parse_bool_garbage_is_none() {
        assert_eq!(parse_bool("yes"), None);
        assert_eq!(parse_bool("abc"), None);
        assert_eq!(parse_bool(""), None);
    }

    #[test]
    fn test_parse_i64_accepts_integers() {
        assert_eq!(parse_i64("336"), Some(336));
        assert_eq!(parse_i64("-5"), Some(-5));
        assert_eq!(parse_i64(&i64::MAX.to_string()), Some(i64::MAX));
    }

    #[test]
    fn test_parse_i64_overflow_saturates() {
        assert_eq!(parse_i64("10000000000000000000000000"), Some(i64::MAX));
        assert_eq!(parse_i64("-10000000000000000000000000"), Some(i64::MIN));
    }

    #[test]
    fn test_parse_i64_trims_whitespace() {
        assert_eq!(parse_i64(" 24 "), Some(24));
        assert_eq!(parse_i64("\t-5\n"), Some(-5));
    }

    #[test]
    fn test_parse_i64_garbage_is_none() {
        assert_eq!(parse_i64("abc"), None);
        assert_eq!(parse_i64(""), None);
        assert_eq!(parse_i64("12x"), None);
    }

    #[test]
    fn test_parse_i32_narrows() {
        assert_eq!(parse_i32(" 42 "), Some(42));
        assert_eq!(parse_i32("-7"), Some(-7));
        assert_eq!(parse_i32(&i32::MAX.to_string()), Some(i32::MAX));
    }

    #[test]
    fn test_parse_i32_rejects_out_of_range() {
        assert_eq!(parse_i32(&(i32::MAX as i64 + 1).to_string()), None);
        assert_eq!(parse_i32(&(i32::MIN as i64 - 1).to_string()), None);
        assert_eq!(parse_i32("10000000000000000000000000"), None);
    }

    #[test]
    fn test_parse_i32_garbage_is_none() {
        assert_eq!(parse_i32("abc"), None);
        assert_eq!(parse_i32(""), None);
    }

    #[test]
    fn test_parse_f64_accepts_fractions() {
        assert_eq!(parse_f64("3"), Some(3.0));
        assert_eq!(parse_f64(" 2.5 "), Some(2.5));
        assert_eq!(parse_f64("-0.5"), Some(-0.5));
    }

    #[test]
    fn test_parse_f64_overflow_is_infinite() {
        assert_eq!(parse_f64("1e400"), Some(f64::INFINITY));
        assert_eq!(parse_f64("-1e400"), Some(f64::NEG_INFINITY));
    }

    #[test]
    fn test_parse_f64_rejects_nan_and_garbage() {
        assert_eq!(parse_f64("nan"), None);
        assert_eq!(parse_f64("abc"), None);
        assert_eq!(parse_f64(""), None);
    }
}
