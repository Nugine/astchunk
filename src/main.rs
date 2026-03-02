use std::fmt::Write as _;
use std::io::Read;

use astchunk::{AstChunkBuilder, CodeWindow, Language, MetadataTemplate};
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

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
enum TemplateArg {
    None,
    #[default]
    Default,
    RepoEval,
    SwebenchLite,
}

impl TemplateArg {
    const fn into_template(self) -> MetadataTemplate {
        match self {
            Self::None => MetadataTemplate::None,
            Self::Default => MetadataTemplate::Default,
            Self::RepoEval => MetadataTemplate::CodeRagBenchRepoEval,
            Self::SwebenchLite => MetadataTemplate::CodeRagBenchSwebenchLite,
        }
    }
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

    /// Metadata template
    #[arg(short, long, value_enum, default_value = "default")]
    template: TemplateArg,

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

/// Extract display-friendly metadata from a `CodeWindow`.
struct ChunkInfo {
    index: usize,
    content: String,
    line_count: String,
    nws_size: String,
    node_count: String,
    start_line: String,
    end_line: String,
}

fn extract_chunk_info(window: &CodeWindow, index: usize) -> ChunkInfo {
    match window {
        CodeWindow::Standard { content, metadata } => ChunkInfo {
            index,
            content: content.clone(),
            line_count: metadata
                .get("line_count")
                .map_or_else(|| "—".into(), std::string::ToString::to_string),
            nws_size: metadata
                .get("chunk_size")
                .map_or_else(|| "—".into(), std::string::ToString::to_string),
            node_count: metadata
                .get("node_count")
                .map_or_else(|| "—".into(), std::string::ToString::to_string),
            start_line: metadata
                .get("start_line_no")
                .map_or_else(|| "—".into(), std::string::ToString::to_string),
            end_line: metadata
                .get("end_line_no")
                .map_or_else(|| "—".into(), std::string::ToString::to_string),
        },
        CodeWindow::SwebenchLite {
            _id: id,
            title,
            text,
        } => ChunkInfo {
            index,
            content: text.clone(),
            line_count: "—".into(),
            nws_size: "—".into(),
            node_count: "—".into(),
            start_line: format!("{title} ({id})"),
            end_line: "—".into(),
        },
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
            info.line_count.clone(),
            info.nws_size.clone(),
            info.node_count.clone(),
            info.start_line.clone(),
            info.end_line.clone(),
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

    let start_line: usize = info.start_line.parse().unwrap_or(0);
    let end_line: usize = info.end_line.parse().unwrap_or(0);
    let width = (end_line + 1).max(1).to_string().len();

    let mut out = header;
    for (i, line) in info.content.lines().enumerate() {
        write!(out, "\n {:>width$} │ {line}", start_line + i).unwrap();
    }
    out
}

fn print_table_output(windows: &[CodeWindow], lang_str: &str, cli: &Cli) {
    let infos: Vec<ChunkInfo> = windows
        .iter()
        .enumerate()
        .map(|(i, w)| extract_chunk_info(w, i))
        .collect();

    let params = build_params_table(lang_str, cli, infos.len());
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

/// Result of processing a single file.
#[derive(serde::Serialize)]
struct FileResult {
    file: String,
    chunks: Vec<CodeWindow>,
}

/// Process a single file: read, parse, chunkify, and return the windows.
fn process_file(path: &camino::Utf8Path, language: Language, cli: &Cli) -> Vec<CodeWindow> {
    let code = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error: failed to read {path}: {e}");
        std::process::exit(1);
    });
    AstChunkBuilder::new(language)
        .max_chunk_size(cli.max_chunk_size)
        .chunk_overlap(cli.overlap)
        .chunk_expansion(cli.expansion)
        .template(cli.template.into_template())
        .chunkify(&code)
}

/// Print table/brief output for a single chunk identified by `--chunk-id`.
fn print_single_chunk(windows: &[CodeWindow], chunk_id: usize, lang_str: &str, cli: &Cli) {
    if chunk_id == 0 || chunk_id > windows.len() {
        eprintln!(
            "Error: --chunk-id {chunk_id} is out of range [1, {}]",
            windows.len()
        );
        std::process::exit(1);
    }
    let window = &windows[chunk_id - 1];
    let info = extract_chunk_info(window, chunk_id - 1);

    let params = build_params_table(lang_str, cli, windows.len());
    println!("{params}");

    println!("{}", format_chunk(&info, windows.len(), !cli.brief));
}

fn main() {
    let cli = Cli::parse();
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

        let windows = AstChunkBuilder::new(language)
            .max_chunk_size(cli.max_chunk_size)
            .chunk_overlap(cli.overlap)
            .chunk_expansion(cli.expansion)
            .template(cli.template.into_template())
            .chunkify(&buf);
        output_single(&cli, &windows, &format!("{language:?}"), "<stdin>");
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
        let mut results: Vec<(String, String, Vec<CodeWindow>)> = resolved
            .par_iter()
            .map(|(path, language)| {
                let windows = process_file(path.as_ref(), *language, &cli);
                (path.to_string(), format!("{language:?}"), windows)
            })
            .collect();

        // Sort by path for deterministic output.
        results.sort_by(|a, b| a.0.cmp(&b.0));

        if results.len() == 1 {
            let (ref path, ref lang_str, ref windows) = results[0];
            output_single(&cli, windows, lang_str, path);
        } else {
            output_multi(&cli, &results);
        }
    }
}

/// Output results for a single source (stdin or one file).
fn output_single(cli: &Cli, windows: &[CodeWindow], lang_str: &str, file_name: &str) {
    if let Some(id) = cli.chunk_id {
        if cli.json {
            if id == 0 || id > windows.len() {
                eprintln!(
                    "Error: --chunk-id {id} is out of range [1, {}]",
                    windows.len()
                );
                std::process::exit(1);
            }
            let result = vec![FileResult {
                file: file_name.to_string(),
                chunks: vec![windows[id - 1].clone()],
            }];
            let json = serde_json::to_string_pretty(&result).expect("failed to serialize");
            println!("{json}");
        } else {
            print_single_chunk(windows, id, lang_str, cli);
        }
    } else if cli.json {
        let result = vec![FileResult {
            file: file_name.to_string(),
            chunks: windows.to_vec(),
        }];
        let json = serde_json::to_string_pretty(&result).expect("failed to serialize");
        println!("{json}");
    } else {
        print_table_output(windows, lang_str, cli);
    }
}

/// Output results for multiple files.
fn output_multi(cli: &Cli, results: &[(String, String, Vec<CodeWindow>)]) {
    if cli.json {
        let json_results: Vec<FileResult> = results
            .iter()
            .map(|(path, _lang, windows)| FileResult {
                file: path.clone(),
                chunks: windows.clone(),
            })
            .collect();
        let json = serde_json::to_string_pretty(&json_results).expect("failed to serialize");
        println!("{json}");
    } else {
        let mut first = true;
        for (path, lang_str, windows) in results {
            if !first {
                println!();
            }
            first = false;
            println!("=== file: {path} ===");
            if let Some(id) = cli.chunk_id {
                print_single_chunk(windows, id, lang_str, cli);
            } else {
                print_table_output(windows, lang_str, cli);
            }
        }
    }
}
