// ============================================================
use chrono::{Duration, NaiveDate, NaiveDateTime};
use rand::distr::{Alphanumeric, Distribution};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use rayon::prelude::*;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::sync::{Arc, Mutex};
const ROW_COUNT: usize = 100_000;
const COLUMN_COUNT: usize = 24;
const ERROR_RATE: f64 = 0.01;
const NULL_RATE: f64 = 0.03;
const OUTPUT_DIR: &str = "datasets_tests";
fn main() {
    create_dir_all(OUTPUT_DIR).unwrap();
    let files = Arc::new(Mutex::new(Vec::<String>::new()));
    generate_dataset("noisy_csv", ',', "csv", Mode::Noisy, &files);
    generate_dataset("noisy_tsv", '\t', "tsv", Mode::Noisy, &files);
    generate_dataset("clean_csv", ',', "csv", Mode::Clean, &files);
    generate_dataset("clean_tsv", '\t', "tsv", Mode::Clean, &files);
    generate_dataset(
        "low_cardinality",
        ',',
        "csv",
        Mode::LowCardinality,
        &files,
    );
    generate_dataset(
        "high_cardinality",
        ',',
        "csv",
        Mode::HighCardinality,
        &files,
    );
    generate_dataset("constant_columns", ',', "csv", Mode::Constant, &files);
    generate_dataset("correlated_columns", ',', "csv", Mode::Correlated, &files);
    generate_dataset("distribution_skew", ',', "csv", Mode::Skew, &files);
    generate_dataset("distribution_uniform", ',', "csv", Mode::Uniform, &files);
    display_files(&files);
}
#[derive(Clone, Copy)]
enum Mode {
    Noisy,
    Clean,
    LowCardinality,
    HighCardinality,
    Constant,
    Correlated,
    Skew,
    Uniform,
}
fn generate_dataset(
    name: &str,
    delim: char,
    extension: &str,
    mode: Mode,
    files: &Arc<Mutex<Vec<String>>>,
) {
    let path = format!("{}/{}.{}", OUTPUT_DIR, name, extension);
    println!("generating {}", path);
    let file = File::create(&path).unwrap();
    let mut writer = BufWriter::with_capacity(64 * 1024 * 1024, file);
    write_header(&mut writer, delim);
    let lines: Vec<String> = (0..ROW_COUNT)
        .into_par_iter()
        .map(|i| generate_row(i, delim, mode))
        .collect();
    for row in lines {
        writer.write_all(row.as_bytes()).unwrap();
        writer.write_all(b"\n").unwrap();
    }
    writer.flush().unwrap();
    files.lock().unwrap().push(path);
}
fn display_files(files: &Arc<Mutex<Vec<String>>>) {
    println!();
    println!("generated files:");
    println!("------------------");
    let files = files.lock().unwrap();
    for f in files.iter() {
        println!("{}", f);
    }
}
fn write_header(writer: &mut BufWriter<File>, delim: char) {
    for i in 0..COLUMN_COUNT {
        write!(writer, "col_{}", i).unwrap();
        if i != COLUMN_COUNT - 1 {
            writer.write_all(&[delim as u8]).unwrap();
        }
    }
    writer.write_all(b"\n").unwrap();
}
fn generate_row(index: usize, delim: char, mode: Mode) -> String {
    let mut rng = StdRng::seed_from_u64(index as u64);
    let mut row = String::with_capacity(4096);
    for col in 0..COLUMN_COUNT {
        let value = generate_value(col, index, &mut rng, mode);
        row.push_str(&value);
        if col != COLUMN_COUNT - 1 {
            row.push(delim);
        }
    }
    row
}
fn generate_value(col: usize, index: usize, rng: &mut StdRng, mode: Mode) -> String {
    match mode {
        Mode::Clean => generate_clean(col, index, rng),
        Mode::Noisy => generate_noisy(col, index, rng),
        Mode::LowCardinality => {
            format!("VAL{}", index % 10)
        }
        Mode::HighCardinality => {
            format!("VAL{}", index)
        }
        Mode::Constant => {
            format!("CONST{}", col)
        }
        Mode::Correlated => {
            format!("{}", index * col)
        }
        Mode::Skew => {
            if rng.random_bool(0.9) {
                "A".into()
            } else {
                format!("B{}", index)
            }
        }
        Mode::Uniform => {
            format!("{}", rng.random_range(0..1_000_000))
        }
    }
}
fn generate_noisy(col: usize, index: usize, rng: &mut StdRng) -> String {
    if rng.random_bool(NULL_RATE) {
        return "".into();
    }
    if rng.random_bool(ERROR_RATE) {
        return format!("ERR{}", index);
    }
    generate_clean(col, index, rng)
}
fn generate_clean(col: usize, index: usize, rng: &mut StdRng) -> String {
    match col % 8 {
        0 => generate_int(index),
        1 => generate_uint(index),
        2 => generate_float(index),
        3 => generate_bool(index),
        4 => generate_date(index),
        5 => generate_timestamp(index),
        6 => generate_string(rng),
        _ => generate_mixed(index),
    }
}
fn generate_int(index: usize) -> String {
    format!("{}", index as i64 - 500_000)
}
fn generate_uint(index: usize) -> String {
    format!("{}", index as u64)
}
fn generate_float(index: usize) -> String {
    format!("{:.6}", index as f64 * 0.1)
}
fn generate_bool(index: usize) -> String {
    if index.is_multiple_of(2) {
        "true".into()
    } else {
        "false".into()
    }
}
fn generate_date(index: usize) -> String {
    let base = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
    let date = base + Duration::days(index as i64);
    date.format("%Y-%m-%d").to_string()
}
fn generate_timestamp(index: usize) -> String {
    let base = NaiveDateTime::parse_from_str("2020-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let ts = base + Duration::seconds(index as i64);
    ts.format("%Y-%m-%d %H:%M:%S").to_string()
}
fn generate_string(rng: &mut StdRng) -> String {
    Alphanumeric
        .sample_iter(rng)
        .take(16)
        .map(char::from)
        .collect()
}
fn generate_mixed(index: usize) -> String {
    format!("DRIFT{}", index)
}
