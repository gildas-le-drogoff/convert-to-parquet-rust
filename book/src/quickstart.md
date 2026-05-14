# Getting Started

## Simple Conversion

```bash
csv_to_parquet file.csv
```

Produces `file.parquet` in the same directory.

## Custom Output

```bash
csv_to_parquet data.csv -o archive/data.parquet
```

## Batch Processing (glob)

```bash
csv_to_parquet *.csv
csv_to_parquet datasets/**/*.csv
```

Each file produces its own `.parquet` file.

## Compressed File

```bash
csv_to_parquet logs.csv.gz
csv_to_parquet logs.csv.zst
```

Decompression is automatic. For other formats (`.bz2`, `.xz`), use standard input:

```bash
bzcat logs.csv.bz2 | csv_to_parquet -
```

## Reverse Conversion

```bash
# Parquet → CSV
csv_to_parquet --to-csv data.parquet -o data.csv

# Parquet → JSONL
csv_to_parquet --to-jsonl data.parquet -o data.jsonl
```

## Inspecting a Parquet File

```bash
csv_to_parquet --view-schema data.parquet
```

Opening a Parquet file in a terminal launches a full-screen viewer (ratatui) for
data preview and on-the-fly export to CSV, JSONL, JSON, or XLSX.
