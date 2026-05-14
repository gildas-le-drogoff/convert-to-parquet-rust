# Pipeline Overview

The conversion pipeline is the heart of the project. It transforms tabular data into a Parquet file efficiently and in parallel.

## General Schema

```
Source File
     │
     ├─ Delimiter detection (first 20 lines)
     ├─ Header detection (heuristics)
     ├─ Schema inference (sample of 10,000 rows)
     │
     ├─ Reading in blocks of 100,000 rows ─────────────┐
     │                                                 │
     │   ┌─ Block 0 ─► Parallel analysis (rayon) ──┐   │
     │   ├─ Block 1 ─► Parallel analysis (rayon) ──┤   │
     │   ├─ Block 2 ─► Parallel analysis (rayon) ──┤   │
     │   └─ Block N ─► Parallel analysis (rayon) ──┘   │
     │                    │                            │
     │       crossbeam channel (ordered by index)      │
     │                    │                            │
     │         Ordered Parquet writing                 │
     │              (ZSTD level 5)                     │
     │                                                 │
     └─────────────────────────────────────────────────┘
     │
     └─ Validation report (stderr)
```

## Detailed Steps

### 1. Input Format Resolution

The file is inspected to determine:

- The delimiter (unless forced with `-d`)
- Presence of a header
- File type (CSV, JSON, XLSX, Parquet...)

### 2. Schema Inference

A sample of 10,000 rows is analyzed to determine the type of each column. See [Type Inference](type-inference.md).

### 3. Parallel Pipeline

The file is read in **blocks of 100,000 rows**. Each block is sent through a crossbeam channel (`block_sender`) to a pool of rayon workers that:

1. Convert each value according to the inferred type
2. Build an Arrow `RecordBatch`
3. Gather metrics (valid values, nulls, errors)

`RecordBatch` objects are returned via a second channel (`batch_sender`) with their original index.

### 4. Ordered Writing

The Parquet writing thread receives batches **out of order** (parallelism does not guarantee order). It reorders them using an internal `BTreeMap` before writing, ensuring that the row order from the source file is preserved.

### 5. Validation Report

Once conversion is complete, a detailed report is displayed on stderr:

```
========== VALIDATION REPORT ==========

CSV rows           1000000
Parquet rows       1000000
Parse errors             0

========== COLUMNS ==========

name                     type           null %      err %    valid %     conf
--------------------------------------------------------------------------------------
id                       Int64           0.00        0.00     100.00   100.00
```

### Progress Ticker

A `ticker` thread runs in the background and updates the progress bar in real-time (MB/s) based on bytes read.
