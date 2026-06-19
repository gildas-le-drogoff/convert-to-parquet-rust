// tests/strict_order_tests.rs
use anyhow::Result;
use arrow::array::{ArrayRef, Int64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use crossbeam::channel::unbounded;
use convert_to_parquet::conversion::start_parquet_writer;
use indicatif::ProgressBar;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;
use std::sync::Arc;
use tempfile::NamedTempFile;

#[test]
fn test_strict_row_order() -> Result<()> {
    let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, false)]));
    let (tx, rx) = unbounded::<(usize, RecordBatch, u64)>();
    let output = NamedTempFile::new()?;
    let handle = start_parquet_writer(
        rx,
        output.path(),
        schema.clone(),
        1024,
        ProgressBar::hidden(),
    )?;

    let batch0 = RecordBatch::try_new(
        schema.clone(),
        vec![Arc::new(Int64Array::from(vec![0, 1, 2])) as ArrayRef],
    )?;
    let batch1 = RecordBatch::try_new(
        schema.clone(),
        vec![Arc::new(Int64Array::from(vec![3, 4, 5])) as ArrayRef],
    )?;
    let batch2 = RecordBatch::try_new(
        schema.clone(),
        vec![Arc::new(Int64Array::from(vec![6, 7, 8])) as ArrayRef],
    )?;

    tx.send((2, batch2, 30))?;
    tx.send((0, batch0, 10))?;
    tx.send((1, batch1, 20))?;
    drop(tx);
    handle.join().unwrap()?;

    let file = File::open(output.path())?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;
    let mut expected = 0i64;
    for batch_result in reader {
        let batch = batch_result?;
        let col = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap();
        for i in 0..col.len() {
            assert_eq!(col.value(i), expected);
            expected += 1;
        }
    }
    assert_eq!(expected, 9);
    Ok(())
}
