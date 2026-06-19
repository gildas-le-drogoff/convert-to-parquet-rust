// ============================================================
// tests/normalized_tests.rs
//
// Integration tests that run convert_to_parquet on every dataset in
// normalized_tests/ and verify:
//   - Conversion succeeds (or fails as expected)
//   - Row count is correct
//   - Schema types match expectations
//   - Content integrity for key datasets
// ============================================================

use anyhow::Result;
use arrow::array::{
    Array, BooleanArray, Date32Array, Float64Array, Int64Array, LargeStringArray,
    RecordBatchReader, StringArray, TimestampMicrosecondArray, TimestampMillisecondArray,
    TimestampNanosecondArray, TimestampSecondArray, UInt64Array,
};
use arrow::datatypes::DataType;
use convert_to_parquet::conversion::convert_convert_to_parquet;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::file::reader::{FileReader, SerializedFileReader};
use std::fs::File;
use std::path::Path;
use tempfile::NamedTempFile;

// ── Helpers ──────────────────────────────────────────────────────────

fn parquet_row_count(path: &Path) -> usize {
    let file = File::open(path).unwrap();
    let reader = SerializedFileReader::new(file).unwrap();
    reader
        .metadata()
        .row_groups()
        .iter()
        .map(|rg| rg.num_rows() as usize)
        .sum()
}

/// Convert a normalized test dataset to Parquet and return (row_count, path).
fn convert(name: &str, full_inference: bool, force_utf8: bool, delimiter: Option<u8>) -> Result<(usize, String)> {
    let input = format!("normalized_tests/{name}");
    // Use a tempfile-based path that is unique per test invocation.
    let tmp = NamedTempFile::new()?;
    let output = tmp.path().to_string_lossy().to_string();
    convert_convert_to_parquet(&input, &output, full_inference, force_utf8, false, delimiter)?;
    let count = parquet_row_count(Path::new(&output));
    tmp.into_temp_path().keep()?; // Don't auto-delete; the caller removes the file explicitly.
    Ok((count, output))
}

fn convert_default(name: &str) -> Result<(usize, String)> {
    convert(name, true, false, None)
}

fn read_parquet_column(path: &Path, col_index: usize) -> Vec<String> {
    let file = File::open(path).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let mut values = Vec::new();
    for batch_result in reader {
        let batch = batch_result.unwrap();
        let col = batch.column(col_index);
        if let Some(arr) = col.as_any().downcast_ref::<LargeStringArray>() {
            for idx in 0..col.len() {
                values.push(if arr.is_null(idx) {
                    String::new()
                } else {
                    arr.value(idx).to_string()
                });
            }
        } else if let Some(arr) = col.as_any().downcast_ref::<StringArray>() {
            for idx in 0..col.len() {
                values.push(if arr.is_null(idx) {
                    String::new()
                } else {
                    arr.value(idx).to_string()
                });
            }
        } else {
            for _idx in 0..col.len() {
                values.push(format!("<{:?}>", col.data_type()));
            }
        }
    }
    values
}

/// Read any column (typed or text) and render each value as a String, with
/// nulls rendered as the empty string. Lets tests assert end-to-end that
/// typed columns hold the right values, not just the right type and row count.
fn read_parquet_column_typed(path: &Path, col_index: usize) -> Vec<String> {
    let file = File::open(path).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let mut values = Vec::new();
    for batch_result in reader {
        let batch = batch_result.unwrap();
        let col = batch.column(col_index);
        let render = |idx: usize| -> String {
            if col.is_null(idx) {
                return String::new();
            }
            macro_rules! fmt {
                ($ty:ty) => {
                    col.as_any()
                        .downcast_ref::<$ty>()
                        .map(|a| a.value(idx).to_string())
                };
            }
            fmt!(Int64Array)
                .or_else(|| fmt!(UInt64Array))
                .or_else(|| fmt!(Float64Array))
                .or_else(|| fmt!(BooleanArray))
                .or_else(|| fmt!(Date32Array))
                .or_else(|| fmt!(TimestampSecondArray))
                .or_else(|| fmt!(TimestampMillisecondArray))
                .or_else(|| fmt!(TimestampMicrosecondArray))
                .or_else(|| fmt!(TimestampNanosecondArray))
                .or_else(|| fmt!(LargeStringArray))
                .or_else(|| fmt!(StringArray))
                .unwrap_or_else(|| format!("<{:?}>", col.data_type()))
        };
        for idx in 0..col.len() {
            values.push(render(idx));
        }
    }
    values
}

fn parquet_column_types(path: &Path) -> Vec<(String, DataType)> {
    let file = File::open(path).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let schema = reader.schema();
    schema
        .fields()
        .iter()
        .map(|f| (f.name().clone(), f.data_type().clone()))
        .collect()
}

// ── 1. Basic conversion & row count ─────────────────────────────────

#[test]
fn test_basic_csv() {
    // test.csv: no header (first line "0" is integer data), 5000 rows,
    // rows "0,0, test" .. "4999,4999, test". Columns 0 and 1 are Int64 and
    // must hold the exact row indices end-to-end.
    let (count, path) = convert_default("test.csv").unwrap();
    assert_eq!(count, 5000);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types[0].1, DataType::Int64);
    assert_eq!(types[1].1, DataType::Int64);
    let col0 = read_parquet_column_typed(Path::new(&path), 0);
    let col1 = read_parquet_column_typed(Path::new(&path), 1);
    assert_eq!(col0.len(), 5000);
    assert_eq!(col0[0], "0");
    assert_eq!(col0[1], "1");
    assert_eq!(col0[4999], "4999");
    assert_eq!(col1[4999], "4999");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_basic_default_csv() {
    // test_default.csv: "0" is a numeric value → treated as data → 5000 data rows
    let (count, path) = convert_default("test_default.csv").unwrap();
    assert_eq!(count, 5000);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_pipe_delimited() {
    // test_pipe.csv: pipe-delimited, no header, 10 rows
    let (count, path) = convert_default("test_pipe.csv").unwrap();
    assert_eq!(count, 10);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types.len(), 3, "should have 3 columns");
    let _ = std::fs::remove_file(&path);
}

// ── 2. Date / timestamp datasets ────────────────────────────────────

#[test]
fn test_date_single_value() {
    // date.csv: 1 row, value "2019-06-05" → Date32
    let (count, path) = convert_default("date.csv").unwrap();
    assert_eq!(count, 1);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types[0].1, DataType::Date32);
    // 2019-06-05 is 18052 days after the Unix epoch.
    let col0 = read_parquet_column_typed(Path::new(&path), 0);
    assert_eq!(col0, vec!["18052"]);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_dateformat_dd_mm_yyyy() {
    // "05/06/2019" → Date32 (dd/mm/yyyy)
    let (count, path) = convert_default("dateformat.csv").unwrap();
    assert_eq!(count, 1);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types[0].1, DataType::Date32);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_dateformat_2() {
    let (count, path) = convert_default("dateformat_2.csv").unwrap();
    assert_eq!(count, 1);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types[0].1, DataType::Date32);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_timestamp_format() {
    // "Mon 30, June 2003, 12:03:10 PM" — date/time format, may be LargeUtf8
    let (count, path) = convert_default("timestampformat.csv").unwrap();
    assert_eq!(count, 1);
    let types = parquet_column_types(Path::new(&path));
    assert!(
        matches!(types[0].1, DataType::Timestamp(_, _) | DataType::LargeUtf8 | DataType::Date32),
        "timestampformat column type: {:?}",
        types[0].1
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_timestamp_with_offset() {
    // Has header "col1" (not detected as header because timestamps don't score),
    // then 4 raw lines: header + 3 timestamps with timezone offset.
    // → 4 rows (no header detected, all 4 lines treated as data)
    let (count, path) = convert_default("timestampoffset.csv").unwrap();
    assert_eq!(count, 4);
    let types = parquet_column_types(Path::new(&path));
    assert!(
        matches!(types[0].1, DataType::Timestamp(_, _) | DataType::LargeUtf8),
        "timestampoffset column type: {:?}",
        types[0].1
    );
    let _ = std::fs::remove_file(&path);
}

// ── 3. Null / empty value handling ──────────────────────────────────

#[test]
fn test_null_csv() {
    // "0||test" — no header, 1 row, middle column empty
    let (count, path) = convert_default("test_null_csv.csv").unwrap();
    assert_eq!(count, 1);
    let vals = read_parquet_column(Path::new(&path), 1);
    assert_eq!(vals.len(), 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_null_option() {
    let (count, path) = convert_default("test_null_option.csv").unwrap();
    assert_eq!(count, 3);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_force_not_null() {
    let (count, path) = convert_default("force_not_null.csv").unwrap();
    assert_eq!(count, 3);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_force_not_null_inull() {
    let (count, path) = convert_default("force_not_null_inull.csv").unwrap();
    assert_eq!(count, 3);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_force_not_null_reordered() {
    let (count, path) = convert_default("force_not_null_reordered.csv").unwrap();
    assert_eq!(count, 3);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_force_quote() {
    let (count, path) = convert_default("force_quote.csv").unwrap();
    assert_eq!(count, 3);
    let _ = std::fs::remove_file(&path);
}

// ── 4. Error / edge-case datasets ───────────────────────────────────

#[test]
fn test_error_invalid_type() {
    // Header "i,j" + 6 data rows (row 5 has "a" in numeric column)
    let (count, path) = convert_default("error_invalid_type.csv").unwrap();
    assert_eq!(count, 6);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_error_too_little() {
    // Header "i,j" + 6 data rows (some with fewer columns)
    let (count, path) = convert_default("error_too_little.csv").unwrap();
    assert_eq!(count, 6);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_error_too_little_single() {
    // Header "i,j" + 1 data row with single value "7"
    let (count, path) = convert_default("error_too_little_single.csv").unwrap();
    assert_eq!(count, 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_error_too_many() {
    // Header "i,j" + 6 data rows (one row has 3 columns)
    let (count, path) = convert_default("error_too_many.csv").unwrap();
    assert_eq!(count, 6);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_error_too_little_end_of_filled_chunk() {
    // Large file, header "i,j" + 1025 data rows
    let (count, path) = convert_default("error_too_little_end_of_filled_chunk.csv").unwrap();
    assert_eq!(count, 1025);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_too_many_values() {
    // Single row "1,2,3,4" — no header, 4 values → 1 row
    let (count, path) = convert_default("too_many_values.csv").unwrap();
    assert_eq!(count, 1);
    let _ = std::fs::remove_file(&path);
}

// ── 5. Newline / line-ending variants ───────────────────────────────

#[test]
fn test_quoted_newline() {
    // CSV with quoted fields containing newlines → 2 data rows
    let (count, path) = convert_default("quoted_newline.csv").unwrap();
    assert_eq!(count, 2);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_mixed_line_endings() {
    // Mixed \n and \r\n line endings → 3 data rows
    let (count, path) = convert_default("mixed_line_endings.csv").unwrap();
    assert_eq!(count, 3);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_windows_newline() {
    // Large file with \r\n line endings, no header → 20000 rows
    let (count, path) = convert_default("windows_newline.csv").unwrap();
    assert_eq!(count, 20000);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_windows_newline_empty() {
    // \r\n file: first line "1\r\n", rest empty → csv crate skips empty lines → 1 row
    let (count, path) = convert_default("windows_newline_empty.csv").unwrap();
    assert_eq!(count, 1);
    let _ = std::fs::remove_file(&path);
}

// ── 6. Pipe-delimited files (with and without override) ─────────────

#[test]
fn test_new_line_string_with_pipe_override() {
    // Pipe-delimited, multiline quoted field.
    // First record "1|6370|371|p1" detected as header → 2 data rows
    let (count, path) = convert("new_line_string.csv", true, false, Some(b'|')).unwrap();
    assert_eq!(count, 2);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types.len(), 4, "pipe-delimited → 4 columns");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_new_line_string_rn_with_pipe() {
    let (count, path) = convert("new_line_string_rn.csv", true, false, Some(b'|')).unwrap();
    assert_eq!(count, 2);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_new_line_string_rn_exc_with_pipe() {
    let (count, path) = convert("new_line_string_rn_exc.csv", true, false, Some(b'|')).unwrap();
    assert_eq!(count, 2);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_multi_column_integer_with_pipe() {
    // 8 raw lines, pipe-delimited, data rows = 8 (trailing empty skipped by csv crate)
    let (count, path) = convert("multi_column_integer.csv", true, false, Some(b'|')).unwrap();
    assert_eq!(count, 8);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types.len(), 3, "pipe-delimited → 3 columns");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_multi_column_integer_rn_with_pipe() {
    let (count, path) = convert("multi_column_integer_rn.csv", true, false, Some(b'|')).unwrap();
    assert_eq!(count, 8);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_multi_column_string_with_pipe() {
    let (count, path) = convert("multi_column_string.csv", true, false, Some(b'|')).unwrap();
    assert_eq!(count, 8);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types.len(), 4, "pipe-delimited → 4 columns");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_multi_column_string_rn_with_pipe() {
    let (count, path) = convert("multi_column_string_rn.csv", true, false, Some(b'|')).unwrap();
    assert_eq!(count, 8);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_long_escaped_value_no_pipe_detected() {
    // long_escaped_value.csv: 0 pipes, 0 commas → default comma → 1 field per row.
    // 1 data row, 1 column.
    let (count, path) = convert_default("long_escaped_value.csv").unwrap();
    assert_eq!(count, 1);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types.len(), 1, "no delimiter found → 1 column");
    let vals = read_parquet_column(Path::new(&path), 0);
    assert_eq!(vals.len(), 1);
    assert!(vals[0].len() > 29000, "long value >29000 chars, got {}", vals[0].len());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_long_escaped_value_unicode_with_commas() {
    // long_escaped_value_unicode.csv: 2 commas → comma-delimited → 3 columns, 1 row
    let (count, path) = convert_default("long_escaped_value_unicode.csv").unwrap();
    assert_eq!(count, 1);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types.len(), 3, "comma-delimited → 3 columns, got {}", types.len());
    let _ = std::fs::remove_file(&path);
}

// ── 7. Large / edge-case files ──────────────────────────────────────

#[test]
fn test_many_empty_lines() {
    // File with "1\n\n\n..." → csv crate skips empty lines → 1 row
    let (count, path) = convert_default("many_empty_lines.csv").unwrap();
    assert_eq!(count, 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_no_newline() {
    // File without trailing newline: 1 header-like line? No, "0,0, test" is data.
    // wc -l says 1023 but actual CSV records = 1024 (last line has no newline)
    let (count, path) = convert_default("no_newline.csv").unwrap();
    assert_eq!(count, 1024);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_no_newline_unicode() {
    let (count, path) = convert_default("no_newline_unicode.csv").unwrap();
    assert_eq!(count, 1024);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_vsize() {
    // 1025 raw split lines → 1024 data rows + trailing empty → 1024 records
    let (count, path) = convert_default("vsize.csv").unwrap();
    assert_eq!(count, 1024);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_issue2518() {
    // 10 rows with quoted commas inside values (e.g. "A,C,T")
    let (count, path) = convert_default("issue2518.csv").unwrap();
    assert_eq!(count, 10);
    // Verify the "A,C,T" value was preserved (column 4, 0-indexed)
    let vals = read_parquet_column(Path::new(&path), 4);
    assert_eq!(vals.len(), 10);
    assert_eq!(vals[0], "A,C,T", "first row col 4");
    // Column 0 is Int64: assert the values round-trip exactly.
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types[0].1, DataType::Int64);
    let col0 = read_parquet_column_typed(Path::new(&path), 0);
    assert_eq!(
        col0,
        vec!["4690", "5", "6", "7", "8", "9", "10", "1090", "11", "1184"]
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_struct_padding() {
    // 15 rows of struct-like strings
    let (count, path) = convert_default("struct_padding.csv").unwrap();
    assert_eq!(count, 15);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_test_long_line() {
    // 2 rows with very long lines (30000+ chars)
    let (count, path) = convert_default("test_long_line.csv").unwrap();
    assert_eq!(count, 2);
    let _ = std::fs::remove_file(&path);
}

// ── 8. Big header (tab-delimited) ───────────────────────────────────

#[test]
fn test_big_header() {
    // Tab-delimited file with 4 header columns and 5 data rows (incl. "----" row)
    let (count, path) = convert_default("big_header.csv").unwrap();
    assert_eq!(count, 5);
    let types = parquet_column_types(Path::new(&path));
    // Header "foo", "bar", "baz", "bam" → 4 columns
    assert_eq!(types.len(), 4);
    let _ = std::fs::remove_file(&path);
}

// ── 9. Compressed files ─────────────────────────────────────────────

#[test]
fn test_compressed_csv_gz_fails() {
    // gzip binary → convert_convert_to_parquet fails (no decompression in this function)
    let input = "normalized_tests/test_comp.csv.gz";
    let output = "/tmp/test_test_comp.csv.gz.parquet";
    let result = convert_convert_to_parquet(input, output, true, false, false, None);
    assert!(result.is_err(), "raw gz should fail (binary)");
    let _ = std::fs::remove_file(output);
}

#[test]
fn test_bgzf_gz_fails() {
    let input = "normalized_tests/bgzf.gz";
    let output = "/tmp/test_bgzf.parquet";
    let result = convert_convert_to_parquet(input, output, true, false, false, None);
    assert!(result.is_err());
    let _ = std::fs::remove_file(output);
}

// ── 10. Empty file ──────────────────────────────────────────────────

#[test]
fn test_empty_csv_fails() {
    let input = "normalized_tests/empty.csv";
    let output = "/tmp/test_empty.parquet";
    let result = convert_convert_to_parquet(input, output, true, false, false, None);
    assert!(result.is_err(), "empty.csv should fail");
    let _ = std::fs::remove_file(output);
}

// ── 11. Unicode normalization ───────────────────────────────────────

#[test]
fn test_nfc() {
    // "ü" (NFC) on line 1, "ü" (NFD, u+combining) on line 2.
    // "ü" detected as header (identifier) → 1 data row
    let (count, path) = convert_default("nfc.csv").unwrap();
    assert_eq!(count, 1);
    let _ = std::fs::remove_file(&path);
}

// ── 12. NDJSON-like content in .csv extension ───────────────────────

#[test]
fn test_5438_ndjson_like() {
    // Two JSON objects as CSV rows → 2 rows, 1 column each
    let (count, path) = convert_default("5438.csv").unwrap();
    assert_eq!(count, 2);
    let vals = read_parquet_column(Path::new(&path), 0);
    assert_eq!(vals.len(), 2);
    assert_eq!(vals[0], r#"{"duck": 1}"#);
    let _ = std::fs::remove_file(&path);
}

// ── 13. Invalid UTF-8 (fails without --force-utf8) ──────────────────

#[test]
fn test_invalid_utf_fails() {
    let input = "normalized_tests/invalid_utf.csv";
    let output = "/tmp/test_invalid_utf.parquet";
    let result = convert_convert_to_parquet(input, output, true, false, false, None);
    assert!(result.is_err());
    let _ = std::fs::remove_file(output);
}

#[test]
fn test_invalid_utf_header_fails() {
    let input = "normalized_tests/invalid_utf_header.csv";
    let output = "/tmp/test_invalid_utf_header.parquet";
    let result = convert_convert_to_parquet(input, output, true, false, false, None);
    assert!(result.is_err());
    let _ = std::fs::remove_file(output);
}

#[test]
fn test_invalid_utf_quoted_fails() {
    let input = "normalized_tests/invalid_utf_quoted.csv";
    let output = "/tmp/test_invalid_utf_quoted.parquet";
    let result = convert_convert_to_parquet(input, output, true, false, false, None);
    assert!(result.is_err());
    let _ = std::fs::remove_file(output);
}

#[test]
fn test_invalid_utf_quoted_nl_fails() {
    let input = "normalized_tests/invalid_utf_quoted_nl.csv";
    let output = "/tmp/test_invalid_utf_quoted_nl.parquet";
    let result = convert_convert_to_parquet(input, output, true, false, false, None);
    assert!(result.is_err());
    let _ = std::fs::remove_file(output);
}

#[test]
fn test_invalid_utf_big() {
    // Large file: first ~54014 bytes are valid UTF-8, then invalid bytes.
    // The csv reader skips invalid records (non-IO errors), so conversion
    // succeeds partially — exactly the 3030 valid rows before the corruption
    // are written.
    let input = "normalized_tests/invalid_utf_big.csv";
    let output = "/tmp/test_invalid_utf_big.parquet";
    let result = convert_convert_to_parquet(input, output, true, false, false, None);
    assert!(result.is_ok(), "partial conversion should succeed: {result:?}");
    assert_eq!(parquet_row_count(Path::new(output)), 3030);
    let _ = std::fs::remove_file(output);
}

// ── 14. Invalid UTF-8 with force_utf8 ───────────────────────────────
// force_utf8 makes schema all-LargeUtf8, but the csv crate still rejects
// invalid UTF-8 bytes at read time → these still fail.

#[test]
fn test_invalid_utf_force_utf8_still_fails() {
    let input = "normalized_tests/invalid_utf.csv";
    let output = "/tmp/test_invalid_utf_force.parquet";
    let result = convert_convert_to_parquet(input, output, true, true, false, None);
    // The csv crate reads StringRecord which requires valid UTF-8
    assert!(result.is_err(), "force_utf8 doesn't affect csv reading");
    let _ = std::fs::remove_file(output);
}

// ── 15. Incompatible type with nullable ─────────────────────────────

#[test]
fn test_incompatible_type_with_nullable() {
    let (count, path) = convert_default("test_incompatible_type_with_nullable.csv").unwrap();
    assert_eq!(count, 2);
    let _ = std::fs::remove_file(&path);
}

// ── 16. Unterminated quoted field ───────────────────────────────────

#[test]
fn test_unterminated_quoted_field() {
    // Unterminated quote: the csv reader consumes everything up to EOF as one
    // quoted field → conversion succeeds with a single row.
    let input = "normalized_tests/unterminated.csv";
    let output = "/tmp/test_unterminated.parquet";
    let result = convert_convert_to_parquet(input, output, true, false, false, None);
    assert!(result.is_ok(), "unterminated quote should still convert: {result:?}");
    assert_eq!(parquet_row_count(Path::new(output)), 1);
    let _ = std::fs::remove_file(output);
}

// ── 17. Blob / escaped byte data ────────────────────────────────────

#[test]
fn test_blob_data() {
    // blob.csv: literal backslash escapes like \x00\x01 — printable text, so
    // it is valid CSV and converts to a single row.
    let input = "normalized_tests/blob.csv";
    let output = "/tmp/test_blob.parquet";
    let result = convert_convert_to_parquet(input, output, true, false, false, None);
    assert!(result.is_ok(), "printable blob should convert: {result:?}");
    assert_eq!(parquet_row_count(Path::new(output)), 1);
    let _ = std::fs::remove_file(output);
}

// ── 18. Invalid UTF-8 list ──────────────────────────────────────────

#[test]
fn test_invalid_utf_list() {
    // "[1, 2]" lines with \xff\xff bytes embedded → invalid UTF-8 → fails
    let input = "normalized_tests/invalid_utf_list.csv";
    let output = "/tmp/test_invalid_utf_list.parquet";
    let result = convert_convert_to_parquet(input, output, true, false, false, None);
    assert!(result.is_err(), "invalid_utf_list.csv has non-UTF-8 bytes → should fail");
    let _ = std::fs::remove_file(output);
}

// ── 19. Partial vs full schema inference ────────────────────────────

#[test]
fn test_full_inference_vs_partial() {
    let fname = "test.csv";
    let input = format!("normalized_tests/{fname}");
    let out_partial = "/tmp/test_test_partial.parquet";
    let out_full = "/tmp/test_test_full.parquet";

    convert_convert_to_parquet(&input, out_partial, false, false, false, None).unwrap();
    convert_convert_to_parquet(&input, out_full, true, false, false, None).unwrap();

    let count_partial = parquet_row_count(Path::new(out_partial));
    let count_full = parquet_row_count(Path::new(out_full));
    assert_eq!(count_partial, count_full);

    let _ = std::fs::remove_file(out_partial);
    let _ = std::fs::remove_file(out_full);
}

// ── 20. Auto-detect delimiter on pipe files (without override) ──────
// When auto-detection picks pipe, the result matches the override case.
// When it fails (e.g. 0 pipes in first lines of long file), comma is used.

#[test]
fn test_pipe_auto_detect_multi_column_integer() {
    // 18 pipes, 0 commas → auto-detection picks pipe → 8 rows, 3 columns.
    let (count, path) = convert_default("multi_column_integer.csv").unwrap();
    assert_eq!(count, 8);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types.len(), 3, "pipe auto-detected → 3 columns");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_pipe_auto_detect_multi_column_string() {
    // 27 pipes, 0 commas → auto-detection picks pipe → 8 rows, 4 columns.
    let (count, path) = convert_default("multi_column_string.csv").unwrap();
    assert_eq!(count, 8);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types.len(), 4, "pipe auto-detected → 4 columns");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_pipe_auto_detect_new_line_string() {
    // 9 pipes, 0 commas → auto-detection picks pipe, same as the explicit
    // pipe-override case → first record taken as header → 2 data rows.
    let (count, path) = convert_default("new_line_string.csv").unwrap();
    assert_eq!(count, 2);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types.len(), 4, "pipe auto-detected → 4 columns");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_pipe_auto_detect_long_escaped_value() {
    // 0 pipes, 0 commas in file → comma default → 1 row, 1 column
    let (count, path) = convert_default("long_escaped_value.csv").unwrap();
    assert_eq!(count, 1);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types.len(), 1, "no delimiter in file → 1 column");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_comma_auto_detect_long_escaped_value_unicode() {
    // 2 commas → comma-detected → 1 row, 3 columns
    let (count, path) = convert_default("long_escaped_value_unicode.csv").unwrap();
    assert_eq!(count, 1);
    let types = parquet_column_types(Path::new(&path));
    assert_eq!(types.len(), 3, "comma-delimited → 3 columns");
    let _ = std::fs::remove_file(&path);
}
