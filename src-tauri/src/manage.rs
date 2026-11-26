// Repository management beyond the core action map: branch/tag/remote
// housekeeping, LFS, sparse checkout, submodules, worktree locks, signing
// and identity config, external tools, and patch files.
//
// Actions here build owned argument lists (several need formatted refs such
// as `refs/tags/<name>`), so `run_action` consults `extended_args` before the
// borrowed-argument map in lib.rs.

use super::*;

fn arg(value: Option<&str>, name: &str) -> Result<String, String> {
    let value = required(value, name)?;
    validate_ref_arg(value)?;
    Ok(value.to_string())
}

fn ref_name(value: Option<&str>, name: &str) -> Result<String, String> {
    let value = arg(value, name)?;
    // check-ref-format's rules, minus the ones git enforces itself anyway.
    if value.contains("..") || value.contains(char::is_whitespace) || value.contains(['~', '^', ':', '?', '*', '[', '\\']) {
        return Err(format!("invalid name: {value:?}"));
    }
    Ok(value)
}

fn path_arg(value: Option<&str>, name: &str) -> Result<String, String> {
    let value = required(value, name)?;
    validate_repo_relative_path(value)?;
    if value.starts_with('-') {
        return Err(format!("invalid path: {value:?}"));
    }
    Ok(value.to_string())
}

fn url_arg(value: Option<&str>) -> Result<String, String> {
    let value = required(value, "url")?.trim();
    if value.starts_with('-') || value.contains(char::is_whitespace) {
        return Err(format!("invalid url: {value:?}"));
    }
    // `ext::` runs arbitrary commands; never accept it from the UI.
    if value.to_ascii_lowercase().starts_with("ext::") {
        return Err("ext:: remotes are not allowed".to_string());
    }
    Ok(value.to_string())
}

/// LFS track patterns and sparse paths are pathspec-like; reject option
/// injection and traversal but allow globs.
fn pattern_arg(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.starts_with('-') || value.split('/').any(|part| part == "..") {
        return Err(format!("invalid pattern: {value:?}"));
    }
    Ok(value.to_string())
}

/// Argument lists for the extended action kinds, or None for a kind the
/// core map in lib.rs handles.
pub(crate) fn extended_args(action: &GitAction) -> Result<Option<Vec<String>>, String> {
    let branch = action.branch.as_deref();
    let target = action.target.as_deref();
    let remote = || -> Result<String, String> {
        let remote = action.remote.as_deref().unwrap_or("origin");
        validate_ref_arg(remote)?;
        Ok(remote.to_string())
    };
    let message = action.message.as_deref();
    let s = |value: &str| value.to_string();

    let args: Vec<String> = match action.kind.as_str() {
        // ---- branches ----
        "renameBranch" => vec![s("branch"), s("-m"), arg(branch, "branch")?, ref_name(target, "new name")?],
        "setUpstream" => vec![s("branch"), s("--set-upstream-to"), arg(target, "upstream")?, arg(branch, "branch")?],
        "unsetUpstream" => vec![s("branch"), s("--unset-upstream"), arg(branch, "branch")?],
        "pushBranch" => vec![s("push"), s("-u"), remote()?, arg(branch, "branch")?],
        "deleteRemoteBranch" => {
            // target is "origin/feature/x": the remote is the first segment.
            let full = arg(target, "remote branch")?;
            let (remote, name) = full.split_once('/').ok_or("expected remote/branch")?;
            validate_ref_arg(name)?;
            vec![s("push"), s(remote), s("--delete"), s(name)]
        }
        "pushTo" => {
            // Push local `branch` to `target` ("origin/name"), e.g. from a drag onto a remote branch.
            let full = arg(target, "remote branch")?;
            let (remote, name) = full.split_once('/').ok_or("expected remote/branch")?;
            validate_ref_arg(remote)?;
            vec![s("push"), s(remote), format!("{}:refs/heads/{}", arg(branch, "branch")?, ref_name(Some(name), "branch")?)]
        }
        "stashBranch" => vec![s("stash"), s("branch"), ref_name(branch, "branch")?, arg(target, "stash")?],
        "mergeNoFf" => vec![s("merge"), s("--no-ff"), s("--no-edit"), arg(target, "target")?],
        "mergeFfOnly" => vec![s("merge"), s("--ff-only"), arg(target, "target")?],
        "mergeSquash" => vec![s("merge"), s("--squash"), arg(target, "target")?],
        "stashPush" => {
            let mut args = vec![s("stash"), s("push")];
            match action.mode.as_deref().unwrap_or("all") {
                "all" => args.push(s("--include-untracked")),
                "tracked" => {}
                "staged" => args.push(s("--staged")),
                "keepIndex" => args.extend([s("--keep-index"), s("--include-untracked")]),
                other => return Err(format!("unsupported stash mode: {other}")),
            }
            args.extend([s("-m"), s(message.unwrap_or("kGit stash"))]);
            if let Some(path) = action.path.as_deref() {
                args.extend([s("--"), path_arg(Some(path), "path")?]);
            }
            args
        }
        // ---- tags ----
        "createAnnotatedTag" => {
            let mut args = vec![s("tag"), s("-a"), ref_name(branch, "tag")?, s("-m"), s(required(message, "message")?)];
            if let Some(target) = target.filter(|value| !value.trim().is_empty()) {
                validate_ref_arg(target)?;
                args.push(s(target));
            }
            args
        }
        "pushTag" => vec![s("push"), remote()?, format!("refs/tags/{}", ref_name(branch, "tag")?)],
        "pushAllTags" => vec![s("push"), remote()?, s("--tags")],
        "deleteRemoteTag" => vec![s("push"), remote()?, s("--delete"), format!("refs/tags/{}", ref_name(branch, "tag")?)],
        // ---- remotes ----
        "addRemote" => vec![s("remote"), s("add"), ref_name(action.remote.as_deref(), "remote")?, url_arg(target)?],
        "removeRemote" => vec![s("remote"), s("remove"), arg(action.remote.as_deref(), "remote")?],
        "renameRemote" => vec![s("remote"), s("rename"), arg(action.remote.as_deref(), "remote")?, ref_name(target, "new name")?],
        "setRemoteUrl" => vec![s("remote"), s("set-url"), arg(action.remote.as_deref(), "remote")?, url_arg(target)?],
        "fetchRemote" => vec![s("fetch"), s("--prune"), remote()?],
        // ---- worktrees ----
        "worktreeLock" => {
            let path = required(action.path.as_deref(), "path")?;
            validate_worktree_path(path)?;
            let mut args = vec![s("worktree"), s("lock")];
            if let Some(reason) = message.filter(|value| !value.trim().is_empty()) {
                args.extend([s("--reason"), s(reason)]);
            }
            args.push(s(path));
            args
        }
        "worktreeUnlock" => {
            let path = required(action.path.as_deref(), "path")?;
            validate_worktree_path(path)?;
            vec![s("worktree"), s("unlock"), s(path)]
        }
        // ---- LFS ----
        "lfsInstall" => vec![s("lfs"), s("install"), s("--local")],
        "lfsTrack" => vec![s("lfs"), s("track"), pattern_arg(required(target, "pattern")?)?],
        "lfsUntrack" => vec![s("lfs"), s("untrack"), pattern_arg(required(target, "pattern")?)?],
        "lfsPull" => vec![s("lfs"), s("pull")],
        "lfsFetch" => vec![s("lfs"), s("fetch"), s("--all")],
        "lfsPushAll" => vec![s("lfs"), s("push"), s("--all"), remote()?],
        "lfsPrune" => vec![s("lfs"), s("prune")],
        // ---- sparse checkout ----
        "sparseSet" => {
            // message holds one path per line.
            let mut args = vec![s("sparse-checkout"), s("set")];
            if action.mode.as_deref() != Some("nonCone") {
                args.push(s("--cone"));
            } else {
                args.push(s("--no-cone"));
            }
            let patterns: Vec<String> = required(message, "paths")?
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(pattern_arg)
                .collect::<Result<_, _>>()?;
            if patterns.is_empty() {
                return Err("enter at least one path".to_string());
            }
            args.extend(patterns);
            args
        }
        "sparseDisable" => vec![s("sparse-checkout"), s("disable")],
        "sparseReapply" => vec![s("sparse-checkout"), s("reapply")],
        // ---- submodules ----
        "submoduleUpdate" => vec![s("submodule"), s("update"), s("--init"), s("--recursive")],
        "submoduleSync" => vec![s("submodule"), s("sync"), s("--recursive")],
        "submoduleAdd" => {
            let mut args = vec![s("submodule"), s("add"), url_arg(target)?];
            if let Some(path) = action.path.as_deref().filter(|value| !value.trim().is_empty()) {
                args.push(path_arg(Some(path), "path")?);
            }
            args
        }
        "submoduleDeinit" => vec![s("submodule"), s("deinit"), s("-f"), s("--"), path_arg(action.path.as_deref(), "path")?],
        // Fully remove: deinit, then drop it from the index and .gitmodules.
        "submoduleRemove" => vec![s("rm"), s("-f"), s("--"), path_arg(action.path.as_deref(), "path")?],
        // ---- misc ----
        "ignorePath" => return Ok(None), // handled in run_extended (file write)
        "discardAll" => vec![s("reset"), s("--hard"), s("HEAD")],
        _ => return Ok(None),
    };
    Ok(Some(args))
}

/// Runs an extended action, or None when the core map should handle it.
pub(crate) fn run_extended(root: &Path, action: &GitAction) -> Result<Option<GitResult>, String> {
    if action.kind == "ignorePath" {
        let path = path_arg(action.path.as_deref(), "path")?;
        return add_to_gitignore(root, &path).map(Some);
    }
    if action.kind == "submoduleRemove" {
        let path = path_arg(action.path.as_deref(), "path")?;
        let deinit = run_git(root, &["submodule", "deinit", "-f", "--", &path]);
        if !deinit.ok {
            return Ok(Some(deinit));
        }
    }
    let Some(args) = extended_args(action)? else {
        return Ok(None);
    };
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    Ok(Some(run_git(root, &args)))
}

fn add_to_gitignore(root: &Path, path: &str) -> Result<GitResult, String> {
    let file = root.join(".gitignore");
    let existing = fs::read_to_string(&file).unwrap_or_default();
    let entry = format!("/{}", path.trim_start_matches('/'));
    if existing.lines().any(|line| line.trim() == entry) {
        return Ok(GitResult { ok: true, stdout: String::new(), stderr: String::new(), code: 0, refresh: true });
    }
    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&entry);
    next.push('\n');
    fs::write(&file, next).map_err(|err| err.to_string())?;
    Ok(GitResult { ok: true, stdout: format!("Added {entry} to .gitignore"), stderr: String::new(), code: 0, refresh: true })
}

// ---- checkout with auto-stash ----

/// Checks out `branch`, carrying uncommitted changes across: stash them,
/// switch, then pop. If the pop conflicts, the stash is kept and the
/// conflict is reported, as `git stash pop` itself does.
pub(crate) fn checkout_autostash(root: &Path, branch: &str) -> Result<GitResult, String> {
    validate_ref_arg(branch)?;
    let dirty = git(root, &["--no-optional-locks", "status", "--porcelain"])?;
    if dirty.trim().is_empty() {
        return Ok(run_git(root, &["checkout", branch]));
    }
    let stash = run_git(root, &["stash", "push", "--include-untracked", "-m", &format!("kGit auto-stash before checking out {branch}")]);
    if !stash.ok {
        return Ok(stash);
    }
    let checkout = run_git(root, &["checkout", branch]);
    if !checkout.ok {
        // Put the changes back where they were.
        run_git(root, &["stash", "pop"]);
        return Ok(checkout);
    }
    let pop = run_git(root, &["stash", "pop"]);
    if pop.ok {
        return Ok(GitResult { stdout: format!("Checked out {branch} and restored your changes"), ..pop });
    }
    Ok(GitResult {
        ok: false,
        stderr: format!(
            "Checked out {branch}, but restoring your changes conflicted. Resolve the conflicts; your changes are also kept in the stash.\n{}",
            pop.stderr
        ),
        ..pop
    })
}

#[tauri::command(async)]
pub fn checkout_with_autostash(state: State<'_, AppState>, branch: String) -> Result<GitResult, String> {
    checkout_autostash(&active_repo(&state)?, &branch)
}

// ---- repository configuration snapshot ----

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Submodule {
    pub path: String,
    pub commit: String,
    /// "current", "uninitialized", "modified" (checked out at another commit), "conflict"
    pub status: String,
    pub describe: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepoConfig {
    pub remotes: Vec<RemoteInfo>,
    pub submodules: Vec<Submodule>,
    pub lfs_available: bool,
    pub lfs_initialized: bool,
    pub lfs_patterns: Vec<String>,
    pub sparse_enabled: bool,
    pub sparse_cone: bool,
    pub sparse_paths: Vec<String>,
    /// Effective values, plus where user.name/email come from ("local"/"global").
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub identity_scope: Option<String>,
    pub signing_key: Option<String>,
    pub gpg_format: Option<String>,
    pub gpg_program: Option<String>,
    pub commit_gpgsign: bool,
    pub tag_gpgsign: bool,
    pub merge_tool: Option<String>,
    pub diff_tool: Option<String>,
}

pub(crate) fn parse_submodules(out: &str) -> Vec<Submodule> {
    out.lines()
        .filter_map(|line| {
            if line.len() < 2 {
                return None;
            }
            let (flag, rest) = line.split_at(1);
            let mut parts = rest.split_whitespace();
            let commit = parts.next()?.to_string();
            let path = parts.next()?.to_string();
            let describe = parts.next().map(|value| value.trim_matches(|c| c == '(' || c == ')').to_string());
            let status = match flag {
                "-" => "uninitialized",
                "+" => "modified",
                "U" => "conflict",
                _ => "current",
            };
            Some(Submodule { path, commit: commit.chars().take(8).collect(), status: status.to_string(), describe })
        })
        .collect()
}

pub(crate) fn parse_lfs_patterns(gitattributes: &str) -> Vec<String> {
    gitattributes
        .lines()
        .filter(|line| line.split_whitespace().any(|attr| attr == "filter=lfs"))
        .filter_map(|line| line.split_whitespace().next().map(ToOwned::to_owned))
        .collect()
}

fn config_get(root: &Path, key: &str) -> Option<String> {
    git_optional(root, &["config", "--get", key]).ok().flatten()
}

fn config_bool(root: &Path, key: &str) -> bool {
    git_optional(root, &["config", "--type=bool", "--get", key])
        .ok()
        .flatten()
        .is_some_and(|value| value == "true")
}

pub(crate) fn repo_config(root: &Path) -> Result<RepoConfig, String> {
    let remote_v = git(root, &["remote", "-v"])?;
    let mut remotes: Vec<RemoteInfo> = Vec::new();
    for line in remote_v.lines() {
        let Some((name, rest)) = line.split_once('\t') else { continue };
        let entry = match remotes.iter_mut().find(|remote| remote.name == name) {
            Some(entry) => entry,
            None => {
                remotes.push(RemoteInfo { name: name.to_string(), ..Default::default() });
                remotes.last_mut().expect("just pushed")
            }
        };
        if let Some(url) = rest.strip_suffix(" (fetch)") {
            entry.fetch_url = url.to_string();
        } else if let Some(url) = rest.strip_suffix(" (push)") {
            entry.push_url = url.to_string();
        }
    }
    let submodules = if root.join(".gitmodules").is_file() {
        parse_submodules(&git_optional(root, &["submodule", "status"])?.unwrap_or_default())
    } else {
        Vec::new()
    };
    let lfs_available = run_git(root, &["lfs", "version"]).ok;
    let lfs_patterns = parse_lfs_patterns(&fs::read_to_string(root.join(".gitattributes")).unwrap_or_default());
    let lfs_initialized = git_optional(root, &["config", "--local", "--get", "filter.lfs.process"])?.is_some()
        || git_dir(root).join("hooks/pre-push").is_file()
            && fs::read_to_string(git_dir(root).join("hooks/pre-push")).unwrap_or_default().contains("git lfs");
    let sparse_enabled = config_bool(root, "core.sparseCheckout");
    let sparse_paths = if sparse_enabled {
        git_optional(root, &["sparse-checkout", "list"])?
            .unwrap_or_default()
            .lines()
            .map(ToOwned::to_owned)
            .collect()
    } else {
        Vec::new()
    };
    let identity_scope = if git_optional(root, &["config", "--local", "--get", "user.email"])?.is_some() {
        Some("local".to_string())
    } else if config_get(root, "user.email").is_some() {
        Some("global".to_string())
    } else {
        None
    };
    Ok(RepoConfig {
        remotes,
        submodules,
        lfs_available,
        lfs_initialized,
        lfs_patterns,
        sparse_enabled,
        sparse_cone: sparse_enabled && config_bool(root, "core.sparseCheckoutCone"),
        sparse_paths,
        user_name: config_get(root, "user.name"),
        user_email: config_get(root, "user.email"),
        identity_scope,
        signing_key: config_get(root, "user.signingkey"),
        gpg_format: config_get(root, "gpg.format"),
        gpg_program: config_get(root, "gpg.program"),
        commit_gpgsign: config_bool(root, "commit.gpgsign"),
        tag_gpgsign: config_bool(root, "tag.gpgsign"),
        merge_tool: config_get(root, "merge.tool"),
        diff_tool: config_get(root, "diff.tool"),
    })
}

#[tauri::command(async)]
pub fn get_repo_config(state: State<'_, AppState>) -> Result<RepoConfig, String> {
    repo_config(&active_repo(&state)?)
}

/// Config keys the UI may write. Anything else is refused, so the settings
/// screens can't be used to set e.g. `core.sshCommand` or `core.hooksPath`.
const WRITABLE_KEYS: &[&str] = &[
    "user.name",
    "user.email",
    "user.signingkey",
    "commit.gpgsign",
    "tag.gpgsign",
    "gpg.format",
    "gpg.program",
    "pull.rebase",
    "fetch.prune",
    "rerere.enabled",
    "merge.tool",
    "diff.tool",
    "push.autoSetupRemote",
];

pub(crate) fn set_config_value(root: &Path, key: &str, value: Option<&str>, global: bool) -> Result<GitResult, String> {
    if !WRITABLE_KEYS.contains(&key) {
        return Err(format!("kGit does not edit {key}"));
    }
    let scope = if global { "--global" } else { "--local" };
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    Ok(match value {
        Some(value) => {
            if value.starts_with('-') && !key.ends_with("signingkey") {
                return Err(format!("invalid value for {key}"));
            }
            run_git(root, &["config", scope, key, value])
        }
        None => {
            let result = run_git(root, &["config", scope, "--unset", key]);
            // Exit 5: the key was not set, which is what we wanted anyway.
            if result.code == 5 {
                GitResult { ok: true, code: 0, ..result }
            } else {
                result
            }
        }
    })
}

#[tauri::command(async)]
pub fn set_config(state: State<'_, AppState>, key: String, value: Option<String>, global: bool) -> Result<GitResult, String> {
    set_config_value(&active_repo(&state)?, &key, value.as_deref(), global)
}

// ---- signature verification for the detail panel ----

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    /// git's %G?: G good, B bad, U good/unknown validity, X/Y expired, R revoked, E can't check, N none.
    pub status: String,
    pub signer: String,
    pub key: String,
}

#[tauri::command(async)]
pub fn get_commit_signature(state: State<'_, AppState>, hash: String) -> Result<Signature, String> {
    commit_signature(&active_repo(&state)?, &hash)
}

pub(crate) fn commit_signature(root: &Path, hash: &str) -> Result<Signature, String> {
    validate_ref_arg(hash)?;
    let out = git(root, &["log", "-1", "--format=%G?%x1f%GS%x1f%GK", hash])?;
    let fields: Vec<&str> = out.split('\u{1f}').collect();
    Ok(Signature {
        status: fields.first().copied().unwrap_or("N").trim().to_string(),
        signer: fields.get(1).copied().unwrap_or_default().to_string(),
        key: fields.get(2).copied().unwrap_or_default().to_string(),
    })
}

// ---- patches ----

/// A patch for sharing: one commit as a mailbox patch (keeps author and
/// message, applies with `git am`), or the uncommitted changes (`git apply`).
pub(crate) fn create_patch(root: &Path, source: &str, target: Option<&str>) -> Result<String, String> {
    match source {
        "commit" => {
            let hash = required(target, "commit")?;
            validate_ref_arg(hash)?;
            git(root, &["format-patch", "-1", "--stdout", hash])
        }
        "range" => {
            let range = required(target, "range")?;
            validate_ref_arg(range)?;
            git(root, &["format-patch", "--stdout", range])
        }
        "staged" => git(root, &["diff", "--binary", "--cached"]),
        "worktree" => {
            // Tracked changes against HEAD; untracked files are added as
            // intent-to-add-free new-file diffs below.
            let mut patch = git(root, &["diff", "--binary", "HEAD"]).unwrap_or_default();
            let untracked = git(root, &["ls-files", "-z", "--others", "--exclude-standard"])?;
            for path in untracked.split('\0').filter(|path| !path.is_empty()) {
                let result = run_git(root, &["diff", "--binary", "--no-index", "--", "/dev/null", path]);
                // --no-index exits 1 when the files differ, which they do.
                if !result.stdout.is_empty() {
                    if !patch.is_empty() && !patch.ends_with('\n') {
                        patch.push('\n');
                    }
                    patch.push_str(&result.stdout);
                }
            }
            Ok(patch)
        }
        other => Err(format!("unknown patch source: {other}")),
    }
}

#[tauri::command(async)]
pub fn make_patch(state: State<'_, AppState>, source: String, target: Option<String>) -> Result<String, String> {
    create_patch(&active_repo(&state)?, &source, target.as_deref())
}

/// Applies a patch: mailbox patches (from format-patch) become commits with
/// `git am -3`; plain diffs are applied to the working tree with a 3-way
/// fallback, or to the index too when `index` is set.
pub(crate) fn apply_patch_text(root: &Path, patch: &str, index: bool) -> GitResult {
    if patch.starts_with("From ") && patch.contains("\nSubject: ") {
        let result = run_git_with_stdin(root, &["am", "-3"], patch);
        if !result.ok {
            // Leave nothing half-applied behind.
            run_git(root, &["am", "--abort"]);
        }
        return result;
    }
    let args: &[&str] = if index { &["apply", "--3way", "--index", "-"] } else { &["apply", "--3way", "-"] };
    run_git_with_stdin(root, args, patch)
}

#[tauri::command(async)]
pub fn apply_patch(state: State<'_, AppState>, patch: String, index: bool) -> Result<GitResult, String> {
    Ok(apply_patch_text(&active_repo(&state)?, &patch, index))
}

/// Writes `content` to an absolute path chosen in a save dialog.
#[tauri::command(async)]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err("path must be absolute".to_string());
    }
    fs::write(&path, content).map_err(|err| err.to_string())
}

#[tauri::command(async)]
pub fn read_text_file(path: String) -> Result<String, String> {
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err("path must be absolute".to_string());
    }
    let metadata = fs::metadata(&path).map_err(|err| err.to_string())?;
    if metadata.len() > 20 * 1024 * 1024 {
        return Err("file is larger than 20 MB".to_string());
    }
    fs::read_to_string(&path).map_err(|err| err.to_string())
}

/// Native save / open dialogs (macOS, like the folder picker). The prompt
/// and default name travel as argv items, never spliced into the script.
fn osascript_path(script: &str, args: &[&str]) -> Result<Option<String>, String> {
    let result = run_command(Command::new("osascript").arg("-e").arg(script).args(args), false);
    if result.ok {
        let path = result.stdout.trim();
        Ok((!path.is_empty()).then(|| path.to_string()))
    } else if result.stderr.contains("User canceled") || result.stderr.contains("-128") {
        Ok(None)
    } else {
        Err(if result.stderr.trim().is_empty() { "unable to open the dialog".to_string() } else { result.stderr })
    }
}

#[tauri::command(async)]
pub fn pick_save_path(prompt: String, default_name: String) -> Result<Option<String>, String> {
    osascript_path(
        "on run argv\ntry\nreturn POSIX path of (choose file name with prompt (item 1 of argv) default name (item 2 of argv))\non error number -128\nreturn \"\"\nend try\nend run",
        &[&prompt, &default_name],
    )
}

#[tauri::command(async)]
pub fn pick_open_file(prompt: String) -> Result<Option<String>, String> {
    osascript_path(
        "on run argv\ntry\nreturn POSIX path of (choose file with prompt (item 1 of argv))\non error number -128\nreturn \"\"\nend try\nend run",
        &[&prompt],
    )
}

// ---- external tools ----

fn app_name(value: &str) -> Result<&str, String> {
    let value = value.trim();
    if value.is_empty() || value.starts_with('-') || value.contains(['/', '\\']) && !value.ends_with(".app") {
        return Err(format!("invalid application: {value:?}"));
    }
    Ok(value)
}

/// Opens the repository (or a file in it) in an external app: an editor or
/// terminal by name, or the file manager when `app` is empty.
#[tauri::command(async)]
pub fn open_external(state: State<'_, AppState>, app: Option<String>, path: Option<String>, reveal: bool) -> Result<GitResult, String> {
    let root = active_repo(&state)?;
    let target = match path.as_deref().filter(|value| !value.is_empty()) {
        Some(path) => {
            validate_repo_relative_path(path)?;
            root.join(path)
        }
        None => root.clone(),
    };
    let mut command = Command::new("open");
    if reveal {
        command.arg("-R");
    } else if let Some(app) = app.as_deref().filter(|value| !value.trim().is_empty()) {
        command.args(["-a", app_name(app)?]);
    }
    command.arg(&target);
    Ok(run_command(&mut command, false))
}

/// Launches the configured (or named) diff/merge tool for one file without
/// waiting: the tool's window outlives this call, and the file watcher picks
/// up the result when a merge tool saves.
#[tauri::command(async)]
pub fn launch_tool(state: State<'_, AppState>, kind: String, path: String, tool: Option<String>) -> Result<GitResult, String> {
    let root = active_repo(&state)?;
    validate_repo_relative_path(&path)?;
    let mut args: Vec<String> = match kind.as_str() {
        "diff" => vec!["difftool".into(), "-y".into()],
        "merge" => vec!["mergetool".into(), "-y".into()],
        other => return Err(format!("unknown tool kind: {other}")),
    };
    if let Some(tool) = tool.as_deref().filter(|value| !value.trim().is_empty()) {
        if tool.starts_with('-') || tool.contains(char::is_whitespace) {
            return Err(format!("invalid tool: {tool:?}"));
        }
        args.push(format!("--tool={tool}"));
    }
    args.extend(["--".into(), path]);
    Command::new("git")
        .args(&args)
        .current_dir(&root)
        .envs(git_env())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| err.to_string())?;
    Ok(GitResult { ok: true, stdout: String::new(), stderr: String::new(), code: 0, refresh: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{act, commit_all, head_subject, run, write_file, TempRepo};

    fn with(kind: &str, setup: impl FnOnce(&mut GitAction)) -> GitAction {
        let mut action = act(kind);
        setup(&mut action);
        action
    }

    #[test]
    fn extended_args_build_and_validate() {
        let args = extended_args(&with("renameBranch", |a| {
            a.branch = Some("old".into());
            a.target = Some("new/name".into());
        }))
        .unwrap()
        .unwrap();
        assert_eq!(args, vec!["branch", "-m", "old", "new/name"]);
        assert!(extended_args(&with("renameBranch", |a| {
            a.branch = Some("old".into());
            a.target = Some("bad name".into());
        }))
        .is_err());
        assert_eq!(
            extended_args(&with("deleteRemoteTag", |a| a.branch = Some("v1".into()))).unwrap().unwrap(),
            vec!["push", "origin", "--delete", "refs/tags/v1"]
        );
        assert_eq!(
            extended_args(&with("deleteRemoteBranch", |a| a.target = Some("upstream/feature/x".into()))).unwrap().unwrap(),
            vec!["push", "upstream", "--delete", "feature/x"]
        );
        assert!(extended_args(&with("addRemote", |a| {
            a.remote = Some("evil".into());
            a.target = Some("ext::sh -c touch% /tmp/pwned".into());
        }))
        .is_err());
        assert!(extended_args(&with("lfsTrack", |a| a.target = Some("--all".into()))).is_err());
        assert_eq!(
            extended_args(&with("sparseSet", |a| a.message = Some("src\n\ndocs/\n".into()))).unwrap().unwrap(),
            vec!["sparse-checkout", "set", "--cone", "src", "docs/"]
        );
        assert!(extended_args(&with("sparseSet", |a| a.message = Some("../outside".into()))).is_err());
        assert!(extended_args(&act("stage")).unwrap().is_none());
    }

    #[test]
    fn branch_tag_and_remote_management_round_trip() {
        let repo = TempRepo::new();
        write_file(repo.path(), "a.txt", "a\n");
        commit_all(repo.path(), "init");
        let root = repo.path();
        run(root, &["branch", "topic"]);
        let ok = |action: GitAction| {
            let result = run_extended(root, &action).unwrap().expect("extended kind");
            assert!(result.ok, "{} failed: {}", action.kind, result.stderr);
        };
        ok(with("renameBranch", |a| {
            a.branch = Some("topic".into());
            a.target = Some("renamed".into());
        }));
        assert!(repository_state(root).unwrap().branches.iter().any(|b| b.name == "renamed"));
        ok(with("createAnnotatedTag", |a| {
            a.branch = Some("v1".into());
            a.message = Some("release one".into());
        }));
        assert_eq!(git(root, &["cat-file", "-t", "v1"]).unwrap(), "tag");
        ok(with("addRemote", |a| {
            a.remote = Some("mirror".into());
            a.target = Some("https://example.com/x.git".into());
        }));
        ok(with("setRemoteUrl", |a| {
            a.remote = Some("mirror".into());
            a.target = Some("git@example.com:x.git".into());
        }));
        let config = repo_config(root).unwrap();
        assert_eq!(config.remotes[0].fetch_url, "git@example.com:x.git");
        ok(with("renameRemote", |a| {
            a.remote = Some("mirror".into());
            a.target = Some("backup".into());
        }));
        ok(with("removeRemote", |a| a.remote = Some("backup".into())));
        assert!(repo_config(root).unwrap().remotes.is_empty());
    }

    #[test]
    fn upstream_can_be_set_and_unset() {
        let origin = TempRepo::new();
        write_file(origin.path(), "a.txt", "a\n");
        commit_all(origin.path(), "init");
        let clone = TempRepo::new();
        run(clone.path(), &["remote", "add", "origin", &origin.path().display().to_string()]);
        run(clone.path(), &["fetch", "origin"]);
        run(clone.path(), &["checkout", "-b", "work", "origin/main"]);
        run(clone.path(), &["branch", "--unset-upstream"]);
        let set = run_extended(clone.path(), &with("setUpstream", |a| {
            a.branch = Some("work".into());
            a.target = Some("origin/main".into());
        }))
        .unwrap()
        .unwrap();
        assert!(set.ok, "{}", set.stderr);
        let state = repository_state(clone.path()).unwrap();
        assert_eq!(state.branches.iter().find(|b| b.name == "work").unwrap().upstream.as_deref(), Some("origin/main"));
        run_extended(clone.path(), &with("unsetUpstream", |a| a.branch = Some("work".into()))).unwrap();
        assert!(repository_state(clone.path()).unwrap().branches[0].upstream.is_none());
    }

    #[test]
    fn autostash_checkout_carries_changes_across() {
        let repo = TempRepo::new();
        write_file(repo.path(), "a.txt", "a\n");
        commit_all(repo.path(), "init");
        run(repo.path(), &["branch", "other"]);
        write_file(repo.path(), "a.txt", "edited\n");
        write_file(repo.path(), "new.txt", "new\n");
        let result = checkout_autostash(repo.path(), "other").unwrap();
        assert!(result.ok, "{}", result.stderr);
        let state = repository_state(repo.path()).unwrap();
        assert_eq!(state.current_branch.as_deref(), Some("other"));
        assert_eq!(fs::read_to_string(repo.path().join("a.txt")).unwrap(), "edited\n");
        assert!(repo.path().join("new.txt").exists());
        assert!(state.stashes.is_empty());
    }

    #[test]
    fn stash_push_modes() {
        let repo = TempRepo::new();
        write_file(repo.path(), "a.txt", "a\n");
        write_file(repo.path(), "b.txt", "b\n");
        commit_all(repo.path(), "init");
        write_file(repo.path(), "a.txt", "staged\n");
        run(repo.path(), &["add", "a.txt"]);
        write_file(repo.path(), "b.txt", "unstaged\n");
        let result = run_extended(repo.path(), &with("stashPush", |a| {
            a.mode = Some("staged".into());
            a.message = Some("only staged".into());
        }))
        .unwrap()
        .unwrap();
        assert!(result.ok, "{}", result.stderr);
        // The unstaged change stays in the working tree.
        assert_eq!(fs::read_to_string(repo.path().join("b.txt")).unwrap(), "unstaged\n");
        assert_eq!(fs::read_to_string(repo.path().join("a.txt")).unwrap(), "a\n");
    }

    #[test]
    fn push_to_and_stash_branch_build_refspecs() {
        assert_eq!(
            extended_args(&with("pushTo", |a| {
                a.branch = Some("feature".into());
                a.target = Some("origin/review/feature".into());
            }))
            .unwrap()
            .unwrap(),
            vec!["push", "origin", "feature:refs/heads/review/feature"]
        );
        assert!(extended_args(&with("pushTo", |a| {
            a.branch = Some("feature".into());
            a.target = Some("origin/bad:name".into());
        }))
        .is_err());
        let repo = TempRepo::new();
        write_file(repo.path(), "a.txt", "a\n");
        commit_all(repo.path(), "init");
        write_file(repo.path(), "a.txt", "wip\n");
        run(repo.path(), &["stash"]);
        let result = run_extended(repo.path(), &with("stashBranch", |a| {
            a.branch = Some("from-stash".into());
            a.target = Some("stash@{0}".into());
        }))
        .unwrap()
        .unwrap();
        assert!(result.ok, "{}", result.stderr);
        let state = repository_state(repo.path()).unwrap();
        assert_eq!(state.current_branch.as_deref(), Some("from-stash"));
        assert!(state.stashes.is_empty());
    }

    #[test]
    fn config_writes_are_limited_to_known_keys() {
        let repo = TempRepo::new();
        assert!(set_config_value(repo.path(), "core.sshCommand", Some("evil"), false).is_err());
        assert!(set_config_value(repo.path(), "user.email", Some("me@x.dev"), false).unwrap().ok);
        assert_eq!(repo_config(repo.path()).unwrap().identity_scope.as_deref(), Some("local"));
        assert!(set_config_value(repo.path(), "commit.gpgsign", None, false).unwrap().ok);
    }

    #[test]
    fn patches_round_trip_between_repositories() {
        let source = TempRepo::new();
        write_file(source.path(), "a.txt", "a\n");
        commit_all(source.path(), "init");
        let target = TempRepo::new();
        run(target.path(), &["pull", &source.path().display().to_string(), "main"]);

        write_file(source.path(), "a.txt", "changed\n");
        commit_all(source.path(), "change a");
        let mailbox = create_patch(source.path(), "commit", Some("HEAD")).unwrap();
        let applied = apply_patch_text(target.path(), &mailbox, false);
        assert!(applied.ok, "{}", applied.stderr);
        assert_eq!(head_subject(target.path()), "change a");

        write_file(source.path(), "a.txt", "wip\n");
        write_file(source.path(), "untracked.txt", "u\n");
        let wip = create_patch(source.path(), "worktree", None).unwrap();
        assert!(wip.contains("untracked.txt"));
        let applied = apply_patch_text(target.path(), &wip, false);
        assert!(applied.ok, "{}", applied.stderr);
        assert_eq!(fs::read_to_string(target.path().join("a.txt")).unwrap(), "wip\n");
        assert!(target.path().join("untracked.txt").exists());
    }

    #[test]
    fn ignore_path_appends_once() {
        let repo = TempRepo::new();
        run_extended(repo.path(), &with("ignorePath", |a| a.path = Some("build/out.o".into()))).unwrap();
        run_extended(repo.path(), &with("ignorePath", |a| a.path = Some("build/out.o".into()))).unwrap();
        assert_eq!(fs::read_to_string(repo.path().join(".gitignore")).unwrap(), "/build/out.o\n");
    }

    #[test]
    fn parses_submodule_status_and_lfs_attributes() {
        let subs = parse_submodules(" 1234567890abcdef vendor/lib (v1.2)\n-abcdef1234567890 vendor/new\n+9999999999999999 vendor/moved (heads/main)");
        assert_eq!(subs.len(), 3);
        assert_eq!(subs[0].status, "current");
        assert_eq!(subs[0].describe.as_deref(), Some("v1.2"));
        assert_eq!(subs[1].status, "uninitialized");
        assert_eq!(subs[2].status, "modified");
        assert_eq!(
            parse_lfs_patterns("*.psd filter=lfs diff=lfs merge=lfs -text\n*.txt text\nassets/** filter=lfs diff=lfs merge=lfs -text"),
            vec!["*.psd", "assets/**"]
        );
    }

    #[test]
    fn worktree_lock_and_sparse_checkout_work() {
        let repo = TempRepo::new();
        fs::create_dir_all(repo.path().join("src")).unwrap();
        fs::create_dir_all(repo.path().join("docs")).unwrap();
        write_file(repo.path(), "src/a.rs", "a\n");
        write_file(repo.path(), "docs/b.md", "b\n");
        commit_all(repo.path(), "init");
        let sparse = run_extended(repo.path(), &with("sparseSet", |a| a.message = Some("src".into()))).unwrap().unwrap();
        assert!(sparse.ok, "{}", sparse.stderr);
        assert!(!repo.path().join("docs/b.md").exists());
        let config = repo_config(repo.path()).unwrap();
        assert!(config.sparse_enabled && config.sparse_cone);
        assert_eq!(config.sparse_paths, vec!["src"]);
        run_extended(repo.path(), &act("sparseDisable")).unwrap();
        assert!(repo.path().join("docs/b.md").exists());
    }
}
