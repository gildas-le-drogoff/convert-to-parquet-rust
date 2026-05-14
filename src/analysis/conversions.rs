// ============================================================
use super::types::ConversionResult;
use crate::utils::{is_null_text, parse_bool, parse_date_ymd, parse_timestamp};
use arrow::datatypes::TimeUnit;

pub fn convert_to_i64(v: &str) -> ConversionResult<i64> {
    if is_null_text(v) {
        return ConversionResult::ExplicitNull;
    }
    match lexical_core::parse::<i64>(v.trim().as_bytes()) {
        Ok(x) => ConversionResult::Valid(x),
        Err(_) => ConversionResult::ConversionError(v.to_string()),
    }
}

pub fn convert_to_u64(v: &str) -> ConversionResult<u64> {
    if is_null_text(v) {
        return ConversionResult::ExplicitNull;
    }
    match lexical_core::parse::<u64>(v.trim().as_bytes()) {
        Ok(x) => ConversionResult::Valid(x),
        Err(_) => ConversionResult::ConversionError(v.to_string()),
    }
}

pub fn convert_to_bool(v: &str) -> ConversionResult<bool> {
    if is_null_text(v) {
        return ConversionResult::ExplicitNull;
    }
    match parse_bool(v) {
        Some(b) => ConversionResult::Valid(b),
        None => ConversionResult::ConversionError(v.to_string()),
    }
}

pub fn convert_to_f64(v: &str) -> ConversionResult<f64> {
    if is_null_text(v) {
        return ConversionResult::ExplicitNull;
    }
    match lexical_core::parse::<f64>(v.trim().as_bytes()) {
        Ok(x) => ConversionResult::Valid(x),
        Err(_) => ConversionResult::ConversionError(v.to_string()),
    }
}

pub fn convert_to_date32(v: &str) -> ConversionResult<i32> {
    if is_null_text(v) {
        return ConversionResult::ExplicitNull;
    }
    match parse_date_ymd(v) {
        Some(days) => ConversionResult::Valid(days),
        None => ConversionResult::ConversionError(v.to_string()),
    }
}

/// Convert directly into ticks of the target unit so sub-millisecond
/// precision survives for Microsecond/Nanosecond columns.
pub fn convert_to_timestamp(v: &str, unit: TimeUnit) -> ConversionResult<i64> {
    if is_null_text(v) {
        return ConversionResult::ExplicitNull;
    }
    match parse_timestamp(v, unit) {
        Some(ticks) => ConversionResult::Valid(ticks),
        None => ConversionResult::ConversionError(v.to_string()),
    }
}
