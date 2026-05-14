# Installation

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (edition 2021)
- An internet connection to download dependencies

## Building from Source

```bash
# Clone the repository
git clone https://github.com/gildas-le-drogoff/csv_to_parquet.git
cd csv_to_parquet

# Build in release mode
cargo build --release
```

The binary is located in `target/release/csv_to_parquet`.

## System Installation

```bash
# Install the binary, man page, and bash completion
make install
```

By default, installation is in `/usr/local`. To change the prefix:

```bash
make install PREFIX=~/.local
```

## Verification

```bash
# Display version
./target/release/csv_to_parquet --version

# Display help
./target/release/csv_to_parquet --help

# Generate man page
./target/release/csv_to_parquet --man | man -l -
```
