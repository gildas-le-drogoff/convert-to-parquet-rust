// ============================================================
use crate::utils::{
    generate_column_names, is_null_text, parse_bool, parse_date_ymd, parse_textual_datetime,
};
use anyhow::{Context, Result};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use chrono::Timelike;
use csv::ReaderBuilder;
use log::{debug, info};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

const SCHEMA_SAMPLE_ROWS: usize = 10_000;

/// Minimum fraction of observed (non-null) values that must parse as a
/// candidate type for that type to be inferred. Values below this fraction
/// are treated as noise and coerced to NULL during conversion.
const TYPE_CONFIDENCE_THRESHOLD: f64 = 0.95;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TimestampFormat {
    Textual,
    Numeric,
}

#[derive(Clone)]
struct TypeState {
    bool_ok: u64,
    date_ok: u64,
    int_ok: u64,
    float_ok: u64,
    min_value: i128,
    max_value: i128,
    ts_seconds: u64,
    ts_millis: u64,
    ts_micros: u64,
    ts_nanos: u64,
    ts_textual: u64,
    total_values: u64,
}

impl TypeState {
    fn new() -> Self {
        Self {
            bool_ok: 0,
            date_ok: 0,
            int_ok: 0,
            float_ok: 0,
            min_value: i128::MAX,
            max_value: i128::MIN,
            ts_seconds: 0,
            ts_millis: 0,
            ts_micros: 0,
            ts_nanos: 0,
            ts_textual: 0,
            total_values: 0,
        }
    }

    /// Each non-null value is tested against every candidate type and counted.
    /// No type is eliminated early: a minority of unparseable values must not
    /// veto a type the majority of the column satisfies.
    fn observe(&mut self, value: &str) {
        if is_null_text(value) {
            return;
        }
        let t = value.trim();
        self.total_values += 1;
        if parse_bool(t).is_some() {
            self.bool_ok += 1;
        }
        self.observe_numeric(t);
        self.observe_temporal(t);
    }

    fn observe_numeric(&mut self, t: &str) {
        if let Ok(v) = t.parse::<i128>() {
            self.int_ok += 1;
            self.min_value = self.min_value.min(v);
            self.max_value = self.max_value.max(v);
        }
        if t.parse::<f64>().is_ok() {
            self.float_ok += 1;
        }
    }

    fn observe_temporal(&mut self, t: &str) {
        if parse_date_ymd(t).is_some() {
            self.date_ok += 1;
        }
        if let Some((unit, format)) = detect_timestamp_unit(t) {
            self.record_timestamp(unit, format);
        }
    }

    fn is_confident(&self, ok: u64) -> bool {
        ok > 0 && (ok as f64 / self.total_values.max(1) as f64) >= TYPE_CONFIDENCE_THRESHOLD
    }

    fn record_timestamp(&mut self, unit: TimeUnit, format: TimestampFormat) {
        if format == TimestampFormat::Textual {
            self.ts_textual += 1;
        }
        match unit {
            TimeUnit::Second => self.ts_seconds += 1,
            TimeUnit::Millisecond => self.ts_millis += 1,
            TimeUnit::Microsecond => self.ts_micros += 1,
            TimeUnit::Nanosecond => self.ts_nanos += 1,
        }
    }

    fn choose_timestamp_unit(&self) -> TimeUnit {
        [
            (TimeUnit::Second, self.ts_seconds),
            (TimeUnit::Millisecond, self.ts_millis),
            (TimeUnit::Microsecond, self.ts_micros),
            (TimeUnit::Nanosecond, self.ts_nanos),
        ]
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(unit, _)| unit)
        .unwrap_or(TimeUnit::Millisecond)
    }

    fn timestamp_type(&self) -> Option<DataType> {
        let total_ts = self.ts_seconds + self.ts_millis + self.ts_micros + self.ts_nanos;
        // A purely numeric column (epoch-looking integers) stays integer unless
        // at least one value is an unambiguous textual datetime.
        let also_integer = self.is_confident(self.int_ok);
        let accepted = (self.ts_textual > 0 || !also_integer) && self.is_confident(total_ts);
        accepted.then(|| DataType::Timestamp(self.choose_timestamp_unit(), None))
    }

    fn integer_type(&self) -> Option<DataType> {
        if !self.is_confident(self.int_ok) {
            return None;
        }
        if self.min_value >= i64::MIN as i128 && self.max_value <= i64::MAX as i128 {
            return Some(DataType::Int64);
        }
        if self.min_value >= 0 && self.max_value <= u64::MAX as i128 {
            return Some(DataType::UInt64);
        }
        None
    }

    fn finalize(&self) -> DataType {
        if self.total_values == 0 {
            return DataType::LargeUtf8;
        }
        if let Some(t) = self.timestamp_type() {
            return t;
        }
        if self.is_confident(self.date_ok) {
            return DataType::Date32;
        }
        if self.is_confident(self.bool_ok) {
            return DataType::Boolean;
        }
        if let Some(t) = self.integer_type() {
            return t;
        }
        if self.is_confident(self.float_ok) {
            return DataType::Float64;
        }
        DataType::LargeUtf8
    }
}

/// Unit implied by the fractional-second digits of a textual timestamp.
fn fractional_unit(nanos: u32) -> TimeUnit {
    if nanos.is_multiple_of(1_000_000) {
        TimeUnit::Millisecond
    } else if nanos.is_multiple_of(1_000) {
        TimeUnit::Microsecond
    } else {
        TimeUnit::Nanosecond
    }
}

fn numeric_unit(x: i128) -> TimeUnit {
    let abs = x.abs();
    if abs < 100_000_000_000 {
        TimeUnit::Second
    } else if abs < 100_000_000_000_000 {
        TimeUnit::Millisecond
    } else if abs < 100_000_000_000_000_000 {
        TimeUnit::Microsecond
    } else {
        TimeUnit::Nanosecond
    }
}

fn detect_timestamp_unit(value: &str) -> Option<(TimeUnit, TimestampFormat)> {
    let v = value.trim();
    if let Some(dt) = parse_textual_datetime(v) {
        return Some((fractional_unit(dt.nanosecond()), TimestampFormat::Textual));
    }
    let x = v.parse::<i128>().ok()?;
    Some((numeric_unit(x), TimestampFormat::Numeric))
}

pub fn infer_schema<P: AsRef<Path>>(
    path: P,
    delimiter: u8,
    full_scan: bool,
    has_header: bool,
) -> Result<Schema> {
    let path_ref = path.as_ref();
    let mut file = BufReader::new(
        File::open(path_ref).with_context(|| format!("cannot open {}", path_ref.display()))?,
    );
    let mut reader = csv_reader(&mut file, delimiter, has_header);
    let column_names = read_column_names(&mut reader, has_header)?;
    info!("[INFERENCE] columns: {}", column_names.len());
    drop(reader);
    file.seek(SeekFrom::Start(0))?;
    let mut reader = csv_reader(&mut file, delimiter, has_header);
    let states = sample_column_states(&mut reader, column_names.len(), full_scan);
    let fields: Vec<Field> = column_names
        .iter()
        .zip(states.iter())
        .map(|(name, state)| {
            let dtype = state.finalize();
            info!("[FINAL TYPE] '{name}' -> {dtype:?}");
            Field::new(name, dtype, true)
        })
        .collect();
    Ok(Schema::new(fields))
}

fn csv_reader<R: Read>(reader: &mut R, delimiter: u8, has_header: bool) -> csv::Reader<&mut R> {
    ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(has_header)
        .flexible(true)
        .from_reader(reader)
}

fn read_column_names<R: Read>(
    reader: &mut csv::Reader<R>,
    has_header: bool,
) -> Result<Vec<String>> {
    if has_header {
        Ok(reader.headers()?.iter().map(str::to_string).collect())
    } else {
        let first = reader
            .records()
            .next()
            .transpose()?
            .context("empty input file")?;
        Ok(generate_column_names(first.len()))
    }
}

fn sample_column_states<R: Read>(
    reader: &mut csv::Reader<R>,
    column_count: usize,
    full_scan: bool,
) -> Vec<TypeState> {
    let mut states: Vec<TypeState> = (0..column_count).map(|_| TypeState::new()).collect();
    for (i, record) in reader.records().enumerate() {
        if !full_scan && i >= SCHEMA_SAMPLE_ROWS {
            debug!("[INFERENCE] sampling stopped at {SCHEMA_SAMPLE_ROWS} rows");
            break;
        }
        let Ok(record) = record else { continue };
        for (column, value) in record.iter().enumerate() {
            if let Some(state) = states.get_mut(column) {
                state.observe(value);
            }
        }
    }
    states
}

pub fn force_schema_to_utf8(schema: &Schema) -> Schema {
    let fields: Vec<Field> = schema
        .fields()
        .iter()
        .map(|f| Field::new(f.name(), DataType::LargeUtf8, true))
        .collect();
    Schema::new(fields)
}
