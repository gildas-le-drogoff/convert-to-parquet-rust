// ============================================================
use crate::analysis::{ColumnMetrics, ErrorCounters};
use crate::schema::{force_schema_to_utf8, infer_schema};
use crate::utils::{detect_delimiter, detect_header};
use crate::BLOCK_SIZE;
use anyhow::{anyhow, Context, Result};
use arrow::datatypes::Schema;
use arrow::record_batch::RecordBatch;
use colored::Colorize;
use counting::count_parquet_lines;
use crossbeam::channel::bounded;
use csv_blocks::{produce_blocks, CsvBlock};
use indicatif::{ProgressBar, ProgressStyle};
pub use parquet_writer::{start_parquet_writer, verify_parquet_schema};
use pipeline::{make_schema_all_nullable, start_analysis_workers};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use ticker::start_ticker;

mod counting;
mod csv_blocks;
mod parquet_writer;
mod pipeline;
mod ticker;

const BLOCK_QUEUE_CAPACITY: usize = 8;
const BATCH_QUEUE_CAPACITY: usize = 8;
const THROUGHPUT_WINDOW: Duration = Duration::from_secs(2);

struct InputFormat {
    delimiter: u8,
    has_header: bool,
}

struct PipelineOutcome {
    csv_records: usize,
    parse_errors: usize,
    metrics: Vec<ColumnMetrics>,
}

/// Stops the ticker thread on scope exit, including error paths.
struct TickerStopGuard(Arc<AtomicBool>);

impl Drop for TickerStopGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

pub fn convert_convert_to_parquet<P: AsRef<Path>, Q: AsRef<Path>>(
    input_path: P,
    output_path: Q,
    full_schema_inference: bool,
    force_utf8: bool,
    force_header: bool,
    delimiter_override: Option<u8>,
) -> Result<()> {
    let input = input_path.as_ref();
    let output = output_path.as_ref();
    let start_instant = Instant::now();
    let format = resolve_input_format(input, delimiter_override, force_header)?;
    let schema = build_schema(input, &format, full_schema_inference, force_utf8)?;
    let progress_bar = build_progress_bar(input)?;
    let outcome = run_pipeline(
        input,
        output,
        schema.clone(),
        &format,
        force_utf8,
        progress_bar,
    )?;
    let parquet_rows = count_parquet_lines(output)?;
    print_validation_report(&outcome, parquet_rows);
    display_metrics_table(&schema, &outcome.metrics);
    eprintln!("\n{} {:.2?}", "Duration".green(), start_instant.elapsed());
    verify_parquet_schema(output).context("Invalid Parquet schema")?;
    Ok(())
}

fn resolve_input_format(input: &Path, delimiter_override: Option<u8>, force_header: bool) -> Result<InputFormat> {
    let delimiter = match delimiter_override {
        Some(d) => {
            let display = if d == b'\t' {
                "\\t".to_string()
            } else {
                (d as char).to_string()
            };
            eprintln!(
                "{} Using forced delimiter: '{}'",
                "[INFO]".yellow().bold(),
                display
            );
            d
        }
        None => detect_delimiter(input)?,
    };
    let has_header = if force_header {
        true
    } else {
        let detected = detect_header(input, delimiter)?;
        if !detected {
            eprintln!(
                "{} {}",
                "[INFO]".yellow().bold(),
                "No header detected, column names generated automatically (col_0, col_1, ...)".yellow()
            );
        }
        detected
    };
    Ok(InputFormat {
        delimiter,
        has_header,
    })
}

fn build_schema(
    input: &Path,
    format: &InputFormat,
    full_inference: bool,
    force_utf8: bool,
) -> Result<Arc<Schema>> {
    eprintln!("{} {}", "[PHASE]".cyan().bold(), "Schema inference".cyan());
    let inferred = infer_schema(input, format.delimiter, full_inference, format.has_header)?;
    let effective = if force_utf8 {
        force_schema_to_utf8(&inferred)
    } else {
        inferred
    };
    let schema = Arc::new(make_schema_all_nullable(effective));
    eprintln!(
        "{} {} {}",
        "[OK]".green().bold(),
        "Schema detected:".green(),
        schema.fields().len().to_string().green().bold()
    );
    Ok(schema)
}

fn build_progress_bar(input: &Path) -> Result<ProgressBar> {
    let total_bytes = std::fs::metadata(input)
        .with_context(|| format!("stat {}", input.display()))?
        .len();
    let progress_bar = ProgressBar::new(total_bytes);
    progress_bar.set_style(
        ProgressStyle::with_template(
            "{elapsed_precise} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ETA {eta} {msg}",
        )?
        .progress_chars("█░"),
    );
    Ok(progress_bar)
}

fn run_pipeline(
    input: &Path,
    output: &Path,
    schema: Arc<Schema>,
    format: &InputFormat,
    force_utf8: bool,
    progress_bar: ProgressBar,
) -> Result<PipelineOutcome> {
    let counters = Arc::new(ErrorCounters::default());
    let (block_sender, block_receiver) = bounded::<CsvBlock>(BLOCK_QUEUE_CAPACITY);
    let (batch_sender, batch_receiver) = bounded::<(usize, RecordBatch, u64)>(BATCH_QUEUE_CAPACITY);
    let global_metrics = Arc::new(Mutex::new(
        schema
            .fields()
            .iter()
            .map(|f| ColumnMetrics::new(f.name()))
            .collect::<Vec<_>>(),
    ));
    let analysis_handle = start_analysis_workers(
        block_receiver,
        batch_sender.clone(),
        schema.clone(),
        global_metrics.clone(),
        force_utf8,
    );
    let writer_handle = start_parquet_writer(
        batch_receiver,
        output,
        schema,
        BLOCK_SIZE,
        progress_bar.clone(),
    )?;
    let stop_ticker = Arc::new(AtomicBool::new(false));
    let _ticker_guard = TickerStopGuard(stop_ticker.clone());
    let ticker_handle = start_ticker(progress_bar.clone(), stop_ticker.clone());
    let produce_result = produce_blocks(
        input,
        BLOCK_SIZE,
        format.delimiter,
        format.has_header,
        &block_sender,
        &counters,
    );
    drop(block_sender);
    let analysis_result = analysis_handle
        .join()
        .map_err(|_| anyhow!("analysis thread panicked"))?;
    drop(batch_sender);
    let writer_result = writer_handle
        .join()
        .map_err(|_| anyhow!("parquet writer thread panicked"))?;
    progress_bar.finish_with_message("Write complete");
    stop_ticker.store(true, Ordering::Relaxed);
    ticker_handle.join().ok();
    writer_result?;
    analysis_result?;
    let csv_records = produce_result?;
    Ok(PipelineOutcome {
        csv_records,
        parse_errors: counters.parse_errors.load(Ordering::Relaxed),
        metrics: collect_metrics(global_metrics)?,
    })
}

fn collect_metrics(global_metrics: Arc<Mutex<Vec<ColumnMetrics>>>) -> Result<Vec<ColumnMetrics>> {
    let mutex = Arc::try_unwrap(global_metrics)
        .map_err(|_| anyhow!("metrics still shared after pipeline shutdown"))?;
    Ok(mutex
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner()))
}

fn print_validation_report(outcome: &PipelineOutcome, parquet_rows: usize) {
    eprintln!(
        "\n{}\n",
        "========== VALIDATION REPORT ==========".magenta().bold()
    );
    eprintln!(
        "{} {:>12}\n{} {:>12}\n{} {:>12}",
        "CSV records".blue(),
        outcome.csv_records.to_string().blue().bold(),
        "Parquet rows".blue(),
        parquet_rows.to_string().blue().bold(),
        "Parse errors".yellow(),
        outcome.parse_errors.to_string().yellow().bold(),
    );
    if parquet_rows == outcome.csv_records {
        eprintln!("{} Consistency validated", "[OK]".green().bold());
    } else {
        let delta = outcome.csv_records as i64 - parquet_rows as i64;
        eprintln!(
            "{} {}",
            "[WARN]".yellow().bold(),
            format!("delta={delta}").yellow()
        );
    }
}

fn display_metrics_table(schema: &arrow::datatypes::Schema, metrics: &[ColumnMetrics]) {
    eprintln!("\n{}\n", "========== COLUMNS ==========".magenta().bold());
    eprintln!(
        "{:<24} {:<12} {:>12} {:>12} {:>12} {:>10}",
        "name", "type", "null %", "err %", "valid %", "conf"
    );
    eprintln!("{}", "-".repeat(86));
    for (i, m) in metrics.iter().enumerate() {
        let total = m.total_values.max(1) as f64;
        let null_rate = m.total_null_text as f64 / total * 100.0;
        let error_rate = m.total_conversion_errors as f64 / total * 100.0;
        let valid_rate = m.total_valid_values as f64 / total * 100.0;
        let final_type = format!("{:?}", schema.fields()[i].data_type());
        eprintln!(
            "{:<24} {:<12} {:>11.2} {:>11.2} {:>11.2} {:>9.2}",
            m.name, final_type, null_rate, error_rate, valid_rate, valid_rate
        );
    }
    eprintln!(
        "\n{}\n",
        "================================".magenta().bold()
    );
}
