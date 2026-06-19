# Supported Input Formats

## CSV / TSV

Delimiters automatically detected: `,` `;` `\t` `|`.

Detection is performed by counting occurrences of each potential delimiter on the first 20 lines. The most frequent and consistent delimiter is selected.

Header is detected via heuristics: comparing the profile of the first line (inferred types) with the rest of the data. If no header is found, columns are named `col_0`, `col_1`, etc.

## JSON / JSONL / NDJSON

Supported formats:

- **Array of objects**: `[{...}, {...}]`
- **Single object**: `{...}`
- **One object per line** (JSONL / NDJSON): `{...}\n{...}\n`

Flat JSON is first converted to intermediate CSV (keys become headers), then the standard pipeline converts it to Parquet.

When nested objects or arrays are detected, a native Arrow path is used instead, preserving the structure as Struct/List columns rather than flattening it to strings.

## Excel / LibreOffice Spreadsheets

Supported formats:

| Extension | Format                               |
| --------- | ------------------------------------ |
| `.xlsx`   | Excel 2007+ (cell-by-cell streaming) |
| `.xlsm`   | Excel with macros                    |
| `.xlsb`   | Binary Excel (streaming)             |
| `.xls`    | Excel 97-2003                        |
| `.ods`    | OpenDocument Spreadsheet             |

Each sheet produces a distinct Parquet file: `<file>__<sheet>.parquet`.

Parallel sheet processing is configurable with `--sheet-concurrency`.

## Compressed Files

The extensions `.gz` (gzip) and `.zst` (Zstandard) are automatically decompressed to a temporary file before conversion.

For unsupported formats (`.bz2`, `.xz`), use standard input:

```bash
bzcat data.csv.bz2 | convert_to_parquet -
```
