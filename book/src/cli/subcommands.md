# Subcommands

The tool automatically detects the operation to perform based on the input file type and provided flags.

## Conversion → Parquet (default mode)

```bash
csv_to_parquet [OPTIONS] <INPUT...>
```

Automatic input format detection:

| Extension                                 | Format Processed                        |
| ----------------------------------------- | --------------------------------------- |
| `.csv`, `.tsv`                            | CSV/TSV (delimiter auto-detected)       |
| `.json`, `.jsonl`, `.ndjson`              | JSON / JSONL / NDJSON                   |
| `.xlsx`, `.xlsm`, `.xlsb`, `.xls`, `.ods` | Excel / LibreOffice Spreadsheet         |
| `.parquet`                                | Interactive schema inspection           |
| `.gz`, `.zst`                             | Automatic decompression then conversion |

## Reverse Conversion: Parquet → CSV

```bash
csv_to_parquet --to-csv <FILE.parquet> -o <OUTPUT.csv>
```

## Reverse Conversion: Parquet → JSONL

```bash
csv_to_parquet --to-jsonl <FILE.parquet> -o <OUTPUT.jsonl>
```

## Schema Inspection

```bash
csv_to_parquet --view-schema <FILE.parquet>
```

Displays Parquet file metadata (compressed/decompressed size per column, null count).

## Interactive Mode

When a Parquet file is passed without an export flag and the terminal is interactive,
a full-screen viewer (ratatui) opens:

1. Data preview with navigation through rows and columns
2. On-the-fly export to CSV, JSONL, JSON, or XLSX

When the output is redirected (non-interactive), it falls back to printing the
schema and statistics instead.

## Generating the Man Page

```bash
csv_to_parquet --man > csv_to_parquet.1
```
