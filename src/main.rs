mod ipc;
mod policy;
mod store;
mod tools;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command as TokioCommand,
};
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
    /// Configure the local filesystem boundary for safe tools.
    Workspace {
        #[command(subcommand)]
        action: WorkspaceCommand,
    },
    /// Discover and execute registered, permission-gated tools.
    Tool {
        #[command(subcommand)]
        action: ToolCommand,
    },
    /// Manage explicitly granted capabilities and evaluate tool policies.
    Policy {
        #[command(subcommand)]
        action: PolicyCommand,
    },
    /// Create and resolve action-scoped approvals.
    Approval {
        #[command(subcommand)]
        action: ApprovalCommand,
    },
    /// Run the optional Qoder Agent SDK worker.
    Agent {
        #[command(subcommand)]
        action: AgentCommand,
    },
    /// Request an emergency stop of eligible Fairy work.
    StopAll,
}

#[derive(Subcommand)]
enum AgentCommand {
    /// Install the pinned Qoder Agent SDK worker dependencies.
    Setup,
    /// Ask Fairy to inspect the configured workspace using the signed-in Qoder account.
    Ask { prompt: String },
}

#[derive(Subcommand)]
enum DaemonCommand {
    /// Check whether the local control plane is reachable.
    Status,
    /// Ask the local control plane to stop.
    Shutdown,
}

#[derive(Subcommand)]
enum WorkspaceCommand {
    /// Set the canonical directory boundary used by filesystem tools.
    Set { path: PathBuf },
    /// Show the configured workspace root.
    Show,
    /// Remove the workspace root configuration.
    Clear,
}

#[derive(Subcommand)]
enum ToolCommand {
    /// List registered tools and their permission metadata.
    List,
    /// Execute one registered tool with JSON input.
    Call {
        name: String,
        input_json: String,
        #[arg(long)]
        approval_id: Option<String>,
    },
}

#[derive(Subcommand)]
enum PolicyCommand {
    /// List explicit capability grants.
    List,
    /// Grant one exact capability (wildcards are rejected).
    Grant { capability: String, note: String },
    /// Revoke one exact capability.
    Revoke { capability: String },
    /// Evaluate a capability/effect/action tuple; this does not execute a tool.
    Check {
        capability: String,
        tool: String,
        #[arg(value_enum)]
        effect: policy::SideEffect,
        #[arg(long)]
        approval_id: Option<String>,
    },
}

#[derive(Subcommand)]
enum ApprovalCommand {
    /// Request approval for one exact action.
    Request { action: String, reason: String },
    /// List pending and resolved approval requests.
    List,
    /// Approve a pending action-scoped request.
    Approve { id: String },
    /// Deny a pending action-scoped request.
    Deny { id: String },
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

async fn agent_setup(agent_dir: &Path) -> Result<()> {
    anyhow::ensure!(
        agent_dir.join("package.json").is_file(),
        "Qoder worker package manifest is missing"
    );
    let mut command = if cfg!(windows) {
        let mut command = TokioCommand::new("cmd");
        command.args(["/C", "npm", "install", "--no-audit", "--no-fund"]);
        command
    } else {
        let mut command = TokioCommand::new("npm");
        command.args(["install", "--no-audit", "--no-fund"]);
        command
    };
    let status = command
        .current_dir(agent_dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .context("start npm to install the Qoder worker")?;
    anyhow::ensure!(
        status.success(),
        "Qoder worker dependency installation failed: {status}"
    );
    Ok(())
}

async fn agent_ask(agent_dir: &Path, data_dir: &Path, store: &Store, prompt: &str) -> Result<()> {
    anyhow::ensure!(
        agent_dir
            .join("node_modules")
            .join("@qoder-ai")
            .join("qoder-agent-sdk")
            .join("package.json")
            .is_file(),
        "Qoder SDK is not installed; run 'fairy agent setup' first"
    );
    anyhow::ensure!(
        ipc::is_running(data_dir).await,
        "Fairy control plane is not running; start it with 'fairy serve' in another terminal"
    );

    let workspace = store
        .get_setting("workspace_root")?
        .context("workspace is not configured; use 'fairy workspace set <directory>'")?;
    let workspace_path = PathBuf::from(&workspace)
        .canonicalize()
        .context("configured workspace is unavailable")?;
    anyhow::ensure!(
        workspace_path.is_dir(),
        "configured workspace is not a directory"
    );

    let worker_path = agent_dir.join("qoder-worker.mjs");
    let mut child = TokioCommand::new("node")
        .arg(worker_path)
        .current_dir(agent_dir)
        .env("FAIRY_DATA_DIR", data_dir)
        .env("FAIRY_WORKSPACE_ROOT", &workspace_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .context("start Node.js Qoder worker; verify Node.js 18+ is on PATH")?;

    let payload = serde_json::to_vec(&serde_json::json!({"prompt":prompt}))?;
    let mut stdin = child
        .stdin
        .take()
        .context("open Qoder worker input stream")?;
    stdin.write_all(&payload).await?;
    stdin.write_all(b"\n").await?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .context("open Qoder worker output stream")?;
    let mut lines = BufReader::new(stdout).lines();
    while let Some(line) = lines.next_line().await? {
        let event: serde_json::Value =
            serde_json::from_str(&line).context("Qoder worker emitted invalid JSON")?;
        match event["type"].as_str().unwrap_or_default() {
            "text" => {
                print!("{}", event["text"].as_str().unwrap_or_default());
                std::io::stdout().flush()?;
            }
            "tool" => {
                eprintln!(
                    "\n[Fairy tool] {}",
                    event["name"].as_str().unwrap_or("unknown")
                );
            }
            "result" => {
                eprintln!(
                    "\n[Qoder result] {}",
                    event["subtype"].as_str().unwrap_or("unknown")
                );
            }
            "usage" => eprintln!("[Qoder usage] {}", event),
            "error" => eprintln!(
                "[Qoder error] {}",
                event["message"].as_str().unwrap_or("unknown error")
            ),
            _ => {}
        }
    }

    let status = child.wait().await.context("wait for Qoder worker")?;
    anyhow::ensure!(
        status.success(),
        "Qoder Agent worker exited unsuccessfully: {status}"
    );
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
        Command::Workspace { action } => match action {
            WorkspaceCommand::Set { path } => {
                let canonical = path
                    .canonicalize()
                    .context("workspace directory does not exist")?;
                anyhow::ensure!(canonical.is_dir(), "workspace path must be a directory");
                let value = canonical.to_string_lossy().to_string();
                store.set_setting("workspace_root", &value)?;
                print_json(&serde_json::json!({"workspace_root":value,"status":"configured"}))?;
            }
            WorkspaceCommand::Show => {
                let root = store.get_setting("workspace_root")?;
                print_json(&serde_json::json!({"workspace_root":root}))?;
            }
            WorkspaceCommand::Clear => {
                let removed = store.remove_setting("workspace_root")?;
                print_json(&serde_json::json!({"removed":removed}))?;
            }
        },
        Command::Tool { action } => match action {
            ToolCommand::List => print_json(&tools::list_tools())?,
            ToolCommand::Call {
                name,
                input_json,
                approval_id,
            } => {
                let result = tools::execute(&store, &name, &input_json, approval_id.as_deref())?;
                print_json(&result)?;
            }
        },
        Command::Policy { action } => match action {
            PolicyCommand::List => print_json(&store.list_capabilities()?)?,
            PolicyCommand::Grant { capability, note } => {
                print_json(&store.grant_capability(&capability, &note)?)?;
            }
            PolicyCommand::Revoke { capability } => {
                anyhow::ensure!(
                    store.revoke_capability(&capability)?,
                    "capability was not granted: {capability}"
                );
                println!("revoked {capability}");
            }
            PolicyCommand::Check {
                capability,
                tool,
                effect,
                approval_id,
            } => {
                let granted = store.has_capability(&capability)?;
                let approved = match approval_id {
                    Some(id) => store.approval_is_approved(&id, &tool)?,
                    None => false,
                };
                let decision = policy::evaluate(granted, effect, approved);
                print_json(&serde_json::json!({
                    "capability": capability,
                    "tool": tool,
                    "effect": effect,
                    "capability_granted": granted,
                    "approval_valid": approved,
                    "decision": decision
                }))?;
            }
        },
        Command::Approval { action } => match action {
            ApprovalCommand::Request { action, reason } => {
                print_json(&store.create_approval(&action, &reason)?)?;
            }
            ApprovalCommand::List => print_json(&store.list_approvals()?)?,
            ApprovalCommand::Approve { id } => {
                anyhow::ensure!(
                    store.resolve_approval(&id, true)?,
                    "approval not found or already resolved"
                );
                println!("approved {id}");
            }
            ApprovalCommand::Deny { id } => {
                anyhow::ensure!(
                    store.resolve_approval(&id, false)?,
                    "approval not found or already resolved"
                );
                println!("denied {id}");
            }
        },
        Command::Agent { action } => {
            let agent_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("agent");
            match action {
                AgentCommand::Setup => agent_setup(&agent_dir).await?,
                AgentCommand::Ask { prompt } => {
                    agent_ask(&agent_dir, &data_dir, &store, &prompt).await?;
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
