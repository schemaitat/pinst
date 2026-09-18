mod app;
mod cli;
mod core;
mod editor;
mod event;
mod ui;

use clap::Parser;
use color_eyre::eyre::Result;

use cli::output::ExitCode;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            // clap prints help/usage itself; we only own the exit code so it
            // matches the documented contract (2 = usage error).
            let _ = err.print();
            let code = match err.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    ExitCode::Success
                }
                _ => ExitCode::Usage,
            };
            std::process::exit(code as i32);
        }
    };
    let json = cli.global.json;
    let command = cli.command.name().to_string();

    let code = match cli::dispatch(cli).await {
        Ok(code) => code,
        Err(report) => {
            // A failure still has to speak the contract: JSON consumers get an
            // envelope with the message in `errors`, humans get it on stderr.
            if json {
                let envelope = cli::output::Envelope::<serde_json::Value>::new(
                    &command,
                    cli::output::Status::Error,
                    Vec::new(),
                )
                .errors(vec![format!("{report:#}")]);
                serde_json::to_writer_pretty(std::io::stdout(), &envelope)?;
                println!();
            } else {
                eprintln!("error: {report:#}");
            }
            if report.downcast_ref::<core::UsageError>().is_some() {
                ExitCode::Usage
            } else {
                ExitCode::Failure
            }
        }
    };

    std::process::exit(code as i32);
}
