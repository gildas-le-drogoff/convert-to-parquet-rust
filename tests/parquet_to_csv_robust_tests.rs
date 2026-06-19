// ============================================================
// tests/parquet_to_csv_robust_tests.rs
//
// Robust coverage for the Parquet → CSV inverse path
// (to_csv::convert_parquet_to_csv).
//
// Unlike inverse_and_inspect_tests.rs (which feeds CSV through the
// inference pipeline first), these tests build Parquet files directly
// from typed Arrow RecordBatches. This decouples the assertions from
// CSV type-inference and exercises the CSV *writer* on every column
// type, on nulls, on values that require RFC-4180 quoting, on Unicode,
// on multi-batch readers, and on the error paths.
// ============================================================

use arrow::array::{
    ArrayRef, BinaryArray, BooleanArray, Date32Array, Float64Array, Int64Array, StringArray,
    TimestampMicrosecondArray,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use convert_to_parquet::to_csv::convert_parquet_to_csv;
use parquet::arrow::arrow_writer::ArrowWriter;
use std::path::Path;
use std::sync::Arc;
use tempfile::{Builder, NamedTempFile};

// ── Helpers ──────────────────────────────────────────────────────────

/// Write a single RecordBatch to a temp .parquet file and return the guard.
fn write_parquet(batch: &RecordBatch) -> NamedTempFile {
    let f = Builder::new().suffix(".parquet").tempfile().unwrap();
    let file = f.reopen().unwrap();
    let mut writer = ArrowWriter::try_new(file, batch.schema(), None).unwrap();
    writer.write(batch).unwrap();
    writer.close().unwrap();
    f
}

/// Build a RecordBatch from named (Field, ArrayRef) columns.
fn batch(columns: Vec<(Field, ArrayRef)>) -> RecordBatch {
    let fields: Vec<Field> = columns.iter().map(|(f, _)| f.clone()).collect();
    let arrays: Vec<ArrayRef> = columns.into_iter().map(|(_, a)| a).collect();
    let schema = Arc::new(Schema::new(fields));
    RecordBatch::try_new(schema, arrays).unwrap()
}

/// Convert a Parquet file to CSV and parse the result into raw records
/// (header included). Quoting is decoded by the csv reader so assertions
/// compare logical field values.
fn to_csv_records(parquet: &Path) -> Vec<Vec<String>> {
    let out = Builder::new().suffix(".csv").tempfile().unwrap();
    convert_parquet_to_csv(parquet, out.path()).unwrap();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(out.path())
        .unwrap();
    reader
        .records()
        .map(|r| r.unwrap().iter().map(str::to_string).collect())
        .collect()
}

/// Read the raw bytes of the produced CSV (for assertions on quoting/layout).
fn to_csv_raw(parquet: &Path) -> String {
    let out = Builder::new().suffix(".csv").tempfile().unwrap();
    convert_parquet_to_csv(parquet, out.path()).unwrap();
    std::fs::read_to_string(out.path()).unwrap()
}

fn nullable(name: &str, ty: DataType) -> Field {
    Field::new(name, ty, true)
}

// ── 1. Header + every scalar type renders correctly ─────────────────

#[test]
fn test_all_scalar_types_render() {
    let batch = batch(vec![
        (
            nullable("i", DataType::Int64),
            Arc::new(Int64Array::from(vec![1, -42, 0])) as ArrayRef,
        ),
        (
            nullable("f", DataType::Float64),
            Arc::new(Float64Array::from(vec![1.5, -0.25, 100.0])) as ArrayRef,
        ),
        (
            nullable("b", DataType::Boolean),
            Arc::new(BooleanArray::from(vec![true, false, true])) as ArrayRef,
        ),
        (
            nullable("s", DataType::Utf8),
            Arc::new(StringArray::from(vec!["alice", "bob", "carol"])) as ArrayRef,
        ),
    ]);
    let parquet = write_parquet(&batch);
    let records = to_csv_records(parquet.path());

    assert_eq!(records[0], vec!["i", "f", "b", "s"], "header");
    assert_eq!(records.len(), 4, "header + 3 rows");
    assert_eq!(records[1][0], "1");
    assert_eq!(records[2][0], "-42");
    assert_eq!(records[3][0], "0");
    assert_eq!(records[1][1], "1.5");
    assert_eq!(records[2][1], "-0.25");
    assert_eq!(records[1][2], "true");
    assert_eq!(records[2][2], "false");
    assert_eq!(records[1][3], "alice");
}

// ── 2. Nulls become empty fields for every type ─────────────────────

#[test]
fn test_nulls_render_as_empty_fields() {
    let batch = batch(vec![
        (
            nullable("i", DataType::Int64),
            Arc::new(Int64Array::from(vec![Some(1), None, Some(3)])) as ArrayRef,
        ),
        (
            nullable("f", DataType::Float64),
            Arc::new(Float64Array::from(vec![None, Some(2.0), None])) as ArrayRef,
        ),
        (
            nullable("s", DataType::Utf8),
            Arc::new(StringArray::from(vec![Some("x"), None, Some("z")])) as ArrayRef,
        ),
    ]);
    let parquet = write_parquet(&batch);
    let records = to_csv_records(parquet.path());

    assert_eq!(records[1], vec!["1", "", "x"]);
    assert_eq!(records[2], vec!["", "2.0", ""]);
    assert_eq!(records[3], vec!["3", "", "z"]);
}

// ── 3. Values needing RFC-4180 quoting survive round-trip ───────────

#[test]
fn test_special_characters_are_quoted() {
    let batch = batch(vec![(
        nullable("v", DataType::Utf8),
        Arc::new(StringArray::from(vec![
            "a,b",            // embedded comma
            "she said \"hi\"", // embedded double quote
            "line1\nline2",   // embedded newline
            "trailing space ",
        ])) as ArrayRef,
    )]);
    let parquet = write_parquet(&batch);

    // Logical values survive decoding.
    let records = to_csv_records(parquet.path());
    assert_eq!(records[1][0], "a,b");
    assert_eq!(records[2][0], "she said \"hi\"");
    assert_eq!(records[3][0], "line1\nline2");
    assert_eq!(records[4][0], "trailing space ");

    // Physical layout must quote the comma/quote/newline fields.
    let raw = to_csv_raw(parquet.path());
    assert!(raw.contains("\"a,b\""), "comma field quoted: {raw:?}");
    assert!(
        raw.contains("\"she said \"\"hi\"\"\""),
        "quote doubled inside quotes: {raw:?}"
    );
}

// ── 4. Unicode (multibyte, combining, emoji) is preserved ───────────

#[test]
fn test_unicode_preserved() {
    let batch = batch(vec![(
        nullable("v", DataType::Utf8),
        Arc::new(StringArray::from(vec!["café", "naïve", "日本語", "🦀"])) as ArrayRef,
    )]);
    let parquet = write_parquet(&batch);
    let records = to_csv_records(parquet.path());

    assert_eq!(records[1][0], "café");
    assert_eq!(records[2][0], "naïve");
    assert_eq!(records[3][0], "日本語");
    assert_eq!(records[4][0], "🦀");
}

// ── 5. Temporal types use Arrow display format ──────────────────────

#[test]
fn test_date32_and_timestamp_render() {
    // 2019-06-05 is 18052 days after the Unix epoch.
    // 1_560_000_000_000_000 µs = 2019-06-08T12:40:00.
    let batch = batch(vec![
        (
            nullable("d", DataType::Date32),
            Arc::new(Date32Array::from(vec![Some(18052), None])) as ArrayRef,
        ),
        (
            nullable("t", DataType::Timestamp(TimeUnit::Microsecond, None)),
            Arc::new(TimestampMicrosecondArray::from(vec![
                Some(1_560_000_000_000_000),
                Some(0),
            ])) as ArrayRef,
        ),
    ]);
    let parquet = write_parquet(&batch);
    let records = to_csv_records(parquet.path());

    assert_eq!(records[1][0], "2019-06-05", "Date32 ISO format");
    assert_eq!(records[2][0], "", "null date → empty");
    assert!(
        records[1][1].starts_with("2019-06-08"),
        "timestamp rendered, got {:?}",
        records[1][1]
    );
    assert!(
        records[2][1].starts_with("1970-01-01"),
        "epoch timestamp, got {:?}",
        records[2][1]
    );
}

// ── 6. Binary columns are hex-encoded ───────────────────────────────

#[test]
fn test_binary_hex_encoded() {
    let values: Vec<&[u8]> = vec![&[0x00, 0x01, 0xff], &[0xde, 0xad, 0xbe, 0xef]];
    let batch = batch(vec![(
        nullable("blob", DataType::Binary),
        Arc::new(BinaryArray::from(values)) as ArrayRef,
    )]);
    let parquet = write_parquet(&batch);
    let records = to_csv_records(parquet.path());

    assert_eq!(records[0], vec!["blob"]);
    assert_eq!(records[1][0], "0001ff");
    assert_eq!(records[2][0], "deadbeef");
}

// ── 7. Reader crosses batch boundaries (> default batch size) ───────

#[test]
fn test_multi_batch_row_count_and_order() {
    // Default parquet reader batch size is 1024; 3000 rows → 3 batches.
    let n = 3000_i64;
    let ints: Vec<i64> = (0..n).collect();
    let batch = batch(vec![(
        nullable("i", DataType::Int64),
        Arc::new(Int64Array::from(ints)) as ArrayRef,
    )]);
    let parquet = write_parquet(&batch);
    let records = to_csv_records(parquet.path());

    assert_eq!(records.len(), 3001, "header + 3000 rows");
    assert_eq!(records[1][0], "0", "first value");
    assert_eq!(records[1024][0], "1023", "last row of batch 1");
    assert_eq!(records[1025][0], "1024", "first row of batch 2");
    assert_eq!(records[3000][0], "2999", "last value preserved across batches");
}

// ── 8. Zero-row parquet produces an empty CSV ───────────────────────

#[test]
fn test_empty_parquet_produces_empty_csv() {
    // A zero-row parquet yields no record batches from the reader, so the
    // arrow CSV writer (which emits its header lazily on the first write)
    // is never invoked → the output file is empty, not even a header row.
    let batch = batch(vec![
        (
            nullable("a", DataType::Int64),
            Arc::new(Int64Array::from(Vec::<i64>::new())) as ArrayRef,
        ),
        (
            nullable("b", DataType::Utf8),
            Arc::new(StringArray::from(Vec::<&str>::new())) as ArrayRef,
        ),
    ]);
    let parquet = write_parquet(&batch);
    let raw = to_csv_raw(parquet.path());

    assert_eq!(raw, "", "zero-row parquet → empty CSV (no header emitted)");
}

// ── 9. Error paths ──────────────────────────────────────────────────

#[test]
fn test_missing_input_errors() {
    let out = Builder::new().suffix(".csv").tempfile().unwrap();
    let result = convert_parquet_to_csv("does_not_exist.parquet", out.path());
    assert!(result.is_err(), "missing input must error");
}

#[test]
fn test_non_parquet_input_errors() {
    // A text file with a .parquet name must fail at reader construction,
    // not produce a corrupt CSV.
    let mut bogus = Builder::new().suffix(".parquet").tempfile().unwrap();
    use std::io::Write;
    bogus.write_all(b"this is not parquet").unwrap();
    bogus.flush().unwrap();

    let out = Builder::new().suffix(".csv").tempfile().unwrap();
    let result = convert_parquet_to_csv(bogus.path(), out.path());
    assert!(result.is_err(), "non-parquet bytes must error");
}

#[test]
fn test_uncreatable_output_errors() {
    let batch = batch(vec![(
        nullable("i", DataType::Int64),
        Arc::new(Int64Array::from(vec![1])) as ArrayRef,
    )]);
    let parquet = write_parquet(&batch);
    // Output directory does not exist → File::create fails.
    let result = convert_parquet_to_csv(parquet.path(), "/nonexistent_dir_xyz/out.csv");
    assert!(result.is_err(), "uncreatable output must error");
}
