# Changelog

## [0.4.0] - 2026-06-19

### ✨ New Features

#### Interactive Viewer (TUI)

- **Parquet Interactive Mode**: opening a `.parquet` file in a terminal launches a full-screen viewer (ratatui)
  - Data preview, navigation, and on-the-fly export to CSV / JSONL / JSON / XLSX
  - Automatic fallback to schema + statistics display when output is redirected

#### New Export Formats (Parquet → …)

- **Parquet → JSON**: export as a JSON array (`src/to_json.rs`)
- **Parquet → XLSX**: export as an Excel workbook (`src/to_xlsx.rs`)
- Unified export module `src/export.rs`

#### Nested JSON Objects

- **Nested JSON**: support for nested objects/arrays via Arrow (`src/json_arrow.rs`)

### 🔧 Technical Improvements

- Exposed a reusable library (`src/lib.rs`) consumed by integration tests
- `rust_xlsxwriter` promoted to a runtime dependency (XLSX export), added `ratatui`
- Regenerated man page (`csv_to_parquet.1`): up-to-date version and synopsis

### 🧪 Tests

- `tests/export_formats_tests.rs`: CSV/JSONL/JSON/XLSX export
- `tests/nested_json_tests.rs`: nested JSON objects
- `tests/noise_robustness_tests.rs`: robustness against noisy data

---

## [0.3.0] - 2026-06-14

### ✨ New Features

#### Support for New Input Formats

- **JSON / JSONL**: automatic conversion of `.json`, `.jsonl`, `.ndjson` files to Parquet
  - Support for arrays of objects, single object, or one object per line
  - Header extraction via two-pass scan for JSONL
- **Excel Files**: support for `.xlsx`, `.xlsm`, `.xlsb`, `.xls`, `.ods`
  - Cell-by-cell streaming for xlsx/xlsb (low memory footprint)
  - Parallel sheet processing (configurable with `--sheet-concurrency`)
  - One sheet = one Parquet file: `<filename>__<sheetname>.parquet`
- **Compressed Files**: support for `.gz` and `.zst` input (automatic decompression to temporary file)

#### Reverse Conversion (Parquet → Other Formats)

- **Parquet → CSV**: new `--to-csv` option
- **Parquet → JSONL**: new `--to-jsonl` option
  - One JSON object per line, null fields omitted
  - Using Arrow formatters for type fidelity

#### Enhanced CLI Interface

- **Glob Expansion**: `csv_to_parquet *.csv` processes all matching files
- **Forced Delimiter**: `--delimiter` (supports `,` `;` `\t` `|`)
- **Flexible Output**: `-o` can be a file or folder (automatic detection)
- **Parquet Interactive Mode**: if interactive terminal, displays schema + statistics then offers CSV/JSONL export
- **Man Page Generation**: `--man` unchanged

#### Parquet File Inspection

- New `--view-schema` command (already existing, but improved)
- Display column statistics: compressed/decompressed sizes, null count

### 🔧 Technical Improvements

#### Temporal Precision

- **Timestamp**: preservation of sub-millisecond precision (microseconds, nanoseconds)
- Direct conversion to target unit without loss (Second, Millisecond, Microsecond, Nanosecond)
- Smarter unit detection (textual or numeric, with plausibility window)

#### CSV Reading Reliability

- **Multi-line Field Handling**: quotes are correctly interpreted
- Blocks sent with their **byte position** (reliable progress bar)
- Block reordering in Parquet writing (even in parallel)

#### Metrics and Progress

- Progress bar based on **MB/s** (replaces "rows/s")
- Ticker thread cleanup (prevents zombie processes)
- Conversion metrics: valid values, explicit nulls, conversion errors, error samples

#### Schema Inference

- Confidence threshold for timestamps: 80% (configurable)
- Improved type detection (bool, date, timestamp, int, uint, float)
- Sampling limited to 10,000 rows (except `--full-schema-inference`)

### 📁 Structural Changes

#### New Modules

- `src/json.rs`: JSON → temporary CSV conversion
- `src/xlsx.rs`: Excel spreadsheet reading
- `src/inspect.rs`: Parquet statistics
- `src/to_csv.rs`: Parquet → CSV
- `src/to_jsonl.rs`: Parquet → JSONL

#### Refactored Modules

- `src/analysis/`: cleaner splitting, generic builders
- `src/conversion/`: parallel pipeline, MB/s ticker, block handling with position
- `src/schema.rs`: types renamed to English, inference logic clarified
- `src/utils.rs`: compression functions, advanced temporal parsing

### 🗑️ Deletions

- Empty module `automatic_rejection_tests.rs`
- Unused variables in `ErrorCounters`
- Dependency on CSV reconstruction in `analyze_block()`

### 🧪 Tests

- New integration tests for JSON, XLSX and multi-line CSV fields
- Timestamp precision tests (microseconds, nanoseconds)
- Verification of strict row ordering in parallel pipeline

### 🌐 Internationalization

- Codebase fully in English: variable names, error messages, logs, comments, and end-user messages
