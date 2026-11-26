// Live updates: watch the open repository's working tree and git directory
// and tell the UI what kind of thing changed, so it refreshes only that part
// (status for an edited file, refs for a fetch or a commit made elsewhere).
//
// Loop safety: kGit's own reads must not write into the watched tree, or each
// refresh would trigger the next. `worktree_status` runs `status` with
// `--no-optional-locks` (no index stat refresh), and lock files are ignored.

use super::*;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult, Debouncer};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub(crate) type RepoWatcher = Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>;

/// Payload of the `repo-changed` event.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepoChange {
    pub root: String,
    /// A tracked or untracked (not ignored) working-tree file changed.
    pub worktree: bool,
    /// The index changed (stage, unstage, or a commit made elsewhere).
    pub index: bool,
    /// HEAD, a branch, tag, remote ref, stash or merge/rebase state changed.
    pub refs: bool,
}

impl RepoChange {
    fn any(&self) -> bool {
        self.worktree || self.index || self.refs
    }
}

/// What a changed path inside a git directory means for the UI, or None when
/// it is noise (objects, lock files, logs written alongside a ref update).
pub(crate) fn classify_git_path(relative: &Path) -> Option<&'static str> {
    let text = relative.to_string_lossy().replace('\\', "/");
    if text.ends_with(".lock") || text.starts_with("objects/") || text.starts_with("lfs/") {
        return None;
    }
    if text == "index" || (text.starts_with("worktrees/") && text.ends_with("/index")) {
        return Some("index");
    }
    if text == "HEAD"
        || text == "ORIG_HEAD"
        || text == "FETCH_HEAD"
        || text == "packed-refs"
        || text == "MERGE_HEAD"
        || text == "CHERRY_PICK_HEAD"
        || text == "REVERT_HEAD"
        || text.starts_with("refs/")
        || text.starts_with("rebase-merge")
        || text.starts_with("rebase-apply")
        || text.starts_with("worktrees/")
    {
        return Some("refs");
    }
    None
}

/// Folds one debounced batch of changed paths into a RepoChange. Working-tree
/// paths that git ignores (build output, node_modules) are dropped with one
/// `git check-ignore` call for the whole batch.
pub(crate) fn summarize(root: &Path, git_dirs: &[PathBuf], paths: &[PathBuf]) -> RepoChange {
    let mut change = RepoChange {
        root: root.display().to_string(),
        ..RepoChange::default()
    };
    let mut worktree_paths = Vec::new();
    'paths: for path in paths {
        for git_dir in git_dirs {
            if let Ok(relative) = path.strip_prefix(git_dir) {
                match classify_git_path(relative) {
                    Some("index") => change.index = true,
                    Some(_) => change.refs = true,
                    None => {}
                }
                continue 'paths;
            }
        }
        if let Ok(relative) = path.strip_prefix(root) {
            // A nested `.git` belongs to a submodule or another repository.
            if relative.components().any(|part| part.as_os_str() == ".git") {
                continue;
            }
            if !relative.as_os_str().is_empty() {
                worktree_paths.push(relative.to_string_lossy().to_string());
            }
        }
    }
    if !worktree_paths.is_empty() {
        let ignored = run_git_with_stdin(root, &["check-ignore", "--stdin", "--no-index"], &worktree_paths.join("\n"));
        let ignored: std::collections::HashSet<&str> = ignored.stdout.lines().collect();
        change.worktree = worktree_paths.iter().any(|path| !ignored.contains(path.as_str()));
    }
    change
}

/// Starts watching `root` (replacing any earlier watcher). Changes arrive on
/// the frontend as `repo-changed` events, debounced to one per 250ms burst.
pub(crate) fn start(app: AppHandle, root: &Path) -> Result<RepoWatcher, String> {
    let git_dir = git_dir(root);
    // A linked worktree keeps its branches and tags in the main repository's
    // common dir, so watch that too or commits made there go unnoticed.
    let common = git_optional(root, &["rev-parse", "--path-format=absolute", "--git-common-dir"])?
        .map(PathBuf::from)
        .unwrap_or_else(|| git_dir.clone());
    let mut git_dirs = vec![git_dir.clone()];
    if common != git_dir {
        git_dirs.push(common.clone());
    }
    let watched_root = root.to_path_buf();
    let dirs = git_dirs.clone();
    let mut debouncer = new_debouncer(Duration::from_millis(250), move |result: DebounceEventResult| {
        let Ok(events) = result else { return };
        let paths: Vec<PathBuf> = events.into_iter().map(|event| event.path).collect();
        let change = summarize(&watched_root, &dirs, &paths);
        if change.any() {
            let _ = app.emit("repo-changed", change);
        }
    })
    .map_err(|err| format!("unable to watch the repository: {err}"))?;
    let watcher = debouncer.watcher();
    watcher
        .watch(root, RecursiveMode::Recursive)
        .map_err(|err| format!("unable to watch {}: {err}", root.display()))?;
    for dir in &git_dirs {
        // Inside the root already for a normal repository; watching it again is harmless.
        if !dir.starts_with(root) {
            watcher
                .watch(dir, RecursiveMode::Recursive)
                .map_err(|err| format!("unable to watch {}: {err}", dir.display()))?;
        }
    }
    Ok(debouncer)
}

/// Watches the open repository; called by the UI after it opens one.
#[tauri::command(async)]
pub fn watch_repository(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let root = active_repo(&state)?;
    let watcher = start(app, &root)?;
    *state
        .watcher
        .lock()
        .map_err(|_| "watcher lock is poisoned".to_string())? = Some(watcher);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{commit_all, write_file, TempRepo};

    #[test]
    fn git_dir_paths_classify_into_index_refs_or_noise() {
        assert_eq!(classify_git_path(Path::new("index")), Some("index"));
        assert_eq!(classify_git_path(Path::new("HEAD")), Some("refs"));
        assert_eq!(classify_git_path(Path::new("refs/heads/main")), Some("refs"));
        assert_eq!(classify_git_path(Path::new("refs/remotes/origin/main")), Some("refs"));
        assert_eq!(classify_git_path(Path::new("rebase-merge/done")), Some("refs"));
        assert_eq!(classify_git_path(Path::new("worktrees/wt/index")), Some("index"));
        assert_eq!(classify_git_path(Path::new("index.lock")), None);
        assert_eq!(classify_git_path(Path::new("refs/heads/main.lock")), None);
        assert_eq!(classify_git_path(Path::new("objects/ab/cdef")), None);
    }

    #[test]
    fn summarize_drops_ignored_files_and_reports_the_rest() {
        let repo = TempRepo::new();
        write_file(repo.path(), ".gitignore", "build/\n");
        commit_all(repo.path(), "init");
        let root = repo.path().canonicalize().unwrap();
        let git = root.join(".git");
        fs::create_dir_all(root.join("build")).unwrap();

        let ignored_only = summarize(&root, &[git.clone()], &[root.join("build/out.o")]);
        assert!(!ignored_only.worktree);

        let edited = summarize(&root, &[git.clone()], &[root.join("build/out.o"), root.join("src.rs")]);
        assert!(edited.worktree);
        assert!(!edited.refs);

        let committed = summarize(&root, &[git.clone()], &[git.join("index"), git.join("refs/heads/main")]);
        assert!(committed.index && committed.refs && !committed.worktree);

        let noise = summarize(&root, &[git.clone()], &[git.join("objects/aa/bb"), git.join("index.lock")]);
        assert!(!noise.any());
    }

    #[test]
    fn status_reads_do_not_touch_the_index() {
        let repo = TempRepo::new();
        write_file(repo.path(), "a.txt", "one\n");
        commit_all(repo.path(), "init");
        let index = repo.path().join(".git/index");
        let before = fs::metadata(&index).unwrap().modified().unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        // Touch the file without changing it: a plain `git status` would
        // rewrite the index to refresh its stat cache.
        write_file(repo.path(), "a.txt", "one\n");
        worktree_status(repo.path(), &git_dir(repo.path())).unwrap();
        let after = fs::metadata(&index).unwrap().modified().unwrap();
        assert_eq!(before, after, "status must not rewrite the index");
    }
}
