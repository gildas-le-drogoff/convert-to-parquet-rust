// ============================================================
use super::conversions::{convert_to_bool, convert_to_timestamp};
use super::types::{ColumnMetrics, ConversionResult};
use crate::utils::is_null_text;
use arrow::array::{
    ArrayRef, BooleanBuilder, GenericBinaryBuilder, GenericStringBuilder, OffsetSizeTrait,
    PrimitiveBuilder,
};
use arrow::datatypes::{
    ArrowPrimitiveType, TimeUnit, TimestampMicrosecondType, TimestampMillisecondType,
    TimestampNanosecondType, TimestampSecondType,
};
use std::sync::Arc;

pub fn build_primitive_column<P: ArrowPrimitiveType>(
    values: &[&str],
    metrics: &mut ColumnMetrics,
    convert: impl Fn(&str) -> ConversionResult<P::Native>,
) -> ArrayRef {
    let mut builder = PrimitiveBuilder::<P>::new();
    for value in values {
        match convert(value) {
            ConversionResult::Valid(x) => {
                builder.append_value(x);
                metrics.total_valid_values += 1;
            }
            ConversionResult::ExplicitNull => {
                builder.append_null();
                metrics.total_null_text += 1;
            }
            ConversionResult::ConversionError(raw) => {
                builder.append_null();
                metrics.total_conversion_errors += 1;
                metrics.error_samples.add(raw);
            }
        }
    }
    Arc::new(builder.finish())
}

pub fn build_bool_column(values: &[&str], metrics: &mut ColumnMetrics) -> ArrayRef {
    let mut builder = BooleanBuilder::new();
    for value in values {
        match convert_to_bool(value) {
            ConversionResult::Valid(x) => {
                builder.append_value(x);
                metrics.total_valid_values += 1;
            }
            ConversionResult::ExplicitNull => {
                builder.append_null();
                metrics.total_null_text += 1;
            }
            ConversionResult::ConversionError(raw) => {
                builder.append_null();
                metrics.total_conversion_errors += 1;
                metrics.error_samples.add(raw);
            }
        }
    }
    Arc::new(builder.finish())
}

pub fn build_timestamp_column(
    values: &[&str],
    metrics: &mut ColumnMetrics,
    unit: TimeUnit,
) -> ArrayRef {
    let convert = |v: &str| convert_to_timestamp(v, unit);
    match unit {
        TimeUnit::Second => build_primitive_column::<TimestampSecondType>(values, metrics, convert),
        TimeUnit::Millisecond => {
            build_primitive_column::<TimestampMillisecondType>(values, metrics, convert)
        }
        TimeUnit::Microsecond => {
            build_primitive_column::<TimestampMicrosecondType>(values, metrics, convert)
        }
        TimeUnit::Nanosecond => {
            build_primitive_column::<TimestampNanosecondType>(values, metrics, convert)
        }
    }
}

fn build_text_column<O: OffsetSizeTrait>(
    values: &[&str],
    metrics: &mut ColumnMetrics,
    keep_null_text: bool,
) -> ArrayRef {
    let mut builder = GenericStringBuilder::<O>::new();
    for value in values {
        if !keep_null_text && is_null_text(value) {
            builder.append_null();
            metrics.total_null_text += 1;
        } else {
            builder.append_value(value);
            metrics.total_valid_values += 1;
        }
    }
    Arc::new(builder.finish())
}

pub fn build_utf8_column(values: &[&str], metrics: &mut ColumnMetrics) -> ArrayRef {
    build_text_column::<i32>(values, metrics, false)
}

pub fn build_large_utf8_column(
    values: &[&str],
    metrics: &mut ColumnMetrics,
    keep_null_text: bool,
) -> ArrayRef {
    build_text_column::<i64>(values, metrics, keep_null_text)
}

fn build_bytes_column<O: OffsetSizeTrait>(
    values: &[&str],
    metrics: &mut ColumnMetrics,
) -> ArrayRef {
    let mut builder = GenericBinaryBuilder::<O>::new();
    for value in values {
        if is_null_text(value) {
            builder.append_null();
            metrics.total_null_text += 1;
        } else {
            builder.append_value(value.as_bytes());
            metrics.total_valid_values += 1;
        }
    }
    Arc::new(builder.finish())
}

pub fn build_binary_column(values: &[&str], metrics: &mut ColumnMetrics) -> ArrayRef {
    build_bytes_column::<i32>(values, metrics)
}

pub fn build_large_binary_column(values: &[&str], metrics: &mut ColumnMetrics) -> ArrayRef {
    build_bytes_column::<i64>(values, metrics)
}
