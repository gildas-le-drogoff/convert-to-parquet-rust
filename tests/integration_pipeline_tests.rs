// tests/integration_pipeline_tests.rs
use arrow::array::{Array, LargeStringArray};
use csv_to_parquet::conversion::convert_csv_to_parquet;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::file::reader::{FileReader, SerializedFileReader};
use std::fs::File;
use std::io::Write;
use tempfile::NamedTempFile;

fn parquet_row_count(path: &std::path::Path) -> usize {
    let file = File::open(path).unwrap();
    let reader = SerializedFileReader::new(file).unwrap();
    reader
        .metadata()
        .row_groups()
        .iter()
        .map(|rg| rg.num_rows() as usize)
        .sum()
}

#[test]
fn test_full_pipeline_row_coherence() {
    let mut csv = NamedTempFile::new().unwrap();
    writeln!(csv, "a,b").unwrap();
    for i in 0..1000 {
        writeln!(csv, "{},{}", i, i * 2).unwrap();
    }
    let output = NamedTempFile::new().unwrap();
    convert_csv_to_parquet(csv.path(), output.path(), true, false, false, None).unwrap();
    assert_eq!(parquet_row_count(output.path()), 1000);
}

#[test]
fn test_quoted_multiline_field_preserved() {
    let mut csv = NamedTempFile::new().unwrap();
    write!(csv, "a,b\n1,\"line1\nline2\"\n2,plain\n").unwrap();
    let output = NamedTempFile::new().unwrap();
    convert_csv_to_parquet(csv.path(), output.path(), true, false, false, None).unwrap();
    assert_eq!(parquet_row_count(output.path()), 2);

    let file = File::open(output.path()).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();
    let mut texts: Vec<String> = Vec::new();
    for batch_result in reader {
        let batch = batch_result.unwrap();
        let col = batch
            .column(1)
            .as_any()
            .downcast_ref::<LargeStringArray>()
            .unwrap();
        for i in 0..col.len() {
            texts.push(col.value(i).to_string());
        }
    }
    assert_eq!(texts, vec!["line1\nline2".to_string(), "plain".to_string()]);
}
