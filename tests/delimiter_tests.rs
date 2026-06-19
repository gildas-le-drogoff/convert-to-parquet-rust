// tests/delimiter_tests.rs
use convert_to_parquet::utils::detect_delimiter;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_detect_semicolon() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "a;b;c").unwrap();
    let d = detect_delimiter(f.path()).unwrap();
    assert_eq!(d, b';');
}

#[test]
fn test_detect_tab() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "a\tb\tc").unwrap();
    let d = detect_delimiter(f.path()).unwrap();
    assert_eq!(d, b'\t');
}
