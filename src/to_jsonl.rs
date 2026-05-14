// ============================================================
// src/to_jsonl.rs — Parquet → JSONL inverse conversion
//
// Delegates formatting to arrow-json: one JSON object per line,
// null fields omitted, temporal types use Arrow's display format.
use anyhow::{Context, Result};
use arrow::json::LineDelimitedWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

const JSONL_WRITER_BUFFER_BYTES: usize = 1 << 20;

/// Returns the number of rows written. Caller reports success.
pub fn convert_parquet_to_jsonl<P: AsRef<Path>, Q: AsRef<Path>>(
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
        .with_context(|| format!("create jsonl output {}", output_path.as_ref().display()))?;
    let buffered = BufWriter::with_capacity(JSONL_WRITER_BUFFER_BYTES, output_file);
    let mut json_writer = LineDelimitedWriter::new(buffered);

    let mut total_rows: usize = 0;
    for batch_result in reader {
        let batch = batch_result.context("read parquet row group")?;
        json_writer.write(&batch).context("write JSONL batch")?;
        total_rows += batch.num_rows();
    }

    json_writer.finish().context("finish JSONL writer")?;
    json_writer
        .into_inner()
        .flush()
        .context("flush JSONL writer")?;
    Ok(total_rows)
}
