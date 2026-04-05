use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Check whether `dir` (or any parent) is inside a git working tree.
pub fn is_git_repo(dir: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run `git init` in `dir`.
pub fn init_repo(dir: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("init")
        .current_dir(dir)
        .output()
        .context("Running git init")?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        Ok(if stdout.is_empty() { stderr } else { stdout })
    } else {
        anyhow::bail!("git init failed: {}", stderr)
    }
}

/// Stage and commit a single file. Creates a commit with the given message.
/// Silently succeeds if git is not installed or not a repo.
pub fn commit_file(file: &Path, message: &str) -> Result<()> {
    let dir = file
        .parent()
        .context("beancount file has no parent directory")?;

    // git add <file>
    let add = Command::new("git")
        .args(["add", "--"])
        .arg(file)
        .current_dir(dir)
        .output()
        .context("git add")?;

    if !add.status.success() {
        let err = String::from_utf8_lossy(&add.stderr);
        anyhow::bail!("git add failed: {}", err);
    }

    // git commit -m <message>
    let commit = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(dir)
        .output()
        .context("git commit")?;

    if !commit.status.success() {
        let err = String::from_utf8_lossy(&commit.stderr);
        // "nothing to commit" is not an error worth surfacing
        if err.contains("nothing to commit") || err.contains("nothing added to commit") {
            return Ok(());
        }
        anyhow::bail!("git commit failed: {}", err);
    }

    Ok(())
}
