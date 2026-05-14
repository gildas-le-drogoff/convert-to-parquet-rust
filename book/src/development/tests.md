# Tests

## Running Tests

```bash
# All tests
cargo test

# With output display
cargo test -- --nocapture

# Specific test
cargo test test_name

# Integration tests only
cargo test --test integration
```

## Test Structure

The project has **13 test files** covering:

### Unit Tests (in `src/`)

- Type inference (`schema.rs`)
- Value conversions (`analysis/conversions.rs`)
- Delimiter detection (`utils.rs`)
- Header detection (`utils.rs`)
- Date parsing (`utils.rs`)
- Timestamp precision (microseconds, nanoseconds)

### Integration Tests (`tests/`)

- `test_schema_inference.rs` — validation of inferred types
- `test_conversions.rs` — value conversion
- `test_delimiter_detection.rs` — automatic detection
- `test_error_handling.rs` — behavior on invalid data
- `test_heuristics.rs` — robustness of heuristics
- `test_integration.rs` — complete pipeline
- `test_block_order.rs` — strict block ordering in parallel
- `test_utils.rs` — utilities
- `test_xlsx_e2e.rs` — Excel → Parquet conversion (round-trip)
- `test_inverse_and_inspect.rs` — Parquet → CSV/JSONL and inspection

### Fixture Generation

The `src/csv_generator.rs` directory contains a test data generation binary:

```bash
cargo run --bin csv_generator
```

Produces CSV/TSV datasets in `datasets_tests/`.

### Python Test

A Python script allows validating Parquet output:

```bash
python3 test_csv_to_parquet.py
```

Requirements: `pyarrow` (`pip install pyarrow`).

## Makefile

```bash
make test    # cargo test
make check   # cargo check (quick verification)
make lint    # cargo clippy -- -D warnings
make fmt     # cargo fmt
```
