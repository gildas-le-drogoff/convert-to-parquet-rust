# Changelog

## [0.4.0] - 2026-06-19

### ✨ New Features

- **TUI Viewer**: full-screen interactive viewer for `.parquet` files with export to CSV/JSONL/JSON/XLSX
- **New exports**: Parquet → JSON (array) and XLSX (Excel workbook)
- **Nested JSON**: support for nested objects/arrays via Arrow
- **New input formats**: JSON/JSONL, Excel (XLSX/XLSM/XLSB/XLS/ODS), compressed GZ/ZST
- **Reverse conversions**: Parquet → CSV and JSONL (one object per line)
- **Enhanced CLI**: glob expansion, forced delimiter, flexible output, man page

### 🔧 Improvements

- **Timestamp precision**: preserves microseconds/nanoseconds without loss
- **CSV parsing**: reliable multi-line fields with byte-position tracking
- **Progress**: MB/s metrics, clean thread shutdown, detailed conversion stats
- **Schema inference**: 80% confidence threshold, sampling limit (10k rows)

### 🧪 Tests

- Integration tests for JSON, XLSX, multi-line CSV, timestamp precision, and row ordering
