# Contributing

Contributions are welcome! Here's how to participate in the project.

## Prerequisites

- **Rust** (edition 2021)
- **Make** (for build targets)

## Workflow

```bash
# 1. Fork the repository

# 2. Clone your fork
git clone https://github.com/<your-username>/csv_to_parquet.git
cd csv_to_parquet

# 3. Create a branch
git checkout -b my-feature

# 4. Code!
# 5. Test
make test

# 6. Format
make fmt

# 7. Lint
make lint

# 8. Commit and push
git push origin my-feature

# 9. Open a Pull Request
```

## Code Structure

```
src/
├── main.rs           # CLI entry point (clap, format dispatch)
├── lib.rs            # Module declarations
├── conversion/
│   ├── mod.rs        # Main pipeline (orchestration)
│   ├── csv_blocks.rs # CSV reading in blocks
│   ├── parquet_writer.rs # Ordered Parquet writing
│   ├── pipeline.rs   # Parallel analysis workers
│   ├── ticker.rs     # MB/s progress bar
│   └── counting.rs   # Parquet row counting
├── analysis/
│   ├── mod.rs        # Block analysis
│   ├── builders.rs   # RecordBatch construction
│   ├── conversions.rs# Value → Arrow type conversion
│   └── types.rs      # Metrics and error counters
├── schema.rs         # Type inference (sampling)
├── utils.rs          # Delimiter/header detection, dates, nulls
├── inspect.rs        # Parquet metadata display
├── json.rs           # JSON → intermediate CSV converter
├── xlsx.rs           # Excel → intermediate CSV converter
├── to_csv.rs         # Parquet → CSV converter
└── to_jsonl.rs       # Parquet → JSONL converter
```

## Conventions

### Code

- **Formatting**: `cargo fmt` (standard rustfmt)
- **Linting**: `cargo clippy -- -D warnings`
- **No warnings**: code must compile without warnings
- **Tests**: any new feature must include tests

### Commits

- Commit messages in English
- Prefixes: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`
- One line ≤ 72 characters for title

### Documentation

Public modules must have a documentation comment (`///` or `//!`). Public functions should document parameters and return value when behavior is not obvious.
