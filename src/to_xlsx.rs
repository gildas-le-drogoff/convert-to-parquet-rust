// ============================================================
// src/to_xlsx.rs — Parquet → XLSX inverse conversion
//
// Each column value is rendered to a string via Arrow's display
// formatter (nulls become empty cells), then written row by row.
// One header row, then one worksheet row per record.
use anyhow::{Context, Result};
use arrow::util::display::{ArrayFormatter, FormatOptions};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use rust_xlsxwriter::Workbook;
use std::fs::File;
use std::path::Path;

/// Returns the number of rows written. Caller reports success.
pub fn convert_parquet_to_xlsx<P: AsRef<Path>, Q: AsRef<Path>>(
    input_path: P,
    output_path: Q,
) -> Result<usize> {
    let file = File::open(input_path.as_ref())
        .with_context(|| format!("open parquet {}", input_path.as_ref().display()))?;
    let builder =
        ParquetRecordBatchReaderBuilder::try_new(file).context("create parquet reader")?;
    let column_names: Vec<String> = builder
        .schema()
        .fields()
        .iter()
        .map(|field| field.name().clone())
        .collect();
    let reader = builder.build().context("build parquet reader")?;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    for (col, name) in column_names.iter().enumerate() {
        worksheet
            .write(0, col as u16, name.as_str())
            .context("write header cell")?;
    }

    let options = FormatOptions::default().with_null("");
    let mut next_row: u32 = 1;
    for batch_result in reader {
        let batch = batch_result.context("read parquet row group")?;
        let formatters: Vec<ArrayFormatter> = batch
            .columns()
            .iter()
            .map(|array| ArrayFormatter::try_new(array.as_ref(), &options))
            .collect::<std::result::Result<_, _>>()
            .context("build column formatter")?;
        for row in 0..batch.num_rows() {
            for (col, formatter) in formatters.iter().enumerate() {
                let cell = formatter.value(row).to_string();
                worksheet
                    .write(next_row, col as u16, cell)
                    .context("write data cell")?;
            }
            next_row += 1;
        }
    }

    workbook
        .save(output_path.as_ref())
        .context("save xlsx workbook")?;
    Ok((next_row - 1) as usize)
}
