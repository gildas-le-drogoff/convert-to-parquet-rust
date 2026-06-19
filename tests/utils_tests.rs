// tests/utils_tests.rs
use arrow::datatypes::TimeUnit;
use convert_to_parquet::utils::{
    detect_header, is_null_text, parse_bool, parse_date_ymd, parse_timestamp, parse_timestamp_ms,
};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_is_null_text() {
    assert!(is_null_text(""));
    assert!(is_null_text(" "));
    assert!(is_null_text("NULL"));
    assert!(is_null_text("NaN"));
    assert!(!is_null_text("0"));
    assert!(!is_null_text("false"));
}

#[test]
fn test_parse_bool() {
    assert_eq!(parse_bool("true"), Some(true));
    assert_eq!(parse_bool("FALSE"), Some(false));
    assert_eq!(parse_bool("1"), Some(true));
    assert_eq!(parse_bool("0"), Some(false));
    assert_eq!(parse_bool("yes"), Some(true));
    assert_eq!(parse_bool("no"), Some(false));
    assert_eq!(parse_bool("maybe"), None);
}

#[test]
fn test_parse_date_ymd() {
    let d1 = parse_date_ymd("1970-01-01").unwrap();
    let d2 = parse_date_ymd("02/01/1970").unwrap();
    assert_eq!(d1, 0);
    assert_eq!(d2, 1);
    assert!(parse_date_ymd("invalid").is_none());
}

#[test]
fn test_parse_timestamp_ms() {
    let t1 = parse_timestamp_ms("1970-01-01 00:00:01").unwrap();
    assert_eq!(t1, 1_000);
    let t2 = parse_timestamp_ms("1000000000").unwrap();
    assert_eq!(t2, 1_000_000_000_000);
    assert!(parse_timestamp_ms("invalid").is_none());
}

#[test]
fn test_parse_timestamp_preserves_sub_millisecond_precision() {
    // Numeric microsecond epoch stays exact in a Microsecond column.
    assert_eq!(
        parse_timestamp("1700000000123456", TimeUnit::Microsecond),
        Some(1_700_000_000_123_456)
    );
    // Textual fractional seconds survive in Microsecond/Nanosecond columns.
    assert_eq!(
        parse_timestamp("2024-01-01T00:00:00.123456Z", TimeUnit::Microsecond),
        Some(1_704_067_200_123_456)
    );
    assert_eq!(
        parse_timestamp("2024-01-01T00:00:00.123456789Z", TimeUnit::Nanosecond),
        Some(1_704_067_200_123_456_789)
    );
    assert_eq!(
        parse_timestamp("1700000000", TimeUnit::Second),
        Some(1_700_000_000)
    );
}

fn csv_temp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    write!(f, "{content}").unwrap();
    f
}

#[test]
fn test_detect_header_single_line_identifiers() {
    let f = csv_temp("name,age,city\n");
    assert!(detect_header(f.path(), b',').unwrap());
}

#[test]
fn test_detect_header_single_line_data() {
    let f = csv_temp("1,2,3\n");
    assert!(!detect_header(f.path(), b',').unwrap());
}

#[test]
fn test_detect_header_typed_data_below_header() {
    let f = csv_temp("a,b\n1,2\n3,4\n");
    assert!(detect_header(f.path(), b',').unwrap());
}
