// ============================================================
// src/utils.rs
use anyhow::{Context, Result};
use arrow::datatypes::TimeUnit;
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use colored::Colorize;
use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const TIMEZONE_DATETIME_FORMATS: [&str; 8] = [
    "%Y-%m-%d %H:%M:%S%:z",
    "%Y-%m-%d %H:%M:%S%.f%:z",
    "%Y-%m-%dT%H:%M:%S%:z",
    "%Y-%m-%dT%H:%M:%S%.f%:z",
    "%Y-%m-%d %H:%M:%S%z",
    "%Y-%m-%d %H:%M:%S%.f%z",
    "%Y-%m-%dT%H:%M:%S%z",
    "%Y-%m-%dT%H:%M:%S%.f%z",
];
const NAIVE_DATETIME_FORMATS: [&str; 6] = [
    "%Y-%m-%d %H:%M:%S%.f",
    "%Y-%m-%dT%H:%M:%S%.f",
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%dT%H:%M:%S",
    "%d/%m/%Y %H:%M:%S",
    "%Y/%m/%d %H:%M:%S",
];
const DATE_FORMATS: [&str; 3] = ["%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y"];
/// Plausibility window for integer epochs, in seconds: 2001-09-09 .. 2096-10-02.
const EPOCH_SECONDS_RANGE: std::ops::Range<i128> = 1_000_000_000..4_000_000_000;

fn colors_enabled() -> bool {
    io::stdout().is_terminal() && io::stderr().is_terminal()
}

pub fn error(msg: impl std::fmt::Display) -> String {
    let s = msg.to_string();
    if colors_enabled() {
        s.red().bold().to_string()
    } else {
        s
    }
}

pub fn warning(msg: impl std::fmt::Display) -> String {
    let s = msg.to_string();
    if colors_enabled() {
        s.yellow().to_string()
    } else {
        s
    }
}

pub fn success(msg: impl std::fmt::Display) -> String {
    let s = msg.to_string();
    if colors_enabled() {
        s.green().to_string()
    } else {
        s
    }
}

pub fn path(p: &Path) -> String {
    let s = p.display().to_string();
    if colors_enabled() {
        s.cyan().to_string()
    } else {
        s
    }
}

pub fn is_null_text(v: &str) -> bool {
    let t = v.trim();
    if t.is_empty() {
        return true;
    }
    matches!(
        t.to_ascii_lowercase().as_str(),
        "null" | "none" | "nan" | "n/a" | "na" | "nd" | "nr" | "-" | "--"
    )
}

pub fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "t" | "y" | "yes" | "on" => Some(true),
        "false" | "0" | "f" | "n" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub fn parse_date_ymd(v: &str) -> Option<i32> {
    let t = v.trim();
    if t.is_empty() {
        return None;
    }
    let date = DATE_FORMATS
        .iter()
        .find_map(|f| NaiveDate::parse_from_str(t, f).ok())?;
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)?;
    i32::try_from((date - epoch).num_days()).ok()
}

/// Parse a textual datetime: RFC3339, explicit offset, or naive interpreted as UTC.
pub fn parse_textual_datetime(v: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(v) {
        return Some(dt.with_timezone(&Utc));
    }
    for format in TIMEZONE_DATETIME_FORMATS {
        if let Ok(dt) = DateTime::parse_from_str(v, format) {
            return Some(dt.with_timezone(&Utc));
        }
    }
    for format in NAIVE_DATETIME_FORMATS {
        if let Ok(dt) = NaiveDateTime::parse_from_str(v, format) {
            return Some(Utc.from_utc_datetime(&dt));
        }
    }
    None
}

fn ticks_per_second(unit: TimeUnit) -> i128 {
    match unit {
        TimeUnit::Second => 1,
        TimeUnit::Millisecond => 1_000,
        TimeUnit::Microsecond => 1_000_000,
        TimeUnit::Nanosecond => 1_000_000_000,
    }
}

/// Source unit of an integer epoch, accepted only within the plausibility window.
fn integer_epoch_unit(x: i128) -> Option<TimeUnit> {
    [
        TimeUnit::Second,
        TimeUnit::Millisecond,
        TimeUnit::Microsecond,
        TimeUnit::Nanosecond,
    ]
    .into_iter()
    .find(|unit| {
        let scale = ticks_per_second(*unit);
        (EPOCH_SECONDS_RANGE.start * scale..EPOCH_SECONDS_RANGE.end * scale).contains(&x)
    })
}

fn rescale_epoch(x: i128, from: TimeUnit, to: TimeUnit) -> Option<i64> {
    let from_scale = ticks_per_second(from);
    let to_scale = ticks_per_second(to);
    let value = if to_scale >= from_scale {
        x.checked_mul(to_scale / from_scale)?
    } else {
        x / (from_scale / to_scale)
    };
    i64::try_from(value).ok()
}

fn datetime_in_unit(dt: &DateTime<Utc>, unit: TimeUnit) -> Option<i64> {
    match unit {
        TimeUnit::Second => Some(dt.timestamp()),
        TimeUnit::Millisecond => Some(dt.timestamp_millis()),
        TimeUnit::Microsecond => Some(dt.timestamp_micros()),
        TimeUnit::Nanosecond => dt.timestamp_nanos_opt(),
    }
}

/// Parse a timestamp string into ticks of `unit`, preserving native precision.
pub fn parse_timestamp(v: &str, unit: TimeUnit) -> Option<i64> {
    let t = v.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(dt) = parse_textual_datetime(t) {
        return datetime_in_unit(&dt, unit);
    }
    let x = t.parse::<i128>().ok()?;
    rescale_epoch(x, integer_epoch_unit(x)?, unit)
}

pub fn parse_timestamp_ms(v: &str) -> Option<i64> {
    parse_timestamp(v, TimeUnit::Millisecond)
}

pub fn detect_delimiter<P: AsRef<Path>>(path: P) -> Result<u8> {
    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    let candidates = [',', ';', '\t', '|'];
    let mut scores = vec![0usize; candidates.len()];
    for row in reader.lines().take(20) {
        let row = row?;
        for (i, c) in candidates.iter().enumerate() {
            scores[i] += row.matches(*c).count();
        }
    }
    let best = scores
        .iter()
        .enumerate()
        .max_by_key(|(_, score)| **score)
        .map(|(i, _)| candidates[i] as u8);
    Ok(best.unwrap_or(b','))
}

fn value_type_score(v: &str) -> f64 {
    let t = v.trim();
    if t.is_empty() {
        return 0.0;
    }
    if t.parse::<i64>().is_ok() {
        return 1.0;
    }
    if t.parse::<f64>().is_ok() {
        return 1.0;
    }
    if parse_bool(t).is_some() {
        return 0.8;
    }
    if parse_date_ymd(t).is_some() {
        return 1.0;
    }
    if parse_timestamp_ms(t).is_some() {
        return 1.0;
    }
    0.0
}

fn looks_like_identifier(v: &str) -> bool {
    let t = v.trim();
    if t.is_empty() || t.len() > 64 {
        return false;
    }
    t.chars().all(|c| {
        c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ' ' | '(' | ')' | '/' | '\'' | '+' | ':')
    })
}

struct SampleStats {
    average_score: f64,
    average_length: f64,
    repetition_rate: f64,
    line_count: usize,
}

pub fn detect_header<P: AsRef<Path>>(path: P, delimiter: u8) -> Result<bool> {
    let file = File::open(&path)?;
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(BufReader::new(file));
    let mut records = reader.records();
    let first_line = match records.next() {
        Some(Ok(r)) => r,
        _ => return Ok(false),
    };
    let column_count = first_line.len();
    if column_count == 0 {
        return Ok(false);
    }
    let first_values: Vec<String> = first_line.iter().map(|s| s.to_string()).collect();
    let first_score: f64 = first_values
        .iter()
        .map(|v| value_type_score(v))
        .sum::<f64>()
        / column_count as f64;
    let first_length: usize = first_values.iter().map(String::len).sum();
    let stats = sample_data_lines(&mut records, &first_values, column_count);
    if stats.line_count == 0 {
        // Single-line file: possible header only if every value looks like an
        // identifier and at least one has no digits (pure text → column name).
        let all_identifiers = first_values.iter().all(|v| looks_like_identifier(v));
        let has_pure_text = first_values
            .iter()
            .any(|v| !v.trim().chars().any(|c| c.is_ascii_digit()));
        return Ok(all_identifiers && has_pure_text);
    }
    // If the first line is substantially longer than the data (descriptive column names
    // over compact values), it is most likely a header.
    if (first_length as f64) > stats.average_length * 2.0
        && first_values.iter().all(|v| looks_like_identifier(v))
    {
        return Ok(true);
    }
    if first_score < stats.average_score - 0.2 {
        return Ok(true);
    }
    Ok(header_heuristics(&first_values, &stats))
}

fn sample_data_lines(
    records: &mut csv::StringRecordsIter<'_, BufReader<File>>,
    first_values: &[String],
    column_count: usize,
) -> SampleStats {
    const MAX_SAMPLE: usize = 200;
    let mut score_sum = 0.0;
    let mut length_sum = 0.0f64;
    let mut repetitions = 0usize;
    let mut line_count = 0usize;
    for record in records.take(MAX_SAMPLE) {
        let Ok(r) = record else { continue };
        score_sum += r.iter().map(value_type_score).sum::<f64>() / column_count.max(1) as f64;
        length_sum += r.iter().map(str::len).sum::<usize>() as f64;
        if r.iter()
            .zip(first_values)
            .any(|(v, p)| !p.is_empty() && v == p.as_str())
        {
            repetitions += 1;
        }
        line_count += 1;
    }
    let denominator = line_count.max(1) as f64;
    SampleStats {
        average_score: score_sum / denominator,
        average_length: length_sum / denominator,
        repetition_rate: repetitions as f64 / denominator,
        line_count,
    }
}

fn header_heuristics(first_values: &[String], stats: &SampleStats) -> bool {
    let all_identifiers = first_values.iter().all(|v| looks_like_identifier(v));
    let mut unique_values = first_values.to_vec();
    unique_values.sort();
    unique_values.dedup();
    let all_unique = unique_values.len() == first_values.len();
    let first_length: usize = first_values.iter().map(String::len).sum();
    let first_is_shorter = (first_length as f64) < stats.average_length * 0.7;
    let few_repetitions = stats.repetition_rate < 0.05;
    // If every non-empty value in the first line parses as a number, it is data, not a header.
    let all_numeric = first_values
        .iter()
        .all(|v| v.trim().is_empty() || v.trim().parse::<i64>().is_ok());
    all_identifiers && all_unique && first_is_shorter && few_repetitions && !all_numeric
}

pub fn generate_column_names(nb: usize) -> Vec<String> {
    (0..nb).map(|i| format!("col_{i}")).collect()
}

/// Strip known compression extension(s), yielding the underlying base name.
/// "data.csv.gz" → "data.csv", "data.parquet.zst" → "data.parquet"
pub fn strip_compression_ext(name: &str) -> String {
    let path = Path::new(name);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "gz" | "gzip" | "zst" | "zstd" | "bz2" | "bzip2" | "xz" | "lzma" => {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                strip_compression_ext(stem)
            } else {
                name.to_string()
            }
        }
        _ => name.to_string(),
    }
}

/// If the file path indicates a compressed format, decompress it to a
/// temporary file and return `(temp_path, Some(temp_guard))`.
/// Otherwise return `(path, None)`.
pub fn decompress_if_needed(path: &Path) -> Result<(PathBuf, Option<NamedTempFile>)> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "gz" | "gzip" => {
            let file = open_compressed(path)?;
            decompress_to_temp(GzDecoder::new(BufReader::new(file)), "gzip")
        }
        "zst" | "zstd" => {
            let file = open_compressed(path)?;
            decompress_to_temp(zstd::Decoder::new(file)?, "zstd")
        }
        "bz2" | "bzip2" | "xz" | "lzma" => anyhow::bail!(
            "{} {} format not supported directly. Use stdin: {} | convert_to_parquet -",
            warning("Hint:"),
            ext,
            if matches!(ext.as_str(), "bz2" | "bzip2") {
                "bzcat file"
            } else {
                "xzcat file"
            }
        ),
        _ => Ok((path.to_path_buf(), None)),
    }
}

fn open_compressed(path: &Path) -> Result<File> {
    File::open(path).with_context(|| format!("open compressed input {}", path.display()))
}

fn decompress_to_temp(
    mut decoder: impl Read,
    label: &str,
) -> Result<(PathBuf, Option<NamedTempFile>)> {
    let temp = NamedTempFile::new()?;
    let temp_path = temp.path().to_path_buf();
    let mut writer = BufWriter::new(temp.reopen()?);
    io::copy(&mut decoder, &mut writer)?;
    writer.flush()?;
    eprintln!(
        "{} Decompressed {label} -> temp file",
        "[OK]".green().bold()
    );
    Ok((temp_path, Some(temp)))
}
