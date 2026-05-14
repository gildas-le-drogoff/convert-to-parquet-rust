// ============================================================
use crate::analysis::ErrorCounters;
use crate::utils::error;
use anyhow::{anyhow, Context, Result};
use crossbeam::channel::Sender;
use csv::{ReaderBuilder, StringRecord};
use std::path::Path;
use std::sync::atomic::Ordering;

pub struct CsvBlock {
    pub index: usize,
    pub records: Vec<StringRecord>,
    /// Byte offset reached in the input after this block, for progress reporting.
    pub bytes_read: u64,
}

/// Read the input as CSV records (quoted multi-line fields included) and send
/// them downstream in blocks. Returns the total number of records produced.
pub fn produce_blocks<P: AsRef<Path>>(
    input_path: P,
    block_size: usize,
    delimiter: u8,
    has_header: bool,
    block_sender: &Sender<CsvBlock>,
    counters: &ErrorCounters,
) -> Result<usize> {
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(has_header)
        .flexible(true)
        .from_path(input_path.as_ref())
        .with_context(|| format!("open csv {}", input_path.as_ref().display()))?;
    let mut block: Vec<StringRecord> = Vec::with_capacity(block_size);
    let mut record = StringRecord::new();
    let mut block_index = 0usize;
    let mut total_records = 0usize;
    loop {
        match reader.read_record(&mut record) {
            Ok(true) => {
                block.push(record.clone());
                total_records += 1;
                if block.len() >= block_size {
                    let full = std::mem::replace(&mut block, Vec::with_capacity(block_size));
                    send_block(block_sender, block_index, full, reader.position().byte())?;
                    block_index += 1;
                }
            }
            Ok(false) => break,
            Err(e) if matches!(e.kind(), csv::ErrorKind::Io(_)) => {
                return Err(e).context("read csv input");
            }
            Err(e) => record_parse_error(&e, counters, total_records + 1),
        }
    }
    if !block.is_empty() {
        send_block(block_sender, block_index, block, reader.position().byte())?;
    }
    Ok(total_records)
}

fn send_block(
    sender: &Sender<CsvBlock>,
    index: usize,
    records: Vec<StringRecord>,
    bytes_read: u64,
) -> Result<()> {
    sender
        .send(CsvBlock {
            index,
            records,
            bytes_read,
        })
        .map_err(|_| anyhow!("analysis stage stopped receiving blocks"))
}

fn record_parse_error(e: &csv::Error, counters: &ErrorCounters, record_number: usize) {
    counters.parse_errors.fetch_add(1, Ordering::Relaxed);
    eprintln!(
        "{}",
        error(format!("[CSV ERROR] record {record_number}: {e}"))
    );
}
