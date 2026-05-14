# Input Converters

Before entering the main pipeline (CSV → Parquet conversion), non-CSV formats are first converted to **intermediate CSV**.

## JSON Converter

Module: `src/json.rs`

### Supported Formats

- **Array of objects**: `[{"col": "val"}, {"col": "val2"}]`
- **Single object**: `{"col": "val"}`
- **JSONL / NDJSON**: `{"col": "val"}\n{"col": "val2"}`

### Operation

1. File is parsed with `serde_json`
2. For JSONL, a first pass scans the first 100 lines to collect all keys (headers)
3. Data is exported to temporary CSV with `csv_writer`
4. This temporary CSV is passed to the standard pipeline

### Handling Missing Values

Missing keys in an object are written as empty strings (which will be treated as nulls by the pipeline).

## Excel / LibreOffice Converter

Module: `src/xlsx.rs`

### Library: `calamine`

Calamine allows reading spreadsheets without depending on Excel or LibreOffice:

| Format  | Support                                       |
| ------- | --------------------------------------------- |
| `.xlsx` | Cell-by-cell streaming (low memory footprint) |
| `.xlsm` | Cell-by-cell streaming                        |
| `.xlsb` | Cell-by-cell streaming                        |
| `.xls`  | Full reading                                  |
| `.ods`  | Full reading                                  |

### Operation

1. **List sheets**: workbook is opened to list sheet names
2. **Export by sheet**: each sheet is exported to temporary CSV
3. **Parallelism**: sheets are processed in parallel (configurable rayon pool)
4. **Name sanitization**: sheet names are sanitized for filesystems

### Output

Each sheet produces a Parquet file named `<file>__<sheet>.parquet`.

## Reverse Converter: Parquet → CSV

Module: `src/to_csv.rs`

Uses the Arrow API to read the Parquet file and write CSV with `csv_writer`.

## Reverse Converter: Parquet → JSONL

Module: `src/to_jsonl.rs`

- One JSON object per line
- Null fields are omitted
- Uses Arrow formatters for type fidelity

## Handling Compressed Files

Module: `src/utils.rs`

Extensions `.gz` and `.zst` are automatically detected. The file is decompressed to a temporary file (deleted at the end of execution thanks to `NamedTempFile` from `tempfile`).

For other formats (`.bz2`, `.xz`), the user must decompress via stdin:

```bash
bzcat data.csv.bz2 | csv_to_parquet -
```
