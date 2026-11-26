// GitHub pull requests, issues and shared patches, through the `gh` CLI
// (same model as gh.rs: no tokens are stored by kGit; gh owns auth).

use super::*;
use serde_json::Value;

fn gh_in(root: Option<&Path>, args: &[&str], stdin: Option<&str>) -> Result<String, String> {
    let gh = gh::resolve_gh().ok_or_else(|| "GitHub CLI not found; install with `brew install gh`.".to_string())?;
    let mut command = Command::new(&gh);
    command.args(args).env("GH_PROMPT_DISABLED", "1").env("GIT_TERMINAL_PROMPT", "0");
    if let Some(root) = root {
        command.current_dir(root);
    }
    let result = match stdin {
        Some(input) => {
            command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            let mut child = command.spawn().map_err(|err| err.to_string())?;
            if let Some(mut pipe) = child.stdin.take() {
                pipe.write_all(input.as_bytes()).map_err(|err| err.to_string())?;
            }
            result_from_output(child.wait_with_output().map_err(|err| err.to_string())?, false)
        }
        None => run_command(&mut command, false),
    };
    if result.ok {
        Ok(result.stdout)
    } else {
        Err(if result.stderr.trim().is_empty() {
            format!("gh {} failed with exit code {}", args.first().unwrap_or(&""), result.code)
        } else {
            result.stderr.trim().to_string()
        })
    }
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub head: String,
    pub base: String,
    pub author: String,
    pub draft: bool,
    pub state: String,
    /// "APPROVED", "CHANGES_REQUESTED", "REVIEW_REQUIRED" or "".
    pub review: String,
    /// Rolled-up CI: "success", "failure", "pending" or "" when there are no checks.
    pub checks: String,
    pub updated_at: String,
    /// `owner/name`, filled for cross-repository searches.
    pub repository: String,
}

pub(crate) fn rollup(checks: &Value) -> String {
    let Some(items) = checks.as_array() else { return String::new() };
    if items.is_empty() {
        return String::new();
    }
    let states: Vec<String> = items
        .iter()
        .map(|item| {
            item.get("conclusion")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .or_else(|| item.get("state").and_then(Value::as_str))
                .or_else(|| item.get("status").and_then(Value::as_str))
                .unwrap_or("")
                .to_ascii_uppercase()
        })
        .collect();
    if states.iter().any(|state| matches!(state.as_str(), "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED")) {
        "failure".to_string()
    } else if states.iter().all(|state| matches!(state.as_str(), "SUCCESS" | "NEUTRAL" | "SKIPPED")) {
        "success".to_string()
    } else {
        "pending".to_string()
    }
}

fn str_at<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    let mut current = value;
    for key in path {
        match current.get(key) {
            Some(next) => current = next,
            None => return "",
        }
    }
    current.as_str().unwrap_or("")
}

pub(crate) fn parse_prs(json: &str) -> Result<Vec<PullRequest>, String> {
    let items: Vec<Value> = serde_json::from_str(json).map_err(|err| format!("unexpected gh output: {err}"))?;
    Ok(items
        .iter()
        .map(|item| PullRequest {
            number: item.get("number").and_then(Value::as_u64).unwrap_or(0),
            title: str_at(item, &["title"]).to_string(),
            url: str_at(item, &["url"]).to_string(),
            head: str_at(item, &["headRefName"]).to_string(),
            base: str_at(item, &["baseRefName"]).to_string(),
            author: str_at(item, &["author", "login"]).to_string(),
            draft: item.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
            state: str_at(item, &["state"]).to_string(),
            review: str_at(item, &["reviewDecision"]).to_string(),
            checks: rollup(item.get("statusCheckRollup").unwrap_or(&Value::Null)),
            updated_at: str_at(item, &["updatedAt"]).to_string(),
            repository: str_at(item, &["repository", "nameWithOwner"]).to_string(),
        })
        .collect())
}

const PR_FIELDS: &str = "number,title,url,headRefName,baseRefName,author,isDraft,state,reviewDecision,statusCheckRollup,updatedAt";

/// Open pull requests of the open repository.
#[tauri::command(async)]
pub fn gh_pr_list(state: State<'_, AppState>) -> Result<Vec<PullRequest>, String> {
    let root = active_repo(&state)?;
    parse_prs(&gh_in(Some(&root), &["pr", "list", "--state", "open", "--limit", "100", "--json", PR_FIELDS], None)?)
}

/// Pull requests across every repository: yours, and those awaiting your review.
#[tauri::command(async)]
pub fn gh_my_prs() -> Result<(Vec<PullRequest>, Vec<PullRequest>), String> {
    let fields = "number,title,url,repository,author,isDraft,state,updatedAt";
    let mine = parse_prs(&gh_in(None, &["search", "prs", "--author", "@me", "--state", "open", "--limit", "50", "--json", fields], None)?)?;
    let review = parse_prs(&gh_in(None, &["search", "prs", "--review-requested", "@me", "--state", "open", "--limit", "50", "--json", fields], None)?)?;
    Ok((mine, review))
}

#[tauri::command(async)]
pub fn gh_pr_create(
    state: State<'_, AppState>,
    title: String,
    body: String,
    base: String,
    head: String,
    draft: bool,
) -> Result<String, String> {
    let root = active_repo(&state)?;
    validate_ref_arg(&base)?;
    validate_ref_arg(&head)?;
    if title.trim().is_empty() {
        return Err("a pull request needs a title".to_string());
    }
    // The body goes through stdin (`--body-file -`) so it can hold anything.
    let mut args = vec!["pr", "create", "--title", title.trim(), "--base", &base, "--head", &head, "--body-file", "-"];
    if draft {
        args.push("--draft");
    }
    Ok(gh_in(Some(&root), &args, Some(&body))?.trim().to_string())
}

#[tauri::command(async)]
pub fn gh_pr_checkout(state: State<'_, AppState>, number: u64) -> Result<GitResult, String> {
    let root = active_repo(&state)?;
    let out = gh_in(Some(&root), &["pr", "checkout", &number.to_string()], None)?;
    Ok(GitResult { ok: true, stdout: out, stderr: String::new(), code: 0, refresh: true })
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub updated_at: String,
}

pub(crate) fn parse_issues(json: &str) -> Result<Vec<Issue>, String> {
    let items: Vec<Value> = serde_json::from_str(json).map_err(|err| format!("unexpected gh output: {err}"))?;
    let names = |value: Option<&Value>, key: &str| -> Vec<String> {
        value
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(|entry| entry.get(key).and_then(Value::as_str)).map(ToOwned::to_owned).collect())
            .unwrap_or_default()
    };
    Ok(items
        .iter()
        .map(|item| Issue {
            number: item.get("number").and_then(Value::as_u64).unwrap_or(0),
            title: str_at(item, &["title"]).to_string(),
            url: str_at(item, &["url"]).to_string(),
            labels: names(item.get("labels"), "name"),
            assignees: names(item.get("assignees"), "login"),
            updated_at: str_at(item, &["updatedAt"]).to_string(),
        })
        .collect())
}

#[tauri::command(async)]
pub fn gh_issue_list(state: State<'_, AppState>, mine: bool) -> Result<Vec<Issue>, String> {
    let root = active_repo(&state)?;
    let mut args = vec!["issue", "list", "--state", "open", "--limit", "100", "--json", "number,title,url,labels,assignees,updatedAt"];
    if mine {
        args.extend(["--assignee", "@me"]);
    }
    parse_issues(&gh_in(Some(&root), &args, None)?)
}

/// A branch name for working on an issue: "123-short-title-slug".
pub(crate) fn issue_branch_name(number: u64, title: &str) -> String {
    let mut slug = String::new();
    for ch in title.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
        if slug.len() >= 48 {
            break;
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        format!("issue-{number}")
    } else {
        format!("{number}-{slug}")
    }
}

#[tauri::command]
pub fn issue_branch(number: u64, title: String) -> String {
    issue_branch_name(number, &title)
}

/// Opens an https link (a pull request, issue or gist) in the default browser.
#[tauri::command(async)]
pub fn open_url(url: String) -> Result<(), String> {
    let url = url.trim();
    if !url.starts_with("https://") || url.contains(char::is_whitespace) {
        return Err(format!("not an https link: {url:?}"));
    }
    let result = run_command(Command::new("open").arg(url), false);
    if result.ok { Ok(()) } else { Err(result.stderr) }
}

// ---- shared patches (secret gists) ----

/// Uploads a patch as a secret gist (visible only to people with the link)
/// and returns its URL.
#[tauri::command(async)]
pub fn share_patch(patch: String, name: String, description: String, public: bool) -> Result<String, String> {
    if patch.trim().is_empty() {
        return Err("the patch is empty".to_string());
    }
    let name: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')).collect();
    let name = if name.is_empty() { "kgit.patch".to_string() } else { name };
    let mut args = vec!["gist", "create", "--filename", &name, "--desc", &description];
    if public {
        args.push("--public");
    }
    args.push("-");
    let out = gh_in(None, &args, Some(&patch))?;
    out.lines()
        .rev()
        .find(|line| line.starts_with("https://"))
        .map(|line| line.trim().to_string())
        .ok_or_else(|| "gh did not return a gist URL".to_string())
}

/// The gist id from a URL or a bare id.
pub(crate) fn gist_id(input: &str) -> Result<String, String> {
    let trimmed = input.trim().trim_end_matches('/');
    let id = trimmed.rsplit('/').next().unwrap_or(trimmed);
    let id = id.split(['#', '?']).next().unwrap_or(id);
    if id.len() >= 8 && id.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(id.to_string())
    } else {
        Err(format!("not a gist link: {input:?}"))
    }
}

#[tauri::command(async)]
pub fn fetch_shared_patch(link: String) -> Result<String, String> {
    let id = gist_id(&link)?;
    gh_in(None, &["gist", "view", &id, "--raw"], None).map(|text| {
        // `--raw` prints the description first when there is one; a patch starts at its first header.
        match text.find("From ").or_else(|| text.find("diff --git")) {
            Some(start) => text[start..].to_string(),
            None => text,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pr_list_with_check_rollup() {
        let json = r#"[{"number":7,"title":"Add x","url":"https://github.com/o/r/pull/7","headRefName":"feat/x","baseRefName":"main",
          "author":{"login":"ann"},"isDraft":false,"state":"OPEN","reviewDecision":"APPROVED",
          "statusCheckRollup":[{"conclusion":"SUCCESS"},{"state":"SUCCESS"}],"updatedAt":"2026-11-01T00:00:00Z"},
          {"number":8,"title":"WIP","url":"u","headRefName":"wip","baseRefName":"main","author":{"login":"bo"},"isDraft":true,
          "state":"OPEN","reviewDecision":"","statusCheckRollup":[{"conclusion":"FAILURE"},{"status":"IN_PROGRESS","conclusion":""}],"updatedAt":""}]"#;
        let prs = parse_prs(json).unwrap();
        assert_eq!(prs[0].checks, "success");
        assert_eq!(prs[0].author, "ann");
        assert_eq!(prs[0].head, "feat/x");
        assert_eq!(prs[1].checks, "failure");
        assert!(prs[1].draft);
        assert_eq!(rollup(&serde_json::json!([{"status": "IN_PROGRESS", "conclusion": ""}])), "pending");
        assert_eq!(rollup(&serde_json::json!([])), "");
    }

    #[test]
    fn parses_issues_and_builds_branch_names() {
        let issues = parse_issues(r#"[{"number":12,"title":"Crash on open","url":"u","labels":[{"name":"bug"}],"assignees":[{"login":"ann"}],"updatedAt":"x"}]"#).unwrap();
        assert_eq!(issues[0].labels, vec!["bug"]);
        assert_eq!(issues[0].assignees, vec!["ann"]);
        assert_eq!(issue_branch_name(12, "Crash on open: NPE in `parse()`!"), "12-crash-on-open-npe-in-parse");
        assert_eq!(issue_branch_name(3, "!!!"), "issue-3");
    }

    #[test]
    fn gist_ids_come_from_links_or_bare_ids() {
        assert_eq!(gist_id("https://gist.github.com/ann/0123456789abcdef").unwrap(), "0123456789abcdef");
        assert_eq!(gist_id("0123456789abcdef/").unwrap(), "0123456789abcdef");
        assert!(gist_id("https://evil.example/--upload").is_err());
    }
}
