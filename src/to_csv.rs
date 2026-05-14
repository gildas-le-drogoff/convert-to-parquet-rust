// ============================================================
// src/to_csv.rs — Parquet → CSV inverse conversion
//
// Delegates formatting to arrow-csv: binary columns are hex-encoded,
// nulls become empty fields, temporal types use Arrow's display format.
use anyhow::{Context, Result};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

const CSV_WRITER_BUFFER_BYTES: usize = 1 << 20;

/// Returns the number of rows written. Caller reports success.
pub fn convert_parquet_to_csv<P: AsRef<Path>, Q: AsRef<Path>>(
    input_path: P,
    output_path: Q,
) -> Result<usize> {
    let file = File::open(input_path.as_ref())
        .with_context(|| format!("open parquet {}", input_path.as_ref().display()))?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .context("create parquet reader")?
        .build()
        .context("build parquet reader")?;

    let output_file = File::create(output_path.as_ref())
        .with_context(|| format!("create csv output {}", output_path.as_ref().display()))?;
    let buffered = BufWriter::with_capacity(CSV_WRITER_BUFFER_BYTES, output_file);
    let mut csv_writer = arrow::csv::WriterBuilder::new()
        .with_header(true)
        .build(buffered);

    let mut total_rows: usize = 0;
    for batch_result in reader {
        let batch = batch_result.context("read parquet row group")?;
        csv_writer.write(&batch).context("write CSV batch")?;
        total_rows += batch.num_rows();
    }

    csv_writer
        .into_inner()
        .flush()
        .context("flush CSV writer")?;
    Ok(total_rows)
}
