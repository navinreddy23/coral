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
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&rendered.json).unwrap_or_default()
        );
    } else {
        println!("{}", rendered.text);
    }
    rendered.code
}
