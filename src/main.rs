use std::fmt::Write as _;
use std::io::Read;

use astchunk::chunker::{CastChunker, CastChunkerOptions, Chunker};
use astchunk::formatter::{CanonicalFormatter, ContextualFormatter, Formatter};
use astchunk::lang::Language;
use astchunk::output::{JsonRecord, RepoEvalRecord, SwebenchLiteRecord};
use astchunk::types::{AstChunk, Document, DocumentId, Origin, TextChunk};
use bytestring::ByteString;
use clap::Parser;
use comfy_table::Table;
use comfy_table::presets::UTF8_FULL;
use rayon::prelude::*;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::time::ChronoLocal;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum LangArg {
    Python,
    Java,
    Cpp,
    Rust,
    Csharp,
    Typescript,
}

impl LangArg {
    const fn into_language(self) -> Language {
        match self {
            Self::Python => Language::Python,
            Self::Java => Language::Java,
            Self::Cpp => Language::Cpp,
            Self::Rust => Language::Rust,
            Self::Csharp => Language::CSharp,
            Self::Typescript => Language::TypeScript,
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default, PartialEq, Eq)]
enum TemplateArg {
    None,
    #[default]
    Default,
    RepoEval,
    SwebenchLite,
}

#[derive(Debug, Parser)]
#[command(version, about = "AST-based code chunking (cAST algorithm)")]
#[allow(clippy::struct_excessive_bools)] // CLI flags naturally use many bools
struct Cli {
    /// Enable debug logging
    #[arg(long)]
    debug: bool,

    /// Programming language of the input (auto-detected from extension if omitted)
    #[arg(short, long)]
    language: Option<LangArg>,

    /// Maximum non-whitespace characters per chunk
    #[arg(short = 's', long, default_value = "1500")]
    max_chunk_size: u32,

    /// Number of AST nodes to overlap between windows
    #[arg(long, default_value = "0")]
    overlap: usize,

    /// Add chunk expansion (ancestry header)
    #[arg(long)]
    expansion: bool,

    /// Export format for JSON output
    #[arg(short, long, value_enum, default_value = "default")]
    template: TemplateArg,

    /// Repository name used in output metadata (required for `repo-eval`)
    #[arg(long)]
    repo: Option<String>,

    /// Logical source path to use for stdin input metadata
    #[arg(long)]
    stdin_path: Option<camino::Utf8PathBuf>,

    /// Output as JSON
    #[arg(long, conflicts_with = "brief")]
    json: bool,

    /// Brief mode: show only the summary table without code content
    #[arg(long, conflicts_with = "json")]
    brief: bool,

    /// Output only the chunk with this 1-based ID
    #[arg(long)]
    chunk_id: Option<usize>,

    /// Source files or directories to chunk (reads from stdin if omitted)
    files: Vec<camino::Utf8PathBuf>,
}

fn init_tracing(debug: bool) {
    let default_level = if debug {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };

    let timer = ChronoLocal::rfc_3339();

    let env_filter = EnvFilter::builder()
        .with_default_directive(default_level.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .pretty()
        .with_timer(timer)
        .with_env_filter(env_filter)
        .init();
}

fn validate_cli(cli: &Cli) -> Result<(), String> {
    if !cli.files.is_empty() && cli.stdin_path.is_some() {
        return Err("--stdin-path can only be used when reading from stdin".to_string());
    }

    if cli.template == TemplateArg::RepoEval && cli.repo.is_none() {
        return Err("--repo is required with --template repo-eval".to_string());
    }

    if cli.files.is_empty()
        && cli.stdin_path.is_none()
        && matches!(
            cli.template,
            TemplateArg::RepoEval | TemplateArg::SwebenchLite
        )
    {
        return Err(format!(
            "--stdin-path is required with --template {} when reading from stdin",
            match cli.template {
                TemplateArg::RepoEval => "repo-eval",
                TemplateArg::SwebenchLite => "swebench-lite",
                TemplateArg::None | TemplateArg::Default => unreachable!(),
            }
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

/// Display-friendly metadata extracted from `AstChunk` + `TextChunk`.
struct ChunkInfo {
    index: usize,
    content: String,
    line_count: u32,
    nws_size: u32,
    node_count: u32,
    start_line: u32,
    end_line: u32,
}

fn extract_chunk_info(ast_chunk: &AstChunk, text_chunk: &TextChunk, index: usize) -> ChunkInfo {
    let line_numbers = text_chunk.source_line_index_range.to_line_number_range();
    ChunkInfo {
        index,
        content: text_chunk.content.to_string(),
        line_count: text_chunk.metrics.content_line_count,
        nws_size: ast_chunk.metrics.nws_size,
        node_count: ast_chunk.metrics.node_count,
        start_line: line_numbers.start,
        end_line: line_numbers.end,
    }
}

/// Extract the first non-empty line of code, truncated to fit in a table cell.
fn first_code_line(content: &str) -> String {
    const MAX_WIDTH: usize = 60;
    let line = content.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    if line.chars().count() <= MAX_WIDTH {
        line.to_string()
    } else {
        let truncated: String = line.chars().take(MAX_WIDTH - 3).collect();
        format!("{truncated}...")
    }
}

/// Build an overview table listing all algorithm parameters (horizontal layout).
fn build_params_table(lang_str: &str, cli: &Cli, chunk_count: usize) -> Table {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "Language",
        "Max Chunk Size",
        "Overlap",
        "Expansion",
        "Template",
        "Total Chunks",
    ]);
    table.add_row(vec![
        lang_str.to_string(),
        cli.max_chunk_size.to_string(),
        cli.overlap.to_string(),
        if cli.expansion {
            "enabled".into()
        } else {
            "disabled".into()
        },
        format!("{:?}", cli.template),
        chunk_count.to_string(),
    ]);
    table
}

fn build_summary_table(infos: &[ChunkInfo], brief: bool) -> Table {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    if brief {
        table.set_header(vec![
            "#",
            "Lines",
            "NWS Size",
            "Nodes",
            "Start Line",
            "End Line",
            "First Line",
        ]);
    } else {
        table.set_header(vec![
            "#",
            "Lines",
            "NWS Size",
            "Nodes",
            "Start Line",
            "End Line",
        ]);
    }
    for info in infos {
        let mut row = vec![
            (info.index + 1).to_string(),
            info.line_count.to_string(),
            info.nws_size.to_string(),
            info.node_count.to_string(),
            info.start_line.to_string(),
            info.end_line.to_string(),
        ];
        if brief {
            row.push(first_code_line(&info.content));
        }
        table.add_row(row);
    }
    table
}

/// Format a single chunk as a compact header line followed by line-numbered code.
fn format_chunk(info: &ChunkInfo, total: usize, include_code: bool) -> String {
    let header = format!(
        "─── Chunk {}/{total} │ Lines: {} │ NWS: {} │ Nodes: {} │ L{}..L{} ───",
        info.index + 1,
        info.line_count,
        info.nws_size,
        info.node_count,
        info.start_line,
        info.end_line,
    );

    if !include_code {
        return header;
    }

    let width = (info.end_line + 1).max(1).to_string().len();

    let mut out = header;
    for (i, line) in info.content.lines().enumerate() {
        let line_no = info.start_line as usize + i;
        write!(out, "\n {line_no:>width$} │ {line}").unwrap();
    }
    out
}

// ---------------------------------------------------------------------------
// Processing pipeline
// ---------------------------------------------------------------------------

/// Processed results for a single file/stdin source.
struct ProcessedFile {
    file: String,
    lang_str: String,
    document: Document,
    ast_chunks: Vec<AstChunk>,
    text_chunks: Vec<TextChunk>,
}

fn build_chunker(cli: &Cli) -> CastChunker {
    let mut options = CastChunkerOptions::default();
    options.max_nws_size = cli.max_chunk_size;
    options.overlap_nodes = cli.overlap;
    CastChunker::new(options)
}

fn build_formatter(cli: &Cli) -> Box<dyn Formatter> {
    if cli.expansion {
        Box::new(ContextualFormatter::default())
    } else {
        Box::new(CanonicalFormatter::default())
    }
}

fn build_origin_for_file(path: &camino::Utf8Path, cli: &Cli) -> Origin {
    Origin {
        path: Some(ByteString::from(path.as_str())),
        repo: cli.repo.as_deref().map(ByteString::from),
        revision: None,
    }
}

fn build_origin_for_stdin(cli: &Cli) -> Origin {
    Origin {
        path: cli
            .stdin_path
            .as_deref()
            .map(|path| ByteString::from(path.as_str())),
        repo: cli.repo.as_deref().map(ByteString::from),
        revision: None,
    }
}

fn process_file(
    path: &camino::Utf8Path,
    language: Language,
    document_id: DocumentId,
    cli: &Cli,
) -> ProcessedFile {
    let code = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error: failed to read {path}: {e}");
        std::process::exit(1);
    });

    let document = Document {
        document_id,
        language,
        source: ByteString::from(code),
        origin: build_origin_for_file(path, cli),
    };

    let chunker = build_chunker(cli);
    let ast_chunks = chunker.chunk(&document).unwrap_or_else(|e| {
        eprintln!("Error: chunking failed for {path}: {e}");
        std::process::exit(1);
    });

    let formatter = build_formatter(cli);
    let text_chunks = formatter
        .format(&document, &ast_chunks)
        .unwrap_or_else(|e| {
            eprintln!("Error: formatting failed for {path}: {e}");
            std::process::exit(1);
        });

    ProcessedFile {
        file: path.to_string(),
        lang_str: format!("{language:?}"),
        document,
        ast_chunks,
        text_chunks,
    }
}

fn process_stdin(language: Language, code: String, cli: &Cli) -> ProcessedFile {
    let document = Document {
        document_id: DocumentId(0),
        language,
        source: ByteString::from(code),
        origin: build_origin_for_stdin(cli),
    };

    let chunker = build_chunker(cli);
    let ast_chunks = chunker.chunk(&document).unwrap_or_else(|e| {
        eprintln!("Error: chunking failed: {e}");
        std::process::exit(1);
    });

    let formatter = build_formatter(cli);
    let text_chunks = formatter
        .format(&document, &ast_chunks)
        .unwrap_or_else(|e| {
            eprintln!("Error: formatting failed: {e}");
            std::process::exit(1);
        });

    ProcessedFile {
        file: "<stdin>".to_string(),
        lang_str: format!("{language:?}"),
        document,
        ast_chunks,
        text_chunks,
    }
}

// ---------------------------------------------------------------------------
// Table / brief output
// ---------------------------------------------------------------------------

fn print_table_output(processed: &ProcessedFile, cli: &Cli) {
    let infos: Vec<ChunkInfo> = processed
        .ast_chunks
        .iter()
        .zip(&processed.text_chunks)
        .enumerate()
        .map(|(i, (ac, tc))| extract_chunk_info(ac, tc, i))
        .collect();

    let params = build_params_table(&processed.lang_str, cli, infos.len());
    println!("{params}");

    let summary = build_summary_table(&infos, cli.brief);
    println!("{summary}");

    if cli.brief {
        return;
    }

    for info in &infos {
        println!("{}\n", format_chunk(info, infos.len(), true));
    }
}

/// Print table/brief output for a single chunk identified by `--chunk-id`.
fn print_single_chunk(processed: &ProcessedFile, chunk_id: usize, cli: &Cli) {
    let total = processed.ast_chunks.len();
    if chunk_id == 0 || chunk_id > total {
        eprintln!("Error: --chunk-id {chunk_id} is out of range [1, {total}]");
        std::process::exit(1);
    }
    let idx = chunk_id - 1;
    let info = extract_chunk_info(&processed.ast_chunks[idx], &processed.text_chunks[idx], idx);

    let params = build_params_table(&processed.lang_str, cli, total);
    println!("{params}");

    println!("{}", format_chunk(&info, total, !cli.brief));
}

// ---------------------------------------------------------------------------
// JSON output
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct FileResult {
    file: String,
    chunks: serde_json::Value,
}

fn export_json(processed: &ProcessedFile, template: TemplateArg) -> serde_json::Value {
    match template {
        TemplateArg::RepoEval => {
            let records = RepoEvalRecord::build(
                &processed.document,
                &processed.ast_chunks,
                &processed.text_chunks,
            )
            .unwrap_or_else(|e| {
                eprintln!("Error: export failed: {e}");
                std::process::exit(1);
            });
            serde_json::to_value(&records).expect("serialization failed")
        }
        TemplateArg::SwebenchLite => {
            let records =
                SwebenchLiteRecord::build(&processed.document, &processed.text_chunks, "")
                    .unwrap_or_else(|e| {
                        eprintln!("Error: export failed: {e}");
                        std::process::exit(1);
                    });
            serde_json::to_value(&records).expect("serialization failed")
        }
        TemplateArg::Default | TemplateArg::None => {
            let records = JsonRecord::build(
                &processed.document,
                &processed.ast_chunks,
                &processed.text_chunks,
            );
            serde_json::to_value(&records).expect("serialization failed")
        }
    }
}

fn export_json_single_chunk(
    processed: &ProcessedFile,
    template: TemplateArg,
    chunk_id: usize,
) -> serde_json::Value {
    let total = processed.ast_chunks.len();
    if chunk_id == 0 || chunk_id > total {
        eprintln!("Error: --chunk-id {chunk_id} is out of range [1, {total}]");
        std::process::exit(1);
    }
    let idx = chunk_id - 1;
    let ac = &processed.ast_chunks[idx..=idx];
    let tc = &processed.text_chunks[idx..=idx];

    match template {
        TemplateArg::RepoEval => {
            let records = RepoEvalRecord::build(&processed.document, ac, tc).unwrap_or_else(|e| {
                eprintln!("Error: export failed: {e}");
                std::process::exit(1);
            });
            serde_json::to_value(&records).expect("serialization failed")
        }
        TemplateArg::SwebenchLite => {
            let records =
                SwebenchLiteRecord::build(&processed.document, tc, "").unwrap_or_else(|e| {
                    eprintln!("Error: export failed: {e}");
                    std::process::exit(1);
                });
            serde_json::to_value(&records).expect("serialization failed")
        }
        TemplateArg::Default | TemplateArg::None => {
            let records = JsonRecord::build(&processed.document, ac, tc);
            serde_json::to_value(&records).expect("serialization failed")
        }
    }
}

// ---------------------------------------------------------------------------
// File discovery
// ---------------------------------------------------------------------------

/// Known file extensions for language auto-detection.
const KNOWN_EXTENSIONS: &[&str] = &[
    "py", "java", "cpp", "cc", "cxx", "h", "hpp", "hxx", "hh", "rs", "cs", "ts", "tsx",
];

/// Expand a list of file/directory paths into individual files with known extensions.
/// Directories are recursively walked while respecting `.gitignore` rules.
/// Results are sorted for deterministic output.
fn expand_paths(paths: &[camino::Utf8PathBuf]) -> Vec<camino::Utf8PathBuf> {
    let mut files = Vec::new();
    for path in paths {
        let std_path: &std::path::Path = path.as_ref();
        if std_path.is_dir() {
            let walker = ignore::WalkBuilder::new(path)
                .sort_by_file_name(std::cmp::Ord::cmp)
                .build();
            for entry in walker.filter_map(Result::ok) {
                if entry.file_type().is_some_and(|ft| ft.is_file()) {
                    let entry_path = entry.path();
                    let has_known_ext = entry_path
                        .extension()
                        .and_then(std::ffi::OsStr::to_str)
                        .is_some_and(|ext| KNOWN_EXTENSIONS.contains(&ext));
                    if has_known_ext
                        && let Ok(utf8) = camino::Utf8PathBuf::try_from(entry_path.to_path_buf())
                    {
                        files.push(utf8);
                    }
                }
            }
        } else {
            files.push(path.clone());
        }
    }
    files.sort();
    files.dedup();
    files
}

/// Determine the language for a file, using the explicit CLI override or auto-detection.
/// Returns `None` (with a warning) if the language cannot be determined.
fn resolve_language(path: &camino::Utf8Path, explicit: Option<LangArg>) -> Option<Language> {
    if let Some(lang) = explicit {
        return Some(lang.into_language());
    }
    let ext = path.extension()?;
    let detected = Language::from_extension(ext);
    if detected.is_none() {
        eprintln!("Warning: skipping {path}: unknown extension \".{ext}\"");
    }
    detected
}

// ---------------------------------------------------------------------------
// Output routing
// ---------------------------------------------------------------------------

/// Output results for a single source (stdin or one file).
fn output_single(cli: &Cli, processed: &ProcessedFile) {
    if let Some(id) = cli.chunk_id {
        if cli.json {
            let chunks = export_json_single_chunk(processed, cli.template, id);
            let result = vec![FileResult {
                file: processed.file.clone(),
                chunks,
            }];
            let json = serde_json::to_string_pretty(&result).expect("serialization failed");
            println!("{json}");
        } else {
            print_single_chunk(processed, id, cli);
        }
    } else if cli.json {
        let chunks = export_json(processed, cli.template);
        let result = vec![FileResult {
            file: processed.file.clone(),
            chunks,
        }];
        let json = serde_json::to_string_pretty(&result).expect("serialization failed");
        println!("{json}");
    } else {
        print_table_output(processed, cli);
    }
}

/// Output results for multiple files.
fn output_multi(cli: &Cli, results: &[ProcessedFile]) {
    if cli.json {
        let json_results: Vec<FileResult> = results
            .iter()
            .map(|p| FileResult {
                file: p.file.clone(),
                chunks: export_json(p, cli.template),
            })
            .collect();
        let json = serde_json::to_string_pretty(&json_results).expect("serialization failed");
        println!("{json}");
    } else {
        let mut first = true;
        for processed in results {
            if !first {
                println!();
            }
            first = false;
            println!("=== file: {} ===", processed.file);
            if let Some(id) = cli.chunk_id {
                print_single_chunk(processed, id, cli);
            } else {
                print_table_output(processed, cli);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();
    if let Err(err) = validate_cli(&cli) {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
    init_tracing(cli.debug);

    if cli.files.is_empty() {
        // --- stdin mode ---
        let Some(lang_arg) = cli.language else {
            eprintln!("Error: --language is required when reading from stdin");
            std::process::exit(1);
        };
        let language = lang_arg.into_language();

        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .expect("failed to read stdin");

        let processed = process_stdin(language, buf, &cli);
        output_single(&cli, &processed);
    } else {
        // --- file/directory mode ---
        let files = expand_paths(&cli.files);

        if files.is_empty() {
            eprintln!("Warning: no files with known extensions found");
            if cli.json {
                println!("[]");
            }
            return;
        }

        // Resolve languages sequentially (may print warnings for skipped files).
        let resolved: Vec<_> = files
            .iter()
            .filter_map(|path| {
                resolve_language(path.as_ref(), cli.language).map(|lang| (path.clone(), lang))
            })
            .collect();

        // Process files in parallel.
        let mut results: Vec<ProcessedFile> = resolved
            .par_iter()
            .enumerate()
            .map(|(i, (path, language))| {
                let doc_id = DocumentId(u32::try_from(i).expect("too many files"));
                process_file(path.as_ref(), *language, doc_id, &cli)
            })
            .collect();

        // Sort by path for deterministic output.
        results.sort_by(|a, b| a.file.cmp(&b.file));

        if results.len() == 1 {
            output_single(&cli, &results[0]);
        } else {
            output_multi(&cli, &results);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cli() -> Cli {
        Cli {
            debug: false,
            language: None,
            max_chunk_size: 1500,
            overlap: 0,
            expansion: false,
            template: TemplateArg::Default,
            repo: None,
            stdin_path: None,
            json: false,
            brief: false,
            chunk_id: None,
            files: vec![],
        }
    }

    #[test]
    fn validate_repo_eval_requires_repo() {
        let mut cli = make_cli();
        cli.template = TemplateArg::RepoEval;
        cli.files.push("src/lib.rs".into());

        let err = validate_cli(&cli).unwrap_err();
        assert_eq!(err, "--repo is required with --template repo-eval");
    }

    #[test]
    fn validate_repo_eval_stdin_requires_path() {
        let mut cli = make_cli();
        cli.template = TemplateArg::RepoEval;
        cli.repo = Some("astchunk".to_string());

        let err = validate_cli(&cli).unwrap_err();
        assert_eq!(
            err,
            "--stdin-path is required with --template repo-eval when reading from stdin"
        );
    }

    #[test]
    fn validate_stdin_path_rejected_for_file_mode() {
        let mut cli = make_cli();
        cli.stdin_path = Some("src/lib.rs".into());
        cli.files.push("src/main.rs".into());

        let err = validate_cli(&cli).unwrap_err();
        assert_eq!(err, "--stdin-path can only be used when reading from stdin");
    }

    #[test]
    fn build_origin_for_file_includes_repo_metadata() {
        let mut cli = make_cli();
        cli.repo = Some("astchunk".to_string());

        let origin = build_origin_for_file(camino::Utf8Path::new("src/lib.rs"), &cli);
        assert_eq!(origin.path.as_deref(), Some("src/lib.rs"));
        assert_eq!(origin.repo.as_deref(), Some("astchunk"));
    }

    #[test]
    fn build_origin_for_stdin_uses_cli_metadata() {
        let mut cli = make_cli();
        cli.repo = Some("astchunk".to_string());
        cli.stdin_path = Some("src/from_stdin.py".into());

        let origin = build_origin_for_stdin(&cli);
        assert_eq!(origin.path.as_deref(), Some("src/from_stdin.py"));
        assert_eq!(origin.repo.as_deref(), Some("astchunk"));
    }
}
