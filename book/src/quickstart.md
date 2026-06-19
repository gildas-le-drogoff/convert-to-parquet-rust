# Getting Started

## Simple Conversion

```bash
convert_to_parquet file.csv
```

Produces `file.parquet` in the same directory.

## Custom Output

```bash
convert_to_parquet data.csv -o archive/data.parquet
```

## Batch Processing (glob)

```bash
convert_to_parquet *.csv
convert_to_parquet datasets/**/*.csv
```

Each file produces its own `.parquet` file.

## Compressed File

```bash
convert_to_parquet logs.csv.gz
convert_to_parquet logs.csv.zst
```

Decompression is automatic. For other formats (`.bz2`, `.xz`), use standard input:

```bash
bzcat logs.csv.bz2 | convert_to_parquet -
```

## Reverse Conversion

```bash
# Parquet → CSV
convert_to_parquet --to-csv data.parquet -o data.csv

# Parquet → JSONL
convert_to_parquet --to-jsonl data.parquet -o data.jsonl
```

## Inspecting a Parquet File

```bash
convert_to_parquet --view-schema data.parquet
```

Opening a Parquet file in a terminal launches a full-screen viewer (ratatui) for
data preview and on-the-fly export to CSV, JSONL, JSON, or XLSX.
