// tests/analysis_tests.rs
use arrow::datatypes::{DataType, Field, Schema};
use csv::StringRecord;
use csv_to_parquet::analysis::analyze_block;
use std::sync::Arc;

fn records(lines: &[&str]) -> Vec<StringRecord> {
    lines
        .iter()
        .map(|line| StringRecord::from(line.split(',').collect::<Vec<_>>()))
        .collect()
}

#[test]
fn test_analyze_block_simple() {
    let block = records(&["1,true,2024-01-01", "2,false,2024-01-02"]);
    let schema = Arc::new(Schema::new(vec![
        Field::new("a", DataType::Int64, true),
        Field::new("b", DataType::Boolean, true),
        Field::new("c", DataType::Date32, true),
    ]));
    let result = analyze_block(&block, schema.clone(), false).unwrap();
    assert_eq!(result.batch.num_rows(), 2);
    assert_eq!(result.batch.num_columns(), 3);
    for metric in &result.metrics {
        assert_eq!(
            metric.total_values,
            metric.total_valid_values + metric.total_null_text + metric.total_conversion_errors
        );
    }
}
