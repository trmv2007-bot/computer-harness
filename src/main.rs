use anyhow::Result;
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use serde::Serialize;
use std::{fs, path::PathBuf, time::Duration};
use tracing::{info, Level};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "fairy",
    version,
    about = "Computer Harness — Fairy runtime CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show runtime paths and local diagnostics.
    Doctor,
    /// Print machine-readable runtime information.
    Status,
    /// Run the foreground runtime supervisor.
    Run,
}

#[derive(Serialize)]
struct Status {
    version: &'static str,
    state: &'static str,
    data_dir: PathBuf,
    config_dir: PathBuf,
}

fn paths() -> Result<(PathBuf, PathBuf)> {
    let dirs = ProjectDirs::from("dev", "ComputerHarness", "Fairy")
        .ok_or_else(|| anyhow::anyhow!("could not determine per-user application directories"))?;
    let data = dirs.data_dir().to_path_buf();
    let config = dirs.config_dir().to_path_buf();
    fs::create_dir_all(&data)?;
    fs::create_dir_all(&config)?;
    Ok((data, config))
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let (data_dir, config_dir) = paths()?;

    match cli.command {
        Command::Doctor => {
            println!("Fairy local diagnostics");
            println!("data_dir={}", data_dir.display());
            println!("config_dir={}", config_dir.display());
            println!("runtime=not_started");
            println!("provider_authentication=not_tested");
            println!("platform_adapter=not_implemented");
        }
        Command::Status => {
            let status = Status {
                version: env!("CARGO_PKG_VERSION"),
                state: "cli_ready_runtime_not_started",
                data_dir,
                config_dir,
            };
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Command::Run => {
            info!("Fairy supervisor started; no computer-control tools are registered yet");
            tokio::select! {
                signal = tokio::signal::ctrl_c() => {
                    signal?;
                    info!("shutdown signal received");
                }
                _ = tokio::time::sleep(Duration::from_secs(24 * 60 * 60)) => {
                    info!("supervisor heartbeat interval elapsed; exiting cleanly");
                }
            }
        }
    }
    Ok(())
}
