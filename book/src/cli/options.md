# General Options

| Option                    | Description                                                               |
| ------------------------- | ------------------------------------------------------------------------- |
| `-o, --output <OUTPUT>`   | Output file or directory. Treated as directory for multiple inputs        |
| `-d, --delimiter <DELIM>` | Force delimiter (`;` `,` `\t` `\|`). Overrides automatic detection        |
| `--force-header`          | Treat the first line as column headers, skipping header auto-detection    |
| `--to-csv`                | Reverse conversion: Parquet → CSV                                         |
| `--to-jsonl`              | Reverse conversion: Parquet → JSONL                                       |
| `--full-schema-inference` | Analyze **entire** file for type inference (instead of first 10,000 rows) |
| `--force-utf8`            | Force all columns to `LargeUtf8` (disables inference, preserves raw data) |
| `--view-schema`           | Display logical and physical schema of a Parquet file                     |
| `--sheet-concurrency <N>` | Number of Excel sheets processed in parallel (default: `ncpu/2`)          |
| `--man`                   | Generate man page in roff format                                          |

## Details

### `--output`

- With a single input file: output file path
- With multiple files (glob): treated as a directory
- If path ends with `/`: always treated as a directory

### `--delimiter`

Accepted values: `,` `;` `\t` `|` or any character. Useful for files without standard extensions.

### `--force-utf8`

Useful for heterogeneous data where type inference would produce unexpected mixing. No semantic loss, but larger files.

### `--force-header`

Forces the first line to be treated as column headers, bypassing the header auto-detection heuristic. Useful when the heuristic misclassifies a header row as data (or vice versa).

### `--full-schema-inference`

By default, only the first 10,000 rows are analyzed for schema inference. This option analyzes the entire file. Slower, but more accurate for files where types vary beyond the first rows.
