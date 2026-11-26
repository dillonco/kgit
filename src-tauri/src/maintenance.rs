// Background repository maintenance. Every git call kGit makes turns off
// git's own auto-gc (see `git_env`), so an action never stalls midway while
// git repacks. Instead, once per session, each repository that is opened gets:
// - an updated commit-graph file, which makes history walks (`rev-list` for
//   the graph, `merge-base`, `merge-tree` for the conflict check) much faster
//   on large repositories;
// - loose objects packed;
// - packs merged geometrically, so the pack count stays logarithmic without
//   rewriting the whole repository. Older git has no geometric repack
//   task, so there it falls back to the incremental repack.
// git takes its own maintenance lock, so a run started elsewhere (a
// scheduled `git maintenance`, another app) makes this one exit quietly.

use super::*;
use std::collections::HashSet;
use std::sync::OnceLock;

const TASKS: [&str; 2] = ["--task=commit-graph", "--task=loose-objects"];
const REPACK: &str = "--task=geometric-repack";
const OLDER_REPACK: &str = "--task=incremental-repack";

fn started() -> &'static Mutex<HashSet<PathBuf>> {
    static STARTED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    STARTED.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn run(root: &Path) -> GitResult {
    let tasks = |tasks: &[&str]| {
        let mut args = vec!["maintenance", "run", "--quiet"];
        args.extend(tasks);
        run_git(root, &args)
    };
    let result = tasks(&TASKS);
    if result.ok {
        // Repacking is an optimisation: a failure here (for example
        // incremental-repack finding no packs yet) does not fail maintenance.
        let mut repack = tasks(&[REPACK]);
        if !repack.ok && repack.stderr.contains("not a valid task") {
            repack = tasks(&[OLDER_REPACK]);
        }
        if !repack.ok {
            eprintln!("kgit: repack in {} skipped: {}", root.display(), repack.stderr.trim());
        }
    }
    result
}

/// Starts maintenance for `root` on a background thread, unless it already
/// ran (or is running) this session. Returns whether a run was started.
pub(crate) fn start(root: &Path) -> bool {
    let key = canonical_or_self(root);
    let fresh = started().lock().map(|mut seen| seen.insert(key)).unwrap_or(false);
    if fresh {
        let root = root.to_path_buf();
        std::thread::spawn(move || {
            let result = run(&root);
            if !result.ok {
                eprintln!("kgit: maintenance in {} failed: {}", root.display(), result.stderr.trim());
            }
        });
    }
    fresh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{commit_all, write_file, TempRepo};

    #[test]
    fn writes_a_commit_graph_and_runs_once_per_repository() {
        let repo = TempRepo::new();
        write_file(repo.path(), "a.txt", "a\n");
        commit_all(repo.path(), "one");
        let result = run(repo.path());
        assert!(result.ok, "{}", result.stderr);
        let git_dir = repo.path().join(".git/objects/info");
        assert!(
            git_dir.join("commit-graph").exists() || git_dir.join("commit-graphs").exists(),
            "maintenance should write a commit-graph"
        );
        assert!(start(repo.path()));
        assert!(!start(repo.path()), "a second open in the same session does not run it again");
    }

    #[test]
    fn git_env_turns_off_auto_gc_for_every_call() {
        let repo = TempRepo::new();
        let seen = run_git(repo.path(), &["config", "--get", "gc.auto"]);
        assert_eq!(seen.stdout.trim(), "0");
        let seen = run_git(repo.path(), &["config", "--get", "maintenance.auto"]);
        assert_eq!(seen.stdout.trim(), "0");
    }
}
