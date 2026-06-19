// tests/error_analysis_tests.rs
use arrow::datatypes::{DataType, Field, Schema};
use csv::StringRecord;
use convert_to_parquet::analysis::analyze_block;
use std::sync::Arc;

#[test]
fn test_explicit_null() {
    let block = vec![StringRecord::from(vec!["NULL"])];
    let schema = Arc::new(Schema::new(vec![Field::new("a", DataType::Int64, true)]));
    let result = analyze_block(&block, schema, false).unwrap();
    let m = &result.metrics[0];
    assert_eq!(m.total_null_text, 1);
    assert_eq!(m.total_valid_values, 0);
}
