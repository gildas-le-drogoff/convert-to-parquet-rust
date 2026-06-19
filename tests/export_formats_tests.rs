// ============================================================
// tests/export_formats_tests.rs
//
// Coverage for the export formats exposed by the interactive viewer:
//   - ExportFormat::convert dispatch (CSV/JSONL/JSON/XLSX)
//   - Parquet → JSON   (to_json::convert_parquet_to_json)
//   - Parquet → XLSX   (to_xlsx::convert_parquet_to_xlsx)
// ============================================================

use calamine::{Data, Reader, Xlsx};
use convert_to_parquet::conversion::convert_convert_to_parquet;
use convert_to_parquet::export::ExportFormat;
use serde_json::Value;
use std::io::Write;
use tempfile::{Builder, NamedTempFile};

fn convert_to_parquet(csv: &str) -> (NamedTempFile, NamedTempFile) {
    let mut input = Builder::new().suffix(".csv").tempfile().unwrap();
    input.write_all(csv.as_bytes()).unwrap();
    input.flush().unwrap();
    let output = Builder::new().suffix(".parquet").tempfile().unwrap();
    convert_convert_to_parquet(input.path(), output.path(), true, false, false, None).unwrap();
    (input, output)
}

#[test]
fn test_export_format_metadata_unique() {
    let exts: Vec<_> = ExportFormat::ALL.iter().map(|f| f.extension()).collect();
    let keys: Vec<_> = ExportFormat::ALL.iter().map(|f| f.hotkey()).collect();
    assert_eq!(exts, ["csv", "jsonl", "json", "xlsx"]);
    // Hotkeys must not collide with vim navigation (h/j/k/l) nor each other.
    for key in &keys {
        assert!(!"hjkl".contains(*key));
    }
    let mut deduped = keys.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(deduped.len(), keys.len());
}

#[test]
fn test_convert_json_array_roundtrip() {
    let (_in, parquet) = convert_to_parquet("id,name\n1,alice\n2,bob\n");
    let out = Builder::new().suffix(".json").tempfile().unwrap();
    let rows = ExportFormat::Json.convert(parquet.path(), out.path()).unwrap();
    assert_eq!(rows, 2);
    let value: Value = serde_json::from_str(&std::fs::read_to_string(out.path()).unwrap()).unwrap();
    let array = value.as_array().unwrap();
    assert_eq!(array.len(), 2);
    assert_eq!(array[0]["id"], 1);
    assert_eq!(array[1]["name"], "bob");
}

#[test]
fn test_convert_xlsx_roundtrip() {
    let (_in, parquet) = convert_to_parquet("id,name\n1,alice\n2,bob\n3,carol\n");
    let out = Builder::new().suffix(".xlsx").tempfile().unwrap();
    let rows = ExportFormat::Xlsx.convert(parquet.path(), out.path()).unwrap();
    assert_eq!(rows, 3);

    let mut workbook: Xlsx<_> = calamine::open_workbook(out.path()).unwrap();
    let sheet_name = workbook.sheet_names()[0].clone();
    let range = workbook.worksheet_range(&sheet_name).unwrap();
    assert_eq!(range.height(), 4); // header + 3 rows
    assert_eq!(range.get((0, 0)), Some(&Data::String("id".to_string())));
    assert_eq!(range.get((0, 1)), Some(&Data::String("name".to_string())));
    assert_eq!(range.get((3, 1)), Some(&Data::String("carol".to_string())));
}

#[test]
fn test_convert_missing_input_errors() {
    let out = Builder::new().suffix(".json").tempfile().unwrap();
    assert!(ExportFormat::Json
        .convert(std::path::Path::new("does_not_exist.parquet"), out.path())
        .is_err());
}
