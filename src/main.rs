use std::io::Read;

use astchunk::{AstChunkBuilder, ChunkOptions, Language, MetadataTemplate};
use clap::Parser;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::time::ChronoLocal;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum LangArg {
    #[cfg(feature = "python")]
    Python,
    #[cfg(feature = "java")]
    Java,
    #[cfg(feature = "csharp")]
    Csharp,
    #[cfg(feature = "typescript")]
    Typescript,
}

impl LangArg {
    const fn into_language(self) -> Language {
        match self {
            #[cfg(feature = "python")]
            Self::Python => Language::Python,
            #[cfg(feature = "java")]
            Self::Java => Language::Java,
            #[cfg(feature = "csharp")]
            Self::Csharp => Language::CSharp,
            #[cfg(feature = "typescript")]
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
struct Cli {
    /// Enable debug logging
    #[arg(long)]
    debug: bool,

    /// Programming language of the input
    #[arg(short, long)]
    language: LangArg,

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

    /// Source file to chunk (reads from stdin if omitted)
    file: Option<String>,
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

fn main() {
    let cli = Cli::parse();
    init_tracing(cli.debug);

    let code = if let Some(path) = &cli.file {
        std::fs::read_to_string(path).expect("failed to read file")
    } else {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .expect("failed to read stdin");
        buf
    };

    let builder = AstChunkBuilder::new(cli.max_chunk_size, cli.language.into_language());
    let options = ChunkOptions {
        chunk_overlap: cli.overlap,
        chunk_expansion: cli.expansion,
        ..ChunkOptions::default()
    };
    let windows = builder.chunkify(&code, cli.template.into_template(), &options);

    let json = serde_json::to_string_pretty(&windows).expect("failed to serialize");
    println!("{json}");
}
