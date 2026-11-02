use anyhow::{Context, Result};
use cap_std::{
    ambient_authority,
    fs::{Dir, File},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::Read,
    path::{Component, Path, PathBuf},
};
use tracing::info;

use crate::{
    policy::{self, AuthorizationDecision, SideEffect},
    store::Store,
};

const MAX_TEXT_FILE_BYTES: u64 = 1024 * 1024;
const MAX_TOOL_INPUT_BYTES: usize = 16 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 1000;

#[derive(Debug, Clone, Serialize)]
pub struct ToolDescriptor {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub required_capability: &'static str,
    pub side_effect: SideEffect,
    pub input_schema: Value,
}

fn registry() -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            name: "filesystem.list",
            version: "1",
            description: "List one directory inside the configured workspace without following child symlinks.",
            required_capability: "filesystem.read",
            side_effect: SideEffect::ReadOnly,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type":"string","description":"Workspace-relative directory; defaults to '.'"}
                },
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "filesystem.read_text",
            version: "1",
            description: "Read a UTF-8 text file up to 1 MiB inside the configured workspace.",
            required_capability: "filesystem.read",
            side_effect: SideEffect::ReadOnly,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type":"string","description":"Workspace-relative file path"}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
    ]
}

pub fn list_tools() -> Vec<ToolDescriptor> {
    registry()
}

/// Execute one registered tool through the capability and approval policy.
/// This deliberately has no shell, arbitrary process, or unrestricted path tool.
pub fn execute(
    store: &Store,
    name: &str,
    input_json: &str,
    approval_id: Option<&str>,
) -> Result<Value> {
    anyhow::ensure!(
        input_json.len() <= MAX_TOOL_INPUT_BYTES,
        "tool input exceeds the 16 KiB limit"
    );
    let descriptor = registry()
        .into_iter()
        .find(|tool| tool.name == name)
        .with_context(|| format!("unknown tool: {name}"))?;

    let granted = store.has_capability(descriptor.required_capability)?;
    let approval_valid = match approval_id {
        Some(id) => store.approval_is_approved(id, descriptor.name)?,
        None => false,
    };
    match policy::evaluate(granted, descriptor.side_effect, approval_valid) {
        AuthorizationDecision::Allowed => {}
        AuthorizationDecision::ApprovalRequired(reason) => {
            store.record_audit_event(
                "tool.execution_denied",
                &json!({"tool":descriptor.name,"reason":"approval_required"}).to_string(),
            )?;
            anyhow::bail!("approval required: {reason}");
        }
        AuthorizationDecision::Denied(reason) => {
            store.record_audit_event(
                "tool.execution_denied",
                &json!({"tool":descriptor.name,"reason":"capability_or_policy_denied"}).to_string(),
            )?;
            anyhow::bail!("tool denied: {reason}");
        }
    }

    let needs_approval = matches!(
        descriptor.side_effect,
        SideEffect::Destructive | SideEffect::Administrator | SideEffect::ExternalCommunication
    );
    if needs_approval {
        let id = approval_id.context("approval ID is required")?;
        anyhow::ensure!(
            store.consume_approval(id, descriptor.name)?,
            "approval is invalid, already consumed, or scoped to another action"
        );
    }

    let root = workspace_root(store)?;
    store.record_audit_event(
        "tool.execution_started",
        &json!({"tool":descriptor.name,"workspace_configured":true}).to_string(),
    )?;
    info!(tool = descriptor.name, "tool execution started");

    let result = match descriptor.name {
        "filesystem.list" => list_directory(&root, input_json),
        "filesystem.read_text" => read_text_file(&root, input_json),
        _ => anyhow::bail!("tool registry entry has no executor: {}", descriptor.name),
    };

    match result {
        Ok(output) => {
            store.record_audit_event(
                "tool.execution_completed",
                &json!({"tool":descriptor.name,"success":true}).to_string(),
            )?;
            Ok(output)
        }
        Err(error) => {
            store.record_audit_event(
                "tool.execution_failed",
                &json!({"tool":descriptor.name,"success":false}).to_string(),
            )?;
            Err(error)
        }
    }
}

/// Open the configured workspace once and retain a directory capability.
/// Subsequent lookups are relative to this handle and cannot escape via parent
/// traversal or an external symlink.
fn workspace_root(store: &Store) -> Result<Dir> {
    let value = store
        .get_setting("workspace_root")?
        .context("workspace is not configured; use 'fairy workspace set <directory>'")?;
    Dir::open_ambient_dir(Path::new(&value), ambient_authority())
        .context("open configured workspace directory")
}

fn resolve_workspace_path(relative: &str) -> Result<PathBuf> {
    anyhow::ensure!(!relative.trim().is_empty(), "path cannot be empty");
    let relative = Path::new(relative);
    anyhow::ensure!(!relative.is_absolute(), "absolute paths are not allowed");

    for component in relative.components() {
        anyhow::ensure!(
            matches!(component, Component::Normal(_) | Component::CurDir),
            "path traversal and rooted path components are not allowed"
        );
    }
    Ok(relative.to_path_buf())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListInput {
    #[serde(default = "current_directory")]
    path: String,
}

fn current_directory() -> String {
    ".".to_owned()
}

fn list_directory(root: &Dir, input_json: &str) -> Result<Value> {
    let input: ListInput = serde_json::from_str(input_json)
        .context("filesystem.list input must be a JSON object with an optional path")?;
    let relative = resolve_workspace_path(&input.path)?;
    let mut entries = Vec::new();
    for entry in root
        .read_dir(&relative)
        .context("read workspace directory")?
    {
        anyhow::ensure!(
            entries.len() < MAX_DIRECTORY_ENTRIES,
            "directory exceeds the 1000-entry listing limit"
        );
        let entry = entry?;
        let file_type = entry
            .file_type()
            .context("inspect workspace directory entry")?;
        let kind = if file_type.is_dir() {
            "directory"
        } else if file_type.is_file() {
            "file"
        } else if file_type.is_symlink() {
            "symlink"
        } else {
            "other"
        };
        let size_bytes = if file_type.is_file() {
            Some(entry.metadata()?.len())
        } else {
            None
        };
        entries.push(json!({
            "name": entry.file_name().to_string_lossy(),
            "kind": kind,
            "size_bytes": size_bytes
        }));
    }
    entries.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["name"].as_str().unwrap_or_default())
    });
    Ok(json!({
        "path": input.path,
        "entry_count": entries.len(),
        "entries": entries
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadInput {
    path: String,
}

fn ensure_not_sensitive_path(relative: &Path) -> Result<()> {
    for component in relative.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        let name = name.to_string_lossy().to_ascii_lowercase();
        let extension = Path::new(&name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();

        let sensitive_directory = matches!(
            name.as_str(),
            ".git"
                | ".ssh"
                | ".aws"
                | ".azure"
                | ".gnupg"
                | ".kube"
                | ".docker"
                | ".npmrc"
                | ".pypirc"
                | ".netrc"
                | "credentials"
                | "secrets"
        );
        let sensitive_file = name == ".env"
            || name.starts_with(".env.")
            || matches!(
                name.as_str(),
                "id_rsa"
                    | "id_ed25519"
                    | "id_ecdsa"
                    | "credentials.json"
                    | "secrets.json"
                    | "authorized_keys"
            )
            || matches!(extension, "pem" | "key" | "p12" | "pfx" | "jks" | "kdbx");
        anyhow::ensure!(
            !sensitive_directory && !sensitive_file,
            "reading likely credential or secret material is blocked: {}",
            relative.display()
        );
    }
    Ok(())
}

fn read_text_file(root: &Dir, input_json: &str) -> Result<Value> {
    let input: ReadInput = serde_json::from_str(input_json)
        .context("filesystem.read_text requires a JSON path field")?;
    let relative = resolve_workspace_path(&input.path)?;
    ensure_not_sensitive_path(&relative)?;
    let file: File = root
        .open(&relative)
        .context("open workspace text file through its directory capability")?;
    let metadata = file.metadata().context("inspect workspace file")?;
    anyhow::ensure!(
        metadata.is_file(),
        "requested workspace path is not a regular file"
    );
    anyhow::ensure!(
        metadata.len() <= MAX_TEXT_FILE_BYTES,
        "file exceeds the 1 MiB text-read limit"
    );
    let mut text = String::new();
    file.take(MAX_TEXT_FILE_BYTES + 1)
        .read_to_string(&mut text)
        .context("file is not valid UTF-8 text or cannot be read")?;
    let bytes_read = u64::try_from(text.len()).context("text length could not be represented")?;
    anyhow::ensure!(
        bytes_read <= MAX_TEXT_FILE_BYTES,
        "file exceeded the 1 MiB text-read limit while being read"
    );
    Ok(json!({
        "path": input.path,
        "size_bytes": metadata.len(),
        "text": text
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn configured_store(root: &Path, grant: bool) -> Store {
        let store = Store::open(Path::new(":memory:")).unwrap();
        store
            .set_setting("workspace_root", root.to_str().unwrap())
            .unwrap();
        if grant {
            store
                .grant_capability("filesystem.read", "Approved workspace read access")
                .unwrap();
        }
        store
    }

    #[test]
    fn registry_is_typed_and_contains_only_bounded_read_tools() {
        let tools = list_tools();
        assert_eq!(tools.len(), 2);
        assert!(tools
            .iter()
            .all(|tool| tool.side_effect == SideEffect::ReadOnly));
        assert!(tools.iter().any(|tool| tool.name == "filesystem.list"));
        assert!(tools.iter().any(|tool| tool.name == "filesystem.read_text"));
    }

    #[test]
    fn read_text_requires_grant_and_reads_only_workspace_files() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("notes.txt"), "Fairy workspace test").unwrap();
        let denied = configured_store(directory.path(), false);
        assert!(execute(
            &denied,
            "filesystem.read_text",
            r#"{"path":"notes.txt"}"#,
            None
        )
        .is_err());

        let allowed = configured_store(directory.path(), true);
        let result = execute(
            &allowed,
            "filesystem.read_text",
            r#"{"path":"notes.txt"}"#,
            None,
        )
        .unwrap();
        assert_eq!(result["text"], "Fairy workspace test");
    }

    #[test]
    fn rejects_absolute_parent_and_unknown_json_fields() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("notes.txt"), "safe").unwrap();
        let store = configured_store(directory.path(), true);
        assert!(execute(
            &store,
            "filesystem.read_text",
            r#"{"path":"../notes.txt"}"#,
            None
        )
        .is_err());
        assert!(execute(
            &store,
            "filesystem.read_text",
            r#"{"path":"/etc/passwd"}"#,
            None
        )
        .is_err());
        assert!(execute(
            &store,
            "filesystem.read_text",
            r#"{"path":"notes.txt","extra":true}"#,
            None
        )
        .is_err());
    }

    #[test]
    fn list_directory_returns_sorted_names_without_following_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("z.txt"), "z").unwrap();
        fs::write(directory.path().join("a.txt"), "a").unwrap();
        fs::create_dir(directory.path().join("folder")).unwrap();
        let store = configured_store(directory.path(), true);
        let result = execute(&store, "filesystem.list", r#"{"path":"."}"#, None).unwrap();
        assert_eq!(result["entry_count"], 3);
        assert_eq!(result["entries"][0]["name"], "a.txt");
        assert_eq!(result["entries"][2]["name"], "z.txt");
    }

    #[test]
    fn blocks_common_secret_and_credential_paths_before_opening() {
        let workspace = tempfile::tempdir().unwrap();
        fs::create_dir(workspace.path().join("secrets")).unwrap();
        fs::write(workspace.path().join(".env"), "API_TOKEN=never-read").unwrap();
        fs::write(workspace.path().join("private.pem"), "private-key").unwrap();
        fs::write(
            workspace.path().join("secrets").join("service.txt"),
            "secret",
        )
        .unwrap();
        let store = configured_store(workspace.path(), true);

        for path in [".env", "private.pem", "secrets/service.txt", ".git/config"] {
            assert!(
                execute(
                    &store,
                    "filesystem.read_text",
                    &format!(r#"{{"path":"{path}"}}"#),
                    None
                )
                .is_err(),
                "sensitive path unexpectedly allowed: {path}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape_from_workspace() {
        use std::os::unix::fs::symlink;
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "outside").unwrap();
        symlink(outside.path(), workspace.path().join("external")).unwrap();
        let store = configured_store(workspace.path(), true);
        assert!(execute(
            &store,
            "filesystem.read_text",
            r#"{"path":"external/secret.txt"}"#,
            None
        )
        .is_err());
    }
}
