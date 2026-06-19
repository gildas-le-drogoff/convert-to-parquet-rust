// tests/heuristic_robustness_tests.rs
use arrow::datatypes::DataType;
use convert_to_parquet::schema::infer_schema;
use std::io::Write;
use tempfile::NamedTempFile;

fn csv_temp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    write!(f, "{content}").unwrap();
    f
}

#[test]
fn test_mostly_positive_integer_column() {
    let mut content = String::from("a\n");
    for _ in 0..995 {
        content.push_str("10\n");
    }
    for _ in 0..5 {
        content.push_str("-1\n");
    }
    let csv = csv_temp(&content);
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert_eq!(schema.fields()[0].data_type(), &DataType::Int64);
}

#[test]
fn test_fully_null_column() {
    let csv = csv_temp(
        "a\n\
         NULL\n\
         NULL\n\
         NULL\n",
    );
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert_eq!(schema.fields()[0].data_type(), &DataType::LargeUtf8);
}

#[test]
fn test_timestamp_with_microseconds() {
    let csv = csv_temp(
        "a\n\
         2024-01-01 12:00:00.123456\n\
         2024-01-02 12:00:00.654321\n",
    );
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert!(matches!(
        schema.fields()[0].data_type(),
        DataType::Timestamp(_, _)
    ));
}
