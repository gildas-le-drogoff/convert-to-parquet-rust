// ============================================================
// tests/inverse_and_inspect_tests.rs
//
// End-to-end coverage for the CLI modes that were previously untested:
//   - Parquet → CSV   (--to-csv,   to_csv::convert_parquet_to_csv)
//   - Parquet → JSONL (--to-jsonl, to_jsonl::convert_parquet_to_jsonl)
//   - Parquet inspection (default action on .parquet, inspect::*)
//   - JSON → Parquet  (default action on .json, json + conversion pipeline)
//
// Each test mirrors the real main.rs flow so the assertions exercise the
// same code paths the binary runs.
// ============================================================

use csv_to_parquet::conversion::convert_csv_to_parquet;
use csv_to_parquet::inspect::{display_parquet_stats, is_parquet};
use csv_to_parquet::json::{export_json_to_csv, is_json, JSON_DELIMITER};
use csv_to_parquet::to_csv::convert_parquet_to_csv;
use csv_to_parquet::to_jsonl::convert_parquet_to_jsonl;
use serde_json::Value;
use std::io::Write;
use std::path::Path;
use tempfile::{Builder, NamedTempFile};

// ── Helpers ──────────────────────────────────────────────────────────

/// Write `content` to a temp file with the given extension and return the guard.
fn temp_with(content: &str, ext: &str) -> NamedTempFile {
    let mut f = Builder::new().suffix(&format!(".{ext}")).tempfile().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    f
}

/// Convert CSV text to a Parquet file and return the (guard, path).
fn csv_to_parquet(csv: &str) -> (NamedTempFile, NamedTempFile) {
    let input = temp_with(csv, "csv");
    let output = Builder::new().suffix(".parquet").tempfile().unwrap();
    convert_csv_to_parquet(input.path(), output.path(), true, false, false, None).unwrap();
    (input, output)
}

/// Read a CSV file into records (header included), so assertions are robust
/// to quoting differences.
fn read_csv_records(path: &Path) -> Vec<Vec<String>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)
        .unwrap();
    reader
        .records()
        .map(|r| r.unwrap().iter().map(str::to_string).collect())
        .collect()
}

/// Parse a JSONL file into one serde_json::Value per non-empty line.
fn read_jsonl(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

// ── Parquet → CSV ────────────────────────────────────────────────────

#[test]
fn test_parquet_to_csv_roundtrip() {
    // CSV → Parquet → CSV must preserve header and typed values.
    let (_in, parquet) = csv_to_parquet("id,name\n1,alice\n2,bob\n3,carol\n");
    let csv_out = Builder::new().suffix(".csv").tempfile().unwrap();
    convert_parquet_to_csv(parquet.path(), csv_out.path()).unwrap();

    let records = read_csv_records(csv_out.path());
    assert_eq!(records[0], vec!["id", "name"]);
    assert_eq!(records[1], vec!["1", "alice"]);
    assert_eq!(records[2], vec!["2", "bob"]);
    assert_eq!(records[3], vec!["3", "carol"]);
    assert_eq!(records.len(), 4, "header + 3 data rows");
}

#[test]
fn test_parquet_to_csv_preserves_nulls_as_empty() {
    // Middle value is an explicit null token → empty CSV field on the way back.
    let (_in, parquet) = csv_to_parquet("id,note\n1,hello\n2,NULL\n3,world\n");
    let csv_out = Builder::new().suffix(".csv").tempfile().unwrap();
    convert_parquet_to_csv(parquet.path(), csv_out.path()).unwrap();

    let records = read_csv_records(csv_out.path());
    assert_eq!(records[2], vec!["2", ""], "NULL token round-trips as empty");
}

#[test]
fn test_parquet_to_csv_missing_input_errors() {
    let out = Builder::new().suffix(".csv").tempfile().unwrap();
    let result = convert_parquet_to_csv("does_not_exist.parquet", out.path());
    assert!(result.is_err());
}

// ── Parquet → JSONL ──────────────────────────────────────────────────

#[test]
fn test_parquet_to_jsonl_values() {
    let (_in, parquet) = csv_to_parquet("id,name\n1,alice\n2,bob\n");
    let jsonl_out = Builder::new().suffix(".jsonl").tempfile().unwrap();
    convert_parquet_to_jsonl(parquet.path(), jsonl_out.path()).unwrap();

    let rows = read_jsonl(jsonl_out.path());
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], Value::from(1));
    assert_eq!(rows[0]["name"], Value::from("alice"));
    assert_eq!(rows[1]["id"], Value::from(2));
    assert_eq!(rows[1]["name"], Value::from("bob"));
}

#[test]
fn test_parquet_to_jsonl_omits_null_fields() {
    // arrow-json omits null fields entirely rather than writing `null`.
    let (_in, parquet) = csv_to_parquet("id,note\n1,hello\n2,NULL\n");
    let jsonl_out = Builder::new().suffix(".jsonl").tempfile().unwrap();
    convert_parquet_to_jsonl(parquet.path(), jsonl_out.path()).unwrap();

    let rows = read_jsonl(jsonl_out.path());
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["note"], Value::from("hello"));
    assert!(
        rows[1].get("note").is_none(),
        "null field should be omitted, got {:?}",
        rows[1]
    );
}

// ── Parquet inspection ───────────────────────────────────────────────

#[test]
fn test_is_parquet_detection() {
    assert!(is_parquet("data.parquet"));
    assert!(is_parquet("DATA.PARQUET"));
    assert!(!is_parquet("data.csv"));
    assert!(!is_parquet("data"));
}

#[test]
fn test_display_parquet_stats_succeeds() {
    let (_in, parquet) = csv_to_parquet("id,name\n1,alice\n2,bob\n3,carol\n");
    // Default action on a .parquet input: must read footer metadata without error.
    assert!(display_parquet_stats(parquet.path()).is_ok());
}

#[test]
fn test_display_parquet_stats_rejects_non_parquet() {
    let csv = temp_with("id,name\n1,alice\n", "csv");
    assert!(display_parquet_stats(csv.path()).is_err());
}

// ── JSON → Parquet (full CLI flow) ───────────────────────────────────

#[test]
fn test_json_array_to_parquet_end_to_end() {
    // Mirrors main.rs convert_json: export JSON → CSV, then CSV → Parquet.
    assert!(is_json(Path::new("x.json")));
    let json = temp_with(r#"[{"id":1,"city":"paris"},{"id":2,"city":"lyon"}]"#, "json");
    let export = export_json_to_csv(json.path()).unwrap();
    assert_eq!(export.row_count, 2);

    let parquet = Builder::new().suffix(".parquet").tempfile().unwrap();
    convert_csv_to_parquet(
        &export.csv_path,
        parquet.path(),
        true,
        false,
        false,
        Some(JSON_DELIMITER),
    )
    .unwrap();

    // Round-trip back to JSONL to assert the values survived the pipeline.
    let jsonl_out = Builder::new().suffix(".jsonl").tempfile().unwrap();
    convert_parquet_to_jsonl(parquet.path(), jsonl_out.path()).unwrap();
    let rows = read_jsonl(jsonl_out.path());
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], Value::from(1));
    assert_eq!(rows[0]["city"], Value::from("paris"));
    assert_eq!(rows[1]["city"], Value::from("lyon"));
}

#[test]
fn test_jsonl_to_parquet_end_to_end() {
    let jsonl = temp_with("{\"a\":1,\"b\":\"x\"}\n{\"a\":2,\"b\":\"y\"}\n", "jsonl");
    let export = export_json_to_csv(jsonl.path()).unwrap();
    assert_eq!(export.row_count, 2);

    let parquet = Builder::new().suffix(".parquet").tempfile().unwrap();
    convert_csv_to_parquet(
        &export.csv_path,
        parquet.path(),
        true,
        false,
        false,
        Some(JSON_DELIMITER),
    )
    .unwrap();

    let csv_out = Builder::new().suffix(".csv").tempfile().unwrap();
    convert_parquet_to_csv(parquet.path(), csv_out.path()).unwrap();
    let records = read_csv_records(csv_out.path());
    assert_eq!(records[0], vec!["a", "b"]);
    assert_eq!(records[1], vec!["1", "x"]);
    assert_eq!(records[2], vec!["2", "y"]);
}
