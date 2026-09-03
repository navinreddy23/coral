use std::io::Write as _;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("CORAL_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let argv: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let json = argv.iter().any(|a| a == "--json");

    let rendered = coral_cli::run(argv).await;
    let text = if json {
        serde_json::to_string_pretty(&rendered.json).unwrap_or_default()
    } else {
        rendered.text
    };

    // Rust masks SIGPIPE, so writing to a closed reader — `coral graph | head` — surfaces as
    // an error that `println!` would panic on. Ending quietly is what every other CLI does.
    let mut out = std::io::stdout().lock();
    match writeln!(out, "{text}").and_then(|()| out.flush()) {
        Ok(()) => rendered.code,
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(coral_cli::output::exit::GIT),
    }
}
