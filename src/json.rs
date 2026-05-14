// ============================================================
// src/json.rs
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub const JSON_DELIMITER: u8 = b',';
const SUPPORTED_EXT: &[&str] = &["json", "jsonl", "ndjson"];

#[derive(Clone, Copy)]
enum JsonKind {
    Lines,
    Document,
}

pub struct JsonCsvExport {
    pub csv_path: PathBuf,
    pub row_count: usize,
    _keep: NamedTempFile,
}

pub fn is_json<P: AsRef<Path>>(path: P) -> bool {
    extension_lower(path.as_ref())
        .map(|e| SUPPORTED_EXT.contains(&e.as_str()))
        .unwrap_or(false)
}

/// Pick a filename suffix for buffered stdin so extension-based routing
/// (`is_json`, `kind_for`) applies. JSON is detected from the leading
/// non-whitespace byte; a fully parseable value is a document, otherwise it
/// is treated as line-delimited (JSONL).
pub fn stdin_suffix(bytes: &[u8]) -> &'static str {
    match bytes.iter().copied().find(|b| !b.is_ascii_whitespace()) {
        Some(b'[') => ".json",
        Some(b'{') if serde_json::from_slice::<Value>(bytes).is_ok() => ".json",
        Some(b'{') => ".jsonl",
        _ => ".csv",
    }
}

pub fn export_json_to_csv<P: AsRef<Path>>(path: P) -> Result<JsonCsvExport> {
    let path_ref = path.as_ref();
    match kind_for(path_ref) {
        JsonKind::Lines => export_jsonl_streaming(path_ref),
        JsonKind::Document => {
            let records = read_json_document(path_ref)?;
            write_records_to_csv(&records)
        }
    }
}

fn export_jsonl_streaming(path: &Path) -> Result<JsonCsvExport> {
    // Pass 1: collect headers without storing records
    let headers = {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let reader = BufReader::with_capacity(1 << 20, file);
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut headers: Vec<String> = Vec::new();
        for (i, line) in reader.lines().enumerate() {
            let line = line.with_context(|| format!("read line {}", i + 1))?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(trimmed)
                .with_context(|| format!("parse JSONL line {}", i + 1))?;
            match value {
                Value::Object(map) => {
                    for key in map.keys() {
                        if !seen.contains(key) {
                            seen.insert(key.clone());
                            headers.push(key.clone());
                        }
                    }
                }
                other => {
                    anyhow::bail!("line {}: expected object, got {}", i + 1, type_name(&other))
                }
            }
        }
        headers
    };

    // Pass 2: write CSV row-by-row
    let temp = NamedTempFile::new()?;
    let writer_file = temp.reopen()?;
    let mut csv_writer = csv::WriterBuilder::new()
        .delimiter(JSON_DELIMITER)
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(BufWriter::with_capacity(1 << 20, writer_file));

    if headers.is_empty() {
        csv_writer.flush()?;
        return Ok(JsonCsvExport {
            csv_path: temp.path().to_path_buf(),
            row_count: 0,
            _keep: temp,
        });
    }

    csv_writer.write_record(&headers)?;

    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::with_capacity(1 << 20, file);
    let mut row_buf: Vec<String> = vec![String::new(); headers.len()];
    let mut total_rows: usize = 0;

    for (i, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("read line {}", i + 1))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(trimmed).with_context(|| format!("parse JSONL line {}", i + 1))?;
        let map = match value {
            Value::Object(m) => m,
            other => anyhow::bail!("line {}: expected object, got {}", i + 1, type_name(&other)),
        };
        for (idx, key) in headers.iter().enumerate() {
            row_buf[idx] = map.get(key).map(cell_value).unwrap_or_default();
        }
        csv_writer.write_record(row_buf.iter().map(|s| s.as_str()))?;
        total_rows += 1;
    }

    csv_writer.flush()?;
    Ok(JsonCsvExport {
        csv_path: temp.path().to_path_buf(),
        row_count: total_rows,
        _keep: temp,
    })
}

fn extension_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}

fn kind_for(path: &Path) -> JsonKind {
    match extension_lower(path).as_deref() {
        Some("jsonl") | Some("ndjson") => JsonKind::Lines,
        _ => JsonKind::Document,
    }
}

/// Load every record as an object map, preserving nested values.
/// Unifies both kinds (document / line-delimited) for the native Arrow path.
pub(crate) fn read_all_objects(path: &Path) -> Result<Vec<serde_json::Map<String, Value>>> {
    match kind_for(path) {
        JsonKind::Document => read_json_document(path),
        JsonKind::Lines => read_jsonl_objects(path),
    }
}

fn read_jsonl_objects(path: &Path) -> Result<Vec<serde_json::Map<String, Value>>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::with_capacity(1 << 20, file);
    let mut records: Vec<serde_json::Map<String, Value>> = Vec::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("read line {}", i + 1))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(trimmed).with_context(|| format!("parse JSONL line {}", i + 1))?;
        match value {
            Value::Object(map) => records.push(map),
            other => anyhow::bail!("line {}: expected object, got {}", i + 1, type_name(&other)),
        }
    }
    Ok(records)
}

fn read_json_document(path: &Path) -> Result<Vec<serde_json::Map<String, Value>>> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)
        .with_context(|| format!("read {}", path.display()))?;
    let value: Value = serde_json::from_str(&buffer).context("parse JSON document")?;
    let mut records: Vec<serde_json::Map<String, Value>> = Vec::new();
    match value {
        Value::Array(items) => {
            for (i, item) in items.into_iter().enumerate() {
                push_object(&mut records, item, i + 1)?;
            }
        }
        Value::Object(_) => push_object(&mut records, value, 1)?,
        other => {
            return Err(anyhow!(
                "unsupported JSON root: expected object or array, got {}",
                type_name(&other)
            ));
        }
    }
    Ok(records)
}

fn push_object(
    records: &mut Vec<serde_json::Map<String, Value>>,
    value: Value,
    position: usize,
) -> Result<()> {
    match value {
        Value::Object(map) => {
            records.push(map);
            Ok(())
        }
        other => Err(anyhow!(
            "record {position}: expected object, got {}",
            type_name(&other)
        )),
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn collect_headers(records: &[serde_json::Map<String, Value>]) -> Vec<String> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut headers: Vec<String> = Vec::new();
    for record in records {
        for key in record.keys() {
            if seen.insert(key.clone()) {
                headers.push(key.clone());
            }
        }
    }
    headers
}

fn cell_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn write_records_to_csv(records: &[serde_json::Map<String, Value>]) -> Result<JsonCsvExport> {
    let temp = NamedTempFile::new()?;
    let writer_file = temp.reopen()?;
    let mut csv_writer = csv::WriterBuilder::new()
        .delimiter(JSON_DELIMITER)
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(BufWriter::with_capacity(1 << 20, writer_file));
    let headers = collect_headers(records);
    if headers.is_empty() {
        csv_writer.flush()?;
        drop(csv_writer);
        return Ok(JsonCsvExport {
            csv_path: temp.path().to_path_buf(),
            row_count: 0,
            _keep: temp,
        });
    }
    csv_writer.write_record(&headers)?;
    let mut row_buf: Vec<String> = vec![String::new(); headers.len()];
    let mut total_rows: usize = 0;
    for record in records {
        for (i, key) in headers.iter().enumerate() {
            row_buf[i] = match record.get(key) {
                Some(value) => cell_value(value),
                None => String::new(),
            };
        }
        csv_writer.write_record(row_buf.iter().map(|s| s.as_str()))?;
        total_rows += 1;
    }
    csv_writer.flush()?;
    drop(csv_writer);
    Ok(JsonCsvExport {
        csv_path: temp.path().to_path_buf(),
        row_count: total_rows,
        _keep: temp,
    })
}

// ============================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_temp(content: &str, ext: &str) -> NamedTempFile {
        let mut temp = tempfile::Builder::new()
            .suffix(&format!(".{ext}"))
            .tempfile()
            .unwrap();
        temp.write_all(content.as_bytes()).unwrap();
        temp.flush().unwrap();
        temp
    }

    fn read_csv(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn is_json_detects_extensions() {
        assert!(is_json(Path::new("a.json")));
        assert!(is_json(Path::new("a.jsonl")));
        assert!(is_json(Path::new("a.ndjson")));
        assert!(!is_json(Path::new("a.csv")));
    }

    #[test]
    fn jsonl_basic() {
        let temp = write_temp("{\"a\":1,\"b\":\"x\"}\n{\"a\":2,\"b\":\"y\"}\n", "jsonl");
        let export = export_json_to_csv(temp.path()).unwrap();
        assert_eq!(export.row_count, 2);
        let csv = read_csv(&export.csv_path);
        assert_eq!(csv, "a,b\n1,x\n2,y\n");
    }

    #[test]
    fn jsonl_heterogeneous_keys() {
        let temp = write_temp("{\"a\":1}\n{\"b\":2}\n{\"a\":3,\"b\":4}\n", "jsonl");
        let export = export_json_to_csv(temp.path()).unwrap();
        assert_eq!(export.row_count, 3);
        let csv = read_csv(&export.csv_path);
        assert_eq!(csv, "a,b\n1,\n,2\n3,4\n");
    }

    #[test]
    fn json_array_of_objects() {
        let temp = write_temp("[{\"a\":1},{\"a\":2}]", "json");
        let export = export_json_to_csv(temp.path()).unwrap();
        assert_eq!(export.row_count, 2);
        let csv = read_csv(&export.csv_path);
        assert_eq!(csv, "a\n1\n2\n");
    }

    #[test]
    fn json_single_object() {
        let temp = write_temp("{\"a\":1,\"b\":\"x\"}", "json");
        let export = export_json_to_csv(temp.path()).unwrap();
        assert_eq!(export.row_count, 1);
        let csv = read_csv(&export.csv_path);
        assert_eq!(csv, "a,b\n1,x\n");
    }

    #[test]
    fn json_nested_serialized() {
        let temp = write_temp("[{\"a\":{\"k\":1},\"b\":[1,2]}]", "json");
        let export = export_json_to_csv(temp.path()).unwrap();
        let csv = read_csv(&export.csv_path);
        assert_eq!(csv, "a,b\n\"{\"\"k\"\":1}\",\"[1,2]\"\n");
    }

    #[test]
    fn json_null_becomes_empty() {
        let temp = write_temp("[{\"a\":null,\"b\":1}]", "json");
        let export = export_json_to_csv(temp.path()).unwrap();
        let csv = read_csv(&export.csv_path);
        assert_eq!(csv, "a,b\n,1\n");
    }

    #[test]
    fn json_bool_formatting() {
        let temp = write_temp("[{\"a\":true,\"b\":false}]", "json");
        let export = export_json_to_csv(temp.path()).unwrap();
        let csv = read_csv(&export.csv_path);
        assert_eq!(csv, "a,b\ntrue,false\n");
    }

    #[test]
    fn jsonl_skips_blank_lines() {
        let temp = write_temp("{\"a\":1}\n\n{\"a\":2}\n", "jsonl");
        let export = export_json_to_csv(temp.path()).unwrap();
        assert_eq!(export.row_count, 2);
    }

    #[test]
    fn json_invalid_root_rejected() {
        let temp = write_temp("[1,2,3]", "json");
        let result = export_json_to_csv(temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn json_scalar_root_rejected() {
        let temp = write_temp("42", "json");
        let result = export_json_to_csv(temp.path());
        assert!(result.is_err());
    }

    // The CSV path is scalar-only: nested values are flattened to JSON strings.
    // Nested structure is instead preserved by the native Arrow path
    // (see `json_arrow`), to which `main` routes any file containing objects
    // or arrays. This test pins the CSV path's documented flatten behavior.
    #[test]
    fn json_csv_path_flattens_nested() {
        let content = r#"[{"id":1,"nested":{"a":1,"b":"x"}}]"#;
        let temp = write_temp(content, "json");
        let export = export_json_to_csv(temp.path()).unwrap();
        let csv = read_csv(&export.csv_path);
        assert_eq!(csv, "id,nested\n1,\"{\"\"a\"\":1,\"\"b\"\":\"\"x\"\"}\"\n");
    }
}
