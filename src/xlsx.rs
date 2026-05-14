// ============================================================
// src/xlsx.rs
use anyhow::{anyhow, Context, Result};
use calamine::{
    open_workbook, open_workbook_auto, Data, DataRef, Dimensions, HeaderRow, Range, Reader, Sheets,
    Xlsb, Xlsx,
};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
pub const XLSX_DELIMITER: u8 = b',';
const SUPPORTED_EXT: &[&str] = &["xlsx", "xls", "xlsm", "xlsb", "ods"];
#[derive(Clone, Copy)]
enum Backend {
    Xlsx,
    Xlsb,
    NonStreaming,
}
pub struct SheetCsvExport {
    pub sheet_name: String,
    pub csv_path: PathBuf,
    pub row_count: usize,
    _keep: NamedTempFile,
}
pub fn is_spreadsheet<P: AsRef<Path>>(path: P) -> bool {
    extension_lower(path.as_ref())
        .map(|e| SUPPORTED_EXT.contains(&e.as_str()))
        .unwrap_or(false)
}
pub fn list_sheet_names<P: AsRef<Path>>(path: P) -> Result<Vec<String>> {
    let workbook: Sheets<BufReader<File>> = open_workbook_auto(path.as_ref())
        .with_context(|| format!("open {}", path.as_ref().display()))?;
    let names = workbook.sheet_names().to_vec();
    if names.is_empty() {
        return Err(anyhow!("no sheets in {}", path.as_ref().display()));
    }
    Ok(names)
}
pub fn sanitize_sheet_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}
pub fn export_sheet_to_csv<P: AsRef<Path>>(path: P, sheet_name: &str) -> Result<SheetCsvExport> {
    let path_ref = path.as_ref().to_path_buf();
    let temp = NamedTempFile::new()?;
    let writer_file = temp.reopen()?;
    let mut csv_writer = csv::WriterBuilder::new()
        .delimiter(XLSX_DELIMITER)
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(BufWriter::with_capacity(1 << 20, writer_file));
    let row_count = match backend_for(&path_ref) {
        Backend::Xlsx => stream_xlsx(&path_ref, sheet_name, &mut csv_writer)?,
        Backend::Xlsb => stream_xlsb(&path_ref, sheet_name, &mut csv_writer)?,
        Backend::NonStreaming => write_range(&path_ref, sheet_name, &mut csv_writer)?,
    };
    csv_writer.flush()?;
    drop(csv_writer);
    let csv_path = temp.path().to_path_buf();
    Ok(SheetCsvExport {
        sheet_name: sheet_name.to_string(),
        csv_path,
        row_count,
        _keep: temp,
    })
}
fn extension_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}
fn backend_for(path: &Path) -> Backend {
    match extension_lower(path).as_deref() {
        Some("xlsx") | Some("xlsm") => Backend::Xlsx,
        Some("xlsb") => Backend::Xlsb,
        _ => Backend::NonStreaming,
    }
}
fn width_from_dimensions(d: Dimensions) -> usize {
    let cols = d.end.1.saturating_sub(d.start.1) + 1;
    (cols as usize).max(1)
}
fn stream_xlsx<W: Write>(
    path: &Path,
    sheet_name: &str,
    writer: &mut csv::Writer<W>,
) -> Result<usize> {
    let mut workbook: Xlsx<_> =
        open_workbook(path).with_context(|| format!("open xlsx {}", path.display()))?;
    workbook.with_header_row(HeaderRow::Row(0));
    let mut reader = workbook
        .worksheet_cells_reader(sheet_name)
        .with_context(|| format!("xlsx reader {sheet_name}"))?;
    let dimensions = reader.dimensions();
    stream_cells(
        dimensions,
        || {
            let cell = reader.next_cell().map_err(|e| anyhow!("xlsx cell: {e}"))?;
            Ok(cell.map(|c| {
                let (row, col) = c.get_position();
                (row, col, format_data_ref(c.get_value()))
            }))
        },
        writer,
    )
}
fn stream_xlsb<W: Write>(
    path: &Path,
    sheet_name: &str,
    writer: &mut csv::Writer<W>,
) -> Result<usize> {
    let mut workbook: Xlsb<_> =
        open_workbook(path).with_context(|| format!("open xlsb {}", path.display()))?;
    let mut reader = workbook
        .worksheet_cells_reader(sheet_name)
        .with_context(|| format!("xlsb reader {sheet_name}"))?;
    let dimensions = reader.dimensions();
    stream_cells(
        dimensions,
        || {
            let cell = reader.next_cell().map_err(|e| anyhow!("xlsb cell: {e}"))?;
            Ok(cell.map(|c| {
                let (row, col) = c.get_position();
                (row, col, format_data_ref(c.get_value()))
            }))
        },
        writer,
    )
}
/// Stream cells (row-major) into CSV rows covering the sheet's used range.
/// Rows without cells inside the range are emitted as empty rows, and column
/// indices are relative to the range start — matching `write_range` semantics.
fn stream_cells<W: Write>(
    dimensions: Dimensions,
    mut next_cell: impl FnMut() -> Result<Option<(u32, u32, String)>>,
    writer: &mut csv::Writer<W>,
) -> Result<usize> {
    let start_col = dimensions.start.1;
    let mut row_buf: Vec<String> = vec![String::new(); width_from_dimensions(dimensions)];
    let mut current_row = dimensions.start.0;
    let mut total_rows = 0usize;
    let mut saw_cell = false;
    while let Some((row, col, value)) = next_cell()? {
        saw_cell = true;
        while current_row < row {
            write_row(writer, &mut row_buf, &mut total_rows)?;
            current_row += 1;
        }
        let slot = col.checked_sub(start_col).map(|c| c as usize);
        if let Some(slot) = slot.and_then(|c| row_buf.get_mut(c)) {
            *slot = value;
        }
    }
    if saw_cell {
        write_row(writer, &mut row_buf, &mut total_rows)?;
    }
    Ok(total_rows)
}
fn write_row<W: Write>(
    writer: &mut csv::Writer<W>,
    row_buf: &mut [String],
    total_rows: &mut usize,
) -> Result<()> {
    writer.write_record(row_buf.iter().map(|s| s.as_str()))?;
    for value in row_buf.iter_mut() {
        value.clear();
    }
    *total_rows += 1;
    Ok(())
}
fn write_range<W: Write>(
    path: &Path,
    sheet_name: &str,
    writer: &mut csv::Writer<W>,
) -> Result<usize> {
    let mut workbook: Sheets<BufReader<File>> =
        open_workbook_auto(path).with_context(|| format!("open {}", path.display()))?;
    let range: Range<Data> = workbook
        .worksheet_range(sheet_name)
        .with_context(|| format!("read sheet {sheet_name}"))?;
    let width = range.width().max(1);
    let mut buf: Vec<String> = Vec::with_capacity(width);
    let mut total: usize = 0;
    for row in range.rows() {
        buf.clear();
        for cell in row {
            buf.push(format_data(cell));
        }
        writer.write_record(&buf)?;
        total += 1;
    }
    Ok(total)
}
fn format_data(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => format_float(*f),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Data::DateTime(dt) => format_serial(dt.as_f64()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("#ERR:{e:?}"),
    }
}
fn format_data_ref(cell: &DataRef) -> String {
    match cell {
        DataRef::Empty => String::new(),
        DataRef::String(s) => s.clone(),
        DataRef::SharedString(s) => s.to_string(),
        DataRef::Float(f) => format_float(*f),
        DataRef::Int(i) => i.to_string(),
        DataRef::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        DataRef::DateTime(dt) => format_serial(dt.as_f64()),
        DataRef::DateTimeIso(s) | DataRef::DurationIso(s) => s.clone(),
        DataRef::Error(e) => format!("#ERR:{e:?}"),
    }
}
fn format_float(f: f64) -> String {
    if f.is_nan() || f.is_infinite() {
        return f.to_string();
    }
    if f.fract() == 0.0 && f.abs() < 1e16 {
        format!("{}", f as i64)
    } else {
        f.to_string()
    }
}
fn format_serial(serial: f64) -> String {
    use chrono::{Duration, NaiveDate, NaiveDateTime};
    let epoch = match NaiveDate::from_ymd_opt(1899, 12, 30).and_then(|d| d.and_hms_opt(0, 0, 0)) {
        Some(e) => e,
        None => return serial.to_string(),
    };
    let days = serial.trunc() as i64;
    let millis = (serial.fract() * 86_400_000.0).round() as i64;
    let dt: NaiveDateTime = epoch + Duration::days(days) + Duration::milliseconds(millis);
    if serial.fract() == 0.0 {
        dt.date().to_string()
    } else {
        dt.format("%Y-%m-%dT%H:%M:%S").to_string()
    }
}
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;
    fn collect_stream(dimensions: Dimensions, cells: Vec<(u32, u32, &str)>) -> Vec<String> {
        let mut writer = csv::WriterBuilder::new().from_writer(vec![]);
        let mut iter = cells.into_iter();
        stream_cells(
            dimensions,
            || Ok(iter.next().map(|(r, c, v)| (r, c, v.to_string()))),
            &mut writer,
        )
        .unwrap();
        let bytes = writer.into_inner().unwrap();
        String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    }
    #[test]
    fn format_float_integer_like() {
        assert_eq!(format_float(0.0), "0");
        assert_eq!(format_float(42.0), "42");
        assert_eq!(format_float(-7.0), "-7");
    }
    #[test]
    fn format_float_decimal() {
        assert_eq!(format_float(1.5), "1.5");
        assert_eq!(format_float(-0.25), "-0.25");
    }
    #[test]
    fn format_float_non_finite() {
        assert_eq!(format_float(f64::NAN), f64::NAN.to_string());
        assert_eq!(format_float(f64::INFINITY), f64::INFINITY.to_string());
    }
    #[test]
    fn format_serial_date_only() {
        let s = format_serial(43831.0);
        assert_eq!(s, "2020-01-01");
    }
    #[test]
    fn format_serial_with_time() {
        let s = format_serial(43831.5);
        assert_eq!(s, "2020-01-01T12:00:00");
    }
    #[test]
    fn width_from_dimensions_basic() {
        let d = Dimensions {
            start: (0, 0),
            end: (10, 4),
        };
        assert_eq!(width_from_dimensions(d), 5);
    }
    #[test]
    fn width_from_dimensions_single_column() {
        let d = Dimensions {
            start: (0, 3),
            end: (10, 3),
        };
        assert_eq!(width_from_dimensions(d), 1);
    }
    #[test]
    fn backend_for_known_extensions() {
        assert!(matches!(backend_for(Path::new("a.xlsx")), Backend::Xlsx));
        assert!(matches!(backend_for(Path::new("a.xlsm")), Backend::Xlsx));
        assert!(matches!(backend_for(Path::new("a.xlsb")), Backend::Xlsb));
        assert!(matches!(
            backend_for(Path::new("a.xls")),
            Backend::NonStreaming
        ));
        assert!(matches!(
            backend_for(Path::new("a.ods")),
            Backend::NonStreaming
        ));
    }
    #[test]
    fn stream_cells_keeps_leading_and_middle_empty_rows() {
        let d = Dimensions {
            start: (0, 0),
            end: (2, 1),
        };
        let lines = collect_stream(d, vec![(2, 1, "v")]);
        assert_eq!(lines, vec![",", ",", ",v"]);
    }
    #[test]
    fn stream_cells_handles_column_offset() {
        let d = Dimensions {
            start: (0, 2),
            end: (1, 3),
        };
        let lines = collect_stream(d, vec![(0, 2, "a"), (0, 3, "b"), (1, 3, "d")]);
        assert_eq!(lines, vec!["a,b", ",d"]);
    }
    #[test]
    fn stream_cells_empty_sheet_writes_nothing() {
        let d = Dimensions {
            start: (0, 0),
            end: (0, 0),
        };
        let lines = collect_stream(d, vec![]);
        assert!(lines.is_empty());
    }
}
