use anyhow::{Context, Result};
use serde_json::{json, Value};
#[cfg(target_os = "linux")]
use std::path::PathBuf;
use std::{path::Path, time::Duration};
#[cfg(any(windows, target_os = "linux"))]
use tokio::process::Command;

#[cfg(windows)]
const WINDOWS_TASK: &str = "ComputerHarnessFairy";
#[cfg(target_os = "linux")]
const LINUX_UNIT: &str = "fairy.service";

/// Install an opt-in, per-user startup entry. No installation happens implicitly.
pub async fn install(_data_dir: &Path) -> Result<String> {
    #[cfg(windows)]
    {
        let executable = std::env::current_exe()
            .context("locate the Fairy executable")?
            .canonicalize()
            .context("resolve the Fairy executable path")?;
        let task_command = format!("\"{}\" serve", executable.display());
        let status = Command::new("schtasks")
            .args([
                "/Create",
                "/TN",
                WINDOWS_TASK,
                "/SC",
                "ONLOGON",
                "/RL",
                "LIMITED",
                "/TR",
            ])
            .arg(task_command)
            .arg("/F")
            .status()
            .await
            .context("start Windows Task Scheduler command")?;
        anyhow::ensure!(
            status.success(),
            "could not register Fairy at user logon: {status}"
        );
        if !crate::ipc::is_running(_data_dir).await {
            let start = Command::new("schtasks")
                .args(["/Run", "/TN", WINDOWS_TASK])
                .status()
                .await
                .context("start Fairy after registering its logon task")?;
            anyhow::ensure!(
                start.success(),
                "could not start Fairy after installation: {start}"
            );
            wait_for_runtime(_data_dir).await?;
        }
        Ok(format!(
            "Installed and started user logon task '{WINDOWS_TASK}'."
        ))
    }

    #[cfg(target_os = "linux")]
    {
        let executable = std::env::current_exe()
            .context("locate the Fairy executable")?
            .canonicalize()
            .context("resolve the Fairy executable path")?;
        let unit_path = linux_unit_path()?;
        let parent = unit_path
            .parent()
            .context("resolve systemd user unit directory")?;
        std::fs::create_dir_all(parent)?;
        let contents = linux_unit_contents(&executable)?;
        let temporary = unit_path.with_extension("service.tmp");
        std::fs::write(&temporary, contents).context("write temporary systemd user unit")?;
        std::fs::rename(&temporary, &unit_path).context("publish systemd user unit")?;
        systemctl(&["daemon-reload"]).await?;
        if let Err(error) = systemctl(&["enable", "--now", LINUX_UNIT]).await {
            let _ = std::fs::remove_file(&unit_path);
            let _ = systemctl(&["daemon-reload"]).await;
            return Err(error).context("enable Fairy's systemd user service");
        }
        wait_for_runtime(_data_dir).await?;
        Ok(format!(
            "Installed, enabled and started '{LINUX_UNIT}' at {}",
            unit_path.display()
        ))
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = data_dir;
        anyhow::bail!(
            "automatic service installation is currently supported on Windows and Linux only"
        )
    }
}

/// Remove startup registration while preserving Fairy's database and configuration.
pub async fn uninstall(_data_dir: &Path) -> Result<String> {
    #[cfg(windows)]
    {
        if crate::ipc::is_running(_data_dir).await {
            crate::ipc::request(_data_dir, crate::ipc::IpcCommand::Shutdown).await?;
        }
        let status = Command::new("schtasks")
            .args(["/Delete", "/TN", WINDOWS_TASK, "/F"])
            .status()
            .await
            .context("start Windows Task Scheduler command")?;
        anyhow::ensure!(
            status.success(),
            "could not remove Fairy's scheduled task: {status}"
        );
        Ok(format!("Removed user logon task '{WINDOWS_TASK}'."))
    }

    #[cfg(target_os = "linux")]
    {
        let unit_path = linux_unit_path()?;
        if unit_path.exists() {
            systemctl(&["disable", "--now", LINUX_UNIT]).await?;
            std::fs::remove_file(&unit_path).context("remove systemd user unit")?;
            systemctl(&["daemon-reload"]).await?;
            Ok(format!(
                "Removed '{LINUX_UNIT}'. Existing task data was preserved."
            ))
        } else {
            Ok(format!(
                "'{LINUX_UNIT}' is not installed; task data was preserved."
            ))
        }
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = data_dir;
        anyhow::bail!(
            "automatic service management is currently supported on Windows and Linux only"
        )
    }
}

pub async fn start(_data_dir: &Path) -> Result<String> {
    #[cfg(windows)]
    {
        let status = Command::new("schtasks")
            .args(["/Run", "/TN", WINDOWS_TASK])
            .status()
            .await
            .context("start Windows Task Scheduler command")?;
        anyhow::ensure!(
            status.success(),
            "could not start Fairy's scheduled task: {status}"
        );
        wait_for_runtime(_data_dir).await?;
        Ok("Fairy started through Windows Task Scheduler.".to_owned())
    }

    #[cfg(target_os = "linux")]
    {
        systemctl(&["start", LINUX_UNIT]).await?;
        wait_for_runtime(_data_dir).await?;
        Ok("Fairy started through systemd.".to_owned())
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = _data_dir;
        anyhow::bail!(
            "automatic service management is currently supported on Windows and Linux only"
        )
    }
}

pub async fn stop(data_dir: &Path) -> Result<String> {
    #[cfg(windows)]
    {
        if crate::ipc::is_running(data_dir).await {
            crate::ipc::request(data_dir, crate::ipc::IpcCommand::Shutdown).await?;
            Ok("Fairy shutdown requested through authenticated local IPC.".to_owned())
        } else {
            Ok("Fairy is not currently running.".to_owned())
        }
    }

    #[cfg(target_os = "linux")]
    {
        let _ = data_dir;
        systemctl(&["stop", LINUX_UNIT]).await?;
        Ok("Fairy stop requested through systemd.".to_owned())
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = data_dir;
        anyhow::bail!(
            "automatic service management is currently supported on Windows and Linux only"
        )
    }
}

async fn wait_for_runtime(data_dir: &Path) -> Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if crate::ipc::is_running(data_dir).await {
            return Ok(());
        }
        anyhow::ensure!(
            tokio::time::Instant::now() < deadline,
            "Fairy service manager started the process, but the control plane did not become reachable within 10 seconds"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

pub async fn status(data_dir: &Path) -> Result<Value> {
    let running = crate::ipc::is_running(data_dir).await;

    #[cfg(windows)]
    {
        let output = Command::new("schtasks")
            .args(["/Query", "/TN", WINDOWS_TASK, "/FO", "LIST"])
            .output()
            .await
            .context("query Windows Task Scheduler")?;
        Ok(json!({
            "manager": "windows_task_scheduler",
            "installed": output.status.success(),
            "running": running,
            "details": String::from_utf8_lossy(&output.stdout).trim()
        }))
    }

    #[cfg(target_os = "linux")]
    {
        let unit_path = linux_unit_path()?;
        let output = Command::new("systemctl")
            .args(["--user", "is-active", LINUX_UNIT])
            .output()
            .await
            .context("query systemd user service")?;
        let manager_state = if output.stdout.is_empty() {
            String::from_utf8_lossy(&output.stderr).trim().to_owned()
        } else {
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        };
        Ok(json!({
            "manager": "systemd_user",
            "installed": unit_path.is_file(),
            "running": running,
            "manager_state": manager_state,
            "unit_path": unit_path
        }))
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = running;
        Ok(json!({
            "manager": "unsupported",
            "installed": false,
            "running": false,
            "message": "Automatic service management is currently supported on Windows and Linux only."
        }))
    }
}

#[cfg(target_os = "linux")]
fn linux_unit_path() -> Result<PathBuf> {
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.config_dir().to_path_buf()))
        .context("could not determine the user config directory")?;
    anyhow::ensure!(
        config_dir.is_absolute(),
        "XDG_CONFIG_HOME must be an absolute path"
    );
    Ok(config_dir.join("systemd").join("user").join(LINUX_UNIT))
}

#[cfg(target_os = "linux")]
fn linux_unit_contents(executable: &Path) -> Result<String> {
    let executable = executable
        .to_str()
        .context("Fairy executable path is not valid UTF-8 for the systemd unit")?;
    anyhow::ensure!(
        !executable.contains('\n') && !executable.contains('\r'),
        "Fairy executable path contains a line break"
    );
    let escaped = executable
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%")
        .replace('$', "$$");
    Ok(format!(
        "[Unit]\nDescription=Computer Harness Fairy Runtime\nAfter=default.target\n\n[Service]\nType=simple\nExecStart=\"{escaped}\" serve\nRestart=on-failure\nRestartSec=3\nKillSignal=SIGINT\nTimeoutStopSec=15\nUMask=0077\n\n[Install]\nWantedBy=default.target\n"
    ))
}

#[cfg(target_os = "linux")]
async fn systemctl(args: &[&str]) -> Result<()> {
    let status = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()
        .await
        .context("start systemctl --user")?;
    anyhow::ensure!(
        status.success(),
        "systemctl --user {:?} failed: {status}",
        args
    );
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn systemd_unit_quotes_paths_and_enables_restart() {
        let path = Path::new("/home/test/Fairy Runtime/bin/fairy");
        let unit = linux_unit_contents(path).unwrap();
        assert!(unit.contains("ExecStart=\"/home/test/Fairy Runtime/bin/fairy\" serve"));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("UMask=0077"));
    }

    #[test]
    fn systemd_unit_escapes_specifiers_and_rejects_newlines() {
        let path = Path::new("/home/test/100%/$fairy");
        let unit = linux_unit_contents(path).unwrap();
        assert!(unit.contains("100%%/$$fairy"));
        assert!(linux_unit_contents(Path::new("/tmp/fairy\nservice")).is_err());
    }
}
