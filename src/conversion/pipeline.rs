// ============================================================
use super::csv_blocks::CsvBlock;
use crate::analysis::{analyze_block, BlockResult, ColumnMetrics};
use anyhow::{anyhow, Result};
use arrow::datatypes::{Field, Schema};
use arrow::record_batch::RecordBatch;
use crossbeam::channel::{Receiver, Sender};
use rayon::prelude::*;
use std::sync::{Arc, Mutex};
use std::thread;

pub fn start_analysis_workers(
    block_receiver: Receiver<CsvBlock>,
    batch_sender: Sender<(usize, RecordBatch, u64)>,
    schema: Arc<Schema>,
    global_metrics: Arc<Mutex<Vec<ColumnMetrics>>>,
    force_utf8: bool,
) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || {
        block_receiver
            .into_iter()
            .par_bridge()
            .try_for_each(|block| {
                let BlockResult { batch, metrics } =
                    analyze_block(&block.records, schema.clone(), force_utf8)?;
                merge_metrics(&global_metrics, &metrics);
                batch_sender
                    .send((block.index, batch, block.bytes_read))
                    .map_err(|_| anyhow!("parquet writer stopped receiving batches"))?;
                Ok(())
            })
    })
}

fn merge_metrics(global_metrics: &Mutex<Vec<ColumnMetrics>>, block_metrics: &[ColumnMetrics]) {
    let mut global = global_metrics
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for (target, source) in global.iter_mut().zip(block_metrics) {
        target.total_values += source.total_values;
        target.total_null_text += source.total_null_text;
        target.total_conversion_errors += source.total_conversion_errors;
        target.total_valid_values += source.total_valid_values;
        for sample in &source.error_samples.values {
            target.error_samples.add(sample.clone());
        }
    }
}

pub fn make_schema_all_nullable(schema: Schema) -> Schema {
    let fields: Vec<Field> = schema
        .fields()
        .iter()
        .map(|field| Field::new(field.name(), field.data_type().clone(), true))
        .collect();
    Schema::new(fields)
}
