mod ipc;
mod store;

use anyhow::Result;
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use serde::Serialize;
use std::{fs, path::PathBuf, time::Duration};
use store::Store;
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
    /// Show local paths, database health and capability readiness.
    Doctor,
    /// Print machine-readable runtime information.
    Status,
    /// Run the local control plane in the foreground.
    Serve,
    /// Inspect or stop the local control plane.
    Daemon {
        #[command(subcommand)]
        action: DaemonCommand,
    },
    /// Run the foreground supervisor shell.
    Run,
    /// Manage durable objectives.
    Task {
        #[command(subcommand)]
        action: TaskCommand,
    },
    /// Request an emergency stop of eligible Fairy work.
    StopAll,
}

#[derive(Subcommand)]
enum DaemonCommand {
    /// Check whether the local control plane is reachable.
    Status,
    /// Ask the local control plane to stop.
    Shutdown,
}

#[derive(Subcommand)]
enum TaskCommand {
    /// Create a durable queued task.
    Create { title: String, objective: String },
    /// List tasks, newest first.
    List,
    /// Show a task by its identifier.
    Show { id: String },
    /// Cancel a queued/running/paused task.
    Cancel { id: String },
    /// Move a task through a validated lifecycle transition.
    SetState { id: String, state: String },
}

#[derive(Serialize)]
struct Status {
    version: &'static str,
    state: &'static str,
    data_dir: PathBuf,
    config_dir: PathBuf,
    database: &'static str,
    task_count: usize,
    event_count: i64,
    model_provider: &'static str,
    computer_control: &'static str,
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

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
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
    let db_path = data_dir.join("fairy.sqlite3");
    let store = Store::open(&db_path)?;

    match cli.command {
        Command::Doctor => {
            println!("Fairy diagnostics");
            println!("data_dir={}", data_dir.display());
            println!("config_dir={}", config_dir.display());
            println!("database=ok ({})", db_path.display());
            println!("task_count={}", store.list_tasks()?.len());
            println!("provider_authentication=not_configured");
            println!("computer_control=not_implemented");
            println!("persistent_service=foreground_control_plane_available");
        }
        Command::Status => {
            let status = Status {
                version: env!("CARGO_PKG_VERSION"),
                state: "local_cli_and_task_store_ready",
                data_dir,
                config_dir,
                database: "ok",
                task_count: store.list_tasks()?.len(),
                event_count: store.event_count()?,
                model_provider: "not_configured",
                computer_control: "not_implemented",
            };
            print_json(&status)?;
        }
        Command::Task { action } => {
            let daemon_online = ipc::is_running(&data_dir).await;
            match action {
                TaskCommand::Create { title, objective } => {
                    if daemon_online {
                        let response = ipc::request(
                            &data_dir,
                            ipc::IpcCommand::CreateTask { title, objective },
                        )
                        .await?;
                        print_json(&response.data)?;
                    } else {
                        print_json(&store.create_task(&title, &objective)?)?;
                    }
                }
                TaskCommand::List => {
                    if daemon_online {
                        let response = ipc::request(&data_dir, ipc::IpcCommand::ListTasks).await?;
                        print_json(&response.data)?;
                    } else {
                        print_json(&store.list_tasks()?)?;
                    }
                }
                TaskCommand::Show { id } => {
                    if daemon_online {
                        let response =
                            ipc::request(&data_dir, ipc::IpcCommand::GetTask { id }).await?;
                        print_json(&response.data)?;
                    } else {
                        match store.get_task(&id)? {
                            Some(task) => print_json(&task)?,
                            None => anyhow::bail!("task not found: {id}"),
                        }
                    }
                }
                TaskCommand::Cancel { id } => {
                    if daemon_online {
                        let response =
                            ipc::request(&data_dir, ipc::IpcCommand::CancelTask { id }).await?;
                        print_json(&response.data)?;
                    } else if store.cancel_task(&id)? {
                        println!("cancelled {id}");
                    } else {
                        anyhow::bail!("task not found or no longer cancellable: {id}");
                    }
                }
                TaskCommand::SetState { id, state } => {
                    if daemon_online {
                        let response =
                            ipc::request(&data_dir, ipc::IpcCommand::SetTaskState { id, state })
                                .await?;
                        print_json(&response.data)?;
                    } else {
                        print_json(&store.transition_task(&id, &state)?)?;
                    }
                }
            }
        }
        Command::StopAll => {
            let marker = data_dir.join("STOP_ALL");
            fs::write(&marker, b"stop requested\n")?;
            info!(path = %marker.display(), "emergency stop marker written");
            println!("Stop request recorded. Active worker termination is unavailable until the worker supervisor is implemented.");
        }
        Command::Serve => {
            ipc::serve(&data_dir, db_path).await?;
        }
        Command::Daemon { action } => match action {
            DaemonCommand::Status => {
                let response = ipc::request(&data_dir, ipc::IpcCommand::Ping).await?;
                print_json(&response)?;
            }
            DaemonCommand::Shutdown => {
                let response = ipc::request(&data_dir, ipc::IpcCommand::Shutdown).await?;
                print_json(&response)?;
            }
        },
        Command::Run => {
            let marker = data_dir.join("STOP_ALL");
            if marker.exists() {
                fs::remove_file(&marker)?;
            }
            info!("Fairy foreground supervisor shell started; no agent workers are registered");
            tokio::select! {
                signal = tokio::signal::ctrl_c() => {
                    signal?;
                    info!("shutdown signal received");
                }
                _ = tokio::time::sleep(Duration::from_secs(24 * 60 * 60)) => {
                    info!("supervisor interval elapsed; exiting cleanly");
                }
            }
        }
    }
    Ok(())
}
