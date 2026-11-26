// Workspaces: named groups of repositories the UI keeps, with a summary of
// each (branch, ahead/behind, uncommitted changes) and fetch or pull across
// all of them at once. Repositories run in parallel, a few at a time.

use super::*;

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoSummary {
    pub path: String,
    pub ok: bool,
    pub error: Option<String>,
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub changes: usize,
    pub conflicts: usize,
}

pub(crate) fn summarize_repo(path: &Path) -> RepoSummary {
    let mut summary = RepoSummary { path: path.display().to_string(), ..Default::default() };
    if !path.is_absolute() {
        summary.error = Some("path must be absolute".to_string());
        return summary;
    }
    match git(path, &["--no-optional-locks", "status", "--porcelain=v2", "--branch"]) {
        Ok(out) => {
            summary.ok = true;
            for line in out.lines() {
                if let Some(name) = line.strip_prefix("# branch.head ") {
                    summary.branch = (name != "(detached)").then(|| name.to_string());
                } else if let Some(upstream) = line.strip_prefix("# branch.upstream ") {
                    summary.upstream = Some(upstream.to_string());
                } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
                    for part in ab.split_whitespace() {
                        if let Some(value) = part.strip_prefix('+') {
                            summary.ahead = value.parse().unwrap_or(0);
                        } else if let Some(value) = part.strip_prefix('-') {
                            summary.behind = value.parse().unwrap_or(0);
                        }
                    }
                } else if line.starts_with("u ") {
                    summary.conflicts += 1;
                    summary.changes += 1;
                } else if !line.starts_with('#') && !line.trim().is_empty() {
                    summary.changes += 1;
                }
            }
        }
        Err(err) => summary.error = Some(err.trim().to_string()),
    }
    summary
}

/// Runs `work` for every path, at most four at a time, keeping input order.
fn in_parallel<T: Send>(paths: &[String], work: impl Fn(&Path) -> T + Sync) -> Vec<T> {
    let mut results: Vec<Option<T>> = (0..paths.len()).map(|_| None).collect();
    for (chunk_index, chunk) in paths.chunks(4).enumerate() {
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunk.iter().map(|path| scope.spawn(|| work(Path::new(path)))).collect();
            for (offset, handle) in handles.into_iter().enumerate() {
                if let Ok(value) = handle.join() {
                    results[chunk_index * 4 + offset] = Some(value);
                }
            }
        });
    }
    results.into_iter().flatten().collect()
}

#[tauri::command(async)]
pub fn workspace_status(paths: Vec<String>) -> Vec<RepoSummary> {
    in_parallel(&paths, summarize_repo)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResult {
    pub path: String,
    pub ok: bool,
    pub message: String,
}

/// `kind`: "fetch" (all remotes, prune) or "pull" (fast-forward only, so a
/// repository that needs a merge is reported instead of changed).
#[tauri::command(async)]
pub fn workspace_run(paths: Vec<String>, kind: String) -> Result<Vec<WorkspaceResult>, String> {
    let args: &[&str] = match kind.as_str() {
        "fetch" => &["fetch", "--all", "--prune"],
        "pull" => &["pull", "--ff-only"],
        other => return Err(format!("unsupported workspace action: {other}")),
    };
    Ok(in_parallel(&paths, |path| {
        if !path.is_absolute() {
            return WorkspaceResult { path: path.display().to_string(), ok: false, message: "path must be absolute".to_string() };
        }
        let result = run_git(path, args);
        let message = if result.ok { result.stdout.trim().to_string() } else { result.stderr.trim().to_string() };
        WorkspaceResult { path: path.display().to_string(), ok: result.ok, message }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{commit_all, run, write_file, TempRepo};

    #[test]
    fn summarizes_branch_changes_and_tracking() {
        let origin = TempRepo::new();
        write_file(origin.path(), "a.txt", "a\n");
        commit_all(origin.path(), "init");
        let clone = TempRepo::new();
        run(clone.path(), &["remote", "add", "origin", &origin.path().display().to_string()]);
        run(clone.path(), &["fetch", "origin"]);
        run(clone.path(), &["checkout", "-b", "main-work", "--track", "origin/main"]);
        write_file(origin.path(), "a.txt", "b\n");
        commit_all(origin.path(), "upstream change");
        run(clone.path(), &["fetch", "origin"]);
        write_file(clone.path(), "new.txt", "n\n");

        let paths = vec![clone.path().display().to_string(), "/definitely/not/a/repo".to_string()];
        let summaries = workspace_status(paths.clone());
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].branch.as_deref(), Some("main-work"));
        assert_eq!(summaries[0].behind, 1);
        assert_eq!(summaries[0].changes, 1);
        assert!(!summaries[1].ok && summaries[1].error.is_some());

        let pulled = workspace_run(vec![clone.path().display().to_string()], "pull".into()).unwrap();
        assert!(pulled[0].ok, "{}", pulled[0].message);
        assert_eq!(workspace_status(vec![clone.path().display().to_string()])[0].behind, 0);
        assert!(workspace_run(vec![], "push".into()).is_err());
    }
}
