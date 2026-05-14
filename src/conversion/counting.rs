// ============================================================
use anyhow::Result;
use parquet::file::reader::{FileReader, SerializedFileReader};
use std::fs::File;
use std::path::Path;

pub fn count_parquet_lines<P: AsRef<Path>>(path: P) -> Result<usize> {
    let file = File::open(path)?;
    let reader = SerializedFileReader::new(file)?;
    Ok(reader
        .metadata()
        .row_groups()
        .iter()
        .map(|rg| rg.num_rows() as usize)
        .sum())
}
