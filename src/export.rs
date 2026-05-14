// ============================================================
// src/export.rs — Parquet export formats and dispatch
use crate::to_csv::convert_parquet_to_csv;
use crate::to_json::convert_parquet_to_json;
use crate::to_jsonl::convert_parquet_to_jsonl;
use crate::to_xlsx::convert_parquet_to_xlsx;
use anyhow::Result;
use std::path::Path;

/// Target format for inverse conversion from Parquet.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Jsonl,
    Json,
    Xlsx,
}

impl ExportFormat {
    /// All formats, in menu order.
    pub const ALL: [ExportFormat; 4] = [
        ExportFormat::Csv,
        ExportFormat::Jsonl,
        ExportFormat::Json,
        ExportFormat::Xlsx,
    ];

    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Jsonl => "jsonl",
            ExportFormat::Json => "json",
            ExportFormat::Xlsx => "xlsx",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Csv => "CSV",
            ExportFormat::Jsonl => "JSONL",
            ExportFormat::Json => "JSON",
            ExportFormat::Xlsx => "XLSX",
        }
    }

    /// Single-key mnemonic used by the interactive viewer (no nav conflict).
    pub fn hotkey(self) -> char {
        match self {
            ExportFormat::Csv => 'c',
            ExportFormat::Jsonl => 'n',
            ExportFormat::Json => 'o',
            ExportFormat::Xlsx => 'x',
        }
    }

    pub fn from_hotkey(key: char) -> Option<ExportFormat> {
        ExportFormat::ALL.into_iter().find(|f| f.hotkey() == key)
    }

    /// Convert `input` Parquet to `output` in this format; returns row count.
    pub fn convert(self, input: &Path, output: &Path) -> Result<usize> {
        match self {
            ExportFormat::Csv => convert_parquet_to_csv(input, output),
            ExportFormat::Jsonl => convert_parquet_to_jsonl(input, output),
            ExportFormat::Json => convert_parquet_to_json(input, output),
            ExportFormat::Xlsx => convert_parquet_to_xlsx(input, output),
        }
    }
}
