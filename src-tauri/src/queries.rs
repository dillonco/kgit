// Read-only queries that back the inspection features: early conflict
// detection, stash contents, the reflog, and rename-following file history.

use super::*;

// ---- early conflict detection ----

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictCheck {
    /// The ref HEAD was test-merged with.
    pub target: String,
    /// False when this git cannot run `merge-tree --write-tree` (< 2.38).
    pub supported: bool,
    /// HEAD already contains the target; nothing to merge.
    pub up_to_date: bool,
    pub conflicts: bool,
    pub files: Vec<String>,
}

/// Test-merges `target` into HEAD without touching the index or working
/// tree (`git merge-tree --write-tree`), and reports the files that would
/// conflict, so a branch drifting into conflict is flagged before a merge
/// or rebase starts.
pub(crate) fn conflict_check(root: &Path, target: &str) -> Result<ConflictCheck, String> {
    validate_ref_arg(target)?;
    let mut check = ConflictCheck {
        target: target.to_string(),
        supported: true,
        up_to_date: false,
        conflicts: false,
        files: Vec::new(),
    };
    if run_git(root, &["merge-base", "--is-ancestor", target, "HEAD"]).ok {
        check.up_to_date = true;
        return Ok(check);
    }
    let result = run_git(
        root,
        &["merge-tree", "--write-tree", "--name-only", "--no-messages", "HEAD", target],
    );
    match result.code {
        0 => Ok(check),
        1 => {
            // First line is the result tree; the rest are conflicted paths.
            let mut files: Vec<String> = result
                .stdout
                .lines()
                .skip(1)
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            files.dedup();
            check.conflicts = true;
            check.files = files;
            Ok(check)
        }
        _ if result.stderr.contains("--write-tree") || result.stderr.contains("usage: git merge-tree") => {
            check.supported = false;
            Ok(check)
        }
        _ => Err(if result.stderr.trim().is_empty() {
            format!("unable to test-merge {target}")
        } else {
            result.stderr
        }),
    }
}

#[tauri::command(async)]
pub fn check_conflicts(state: State<'_, AppState>, target: String) -> Result<ConflictCheck, String> {
    conflict_check(&active_repo(&state)?, &target)
}

/// The branch `branch` is likely to be merged back into: the branch it was
/// created from, when the reflog still says, else the repository default
/// (origin's HEAD, else main/master). Stacked branches are then checked
/// against their parent rather than main. With no branch, the default.
#[tauri::command(async)]
pub fn get_base_branch(state: State<'_, AppState>, branch: Option<String>) -> Result<String, String> {
    let root = active_repo(&state)?;
    if let Some(branch) = branch.as_deref().filter(|name| !name.trim().is_empty()) {
        validate_ref_arg(branch)?;
        if let Some(base) = created_from(&root, branch)? {
            return Ok(base);
        }
    }
    default_base_branch(&root)
}

/// Reads where `branch` was created from out of the reflogs:
/// - its own reflog's first entry, `branch: Created from <start>`, written by
///   `git branch`, `checkout -b`, `switch -c` and `worktree add -b`;
/// - when that start is `HEAD` (no start point was named), the HEAD reflog's
///   oldest `checkout: moving from <base> to <branch>`.
/// Only a branch that still exists counts: a commit hash, a detached start or
/// a since-deleted branch gives None, and the caller falls back.
pub(crate) fn created_from(root: &Path, branch: &str) -> Result<Option<String>, String> {
    let own = format!("refs/heads/{branch}");
    let Some(log) = git_optional(root, &["reflog", "show", "--format=%gs", &own, "--"])? else {
        return Ok(None);
    };
    let Some(start) = log.lines().last().and_then(|line| line.strip_prefix("branch: Created from ")) else {
        return Ok(None);
    };
    let mut start = start.trim().to_string();
    if start == "HEAD" {
        let moved = format!(" to {branch}");
        let head_log = git_optional(root, &["reflog", "show", "--format=%gs", "HEAD", "--"])?.unwrap_or_default();
        let Some(from) = head_log
            .lines()
            .filter_map(|line| line.strip_prefix("checkout: moving from "))
            .filter_map(|rest| rest.strip_suffix(moved.as_str()))
            .last()
        else {
            return Ok(None);
        };
        start = from.to_string();
    }
    let start = start.strip_prefix("refs/heads/").or_else(|| start.strip_prefix("refs/remotes/")).unwrap_or(&start).to_string();
    if start == branch || start.is_empty() || start.starts_with('-') {
        return Ok(None);
    }
    for candidate in [format!("refs/heads/{start}"), format!("refs/remotes/{start}")] {
        if git_optional(root, &["rev-parse", "--verify", "--quiet", &candidate])?.is_some() {
            return Ok(Some(start));
        }
    }
    Ok(None)
}

// ---- stashes ----

fn validate_stash(name: &str) -> Result<(), String> {
    let valid = name
        .strip_prefix("stash@{")
        .and_then(|rest| rest.strip_suffix('}'))
        .is_some_and(|index| !index.is_empty() && index.chars().all(|c| c.is_ascii_digit()));
    if valid {
        Ok(())
    } else {
        Err(format!("invalid stash name: {name:?}"))
    }
}

/// A stash shown like a commit: its tracked changes against the commit it
/// was made on, plus any untracked files it saved (its third parent).
pub(crate) fn stash_detail(root: &Path, name: &str) -> Result<CommitDetail, String> {
    validate_stash(name)?;
    let mut detail = commit_detail(root, name)?;
    if let Some(untracked) = git_optional(root, &["ls-tree", "-r", "--name-only", &format!("{name}^3")])? {
        for path in untracked.lines().filter(|line| !line.trim().is_empty()) {
            if !detail.files.iter().any(|file| file.path == path) {
                detail.files.push(CommitFileChange {
                    status: "A".to_string(),
                    path: path.to_string(),
                });
            }
        }
    }
    Ok(detail)
}

#[tauri::command(async)]
pub fn get_stash_detail(state: State<'_, AppState>, name: String) -> Result<CommitDetail, String> {
    stash_detail(&active_repo(&state)?, &name)
}

pub(crate) fn stash_file_diff(root: &Path, name: &str, path: String) -> Result<FileDiff, String> {
    validate_stash(name)?;
    validate_repo_relative_path(&path)?;
    let mut diff = git_optional(root, &["diff", &format!("{name}^1"), name, "--", &path])?.unwrap_or_default();
    if diff.is_empty() {
        // Untracked when stashed: render the saved file as an addition.
        if let Some(content) = git_optional(root, &["show", &format!("{name}^3:{path}")])? {
            diff = format!("diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n");
            for line in content.lines() {
                diff.push('+');
                diff.push_str(line);
                diff.push('\n');
            }
        }
    }
    Ok(FileDiff {
        path,
        staged: false,
        binary: diff.contains("Binary files") || diff.contains("GIT binary patch"),
        diff,
    })
}

#[tauri::command(async)]
pub fn get_stash_file_diff(state: State<'_, AppState>, name: String, path: String) -> Result<FileDiff, String> {
    stash_file_diff(&active_repo(&state)?, &name, path)
}

// ---- reflog ----

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReflogEntry {
    pub hash: String,
    pub short_hash: String,
    /// `HEAD@{3}`
    pub selector: String,
    /// "commit", "checkout", "reset", "rebase (finish)", ...
    pub action: String,
    pub message: String,
    pub relative_date: String,
}

pub(crate) fn parse_reflog(out: &str) -> Vec<ReflogEntry> {
    out.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\u{1f}').collect();
            if fields.len() != 4 {
                return None;
            }
            let (action, message) = match fields[2].split_once(": ") {
                Some((action, message)) => (action.to_string(), message.to_string()),
                None => (fields[2].to_string(), String::new()),
            };
            Some(ReflogEntry {
                hash: fields[0].to_string(),
                short_hash: fields[0].chars().take(8).collect(),
                selector: fields[1].to_string(),
                action,
                message,
                relative_date: fields[3].to_string(),
            })
        })
        .collect()
}

/// HEAD's reflog, newest first: every commit, checkout, reset and rebase,
/// including commits no branch points at any more.
pub(crate) fn reflog(root: &Path, limit: usize) -> Result<Vec<ReflogEntry>, String> {
    let limit = limit.clamp(1, 2000).to_string();
    let out = git_optional(
        root,
        &["reflog", "show", "-n", &limit, "--format=%H%x1f%gd%x1f%gs%x1f%cr", "HEAD"],
    )?
    .unwrap_or_default();
    Ok(parse_reflog(&out))
}

#[tauri::command(async)]
pub fn get_reflog(state: State<'_, AppState>, limit: Option<usize>) -> Result<Vec<ReflogEntry>, String> {
    reflog(&active_repo(&state)?, limit.unwrap_or(200))
}

// ---- file history that follows renames ----

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileHistoryEntry {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub email: String,
    pub relative_date: String,
    pub subject: String,
    /// The file's path in this commit; differs from today's after a rename.
    pub path: String,
    pub status: String,
}

pub(crate) fn parse_file_history(out: &str) -> Vec<FileHistoryEntry> {
    let mut entries = Vec::new();
    for record in out.split('\u{1e}') {
        let record = record.trim_matches('\n');
        if record.is_empty() {
            continue;
        }
        let mut lines = record.lines();
        let Some(header) = lines.next() else { continue };
        let fields: Vec<&str> = header.split('\u{1f}').collect();
        if fields.len() != 5 {
            continue;
        }
        // --name-status lines: "M\tpath" or "R087\told\tnew".
        let (status, path) = lines
            .find(|line| !line.trim().is_empty())
            .map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                (
                    parts.first().map(|s| s.chars().take(1).collect()).unwrap_or_default(),
                    parts.last().copied().unwrap_or_default().to_string(),
                )
            })
            .unwrap_or_default();
        entries.push(FileHistoryEntry {
            hash: fields[0].to_string(),
            short_hash: fields[0].chars().take(8).collect(),
            author: fields[1].to_string(),
            email: fields[2].to_string(),
            relative_date: fields[3].to_string(),
            subject: fields[4].to_string(),
            path,
            status,
        });
    }
    entries
}

pub(crate) fn file_history_entries(root: &Path, path: &str) -> Result<Vec<FileHistoryEntry>, String> {
    validate_repo_relative_path(path)?;
    let out = git(
        root,
        &[
            "log",
            "--follow",
            "--name-status",
            "-n",
            "500",
            "--format=%x1e%H%x1f%an%x1f%ae%x1f%ar%x1f%s",
            "--",
            path,
        ],
    )?;
    Ok(parse_file_history(&out))
}

#[tauri::command(async)]
pub fn get_file_history_entries(state: State<'_, AppState>, path: String) -> Result<Vec<FileHistoryEntry>, String> {
    file_history_entries(&active_repo(&state)?, &path)
}

// ---- structured blame ----

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub summary: String,
    /// Unix seconds of the authoring commit.
    pub time: i64,
    pub line: usize,
    pub text: String,
}

/// Parses `git blame --porcelain`, which only repeats a commit's metadata the
/// first time that commit appears.
pub(crate) fn parse_blame(out: &str) -> Vec<BlameLine> {
    let mut meta: std::collections::HashMap<String, (String, String, i64)> = Default::default();
    let mut lines = Vec::new();
    let mut current: Option<(String, usize)> = None;
    for line in out.lines() {
        if let Some(text) = line.strip_prefix('\t') {
            if let Some((hash, number)) = current.take() {
                let (author, summary, time) = meta.get(&hash).cloned().unwrap_or_default();
                lines.push(BlameLine {
                    short_hash: hash.chars().take(8).collect(),
                    hash,
                    author,
                    summary,
                    time,
                    line: number,
                    text: text.to_string(),
                });
            }
            continue;
        }
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() >= 3 && parts[0].len() == 40 && parts[0].chars().all(|c| c.is_ascii_hexdigit()) {
            let hash = parts[0].to_string();
            meta.entry(hash.clone()).or_default();
            current = Some((hash, parts[2].parse().unwrap_or(0)));
            continue;
        }
        if let Some((hash, _)) = &current {
            let entry = meta.entry(hash.clone()).or_default();
            if let Some(author) = line.strip_prefix("author ") {
                entry.0 = author.to_string();
            } else if let Some(summary) = line.strip_prefix("summary ") {
                entry.1 = summary.to_string();
            } else if let Some(time) = line.strip_prefix("author-time ") {
                entry.2 = time.trim().parse().unwrap_or(0);
            }
        }
    }
    lines
}

pub(crate) fn blame_lines(root: &Path, path: &str) -> Result<Vec<BlameLine>, String> {
    validate_repo_relative_path(path)?;
    Ok(parse_blame(&git(root, &["blame", "--porcelain", "--", path])?))
}

#[tauri::command(async)]
pub fn get_blame_lines(state: State<'_, AppState>, path: String) -> Result<Vec<BlameLine>, String> {
    blame_lines(&active_repo(&state)?, &path)
}

// ---- quick open ----

/// Tracked plus untracked-but-not-ignored files, for ⌘P quick open.
pub(crate) fn repo_files(root: &Path) -> Result<Vec<String>, String> {
    let out = git(root, &["ls-files", "-z", "--cached", "--others", "--exclude-standard"])?;
    let mut files: Vec<String> = out.split('\0').filter(|path| !path.is_empty()).map(ToOwned::to_owned).collect();
    files.sort();
    files.dedup();
    Ok(files)
}

#[tauri::command(async)]
pub fn list_files(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    repo_files(&active_repo(&state)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{commit_all, run, write_file, TempRepo};

    fn diverge(repo: &TempRepo, ours: &str, theirs: &str) {
        write_file(repo.path(), "f.txt", "base\n");
        commit_all(repo.path(), "base");
        run(repo.path(), &["checkout", "-b", "other"]);
        write_file(repo.path(), "f.txt", theirs);
        commit_all(repo.path(), "theirs");
        run(repo.path(), &["checkout", "main"]);
        write_file(repo.path(), "f.txt", ours);
        commit_all(repo.path(), "ours");
    }

    #[test]
    fn conflict_check_names_conflicting_files_without_touching_the_tree() {
        let repo = TempRepo::new();
        diverge(&repo, "ours\n", "theirs\n");
        let check = conflict_check(repo.path(), "other").unwrap();
        assert!(check.supported && check.conflicts && !check.up_to_date);
        assert_eq!(check.files, vec!["f.txt"]);
        // Nothing was merged: no MERGE_HEAD, clean tree.
        let state = repository_state(repo.path()).unwrap();
        assert!(!state.merging && state.files.is_empty());
    }

    #[test]
    fn conflict_check_is_clean_or_up_to_date_when_it_should_be() {
        let repo = TempRepo::new();
        write_file(repo.path(), "a.txt", "a\n");
        commit_all(repo.path(), "a");
        run(repo.path(), &["checkout", "-b", "other"]);
        write_file(repo.path(), "b.txt", "b\n");
        commit_all(repo.path(), "b");
        run(repo.path(), &["checkout", "main"]);
        write_file(repo.path(), "c.txt", "c\n");
        commit_all(repo.path(), "c");
        let clean = conflict_check(repo.path(), "other").unwrap();
        assert!(!clean.conflicts && !clean.up_to_date);
        run(repo.path(), &["merge", "--no-edit", "other"]);
        assert!(conflict_check(repo.path(), "other").unwrap().up_to_date);
        assert!(conflict_check(repo.path(), "--output=x").is_err());
    }

    #[test]
    fn stash_detail_includes_untracked_files_and_their_diff() {
        let repo = TempRepo::new();
        write_file(repo.path(), "a.txt", "one\n");
        commit_all(repo.path(), "init");
        write_file(repo.path(), "a.txt", "two\n");
        write_file(repo.path(), "new.txt", "fresh\n");
        run(repo.path(), &["stash", "push", "-u", "-m", "both"]);
        let detail = stash_detail(repo.path(), "stash@{0}").unwrap();
        let paths: Vec<&str> = detail.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"a.txt") && paths.contains(&"new.txt"));
        let tracked = stash_file_diff(repo.path(), "stash@{0}", "a.txt".into()).unwrap();
        assert!(tracked.diff.contains("+two"));
        let untracked = stash_file_diff(repo.path(), "stash@{0}", "new.txt".into()).unwrap();
        assert!(untracked.diff.contains("+fresh"));
        assert!(stash_detail(repo.path(), "HEAD; rm").is_err());
        assert!(stash_detail(repo.path(), "stash@{x}").is_err());
    }

    #[test]
    fn reflog_lists_checkouts_and_resets_newest_first() {
        let repo = TempRepo::new();
        write_file(repo.path(), "a.txt", "1\n");
        commit_all(repo.path(), "first");
        write_file(repo.path(), "a.txt", "2\n");
        commit_all(repo.path(), "second");
        run(repo.path(), &["reset", "--hard", "HEAD~1"]);
        let entries = reflog(repo.path(), 10).unwrap();
        assert_eq!(entries[0].action, "reset");
        assert_eq!(entries[0].selector, "HEAD@{0}");
        assert!(entries.iter().any(|e| e.message == "second"));
    }

    #[test]
    fn file_history_follows_a_rename() {
        let repo = TempRepo::new();
        write_file(repo.path(), "old.txt", "line one\nline two\nline three\n");
        commit_all(repo.path(), "add");
        run(repo.path(), &["mv", "old.txt", "new.txt"]);
        commit_all(repo.path(), "rename");
        let entries = file_history_entries(repo.path(), "new.txt").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "new.txt");
        assert_eq!(entries[0].status, "R");
        assert_eq!(entries[1].path, "old.txt");
    }

    #[test]
    fn repo_files_include_untracked_but_not_ignored() {
        let repo = TempRepo::new();
        write_file(repo.path(), ".gitignore", "*.log\n");
        write_file(repo.path(), "a.txt", "a\n");
        commit_all(repo.path(), "init");
        write_file(repo.path(), "new.txt", "n\n");
        write_file(repo.path(), "debug.log", "x\n");
        assert_eq!(repo_files(repo.path()).unwrap(), vec![".gitignore", "a.txt", "new.txt"]);
    }

    #[test]
    fn blame_attributes_each_line_to_its_commit() {
        let repo = TempRepo::new();
        write_file(repo.path(), "a.txt", "one\ntwo\n");
        commit_all(repo.path(), "first");
        write_file(repo.path(), "a.txt", "one\nchanged\n");
        commit_all(repo.path(), "second");
        let lines = blame_lines(repo.path(), "a.txt").unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].summary, "first");
        assert_eq!(lines[1].summary, "second");
        assert_eq!(lines[1].text, "changed");
        assert_eq!(lines[1].author, "Test User");
        assert!(lines[1].time > 0);
    }

    #[test]
    fn base_branch_comes_from_where_the_branch_was_created() {
        let repo = TempRepo::new();
        write_file(repo.path(), "a.txt", "a\n");
        commit_all(repo.path(), "init");
        // Named start point: `branch: Created from main`.
        run(repo.path(), &["branch", "stack-1", "main"]);
        assert_eq!(created_from(repo.path(), "stack-1").unwrap().as_deref(), Some("main"));
        // No start point: the reflog says HEAD, so the HEAD reflog decides.
        run(repo.path(), &["checkout", "stack-1"]);
        run(repo.path(), &["checkout", "-b", "stack-2"]);
        assert_eq!(created_from(repo.path(), "stack-2").unwrap().as_deref(), Some("stack-1"));
        // Created from a commit hash: unknown, so the caller falls back.
        let head = run(repo.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
        run(repo.path(), &["branch", "from-hash", &head]);
        assert_eq!(created_from(repo.path(), "from-hash").unwrap(), None);
        // The base was deleted since: also unknown.
        run(repo.path(), &["checkout", "main"]);
        run(repo.path(), &["branch", "temp-base"]);
        run(repo.path(), &["branch", "on-temp", "temp-base"]);
        run(repo.path(), &["branch", "-D", "temp-base"]);
        assert_eq!(created_from(repo.path(), "on-temp").unwrap(), None);
        assert_eq!(created_from(repo.path(), "no-such-branch").unwrap(), None);
    }
}
