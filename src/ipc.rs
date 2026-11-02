use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::watch,
    time::timeout,
};
use tracing::{info, warn};

use crate::store::Store;

const TOKEN_FILE: &str = "ipc-token";
const ENDPOINT_FILE: &str = "ipc-endpoint";
const MAX_REQUEST_BYTES: usize = 64 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum IpcCommand {
    Ping,
    ListTasks,
    GetTask { id: String },
    CreateTask { title: String, objective: String },
    CancelTask { id: String },
    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    token: String,
    action: IpcCommand,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    pub message: String,
    pub data: Value,
}

impl Response {
    fn success(message: impl Into<String>, data: Value) -> Self {
        Self { ok: true, message: message.into(), data }
    }

    fn failure(message: impl Into<String>) -> Self {
        Self { ok: false, message: message.into(), data: Value::Null }
    }
}

/// Run the local control-plane listener in the foreground.
///
/// The listener binds exclusively to loopback. A random per-user token is stored
/// in the application data directory and is never returned by the protocol.
pub async fn serve(data_dir: &Path, db_path: PathBuf) -> Result<()> {
    fs::create_dir_all(data_dir)?;
    let token_path = data_dir.join(TOKEN_FILE);
    if !token_path.exists() {
        let token = uuid::Uuid::new_v4().simple().to_string();
        fs::write(&token_path, token.as_bytes()).context("write local IPC token")?;
        restrict_token_permissions(&token_path)?;
    }
    let token = fs::read_to_string(&token_path)
        .context("read local IPC token")?
        .trim()
        .to_owned();
    anyhow::ensure!(token.len() >= 32, "local IPC token is invalid; remove ipc-token and restart");

    let endpoint_path = data_dir.join(ENDPOINT_FILE);
    if endpoint_path.exists() {
        if request(data_dir, IpcCommand::Ping).await.is_ok() {
            anyhow::bail!("Fairy control plane is already running");
        }
        let _ = fs::remove_file(&endpoint_path);
    }

    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .context("bind local loopback control plane")?;
    let address = listener.local_addr()?;
    fs::write(&endpoint_path, address.to_string()).context("publish local IPC endpoint")?;
    info!(%address, "Fairy local control plane listening");

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    loop {
        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
            accepted = listener.accept() => {
                let (stream, peer) = accepted.context("accept local IPC connection")?;
                // The listener is bound to 127.0.0.1, so no remote interface is exposed.
                let token = token.clone();
                let db_path = db_path.clone();
                let shutdown = shutdown_tx.clone();
                tokio::spawn(async move {
                    if let Err(error) = handle_client(stream, token, db_path, shutdown).await {
                        warn!(%peer, error = %error, "local IPC request failed");
                    }
                });
            }
            signal = tokio::signal::ctrl_c() => {
                signal.context("wait for shutdown signal")?;
                info!("shutdown signal received");
                break;
            }
        }
    }

    let _ = fs::remove_file(endpoint_path);
    info!("Fairy local control plane stopped");
    Ok(())
}

pub async fn request(data_dir: &Path, action: IpcCommand) -> Result<Response> {
    let endpoint = fs::read_to_string(data_dir.join(ENDPOINT_FILE))
        .context("Fairy control plane endpoint is unavailable; start it with 'fairy serve'")?;
    let token = fs::read_to_string(data_dir.join(TOKEN_FILE))
        .context("Fairy local IPC token is unavailable")?;
    let address = endpoint.trim();
    let mut stream = timeout(Duration::from_secs(2), TcpStream::connect(address))
        .await
        .context("timed out connecting to Fairy control plane")?
        .context("could not connect to Fairy control plane")?;

    let request = Request { token: token.trim().to_owned(), action };
    let mut encoded = serde_json::to_vec(&request)?;
    encoded.push(b'\n');
    stream.write_all(&encoded).await?;
    stream.flush().await?;

    let mut line = String::new();
    let read = timeout(
        Duration::from_secs(3),
        BufReader::new(stream).take(MAX_REQUEST_BYTES as u64).read_line(&mut line),
    )
    .await
    .context("timed out waiting for Fairy control plane response")??;
    anyhow::ensure!(read > 0, "Fairy control plane closed the connection without a response");
    let response: Response = serde_json::from_str(&line).context("invalid control plane response")?;
    anyhow::ensure!(response.ok, "{}", response.message);
    Ok(response)
}

async fn handle_client(
    stream: TcpStream,
    expected_token: String,
    db_path: PathBuf,
    shutdown: watch::Sender<bool>,
) -> Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half).take(MAX_REQUEST_BYTES as u64);
    let mut line = String::new();
    let count = reader.read_line(&mut line).await?;
    let response = if count == 0 || count >= MAX_REQUEST_BYTES {
        Response::failure("request is empty or exceeds the 64 KiB limit")
    } else {
        match serde_json::from_str::<Request>(&line) {
            Ok(request) if request.token == expected_token => {
                dispatch(request.action, db_path, shutdown).await
            }
            Ok(_) => Response::failure("authentication failed"),
            Err(_) => Response::failure("malformed request"),
        }
    };
    let mut encoded = serde_json::to_vec(&response)?;
    encoded.push(b'\n');
    write_half.write_all(&encoded).await?;
    write_half.shutdown().await?;
    Ok(())
}

async fn dispatch(
    action: IpcCommand,
    db_path: PathBuf,
    shutdown: watch::Sender<bool>,
) -> Response {
    match action {
        IpcCommand::Ping => Response::success("control plane is running", json!({"state":"running"})),
        IpcCommand::Shutdown => {
            let _ = shutdown.send(true);
            Response::success("shutdown requested", json!({"state":"stopping"}))
        }
        action => {
            let result = tokio::task::spawn_blocking(move || -> Result<Value> {
                let store = Store::open(&db_path)?;
                match action {
                    IpcCommand::ListTasks => Ok(json!(store.list_tasks()?)),
                    IpcCommand::GetTask { id } => match store.get_task(&id)? {
                        Some(task) => Ok(json!(task)),
                        None => anyhow::bail!("task not found: {id}"),
                    },
                    IpcCommand::CreateTask { title, objective } => {
                        Ok(json!(store.create_task(&title, &objective)?))
                    }
                    IpcCommand::CancelTask { id } => {
                        anyhow::ensure!(store.cancel_task(&id)?, "task not found or no longer cancellable");
                        Ok(json!({"id":id,"state":"cancelled"}))
                    }
                    IpcCommand::Ping | IpcCommand::Shutdown => unreachable!("handled above"),
                }
            })
            .await;
            match result {
                Ok(Ok(data)) => Response::success("ok", data),
                Ok(Err(error)) => Response::failure(format!("{error:#}")),
                Err(error) => Response::failure(format!("database worker failed: {error}")),
            }
        }
    }
}

#[cfg(unix)]
fn restrict_token_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_token_permissions(_path: &Path) -> Result<()> {
    // On Windows the file inherits the current user's profile-directory ACL.
    // A dedicated Windows ACL implementation is still required before release.
    Ok(())
}
