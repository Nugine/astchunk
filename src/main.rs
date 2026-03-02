use clap::Parser;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::time::ChronoLocal;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Enable debug logging
    #[arg(long)]
    debug: bool,
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

    // TODO: implement CLI behavior
}
