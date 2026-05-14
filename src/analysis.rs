// ============================================================
use anyhow::Result;
use arrow::array::ArrayRef;
use arrow::datatypes::{DataType, Date32Type, Float64Type, Int64Type, Schema, UInt64Type};
use arrow::record_batch::RecordBatch;
use csv::StringRecord;
use std::sync::Arc;

mod builders;
mod conversions;
pub mod types;

use builders::{
    build_binary_column, build_bool_column, build_large_binary_column, build_large_utf8_column,
    build_primitive_column, build_timestamp_column, build_utf8_column,
};
use conversions::{convert_to_date32, convert_to_f64, convert_to_i64, convert_to_u64};
pub use types::{BlockResult, ColumnMetrics, ConversionResult, ErrorCounters, ErrorSample};

pub fn analyze_block(
    records: &[StringRecord],
    schema: Arc<Schema>,
    force_utf8: bool,
) -> Result<BlockResult> {
    let column_count = schema.fields().len();
    let columns = split_columns(records, column_count);
    let mut metrics: Vec<ColumnMetrics> = schema
        .fields()
        .iter()
        .map(|f| ColumnMetrics::new(f.name()))
        .collect();
    for metric in &mut metrics {
        metric.total_values = records.len();
    }
    let mut arrays: Vec<ArrayRef> = Vec::with_capacity(column_count);
    for (i, field) in schema.fields().iter().enumerate() {
        arrays.push(build_column(
            field.data_type(),
            &columns[i],
            &mut metrics[i],
            force_utf8,
        ));
    }
    Ok(BlockResult {
        batch: RecordBatch::try_new(schema, arrays)?,
        metrics,
    })
}

/// Transpose records into per-column value slices, borrowing from `records`.
fn split_columns(records: &[StringRecord], column_count: usize) -> Vec<Vec<&str>> {
    let mut columns: Vec<Vec<&str>> = vec![Vec::with_capacity(records.len()); column_count];
    for record in records {
        for (i, column) in columns.iter_mut().enumerate() {
            column.push(record.get(i).unwrap_or(""));
        }
    }
    columns
}

fn build_column(
    dtype: &DataType,
    values: &[&str],
    metrics: &mut ColumnMetrics,
    force_utf8: bool,
) -> ArrayRef {
    match dtype {
        DataType::Int64 => build_primitive_column::<Int64Type>(values, metrics, convert_to_i64),
        DataType::UInt64 => build_primitive_column::<UInt64Type>(values, metrics, convert_to_u64),
        DataType::Boolean => build_bool_column(values, metrics),
        DataType::Float64 => build_primitive_column::<Float64Type>(values, metrics, convert_to_f64),
        DataType::Date32 => {
            build_primitive_column::<Date32Type>(values, metrics, convert_to_date32)
        }
        DataType::Timestamp(unit, _) => build_timestamp_column(values, metrics, *unit),
        DataType::Binary => build_binary_column(values, metrics),
        DataType::LargeBinary => build_large_binary_column(values, metrics),
        DataType::Utf8 => build_utf8_column(values, metrics),
        _ => build_large_utf8_column(values, metrics, force_utf8),
    }
}
