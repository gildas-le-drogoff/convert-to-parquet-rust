// ============================================================
use csv_to_parquet::conversion::convert_csv_to_parquet;
use csv_to_parquet::xlsx::{export_sheet_to_csv, list_sheet_names};
use parquet::file::reader::{FileReader, SerializedFileReader};
use rust_xlsxwriter::Workbook;
use std::fs::File;
use std::path::PathBuf;
use tempfile::TempDir;

struct Fixture {
    _dir: TempDir,
    xlsx_path: PathBuf,
}

fn build_workbook(sheets: &[(&str, &[&[&str]])]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let xlsx_path = dir.path().join("sample.xlsx");
    let mut wb = Workbook::new();
    for (name, rows) in sheets {
        let ws = wb.add_worksheet().set_name(*name).unwrap();
        for (r, row) in rows.iter().enumerate() {
            for (c, value) in row.iter().enumerate() {
                ws.write_string(r as u32, c as u16, *value).unwrap();
            }
        }
    }
    wb.save(&xlsx_path).unwrap();
    Fixture {
        _dir: dir,
        xlsx_path,
    }
}

fn count_parquet_rows(path: &PathBuf) -> usize {
    let file = File::open(path).unwrap();
    let reader = SerializedFileReader::new(file).unwrap();
    reader
        .metadata()
        .row_groups()
        .iter()
        .map(|rg| rg.num_rows() as usize)
        .sum()
}

#[test]
fn list_sheets_returns_all_names() {
    let fx = build_workbook(&[
        ("Alpha", &[&["a", "b"], &["1", "2"]]),
        ("Beta", &[&["x", "y"], &["3", "4"]]),
    ]);
    let names = list_sheet_names(&fx.xlsx_path).unwrap();
    assert_eq!(names, vec!["Alpha".to_string(), "Beta".to_string()]);
    drop(fx);
}

#[test]
fn export_sheet_produces_streamable_csv() {
    let fx = build_workbook(&[(
        "Data",
        &[
            &["id", "label", "value"],
            &["1", "alpha", "10"],
            &["2", "beta", "20"],
            &["3", "gamma", "30"],
        ],
    )]);
    let export = export_sheet_to_csv(&fx.xlsx_path, "Data").unwrap();
    assert_eq!(export.sheet_name, "Data");
    assert_eq!(export.row_count, 4);
    let csv = std::fs::read_to_string(&export.csv_path).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0], "id,label,value");
    assert_eq!(lines[3], "3,gamma,30");
    drop(fx);
}

#[test]
fn roundtrip_xlsx_to_parquet_row_count_matches() {
    let rows_count = 256usize;
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(rows_count + 1);
    rows.push(vec!["idx".into(), "name".into(), "amount".into()]);
    for i in 0..rows_count {
        rows.push(vec![
            i.to_string(),
            format!("row_{i}"),
            format!("{}.5", i * 2),
        ]);
    }
    let dir = tempfile::tempdir().unwrap();
    let xlsx_path = dir.path().join("sample.xlsx");
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet().set_name("Big").unwrap();
    for (r, row) in rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            ws.write_string(r as u32, c as u16, v).unwrap();
        }
    }
    wb.save(&xlsx_path).unwrap();
    let export = export_sheet_to_csv(&xlsx_path, "Big").unwrap();
    let parquet_path = dir.path().join("Big.parquet");
    convert_csv_to_parquet(&export.csv_path, &parquet_path, false, false, false, None).unwrap();
    assert_eq!(count_parquet_rows(&parquet_path), rows_count);
}

#[test]
fn export_handles_sparse_cells() {
    let dir = tempfile::tempdir().unwrap();
    let xlsx_path = dir.path().join("sparse.xlsx");
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet().set_name("Sparse").unwrap();
    ws.write_string(0, 0, "h0").unwrap();
    ws.write_string(0, 2, "h2").unwrap();
    ws.write_string(2, 1, "v1").unwrap();
    wb.save(&xlsx_path).unwrap();
    let export = export_sheet_to_csv(&xlsx_path, "Sparse").unwrap();
    let csv = std::fs::read_to_string(&export.csv_path).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "h0,,h2");
    assert_eq!(lines[1], ",,");
    assert_eq!(lines[2], ",v1,");
}
// ============================================================
