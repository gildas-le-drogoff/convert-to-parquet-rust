// tests/conversions_tests.rs
use arrow::datatypes::{DataType, Field, Schema};
use csv::StringRecord;
use csv_to_parquet::analysis::analyze_block;
use std::sync::Arc;

fn single_column(values: &[&str]) -> Vec<StringRecord> {
    values
        .iter()
        .map(|v| StringRecord::from(vec![*v]))
        .collect()
}

#[test]
fn test_i64_overflow_via_analysis() {
    let block = single_column(&["9223372036854775808"]);
    let schema = Arc::new(Schema::new(vec![Field::new("a", DataType::Int64, true)]));
    let result = analyze_block(&block, schema, false).unwrap();
    let m = &result.metrics[0];
    assert_eq!(m.total_conversion_errors, 1);
    assert_eq!(m.total_valid_values, 0);
}

#[test]
fn test_u64_negative_via_analysis() {
    let block = single_column(&["-1"]);
    let schema = Arc::new(Schema::new(vec![Field::new("a", DataType::UInt64, true)]));
    let result = analyze_block(&block, schema, false).unwrap();
    let m = &result.metrics[0];
    assert_eq!(m.total_conversion_errors, 1);
}
