// ============================================================
// src/json_arrow.rs — native JSON/JSONL → Parquet path preserving nesting.
//
// The CSV pipeline is inherently flat: nested objects and arrays collapse to
// JSON strings. This module bypasses CSV and uses arrow-json to infer Arrow
// Struct/List types and decode records directly into RecordBatches, which the
// Parquet writer then stores with their nested logical types. The inverse
// (Parquet → JSON/JSONL) already reconstructs nesting via arrow-json writers.
use crate::json::read_all_objects;
use crate::BLOCK_SIZE;
use anyhow::{Context, Result};
use arrow::datatypes::{DataType, Field, FieldRef, Fields, Schema};
use arrow::json::reader::infer_json_schema_from_seekable;
use arrow::json::ReaderBuilder;
use parquet::arrow::arrow_writer::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;
use serde_json::Value;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use tempfile::NamedTempFile;

const PARQUET_ZSTD_LEVEL: i32 = 5;

/// True when any record holds a nested value (object or array). Such files
/// must take the native Arrow path; purely scalar files stay on the CSV path
/// to keep its richer scalar inference (timestamps, dates, booleans).
pub fn json_has_nested<P: AsRef<Path>>(path: P) -> Result<bool> {
    let records = read_all_objects(path.as_ref())?;
    Ok(records.iter().any(record_has_nested))
}

fn record_has_nested(record: &serde_json::Map<String, Value>) -> bool {
    record
        .values()
        .any(|v| matches!(v, Value::Object(_) | Value::Array(_)))
}

/// Convert a JSON/JSONL file to Parquet, preserving nested structure as Arrow
/// Struct/List columns. Returns the number of rows written.
pub fn convert_json_to_parquet<P: AsRef<Path>, Q: AsRef<Path>>(
    input_path: P,
    output_path: Q,
) -> Result<usize> {
    let records = read_all_objects(input_path.as_ref())?;
    if records.is_empty() {
        return Ok(0);
    }
    let ndjson = write_ndjson_temp(&records)?;
    let schema = infer_nested_schema(ndjson.path())?;
    write_parquet(ndjson.path(), output_path.as_ref(), schema)
}

/// Serialize records as line-delimited JSON to a temp file consumed twice:
/// once for schema inference (needs Seek), once for decoding.
fn write_ndjson_temp(records: &[serde_json::Map<String, Value>]) -> Result<NamedTempFile> {
    let temp = NamedTempFile::new().context("create ndjson temp")?;
    let file = temp.reopen().context("reopen ndjson temp")?;
    let mut writer = BufWriter::with_capacity(1 << 20, file);
    for record in records {
        serde_json::to_writer(&mut writer, record).context("serialize record")?;
        writer.write_all(b"\n").context("write ndjson newline")?;
    }
    writer.flush().context("flush ndjson temp")?;
    Ok(temp)
}

fn infer_nested_schema(ndjson_path: &Path) -> Result<Schema> {
    let file = File::open(ndjson_path).context("open ndjson for inference")?;
    let mut reader = BufReader::new(file);
    let (schema, _) =
        infer_json_schema_from_seekable(&mut reader, None).context("infer JSON schema")?;
    Ok(all_nullable_schema(&schema))
}

fn write_parquet(ndjson_path: &Path, output: &Path, schema: Schema) -> Result<usize> {
    let schema = Arc::new(schema);
    let reader = ReaderBuilder::new(schema.clone())
        .with_batch_size(BLOCK_SIZE)
        .build(BufReader::new(
            File::open(ndjson_path).context("open ndjson for decoding")?,
        ))
        .context("build JSON reader")?;
    let output_file = File::create(output)
        .with_context(|| format!("create parquet {}", output.display()))?;
    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(PARQUET_ZSTD_LEVEL)?))
        .build();
    let mut writer = ArrowWriter::try_new(output_file, schema, Some(properties))
        .context("create parquet writer")?;
    let mut rows = 0usize;
    for batch in reader {
        let batch = batch.context("decode JSON batch")?;
        writer.write(&batch).context("write parquet batch")?;
        rows += batch.num_rows();
    }
    writer.close().context("close parquet writer")?;
    Ok(rows)
}

/// Force nullability across the whole tree. Arrow may infer a nested field as
/// non-null from a sample where the key is always present; a later record
/// missing that key would then fail the writer's null check.
fn all_nullable_schema(schema: &Schema) -> Schema {
    let fields: Vec<Field> = schema.fields().iter().map(|f| all_nullable_field(f)).collect();
    Schema::new(fields)
}

fn all_nullable_field(field: &Field) -> Field {
    Field::new(field.name(), all_nullable_type(field.data_type()), true)
}

fn all_nullable_type(data_type: &DataType) -> DataType {
    match data_type {
        DataType::Struct(fields) => DataType::Struct(map_fields(fields)),
        DataType::List(field) => DataType::List(map_field_ref(field)),
        DataType::LargeList(field) => DataType::LargeList(map_field_ref(field)),
        other => other.clone(),
    }
}

fn map_fields(fields: &Fields) -> Fields {
    fields.iter().map(|f| all_nullable_field(f)).collect()
}

fn map_field_ref(field: &FieldRef) -> FieldRef {
    Arc::new(all_nullable_field(field))
}

// ============================================================
#[cfg(test)]
mod tests {
    use super::*;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    fn write_temp(content: &str, ext: &str) -> NamedTempFile {
        let mut temp = tempfile::Builder::new()
            .suffix(&format!(".{ext}"))
            .tempfile()
            .unwrap();
        temp.write_all(content.as_bytes()).unwrap();
        temp.flush().unwrap();
        temp
    }

    fn parquet_schema(path: &Path) -> Schema {
        let file = File::open(path).unwrap();
        let reader = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
        reader.schema().as_ref().clone()
    }

    #[test]
    fn detects_nested_object() {
        let temp = write_temp(r#"[{"id":1,"meta":{"a":1}}]"#, "json");
        assert!(json_has_nested(temp.path()).unwrap());
    }

    #[test]
    fn detects_nested_array() {
        let temp = write_temp(r#"[{"id":1,"tags":[1,2,3]}]"#, "json");
        assert!(json_has_nested(temp.path()).unwrap());
    }

    #[test]
    fn scalar_only_not_nested() {
        let temp = write_temp(r#"[{"id":1,"name":"x"}]"#, "json");
        assert!(!json_has_nested(temp.path()).unwrap());
    }

    #[test]
    fn infers_struct_column() {
        let input = write_temp(
            r#"[{"id":1,"meta":{"a":1,"b":"x","c":true}}]"#,
            "json",
        );
        let out = write_temp("", "parquet");
        let rows = convert_json_to_parquet(input.path(), out.path()).unwrap();
        assert_eq!(rows, 1);
        let schema = parquet_schema(out.path());
        let meta = schema.field_with_name("meta").unwrap();
        let DataType::Struct(fields) = meta.data_type() else {
            panic!("meta must be a Struct, got {:?}", meta.data_type());
        };
        assert_eq!(fields.len(), 3);
        assert_eq!(fields.find("a").unwrap().1.data_type(), &DataType::Int64);
    }

    #[test]
    fn infers_list_column() {
        let input = write_temp(r#"[{"id":1,"tags":[1,2,3]}]"#, "json");
        let out = write_temp("", "parquet");
        convert_json_to_parquet(input.path(), out.path()).unwrap();
        let schema = parquet_schema(out.path());
        let tags = schema.field_with_name("tags").unwrap();
        assert!(matches!(tags.data_type(), DataType::List(_)));
    }

    #[test]
    fn nested_fields_are_nullable() {
        let input = write_temp(r#"[{"id":1,"meta":{"a":1}}]"#, "json");
        let out = write_temp("", "parquet");
        convert_json_to_parquet(input.path(), out.path()).unwrap();
        let schema = parquet_schema(out.path());
        let DataType::Struct(fields) = schema.field_with_name("meta").unwrap().data_type() else {
            panic!("meta must be a Struct");
        };
        assert!(fields.iter().all(|f| f.is_nullable()));
    }

    #[test]
    fn empty_input_writes_no_rows() {
        let input = write_temp("[]", "json");
        let out = write_temp("", "parquet");
        assert_eq!(convert_json_to_parquet(input.path(), out.path()).unwrap(), 0);
    }

    #[test]
    fn jsonl_nested_missing_key_tolerated() {
        // Second record omits the nested key entirely; all-nullable coercion
        // must let the writer accept it.
        let input = write_temp(
            "{\"id\":1,\"meta\":{\"a\":1}}\n{\"id\":2}\n",
            "jsonl",
        );
        let out = write_temp("", "parquet");
        let rows = convert_json_to_parquet(input.path(), out.path()).unwrap();
        assert_eq!(rows, 2);
    }
}
