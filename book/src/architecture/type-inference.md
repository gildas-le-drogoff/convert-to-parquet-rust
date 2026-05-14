# Type Inference

Type inference is a critical phase: it determines the Parquet schema that will be used for all data.

## Method

Analysis is performed on a **sample of 10,000 rows** (except with `--full-schema-inference`). For each column, a `TypeState` accumulates observations:

```rust
struct TypeState {
    can_bool: bool,
    can_date: bool,
    can_timestamp: bool,
    can_int: bool,
    can_uint: bool,
    can_float: bool,
    min_value: i128,
    max_value: i128,
    // timestamp format counters
    ts_seconds: u64,
    ts_millis: u64,
    ts_micros: u64,
    ts_nanos: u64,
    ts_textual: u64,
    total_values: u64,
}
```

Each non-null value disables incompatible types. For example, a value containing a `.` disables `can_bool`, `can_int`, and `can_uint`.

## Inferred Types

| Parquet Type    | Detection Conditions                                                 |
| --------------- | -------------------------------------------------------------------- |
| `Boolean`       | `true`/`false`, `yes`/`no`, `y`/`n`, `on`/`off`, `t`/`f`, `0`/`1`    |
| `Int64`         | Signed integers within i64 range                                     |
| `UInt64`        | Positive integers exceeding i64 but within u64 range                 |
| `Float64`       | Numbers containing `.`, `e` or `E`                                   |
| `Date32`        | Formats `YYYY-MM-DD`, `DD/MM/YYYY`, `MM/DD/YYYY`                     |
| `Timestamp(ms)` | RFC3339, `YYYY-MM-DD HH:MM:SS`, variants with fractions and timezone |
| `LargeUtf8`     | Anything that doesn't match above                                    |

## Type Precedence

Types are tested in this order. The first type still possible at the end of sampling is selected:

1. `Boolean`
2. `Int64` / `UInt64`
3. `Float64`
4. `Date32`
5. `Timestamp(ms)`
6. `LargeUtf8` (fallback)

Timestamps have a **confidence threshold** of 80%: if less than 80% of non-null values in a column are valid timestamps, the type is downgraded to `LargeUtf8`.

## Null Values

These values are treated as null regardless of column:

`null`, `NULL`, `None`, `NaN`, `N/A`, `na`, `nd`, `nr`, `-`, `--` and empty strings.

## Ambiguous Dates

For `DD/MM/YYYY` vs `MM/DD/YYYY` formats: both are tested, first successful parse wins. If both are valid, `DD/MM/YYYY` is retained.

## `--force-utf8` Mode

When this flag is enabled, inference is disabled and all columns are forced to `LargeUtf8`. Raw data is preserved without any conversion.

## Full Inference (`--full-schema-inference`)

By default, only 10,000 rows are sampled. This flag analyzes the entire file, which is slower but more robust for files whose structure varies beyond the first rows.

## Time Precision

The time unit is automatically detected:

- **Seconds**: Unix timestamps < 10^10
- **Milliseconds**: timestamps between 10^10 and 10^13
- **Microseconds**: timestamps between 10^13 and 10^16
- **Nanoseconds**: timestamps > 10^16

Textual values are parsed with `chrono` and converted to milliseconds.
