use anyhow::{Context, Result};
use std::{fs, path::{Path, PathBuf}};

/// Local, offline emergency-stop latch for Fairy-managed agent work.
/// It intentionally does not stop unrelated processes or user applications.
#[derive(Debug, Clone)]
pub struct StopLatch {
    marker: PathBuf,
}

impl StopLatch {
    pub fn new(data_dir: &Path) -> Self {
        Self { marker: data_dir.join("STOP_ALL") }
    }

    pub fn is_stopped(&self) -> bool {
        self.marker.is_file()
    }

    pub fn activate(&self) -> Result<()> {
        let parent = self.marker.parent().context("stop marker has no parent directory")?;
        fs::create_dir_all(parent)?;
        if !self.marker.exists() {
            let token = uuid::Uuid::new_v4().to_string();
            fs::write(&self.marker, format!("{token}\n")).context("write emergency stop latch")?;
        }
        Ok(())
    }

    /// Resume is explicit. Starting/restarting the runtime never removes this latch.
    pub fn resume(&self) -> Result<bool> {
        if self.marker.exists() {
            fs::remove_file(&self.marker).context("remove emergency stop latch")?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latch_blocks_until_explicit_resume_and_is_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let latch = StopLatch::new(directory.path());
        assert!(!latch.is_stopped());

        latch.activate().unwrap();
        assert!(latch.is_stopped());
        let marker = fs::read_to_string(directory.path().join("STOP_ALL")).unwrap();
        assert!(!marker.trim().is_empty());

        // Repeated activation does not silently reset or remove the stop request.
        latch.activate().unwrap();
        assert_eq!(fs::read_to_string(directory.path().join("STOP_ALL")).unwrap(), marker);

        assert!(latch.resume().unwrap());
        assert!(!latch.is_stopped());
        assert!(!latch.resume().unwrap());
    }

    #[test]
    fn latch_is_shared_by_independent_cli_handles() {
        let directory = tempfile::tempdir().unwrap();
        let cli = StopLatch::new(directory.path());
        let worker_supervisor = StopLatch::new(directory.path());
        cli.activate().unwrap();
        assert!(worker_supervisor.is_stopped());
        assert!(cli.resume().unwrap());
        assert!(!worker_supervisor.is_stopped());
    }
}
