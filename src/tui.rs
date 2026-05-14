// ============================================================
// src/tui.rs — Interactive Parquet viewer (ratatui)
//
// Launched on a `.parquet` input in a terminal: previews a bounded
// sample of rows in a scrollable table and exports the full file to
// CSV / JSONL / JSON / XLSX on a single keypress.
use crate::export::ExportFormat;
use anyhow::{Context, Result};
use arrow::util::display::{ArrayFormatter, FormatOptions};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;
use std::fs::File;
use std::path::Path;

const PREVIEW_ROWS: usize = 1000;
const CELL_MAX: usize = 40;
const PAGE_STEP: usize = 20;

/// In-memory preview: bounded rows plus full-file counts for the header.
struct Preview {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    widths: Vec<u16>,
    total_rows: usize,
    file_size: u64,
}

fn load_preview(file: &Path) -> Result<Preview> {
    let handle =
        File::open(file).with_context(|| format!("open parquet {}", file.display()))?;
    let file_size = handle.metadata().context("read file metadata")?.len();
    let builder =
        ParquetRecordBatchReaderBuilder::try_new(handle).context("create parquet reader")?;
    let total_rows = builder.metadata().file_metadata().num_rows().max(0) as usize;
    let columns: Vec<String> = builder
        .schema()
        .fields()
        .iter()
        .map(|field| field.name().clone())
        .collect();
    let reader = builder.build().context("build parquet reader")?;

    let options = FormatOptions::default().with_null("");
    let mut rows: Vec<Vec<String>> = Vec::new();
    'outer: for batch_result in reader {
        let batch = batch_result.context("read parquet row group")?;
        let formatters: Vec<ArrayFormatter> = batch
            .columns()
            .iter()
            .map(|array| ArrayFormatter::try_new(array.as_ref(), &options))
            .collect::<std::result::Result<_, _>>()
            .context("build column formatter")?;
        for row in 0..batch.num_rows() {
            let cells = formatters
                .iter()
                .map(|formatter| truncate(&formatter.value(row).to_string()))
                .collect();
            rows.push(cells);
            if rows.len() >= PREVIEW_ROWS {
                break 'outer;
            }
        }
    }

    let widths = column_widths(&columns, &rows);
    Ok(Preview {
        columns,
        rows,
        widths,
        total_rows,
        file_size,
    })
}

/// Truncate a display cell to `CELL_MAX` chars, marking elision.
fn truncate(value: &str) -> String {
    if value.chars().count() <= CELL_MAX {
        return value.to_string();
    }
    let kept: String = value.chars().take(CELL_MAX - 1).collect();
    format!("{kept}…")
}

/// Per-column display width: max of header and sampled cells, capped.
fn column_widths(columns: &[String], rows: &[Vec<String>]) -> Vec<u16> {
    columns
        .iter()
        .enumerate()
        .map(|(col, name)| {
            let cell_max = rows
                .iter()
                .filter_map(|r| r.get(col))
                .map(|c| c.chars().count())
                .max()
                .unwrap_or(0);
            cell_max.max(name.chars().count()).clamp(3, CELL_MAX) as u16
        })
        .collect()
}

/// Viewer state: scroll position and last export status.
struct App {
    preview: Preview,
    state: TableState,
    selected: usize,
    col_offset: usize,
    status: String,
}

impl App {
    fn new(preview: Preview) -> Self {
        App {
            preview,
            state: TableState::default(),
            selected: 0,
            col_offset: 0,
            status: String::new(),
        }
    }

    fn last_row(&self) -> usize {
        self.preview.rows.len().saturating_sub(1)
    }

    fn last_col(&self) -> usize {
        self.preview.columns.len().saturating_sub(1)
    }

    /// Returns true when the viewer should quit.
    fn handle_key(
        &mut self,
        code: KeyCode,
        file: &Path,
        resolve_output: &dyn Fn(ExportFormat) -> std::path::PathBuf,
    ) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.last_row());
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.selected = (self.selected + PAGE_STEP).min(self.last_row());
            }
            KeyCode::PageUp => {
                self.selected = self.selected.saturating_sub(PAGE_STEP);
            }
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = self.last_row(),
            KeyCode::Right | KeyCode::Char('l') => {
                self.col_offset = (self.col_offset + 1).min(self.last_col());
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.col_offset = self.col_offset.saturating_sub(1);
            }
            KeyCode::Char(c) => {
                if let Some(format) = ExportFormat::from_hotkey(c) {
                    self.export(format, file, resolve_output);
                }
            }
            _ => {}
        }
        false
    }

    fn export(
        &mut self,
        format: ExportFormat,
        file: &Path,
        resolve_output: &dyn Fn(ExportFormat) -> std::path::PathBuf,
    ) {
        let output = resolve_output(format);
        self.status = match format.convert(file, &output) {
            Ok(rows) => format!(
                "Exported {rows} rows -> {} ({})",
                output.display(),
                format.label()
            ),
            Err(error) => format!("Export failed ({}): {error:#}", format.label()),
        };
    }

    fn render(&mut self, frame: &mut Frame) {
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Fill(1),
            Constraint::Length(2),
        ])
        .areas(frame.area());
        self.render_header(frame, header);
        self.render_table(frame, body);
        self.render_footer(frame, footer);
    }

    fn render_header(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let shown = self.preview.rows.len();
        let truncated = if self.preview.total_rows > shown {
            format!(" (preview {shown})")
        } else {
            String::new()
        };
        let lines = vec![
            Line::from(format!(
                "rows {}{}   columns {}   size {}",
                self.preview.total_rows,
                truncated,
                self.preview.columns.len(),
                format_bytes(self.preview.file_size),
            )),
            Line::from(format!(
                "row {}/{}   col {}/{}",
                (self.selected + 1).min(shown.max(1)),
                shown,
                (self.col_offset + 1).min(self.preview.columns.len().max(1)),
                self.preview.columns.len(),
            )),
        ];
        let block = Block::bordered().title("Parquet viewer");
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_table(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let offset = self.col_offset;
        let header = Row::new(
            self.preview.columns[offset..]
                .iter()
                .map(|name| Cell::from(name.as_str())),
        )
        .style(Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED));
        let rows = self.preview.rows.iter().map(|cells| {
            Row::new(cells[offset.min(cells.len())..].iter().map(String::as_str))
        });
        let widths: Vec<Constraint> = self.preview.widths[offset..]
            .iter()
            .map(|w| Constraint::Length(*w))
            .collect();
        let table = Table::new(rows, widths)
            .header(header)
            .column_spacing(2)
            .row_highlight_style(Style::default().reversed())
            .block(Block::bordered());
        self.state.select(Some(self.selected));
        frame.render_stateful_widget(table, area, &mut self.state);
    }

    fn render_footer(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let keys = ExportFormat::ALL
            .iter()
            .map(|f| format!("[{}]{}", f.hotkey(), f.label()))
            .collect::<Vec<_>>()
            .join("  ");
        let lines = vec![
            Line::from(format!("Export: {keys}   [q]uit")),
            Line::from(self.status.as_str()).style(Style::default().add_modifier(Modifier::DIM)),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Run the interactive viewer until the user quits.
/// `resolve_output` maps a chosen format to its output path (honouring `--output`).
pub fn run_viewer(
    file: &Path,
    resolve_output: &dyn Fn(ExportFormat) -> std::path::PathBuf,
) -> Result<()> {
    let mut app = App::new(load_preview(file)?);
    let terminal = ratatui::try_init().context("init terminal")?;
    let result = drive(terminal, &mut app, file, resolve_output);
    ratatui::restore();
    result
}

fn drive(
    mut terminal: ratatui::DefaultTerminal,
    app: &mut App,
    file: &Path,
    resolve_output: &dyn Fn(ExportFormat) -> std::path::PathBuf,
) -> Result<()> {
    loop {
        terminal.draw(|frame| app.render(frame)).context("draw")?;
        match event::read().context("read event")? {
            Event::Key(key)
                if key.kind == KeyEventKind::Press
                    && app.handle_key(key.code, file, resolve_output) =>
            {
                return Ok(());
            }
            _ => {}
        }
    }
}
