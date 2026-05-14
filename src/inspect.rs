// ============================================================
// src/inspect.rs — Parquet inspection: file and per-column statistics
//
// Statistics come from footer metadata only; no data pages are read.
use anyhow::{Context, Result};
use parquet::file::metadata::{ColumnChunkMetaData, ParquetMetaData};
use parquet::file::reader::{FileReader, SerializedFileReader};
use std::fs::File;
use std::path::Path;

/// Detect Parquet input by extension.
pub fn is_parquet(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("parquet"))
}

/// Display file-level and per-column statistics from the Parquet footer.
pub fn display_parquet_stats(path: &Path) -> Result<()> {
    let file = File::open(path).with_context(|| format!("open parquet {}", path.display()))?;
    let file_size = file.metadata().context("read file metadata")?.len();
    let reader = SerializedFileReader::new(file).context("read parquet footer")?;
    let metadata = reader.metadata();
    display_file_summary(metadata, file_size);
    display_column_stats(metadata);
    Ok(())
}

fn display_file_summary(metadata: &ParquetMetaData, file_size: u64) {
    let file_meta = metadata.file_metadata();
    let uncompressed: i64 = metadata
        .row_groups()
        .iter()
        .map(|group| group.total_byte_size())
        .sum();
    eprintln!("[STATS]");
    eprintln!("  rows          {}", file_meta.num_rows());
    eprintln!("  row groups    {}", metadata.num_row_groups());
    eprintln!("  columns       {}", file_meta.schema_descr().num_columns());
    eprintln!("  file size     {}", format_bytes(file_size));
    eprintln!(
        "  uncompressed  {}",
        format_bytes(uncompressed.max(0) as u64)
    );
    if let Some(created_by) = file_meta.created_by() {
        eprintln!("  created by    {created_by}");
    }
    eprintln!();
}

/// Per-column footprint aggregated across all row groups.
struct ColumnUsage {
    name: String,
    compressed_bytes: i64,
    uncompressed_bytes: i64,
    null_count: Option<u64>,
}

fn aggregate_column_usage(metadata: &ParquetMetaData) -> Vec<ColumnUsage> {
    let mut usages: Vec<ColumnUsage> = metadata
        .file_metadata()
        .schema_descr()
        .columns()
        .iter()
        .map(|column| ColumnUsage {
            name: column.path().string(),
            compressed_bytes: 0,
            uncompressed_bytes: 0,
            null_count: Some(0),
        })
        .collect();
    for group in metadata.row_groups() {
        for (usage, chunk) in usages.iter_mut().zip(group.columns()) {
            usage.compressed_bytes += chunk.compressed_size();
            usage.uncompressed_bytes += chunk.uncompressed_size();
            usage.null_count = match (usage.null_count, chunk_null_count(chunk)) {
                (Some(total), Some(count)) => Some(total + count),
                _ => None,
            };
        }
    }
    usages
}

/// Null count is absent when a writer omitted page statistics.
fn chunk_null_count(chunk: &ColumnChunkMetaData) -> Option<u64> {
    chunk.statistics().and_then(|s| s.null_count_opt())
}

fn display_column_stats(metadata: &ParquetMetaData) {
    let usages = aggregate_column_usage(metadata);
    let name_width = usages
        .iter()
        .map(|u| u.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    eprintln!(
        "{:<width_n$}  {:>12}  {:>12}  {:>10}",
        "name",
        "compressed",
        "raw",
        "nulls",
        width_n = name_width,
    );
    eprintln!("{}", "-".repeat(name_width + 42));
    for usage in &usages {
        let nulls = usage
            .null_count
            .map_or_else(|| "n/a".to_string(), |n| n.to_string());
        eprintln!(
            "{:<width_n$}  {:>12}  {:>12}  {:>10}",
            usage.name,
            format_bytes(usage.compressed_bytes.max(0) as u64),
            format_bytes(usage.uncompressed_bytes.max(0) as u64),
            nulls,
            width_n = name_width,
        );
    }
    eprintln!();
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}
