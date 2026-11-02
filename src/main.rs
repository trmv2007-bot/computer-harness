mod ipc;
mod policy;
mod service;
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
use store::Store;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command as TokioCommand,
};
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
    /// Install and manage the optional user-level background service.
    Service {
        #[command(subcommand)]
        action: ServiceCommand,
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
enum ServiceCommand {
    /// Install and enable Fairy at user logon (Windows) or as a systemd user service (Linux).
    Install,
    /// Stop, disable and remove the user-level service registration.
    Uninstall,
    /// Start the installed service.
    Start,
    /// Stop the running service.
    Stop,
    /// Show service registration and runtime status.
    Status,
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
    /// Run a user-triggered, read-only Qoder inspection for a queued task.
    Inspect { id: String },
    /// Show the most recent persisted agent attempt for a task.
    Result { id: String },
    /// Accept a successful inspected result after owner review.
    Accept { id: String },
    /// Cancel a queued or paused task. Running attempts are not killed by this command.
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

struct AgentOutcome {
    result: String,
    usage: Option<serde_json::Value>,
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

async fn agent_ask(
    agent_dir: &Path,
    data_dir: &Path,
    store: &Store,
    prompt: &str,
) -> Result<AgentOutcome> {
    const AGENT_TIMEOUT: Duration = Duration::from_secs(5 * 60);

    anyhow::ensure!(
        prompt.len() <= 32 * 1024,
        "prompt exceeds the 32 KiB Qoder worker limit"
    );
    anyhow::ensure!(
        store.has_capability("filesystem.read")?,
        "filesystem.read capability must be explicitly granted before starting Fairy's Qoder worker"
    );
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

    let execution = tokio::time::timeout(AGENT_TIMEOUT, async {
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
        let mut final_result: Option<String> = None;
        let mut final_error: Option<String> = None;
        let mut usage: Option<serde_json::Value> = None;
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
                    let subtype = event["subtype"].as_str().unwrap_or("unknown");
                    let is_error = event["isError"].as_bool().unwrap_or(true);
                    eprintln!("\n[Qoder result] {subtype} (is_error={is_error})");
                    if subtype == "success" && !is_error {
                        final_result = event["result"].as_str().map(str::to_owned);
                    } else {
                        final_error = event["errors"]
                            .as_array()
                            .map(|items| {
                                items
                                    .iter()
                                    .filter_map(serde_json::Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join("; ")
                            })
                            .filter(|value| !value.is_empty())
                            .or_else(|| event["result"].as_str().map(str::to_owned))
                            .or_else(|| {
                                Some(format!("Qoder result ended with subtype '{subtype}'"))
                            });
                    }
                }
                "usage" => {
                    usage = Some(event.clone());
                    eprintln!("[Qoder usage] {}", event);
                }
                "error" => {
                    let message = event["message"]
                        .as_str()
                        .unwrap_or("unknown error")
                        .to_owned();
                    eprintln!("[Qoder error] {message}");
                    final_error = Some(message);
                }
                _ => {}
            }
        }

        let status = child.wait().await.context("wait for Qoder worker")?;
        if !status.success() {
            anyhow::bail!(
                "Qoder Agent worker exited unsuccessfully ({status}): {}",
                final_error
                    .as_deref()
                    .unwrap_or("no error detail was provided")
            );
        }
        if let Some(error) = final_error {
            anyhow::bail!("Qoder Agent request failed: {error}");
        }
        let result = final_result.context("Qoder worker exited without a successful result")?;
        anyhow::ensure!(
            !result.trim().is_empty(),
            "Qoder worker returned an empty successful result"
        );
        Ok::<AgentOutcome, anyhow::Error>(AgentOutcome { result, usage })
    })
    .await;

    match execution {
        Ok(result) => result,
        Err(_) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            anyhow::bail!(
                "Qoder Agent request exceeded the 5-minute execution limit; the worker process was terminated"
            );
        }
    }
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
                TaskCommand::Inspect { id } => {
                    let agent_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("agent");
                    let task = store
                        .get_task(&id)?
                        .with_context(|| format!("task not found: {id}"))?;
                    let prompt = format!(
                        "Inspect this queued task using only the provided read-only Fairy workspace tools.\n\nTitle: {}\nObjective: {}\n\nConstraints:\n- Do not modify, create, delete, or rename files.\n- Do not run shell commands or other processes.\n- Use only the provided workspace listing and text-reading tools.\n- Never read credential or secret files.\n- Distinguish verified observations from assumptions.\n- If the objective requires writing code or changing the computer, explain that this read-only pass cannot perform it and provide a concrete next-step plan.\n- Do not claim the objective is complete merely because this inspection finished.",
                        task.title, task.objective
                    );
                    let run = store.start_task_run(&id, &prompt)?;
                    match agent_ask(&agent_dir, &data_dir, &store, &prompt).await {
                        Ok(outcome) => {
                            let finished = store.finish_task_run(
                                &run.id,
                                true,
                                Some(&outcome.result),
                                None,
                                outcome.usage.as_ref(),
                            )?;
                            print_json(&finished)?;
                            eprintln!("Task is paused for owner review; the agent result is not proof of task completion.");
                        }
                        Err(error) => {
                            let detail = error.to_string();
                            if let Err(persist_error) = store.finish_task_run(
                                &run.id,
                                false,
                                None,
                                Some(&detail),
                                None,
                            ) {
                                return Err(error.context(format!(
                                    "additionally failed to persist the task-run failure: {persist_error}"
                                )));
                            }
                            return Err(error);
                        }
                    }
                }
                TaskCommand::Result { id } => {
                    let run = store
                        .latest_task_run(&id)?
                        .with_context(|| format!("no agent attempts recorded for task: {id}"))?;
                    print_json(&run)?;
                }
                TaskCommand::Accept { id } => {
                    let latest = store
                        .latest_task_run(&id)?
                        .with_context(|| format!("no agent attempts recorded for task: {id}"))?;
                    anyhow::ensure!(
                        latest.status == "succeeded",
                        "the latest agent attempt did not succeed; inspect the result or retry first"
                    );
                    let accepted = store.transition_task(&id, "completed")?;
                    print_json(&accepted)?;
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
        Command::Service { action } => match action {
            ServiceCommand::Install => println!("{}", service::install(&data_dir).await?),
            ServiceCommand::Uninstall => println!("{}", service::uninstall(&data_dir).await?),
            ServiceCommand::Start => println!("{}", service::start(&data_dir).await?),
            ServiceCommand::Stop => println!("{}", service::stop(&data_dir).await?),
            ServiceCommand::Status => print_json(&service::status(&data_dir).await?)?,
        },
        Command::Agent { action } => {
            let agent_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("agent");
            match action {
                AgentCommand::Setup => agent_setup(&agent_dir).await?,
                AgentCommand::Ask { prompt } => {
                    let _outcome = agent_ask(&agent_dir, &data_dir, &store, &prompt).await?;
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
