use std::fs;
use std::process::Command;

enum GitAvailability {
    Available,
    NotInstalled,
    NotARepo,
}

/// Reads a schema source, which is either a plain file path or a git
/// `<rev>:<path>` reference (e.g. `HEAD:schema.xml`, `v1.2.0:schema/schema.xml`).
///
/// A value is only treated as a git reference when it contains a `:` *and*
/// the part before the first `:` resolves to a real commit - this keeps
/// plain file paths (including Windows drive letters like `C:\schema.xml`)
/// working unchanged. If git isn't installed or the current directory isn't
/// a git repository, a value that looks like `<rev>:<path>` is treated as a
/// plain file path too, so the resulting "file not found" error stays
/// accurate about what was actually tried.
pub fn read_schema_source(source: &str) -> Result<String, String> {
    if let Some((rev, path)) = maybe_git_ref(source) {
        return read_from_git(rev, path, source);
    }

    fs::read_to_string(source).map_err(|err| format!("failed to read schema file '{}': {}", source, err))
}

/// Reads `path` from disk and its `HEAD` version from git, for the `--file`
/// shortcut. Unlike `read_schema_source`, git usage here is unambiguous (the
/// caller explicitly asked to compare against HEAD), so a missing git
/// binary or missing repository is reported as a clear, specific error
/// instead of silently falling back to reading a literal `HEAD:<path>` file.
pub fn read_file_against_head(path: &str) -> Result<(String, String), String> {
    match check_git_availability() {
        GitAvailability::NotInstalled => {
            return Err("--file requires git, but the 'git' command was not found on PATH.".to_string());
        }
        GitAvailability::NotARepo => {
            return Err("--file requires a git repository, but the current directory is not inside one.".to_string());
        }
        GitAvailability::Available => {}
    }

    if !git_rev_exists("HEAD") {
        return Err("--file requires a commit at HEAD, but this repository has no commits yet.".to_string());
    }

    let source = format!("HEAD:{}", path);
    let old = read_from_git("HEAD", path, &source)?;
    let new = fs::read_to_string(path).map_err(|err| format!("failed to read schema file '{}': {}", path, err))?;
    Ok((old, new))
}

fn maybe_git_ref(source: &str) -> Option<(&str, &str)> {
    let (rev, path) = source.split_once(':')?;
    if rev.is_empty() || path.is_empty() {
        return None;
    }
    if !matches!(check_git_availability(), GitAvailability::Available) {
        return None;
    }
    if !git_rev_exists(rev) {
        return None;
    }
    Some((rev, path))
}

fn check_git_availability() -> GitAvailability {
    match Command::new("git").args(["rev-parse", "--is-inside-work-tree"]).output() {
        Ok(output) if output.status.success() => GitAvailability::Available,
        Ok(_) => GitAvailability::NotARepo,
        Err(_) => GitAvailability::NotInstalled,
    }
}

fn git_rev_exists(rev: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", &format!("{}^{{commit}}", rev)])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn read_from_git(rev: &str, path: &str, source: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["show", &format!("{}:{}", rev, path)])
        .output()
        .map_err(|err| format!("failed to run git for '{}': {}", source, err))?;

    if !output.status.success() {
        return Err(format!(
            "failed to read '{}' from git: {}",
            source,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    String::from_utf8(output.stdout).map_err(|err| format!("'{}' is not valid UTF-8: {}", source, err))
}
