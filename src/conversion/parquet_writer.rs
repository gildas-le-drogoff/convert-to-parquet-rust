// ============================================================
use anyhow::Result;
use arrow::record_batch::RecordBatch;
use crossbeam::channel::Receiver;
use indicatif::ProgressBar;
use parquet::arrow::arrow_writer::ArrowWriter;
use parquet::basic::{Compression, ConvertedType, ZstdLevel};
use parquet::file::properties::WriterProperties;
use parquet::file::reader::{FileReader, SerializedFileReader};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::thread;

const PARQUET_ZSTD_LEVEL: i32 = 5;

pub fn start_parquet_writer<Q: AsRef<Path>>(
    batch_receiver: Receiver<(usize, RecordBatch, u64)>,
    output_path: Q,
    schema: Arc<arrow::datatypes::Schema>,
    max_row_group_rows: usize,
    progress_bar: ProgressBar,
) -> Result<thread::JoinHandle<Result<()>>> {
    let output_file = File::create(output_path)?;
    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(PARQUET_ZSTD_LEVEL)?))
        .set_max_row_group_row_count(Some(max_row_group_rows))
        .build();
    let mut writer = ArrowWriter::try_new(output_file, schema, Some(properties))?;
    Ok(thread::spawn(move || {
        progress_bar.set_message("Writing parquet");
        let mut pending: BTreeMap<usize, (RecordBatch, u64)> = BTreeMap::new();
        let mut expected_index = 0usize;
        for (index, batch, bytes_read) in batch_receiver {
            pending.insert(index, (batch, bytes_read));
            while let Some((batch, bytes_read)) = pending.remove(&expected_index) {
                writer.write(&batch)?;
                progress_bar.set_position(bytes_read);
                expected_index += 1;
            }
        }
        progress_bar.set_message("Finalizing");
        writer.close()?;
        Ok(())
    }))
}

pub fn verify_parquet_schema<P: AsRef<Path>>(path: P) -> Result<()> {
    let file = File::open(&path)?;
    let reader = SerializedFileReader::new(file)?;
    let schema = reader.metadata().file_metadata().schema_descr();
    eprintln!("\n[SCHEMA] {}\n", path.as_ref().display());
    display_schema_table(schema);
    Ok(())
}

fn display_schema_table(schema: &parquet::schema::types::SchemaDescriptor) {
    let columns = schema.columns();
    let name_width = columns
        .iter()
        .map(|c| c.path().string().len())
        .max()
        .unwrap_or(4)
        .max(4);
    let logical_width = columns
        .iter()
        .map(|c| logical_type(c.as_ref()))
        .map(|s| s.len())
        .max()
        .unwrap_or(7)
        .max(7);
    let physical_width = columns
        .iter()
        .map(|c| format!("{:?}", c.physical_type()).len())
        .max()
        .unwrap_or(8)
        .max(8);
    let index_width = columns.len().to_string().len().max(2);
    let total_width = index_width + name_width + logical_width + physical_width + 10;
    eprintln!(
        "{:>width_i$}  {:<width_n$}  {:<width_l$}  {:<width_p$}",
        "#",
        "name",
        "logical",
        "physical",
        width_i = index_width,
        width_n = name_width,
        width_l = logical_width,
        width_p = physical_width,
    );
    eprintln!("{}", "-".repeat(total_width));
    for (index, column) in columns.iter().enumerate() {
        let name = column.path().string();
        let logical = logical_type(column.as_ref());
        let physical = format!("{:?}", column.physical_type());
        eprintln!(
            "{:>width_i$}  {:<width_n$}  {:<width_l$}  {:<width_p$}",
            index,
            name,
            logical,
            physical,
            width_i = index_width,
            width_n = name_width,
            width_l = logical_width,
            width_p = physical_width,
        );
    }
    eprintln!();
}

fn logical_type(column: &parquet::schema::types::ColumnDescriptor) -> String {
    if let Some(logical) = column.logical_type_ref() {
        format!("{logical:?}")
    } else {
        let converted = column.converted_type();
        if converted != ConvertedType::NONE {
            format!("{converted:?}")
        } else {
            "NONE".to_string()
        }
    }
}
