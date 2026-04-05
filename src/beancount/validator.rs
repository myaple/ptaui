use std::path::Path;
use std::process::Command;

pub enum CheckResult {
    Ok,
    Errors(Vec<String>),
    NotInstalled,
}

/// Run `bean-check` on the given file. Returns parsed error lines.
pub fn bean_check(path: &Path) -> CheckResult {
    match Command::new("bean-check").arg(path).output() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => CheckResult::NotInstalled,
        Err(_) => CheckResult::NotInstalled,
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let combined = format!("{}{}", stderr, stdout);
            if output.status.success() && combined.trim().is_empty() {
                CheckResult::Ok
            } else {
                let errors: Vec<String> = combined
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| l.to_string())
                    .collect();
                CheckResult::Errors(errors)
            }
        }
    }
}
