// ============================================================
// src/main.rs — CLI entry point
use anyhow::{anyhow, Context, Result};
use clap::{CommandFactory, Parser};
use clap_mangen::Man;
use convert_to_parquet::conversion::{convert_convert_to_parquet, verify_parquet_schema};
use convert_to_parquet::export::ExportFormat;
use convert_to_parquet::inspect::{display_parquet_stats, is_parquet};
use convert_to_parquet::json::{export_json_to_csv, is_json, stdin_suffix};
use convert_to_parquet::json_arrow::{convert_json_to_parquet, json_has_nested};
use convert_to_parquet::tui::run_viewer;
use convert_to_parquet::utils::{
    decompress_if_needed, error, path, strip_compression_ext, success, warning,
};
use convert_to_parquet::xlsx::{
    export_sheet_to_csv, is_spreadsheet, list_sheet_names, sanitize_sheet_name,
};
use log::info;
use rayon::prelude::*;
use std::fs::File;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::Builder;

#[derive(Parser, Debug)]
#[command(
    name = "convert_to_parquet",
    version,
    about = "Convert CSV/TSV/XLSX/JSON/Parquet",
    long_about = r#"Convert CSV/TSV, spreadsheets (xlsx/xlsm/xlsb/xls/ods), JSON/JSONL to Parquet,
or convert Parquet back to CSV (--to-csv) or JSONL (--to-jsonl).

Running on a Parquet file without an export flag opens an interactive
viewer (data preview + export to CSV/JSONL/JSON/XLSX) when run in a
terminal, or prints schema and statistics when output is redirected.

Supports glob patterns (convert_to_parquet *.csv), compressed input (.gz, .zst),
and forced delimiter override (--delimiter).

Spreadsheets produce one Parquet per sheet: <stem>__<sheet>.parquet
Sheets are processed in parallel; xlsx/xlsm/xlsb are streamed cell-by-cell.
JSON sources: array of objects, single object, or NDJSON/JSONL (one object per line)."#
)]
struct CommandInterface {
    #[arg(
        long,
        help = "Infer schema from the entire file instead of the first 10000 rows"
    )]
    full_schema_inference: bool,
    #[arg(long, help = "Display the logical and physical schema of a Parquet file")]
    view_schema: bool,
    #[arg(
        long,
        help = "Force all columns to LargeUtf8 (disable inference, preserve raw data)"
    )]
    force_utf8: bool,
    #[arg(long, help = "Generate the man page (roff) on stdout")]
    man: bool,
    #[arg(
        short = 'o',
        long,
        value_name = "OUTPUT",
        help = "Output file or directory (with multiple inputs, treated as directory)"
    )]
    output: Option<String>,
    #[arg(
        short = 'd',
        long,
        value_name = "DELIM",
        help = "Force delimiter character: ',' ';' '\\t' '|' (bypasses auto-detection)"
    )]
    delimiter: Option<String>,
    #[arg(
        long,
        help = "Treat the first line as column headers, skipping automatic header detection"
    )]
    force_header: bool,
    #[arg(long, default_value_t = default_sheet_concurrency(), value_name = "N",
          help = "Concurrent sheets (XLSX) [default: ncpu/2]")]
    sheet_concurrency: usize,
    #[arg(
        long,
        help = "Inverse conversion: Parquet → CSV",
        conflicts_with = "to_jsonl"
    )]
    to_csv: bool,
    #[arg(
        long,
        help = "Inverse conversion: Parquet → JSONL",
        conflicts_with = "to_csv"
    )]
    to_jsonl: bool,
    #[arg(
        value_name = "INPUT",
        help = "Input file(s); supports glob patterns and '-' for stdin"
    )]
    input: Vec<String>,
}

fn default_sheet_concurrency() -> usize {
    (num_cpus::get() / 2).max(1)
}

impl CommandInterface {
    fn requested_export(&self) -> Option<ExportFormat> {
        if self.to_csv {
            return Some(ExportFormat::Csv);
        }
        if self.to_jsonl {
            return Some(ExportFormat::Jsonl);
        }
        None
    }
}

// ── Parquet export (inverse conversion) ─────────────────────────────

fn export_parquet_batch(cli: &CommandInterface, format: ExportFormat) -> Result<()> {
    let files = collect_inputs(&cli.input)?;
    if files.is_empty() {
        display_help();
        anyhow::bail!("No input provided for --to-{}", format.extension());
    }
    prepare_output_dir(cli, files.len())?;
    for file in &files {
        let output = resolve_output(file, cli, format.extension());
        export_parquet_file(file, &output, format)?;
    }
    Ok(())
}

fn export_parquet_file(file: &str, output: &Path, format: ExportFormat) -> Result<()> {
    eprintln!("[PHASE] Parquet -> {}: {}", format.label(), file);
    let rows = format
        .convert(Path::new(file), output)
        .with_context(|| format!("convert {} -> {}", file, output.display()))?;
    eprintln!(
        "{} {} rows -> {}",
        success("[OK]"),
        rows,
        path(output)
    );
    Ok(())
}

// ── Parquet inspection (default action on .parquet input) ──────────

/// In a terminal: launch the interactive viewer (preview + export).
/// Non-interactive runs (piped stdin) print schema and stats, then stop.
fn inspect_parquet_interactive(
    file: &str,
    cli: &CommandInterface,
    original_input: Option<&str>,
) -> Result<()> {
    verify_parquet_schema(file)?;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return display_parquet_stats(Path::new(file));
    }
    let input_for_output = original_input.unwrap_or(file).to_string();
    let resolve = |format: ExportFormat| {
        resolve_output(&input_for_output, cli, format.extension())
    };
    run_viewer(Path::new(file), &resolve)
}

fn main() {
    if let Err(e) = execute() {
        eprintln!("{} {}", error("Error:"), e);
        std::process::exit(1);
    }
}

fn execute() -> Result<()> {
    let cli = CommandInterface::parse();

    if cli.man {
        generate_manpage(None)?;
        return Ok(());
    }

    // --view-schema: single file inspection
    if cli.view_schema {
        let file = cli
            .input
            .first()
            .ok_or_else(|| anyhow!("--view-schema requires an input file"))?;
        return verify_parquet_schema(file);
    }

    // --to-csv / --to-jsonl: Parquet → CSV/JSONL inverse conversion
    if let Some(format) = cli.requested_export() {
        return export_parquet_batch(&cli, format);
    }

    // Stdin: explicit "-", or no input arg with a redirected stream
    let stdin_requested = (cli.input.len() == 1 && cli.input[0] == "-")
        || (cli.input.is_empty() && !io::stdin().is_terminal());
    if stdin_requested {
        return convert_from_stdin(&cli);
    }

    // Collect and expand input files
    let files = collect_inputs(&cli.input)?;
    if files.is_empty() {
        display_help();
        anyhow::bail!("No input provided");
    }
    prepare_output_dir(&cli, files.len())?;

    // Parse delimiter override
    let delim_override = cli.delimiter.as_ref().map(|d| parse_delimiter(d));

    // Single file: handle compressed input first
    if files.len() == 1 {
        let file = &files[0];
        let (actual_path, _guard) = decompress_if_needed(Path::new(file))?;
        let path_str = actual_path.to_string_lossy().to_string();

        if is_parquet(&path_str) {
            return inspect_parquet_interactive(&path_str, &cli, Some(file));
        }
        if is_spreadsheet(&path_str) {
            return convert_spreadsheet(&path_str, &cli, delim_override);
        }
        if is_json(&path_str) {
            return convert_json(&path_str, &cli, delim_override);
        }
        return convert_single_csv(&path_str, &cli, delim_override, Some(file));
    }

    // Multiple files: batch process
    for file in &files {
        let (actual_path, _guard) = decompress_if_needed(Path::new(file))?;
        let path_str = actual_path.to_string_lossy().to_string();
        eprintln!("[BATCH] Processing {}", path(Path::new(file)));

        if is_parquet(&path_str) {
            inspect_parquet_interactive(&path_str, &cli, Some(file))?;
        } else if is_spreadsheet(&path_str) {
            convert_spreadsheet(&path_str, &cli, delim_override)?;
        } else if is_json(&path_str) {
            convert_json(&path_str, &cli, delim_override)?;
        } else {
            convert_single_csv(&path_str, &cli, delim_override, Some(file))?;
        }
    }

    Ok(())
}

// ── Input expansion ─────────────────────────────────────────────────

/// Expand input arguments: glob patterns are expanded, plain files passed through.
fn collect_inputs(inputs: &[String]) -> Result<Vec<String>> {
    if inputs.is_empty() {
        return Ok(vec![]);
    }
    let mut files: Vec<String> = Vec::new();
    for input in inputs {
        if input == "-" {
            files.push(input.clone());
            continue;
        }
        if input.contains('*') || input.contains('?') || input.contains('[') {
            let mut found = false;
            let entries =
                glob::glob(input).with_context(|| format!("invalid glob pattern: {}", input))?;
            for entry in entries {
                match entry {
                    Ok(p) => {
                        files.push(p.to_string_lossy().to_string());
                        found = true;
                    }
                    Err(e) => eprintln!("{} {}", warning("Warning:"), e),
                }
            }
            if !found {
                anyhow::bail!("No files matched pattern: {}", input);
            }
        } else {
            if !Path::new(input).exists() {
                anyhow::bail!("File not found: {}", input);
            }
            files.push(input.clone());
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// Parse a delimiter string to a byte.
/// Accepts ',' ';' '\t' '|' and escaped forms like "\\t", "\t"
fn parse_delimiter(s: &str) -> u8 {
    match s {
        "\\t" | "\t" => b'\t',
        "," => b',',
        ";" => b';',
        "|" => b'|',
        other => {
            // Take first character as literal delimiter
            other.as_bytes().first().copied().unwrap_or(b',')
        }
    }
}

// ── Output path resolution ──────────────────────────────────────────

/// With multiple inputs (or a trailing separator), `--output` designates a
/// directory: create it upfront so `resolve_output` treats it as one instead
/// of silently writing every conversion to the same file.
fn prepare_output_dir(cli: &CommandInterface, input_count: usize) -> Result<()> {
    let Some(out) = &cli.output else {
        return Ok(());
    };
    let is_dir_target =
        input_count > 1 || out.ends_with('/') || out.ends_with(std::path::MAIN_SEPARATOR);
    if is_dir_target {
        std::fs::create_dir_all(out).with_context(|| format!("create output directory {out}"))?;
    }
    Ok(())
}

/// Resolve the output path for a single input file.
/// - If `--output` flag is given: use it (or treat as directory for multiple inputs)
/// - Otherwise: derive from input filename (replace extension with target ext)
fn resolve_output(input: &str, cli: &CommandInterface, target_ext: &str) -> PathBuf {
    match &cli.output {
        None => build_output_path(input, target_ext),
        Some(out) => {
            let out_path = PathBuf::from(out);
            // Treat as directory if it exists as one or ends with a separator
            if out_path.is_dir() || out.ends_with('/') || out.ends_with(std::path::MAIN_SEPARATOR) {
                let basename = build_output_path(input, target_ext)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                out_path.join(basename)
            } else {
                out_path
            }
        }
    }
}

/// Derive an output filename from an input path, replacing the extension.
/// Strips compression extensions first (data.csv.gz -> data.parquet).
fn build_output_path(input: &str, target_ext: &str) -> PathBuf {
    let stripped = strip_compression_ext(input);
    let input_path = Path::new(&stripped);
    let mut dir = input_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let base = input_path.file_stem().unwrap_or_default().to_string_lossy();
    dir.push(format!("{base}.{target_ext}"));
    dir
}

// ── Conversion helpers ──────────────────────────────────────────────

fn convert_single_csv(
    file: &str,
    cli: &CommandInterface,
    delim_override: Option<u8>,
    original_input: Option<&str>,
) -> Result<()> {
    let input_path = PathBuf::from(file);
    let output_path = resolve_output(original_input.unwrap_or(file), cli, "parquet");
    run_csv_conversion(&input_path, &output_path, cli, delim_override)
}

fn convert_json(file: &str, cli: &CommandInterface, delim_override: Option<u8>) -> Result<()> {
    let input_path = PathBuf::from(file);
    let output_path = resolve_output(file, cli, "parquet");
    convert_json_path(&input_path, &output_path, cli, delim_override)
}

fn convert_json_path(
    input_path: &Path,
    output_path: &Path,
    cli: &CommandInterface,
    delim_override: Option<u8>,
) -> Result<()> {
    if json_has_nested(input_path).with_context(|| format!("scan JSON {}", input_path.display()))? {
        return convert_json_nested(input_path, output_path);
    }
    eprintln!("[PHASE] JSON: extracting records");
    let export = export_json_to_csv(input_path)
        .with_context(|| format!("export JSON {}", input_path.display()))?;
    if export.row_count == 0 {
        eprintln!("{}", warning("JSON input empty, no Parquet produced"));
        return Ok(());
    }
    eprintln!("[OK] {} record(s) extracted", export.row_count);
    run_csv_conversion(&export.csv_path, output_path, cli, delim_override)
}

/// Native path for JSON containing nested objects/arrays: preserves structure
/// as Arrow Struct/List columns instead of flattening to strings.
fn convert_json_nested(input_path: &Path, output_path: &Path) -> Result<()> {
    eprintln!("[PHASE] JSON: nested structure detected, using native Arrow path");
    let rows = convert_json_to_parquet(input_path, output_path)
        .with_context(|| format!("convert nested JSON {}", input_path.display()))?;
    if rows == 0 {
        eprintln!("{}", warning("JSON input empty, no Parquet produced"));
        return Ok(());
    }
    verify_parquet_schema(output_path).context("Invalid Parquet schema")?;
    eprintln!(
        "{} {} record(s) -> {}",
        success("[OK]"),
        rows,
        path(output_path)
    );
    Ok(())
}

fn convert_spreadsheet(
    file: &str,
    cli: &CommandInterface,
    delim_override: Option<u8>,
) -> Result<()> {
    let input_path = PathBuf::from(file);
    eprintln!("[PHASE] Spreadsheet: listing sheets");
    let sheets = list_sheet_names(&input_path)?;
    eprintln!("[OK] {} sheet(s)", sheets.len());
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .context("invalid input filename")?
        .to_string();
    let parent = resolve_output_dir(cli);
    let sheet_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(cli.sheet_concurrency.max(1))
        .thread_name(|i| format!("sheet-{i}"))
        .build()
        .context("build sheet pool")?;
    let stderr_lock = Mutex::new(());
    let cli_ref = cli;
    let input_ref = &input_path;
    let stem_ref = &stem;
    let parent_ref = &parent;
    let lock_ref = &stderr_lock;
    let delim = delim_override;
    sheet_pool.install(|| {
        sheets.par_iter().try_for_each(|sheet_name| -> Result<()> {
            let safe = sanitize_sheet_name(sheet_name);
            let output_path = parent_ref.join(format!("{stem_ref}__{safe}.parquet"));
            {
                let _g = lock_ref.lock().unwrap();
                eprintln!("[SHEET start] '{sheet_name}' -> {}", output_path.display());
            }
            let export = export_sheet_to_csv(input_ref, sheet_name)
                .with_context(|| format!("export sheet '{sheet_name}'"))?;
            if export.row_count == 0 {
                let _g = lock_ref.lock().unwrap();
                eprintln!("[SHEET skip] '{sheet_name}' empty");
                return Ok(());
            }
            run_csv_conversion(&export.csv_path, &output_path, cli_ref, delim).with_context(
                || format!("convert sheet '{sheet_name}' rows={}", export.row_count),
            )?;
            {
                let _g = lock_ref.lock().unwrap();
                eprintln!(
                    "[SHEET done] '{sheet_name}' rows={} -> {}",
                    export.row_count,
                    output_path.display()
                );
            }
            Ok(())
        })
    })
}

/// Resolve the output directory for spreadsheet sheets.
/// Uses --output if given (treats it as a directory), otherwise input file's parent.
fn resolve_output_dir(cli: &CommandInterface) -> PathBuf {
    match &cli.output {
        Some(out) => {
            let p = PathBuf::from(out);
            if p.is_dir() || out.ends_with('/') || out.ends_with(std::path::MAIN_SEPARATOR) {
                p
            } else {
                // If --output was given as a file path, use its parent
                p.parent()
                    .filter(|d| !d.as_os_str().is_empty())
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            }
        }
        None => PathBuf::from("."),
    }
}

fn run_csv_conversion(
    input_path: &Path,
    output_path: &Path,
    cli: &CommandInterface,
    delimiter_override: Option<u8>,
) -> Result<()> {
    if cli.force_utf8 {
        eprintln!("{}", warning("--force-utf8 active"));
    }
    convert_convert_to_parquet(
        input_path,
        output_path,
        cli.full_schema_inference,
        cli.force_utf8,
        cli.force_header,
        delimiter_override,
    )
    .with_context(|| {
        format!(
            "Conversion failed {} -> {}",
            path(input_path),
            path(output_path)
        )
    })?;
    eprintln!("{} {}", success("Conversion complete:"), path(output_path));
    Ok(())
}

fn convert_from_stdin(cli: &CommandInterface) -> Result<()> {
    if io::stdin().is_terminal() {
        display_help();
        anyhow::bail!("Stdin requested but no stream is redirected");
    }
    let mut buffer = Vec::new();
    io::stdin().read_to_end(&mut buffer)?;
    if buffer.is_empty() {
        display_help();
        anyhow::bail!("Empty stdin");
    }
    // Buffer to a temp file carrying a content-derived suffix so the regular
    // extension-based dispatch (JSON/JSONL vs CSV) applies.
    let temp = Builder::new()
        .suffix(stdin_suffix(&buffer))
        .tempfile()
        .context("create stdin temp file")?;
    temp.as_file().write_all(&buffer)?;
    temp.as_file().sync_all()?;
    info!("Stdin -> {:?}", temp.path());

    let path_str = temp.path().to_string_lossy().to_string();
    let output_path = stdin_output_path(cli);
    let delim_override = cli.delimiter.as_ref().map(|d| parse_delimiter(d));
    if is_json(&path_str) {
        return convert_json_path(temp.path(), &output_path, cli, delim_override);
    }
    run_csv_conversion(temp.path(), &output_path, cli, delim_override)
}

fn stdin_output_path(cli: &CommandInterface) -> PathBuf {
    cli.output
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("stdin.parquet"))
}

// ── Utilities (carried over) ────────────────────────────────────────

fn generate_manpage(output: Option<PathBuf>) -> Result<()> {
    let mut cmd = CommandInterface::command();
    cmd = cmd.name("convert_to_parquet");
    let man = Man::new(cmd);
    match output {
        Some(path) => {
            let mut file = File::create(path)?;
            man.render(&mut file)?;
        }
        None => {
            let mut stdout = io::stdout();
            man.render(&mut stdout)?;
        }
    }
    Ok(())
}

fn display_help() {
    let mut command = CommandInterface::command();
    let _ = command.print_help();
    eprintln!();
}
