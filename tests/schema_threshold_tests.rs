// tests/schema_threshold_tests.rs
use arrow::datatypes::DataType;
use csv_to_parquet::schema::infer_schema;
use std::io::Write;
use tempfile::NamedTempFile;

fn csv_temp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    write!(f, "{content}").unwrap();
    f
}

#[test]
fn test_inference_pure_timestamp() {
    let mut content = String::from("a\n");
    for _ in 0..1000 {
        content.push_str("2024-01-01 00:00:00\n");
    }
    let csv = csv_temp(&content);
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert!(matches!(
        schema.fields()[0].data_type(),
        DataType::Timestamp(_, _)
    ));
}

// 99.5% valid timestamps + 0.5% garbage: above the confidence threshold,
// so the majority type wins and the noise is coerced to NULL at conversion.
#[test]
fn test_inference_timestamp_tolerates_minority_noise() {
    let mut content = String::from("a\n");
    for _ in 0..995 {
        content.push_str("2024-01-01 00:00:00\n");
    }
    for _ in 0..5 {
        content.push_str("invalid\n");
    }
    let csv = csv_temp(&content);
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert!(matches!(
        schema.fields()[0].data_type(),
        DataType::Timestamp(_, _)
    ));
}

#[test]
fn test_inference_float_vs_int() {
    let csv = csv_temp(
        "a\n\
         1\n\
         2\n\
         3.14\n",
    );
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert_eq!(schema.fields()[0].data_type(), &DataType::Float64);
}
