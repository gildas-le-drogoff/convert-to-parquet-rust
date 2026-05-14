# csv-to-parquet

**csv-to-parquet** is a multi-format converter to [Parquet](https://parquet.apache.org/) written in Rust.

It converts **CSV/TSV**, **JSON/JSONL/NDJSON** and **Excel spreadsheets** (xlsx/xlsm/xlsb/xls/ods) to compressed Parquet files (ZSTD), with automatic schema inference and parallel processing.

It also supports reverse conversion: Parquet → CSV and Parquet → JSONL.

## Key Features

- **Automatic type inference**: Boolean, Int64, UInt64, Float64, Date32, Timestamp(ms), LargeUtf8
- **ZSTD compression** (level 5) for optimized Parquet files
- **Parallelism**: reading in blocks of 100,000 rows, parallel conversion (rayon), ordered writing (crossbeam)
- **Automatic detection** of delimiter and header
- **Glob patterns**: `csv_to_parquet *.csv`
- **stdin input**: `cat data.csv | csv_to_parquet -`
- **Compressed files**: `.gz`, `.zst` automatically decompressed
- **Detailed validation report** for each conversion
- **Interactive mode**: full-screen Parquet viewer (ratatui) with data preview and export to CSV / JSONL / JSON / XLSX

## Supported Formats

| Input                          | Output                         |
| ------------------------------ | ------------------------------ |
| CSV, TSV, and other delimiters | Parquet (.parquet)             |
| JSON, JSONL, NDJSON            | Parquet                        |
| XLSX, XLSM, XLSB, XLS, ODS     | Parquet (one sheet = one file) |
| Parquet                        | CSV / JSONL (CLI), JSON / XLSX (viewer) |
