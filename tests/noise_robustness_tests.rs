// tests/noise_robustness_tests.rs
//
// Robustness = a minority of unparseable values must not demote a column to
// text, and those values must become NULL at conversion without panicking.
use arrow::array::{Array, Int64Array};
use arrow::datatypes::DataType;
use convert_to_parquet::conversion::convert_convert_to_parquet;
use convert_to_parquet::schema::infer_schema;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

fn csv_temp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    write!(f, "{content}").unwrap();
    f
}

/// `valid` copies of `good`, then `noise` copies of `bad`, single column `a`.
fn noisy_column(good: &str, valid: usize, bad: &str, noise: usize) -> String {
    let mut s = String::from("a\n");
    for _ in 0..valid {
        s.push_str(good);
        s.push('\n');
    }
    for _ in 0..noise {
        s.push_str(bad);
        s.push('\n');
    }
    s
}

// ---- inference tolerates minority noise -----------------------------------

#[test]
fn test_int_tolerates_minority_noise() {
    let csv = csv_temp(&noisy_column("42", 990, "boom", 10));
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert_eq!(schema.fields()[0].data_type(), &DataType::Int64);
}

#[test]
fn test_float_tolerates_minority_noise() {
    let csv = csv_temp(&noisy_column("3.14", 990, "boom", 10));
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert_eq!(schema.fields()[0].data_type(), &DataType::Float64);
}

#[test]
fn test_bool_tolerates_minority_noise() {
    let csv = csv_temp(&noisy_column("true", 990, "boom", 10));
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert_eq!(schema.fields()[0].data_type(), &DataType::Boolean);
}

#[test]
fn test_date_tolerates_minority_noise() {
    let csv = csv_temp(&noisy_column("2024-06-16", 990, "boom", 10));
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert_eq!(schema.fields()[0].data_type(), &DataType::Date32);
}

// ---- noise above threshold demotes to text --------------------------------

#[test]
fn test_majority_noise_falls_back_to_text() {
    // 60% garbage / 40% int: below the 0.95 confidence floor.
    let csv = csv_temp(&noisy_column("42", 400, "boom", 600));
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert_eq!(schema.fields()[0].data_type(), &DataType::LargeUtf8);
}

#[test]
fn test_threshold_boundary_just_below_falls_back_to_text() {
    // 94% int: just under 0.95, must not be inferred as Int64.
    let csv = csv_temp(&noisy_column("42", 940, "boom", 60));
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert_eq!(schema.fields()[0].data_type(), &DataType::LargeUtf8);
}

#[test]
fn test_threshold_boundary_exactly_at_floor_is_typed() {
    // Exactly 95% int: at the floor, the type is accepted.
    let csv = csv_temp(&noisy_column("42", 950, "boom", 50));
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert_eq!(schema.fields()[0].data_type(), &DataType::Int64);
}

// ---- end-to-end: noisy cells become NULL, no panic ------------------------

fn read_int64_column(path: &Path) -> Vec<Option<i64>> {
    let file = File::open(path).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let mut out = Vec::new();
    for batch in reader {
        let batch = batch.unwrap();
        let col = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap();
        for i in 0..col.len() {
            out.push(col.is_valid(i).then(|| col.value(i)));
        }
    }
    out
}

#[test]
fn test_end_to_end_noise_coerced_to_null() {
    let csv = csv_temp(&noisy_column("7", 990, "boom", 10));
    let output = NamedTempFile::new().unwrap();
    convert_convert_to_parquet(csv.path(), output.path(), true, false, false, None).unwrap();

    let values = read_int64_column(output.path());
    assert_eq!(values.len(), 1000);
    let nulls = values.iter().filter(|v| v.is_none()).count();
    let valid: Vec<i64> = values.into_iter().flatten().collect();
    assert_eq!(nulls, 10);
    assert_eq!(valid.len(), 990);
    assert!(valid.iter().all(|&v| v == 7));
}
