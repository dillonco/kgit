// AI assistance through the Claude API (Messages API over HTTPS): commit
// messages from the staged diff, commit explanations, and merge-conflict
// resolution with a confidence per conflict.
//
// Rust has no official Anthropic SDK, so this speaks raw HTTP through the
// system `curl` (in keeping with kGit shelling out to git and gh rather than
// linking clients). The API key lives in the macOS keychain and reaches curl
// on stdin as a config line, never on a command line where `ps` could see it.
// Nothing is sent unless the user has turned AI on in Settings.

use super::*;
use serde_json::{json, Value};

pub(crate) const DEFAULT_MODEL: &str = "claude-opus-5-5";
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
/// Opus 5.5 can decline a request on safety grounds; with server-side
/// fallbacks the API re-runs it on a suitable model inside the same call.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
const KEYCHAIN_SERVICE: &str = "kgit-anthropic-api-key";
const KEYCHAIN_ACCOUNT: &str = "default";
/// Larger inputs are refused with a clear message rather than truncated.
const MAX_INPUT_BYTES: usize = 600_000;

// ---- API key storage ----

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() < 400 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn keychain_key() -> Option<String> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-a", KEYCHAIN_ACCOUNT, "-w"])
        .output()
        .ok()?;
    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (output.status.success() && valid_key(&key)).then_some(key)
}

/// The key to use: the one saved in kGit's keychain item, else ANTHROPIC_API_KEY.
fn api_key() -> Option<String> {
    keychain_key().or_else(|| std::env::var("ANTHROPIC_API_KEY").ok().filter(|key| valid_key(key)))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub has_key: bool,
    /// "keychain", "environment" or null.
    pub source: Option<String>,
    pub model: String,
}

#[tauri::command(async)]
pub fn ai_status() -> AiStatus {
    let (has_key, source) = if keychain_key().is_some() {
        (true, Some("keychain".to_string()))
    } else if std::env::var("ANTHROPIC_API_KEY").ok().is_some_and(|key| valid_key(&key)) {
        (true, Some("environment".to_string()))
    } else {
        (false, None)
    };
    AiStatus { has_key, source, model: DEFAULT_MODEL.to_string() }
}

/// Saves (or, with an empty key, removes) the API key in the login keychain.
/// `security -i` reads its command from stdin, keeping the key out of argv.
#[tauri::command(async)]
pub fn ai_set_key(key: String) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        let _ = Command::new("security")
            .args(["delete-generic-password", "-s", KEYCHAIN_SERVICE, "-a", KEYCHAIN_ACCOUNT])
            .output();
        return Ok(());
    }
    if !valid_key(key) {
        return Err("That doesn't look like an API key (expected letters, digits, - and _).".to_string());
    }
    let mut child = Command::new("security")
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("unable to reach the keychain: {err}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        writeln!(stdin, "add-generic-password -U -s {KEYCHAIN_SERVICE} -a {KEYCHAIN_ACCOUNT} -w {key}")
            .map_err(|err| err.to_string())?;
    }
    let output = child.wait_with_output().map_err(|err| err.to_string())?;
    if keychain_key().as_deref() == Some(key) {
        Ok(())
    } else {
        Err(format!("the keychain did not save the key: {}", String::from_utf8_lossy(&output.stderr).trim()))
    }
}

// ---- the request ----

/// The Messages API request body. Structured output (`output_config.format`)
/// guarantees the reply parses against `schema`.
pub(crate) fn request_body(model: &str, effort: &str, system: &str, user: &str, schema: &Value) -> Value {
    json!({
        "model": model,
        "max_tokens": 16000,
        "system": system,
        "messages": [{ "role": "user", "content": user }],
        "output_config": {
            "effort": effort,
            "format": { "type": "json_schema", "schema": schema }
        },
        "fallbacks": "default"
    })
}

/// Turns an HTTP status and body into the reply's JSON, or a message a person
/// can act on.
pub(crate) fn parse_response(status: u16, body: &str) -> Result<Value, String> {
    let parsed: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let api_message = parsed
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match status {
        200 => {}
        401 => return Err("The Claude API rejected the API key. Check it in Settings › AI.".to_string()),
        403 => return Err(format!("The API key isn't allowed to do this. {api_message}")),
        404 => return Err(format!("Model or endpoint not found. {api_message}")),
        413 => return Err("The request is too large for the API.".to_string()),
        429 => return Err("Rate limited by the Claude API. Wait a moment and try again.".to_string()),
        500..=599 => return Err(format!("The Claude API is having trouble ({status}). Try again shortly. {api_message}")),
        _ => return Err(format!("Claude API error {status}: {api_message}")),
    }
    match parsed.get("stop_reason").and_then(Value::as_str) {
        Some("refusal") => return Err("Claude declined this request.".to_string()),
        Some("max_tokens") => return Err("The answer was cut off (output limit reached).".to_string()),
        _ => {}
    }
    let text: String = parsed
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    serde_json::from_str(&text).map_err(|err| format!("Claude's answer was not valid JSON: {err}"))
}

fn call_claude(model: &str, effort: &str, system: &str, user: &str, schema: &Value) -> Result<Value, String> {
    if user.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "This is {} KB of changes, more than kGit sends in one request ({} KB). Stage or select fewer files and try again.",
            user.len() / 1024,
            MAX_INPUT_BYTES / 1024
        ));
    }
    let key = api_key().ok_or_else(|| "Add a Claude API key in Settings › AI first.".to_string())?;
    let body = request_body(model, effort, system, user, schema).to_string();

    // The body goes in a private temp file; the headers, key included, go on
    // stdin as a curl config so they never appear in the process list.
    let dir = std::env::temp_dir().join(format!("kgit-ai-{}-{}", std::process::id(), rand_suffix()));
    fs::create_dir(&dir).map_err(|err| err.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    let body_path = dir.join("body.json");
    let outcome = (|| {
        fs::write(&body_path, &body).map_err(|err| err.to_string())?;
        let config = format!(
            "header = \"x-api-key: {key}\"\nheader = \"anthropic-version: {API_VERSION}\"\nheader = \"anthropic-beta: {FALLBACK_BETA}\"\nheader = \"content-type: application/json\"\n"
        );
        let mut child = Command::new("curl")
            .args(["-sS", "--max-time", "600", "-K", "-", "-w", "\n%{http_code}", "--data-binary"])
            .arg(format!("@{}", body_path.display()))
            .arg(API_URL)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| format!("unable to run curl: {err}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(config.as_bytes()).map_err(|err| err.to_string())?;
        }
        let output = child.wait_with_output().map_err(|err| err.to_string())?;
        if !output.status.success() {
            return Err(format!("Couldn't reach the Claude API: {}", String::from_utf8_lossy(&output.stderr).trim()));
        }
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let (response, status) = stdout.rsplit_once('\n').ok_or("empty response from the Claude API")?;
        parse_response(status.trim().parse().unwrap_or(0), response)
    })();
    let _ = fs::remove_dir_all(&dir);
    outcome
}

fn rand_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0)
}

fn model_or_default(model: Option<String>) -> String {
    model
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.'))
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

// ---- commit messages ----

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitMessageSuggestion {
    pub subject: String,
    pub body: String,
}

const COMMIT_SYSTEM: &str = "You write git commit messages. Given a staged diff, reply with a subject line \
in the imperative mood, at most 72 characters, no trailing period, following the repository's convention \
when recent subjects show one (for example a `feat:` / `fix:` prefix). The body explains what changed and \
why, wrapped at 72 columns, and is an empty string when the subject says it all. Describe only what the \
diff shows.";

pub(crate) fn commit_prompt(diff: &str, recent_subjects: &str, branch: &str) -> String {
    format!(
        "Branch: {branch}\n\nRecent commit subjects in this repository:\n{recent_subjects}\n\nStaged diff:\n<diff>\n{diff}\n</diff>"
    )
}

#[tauri::command(async)]
pub fn ai_commit_message(state: State<'_, AppState>, model: Option<String>) -> Result<CommitMessageSuggestion, String> {
    let root = active_repo(&state)?;
    let diff = git(&root, &["diff", "--cached", "--no-color", "--no-ext-diff"])?;
    if diff.trim().is_empty() {
        return Err("Stage some changes first; the message is written from the staged diff.".to_string());
    }
    let recent = git_optional(&root, &["log", "-15", "--format=%s"])?.unwrap_or_default();
    let branch = git_optional(&root, &["branch", "--show-current"])?.unwrap_or_default();
    let schema = json!({
        "type": "object",
        "properties": { "subject": { "type": "string" }, "body": { "type": "string" } },
        "required": ["subject", "body"],
        "additionalProperties": false
    });
    let value = call_claude(&model_or_default(model), "low", COMMIT_SYSTEM, &commit_prompt(&diff, &recent, &branch), &schema)?;
    let mut suggestion: CommitMessageSuggestion = serde_json::from_value(value).map_err(|err| err.to_string())?;
    suggestion.subject = suggestion.subject.lines().next().unwrap_or_default().trim().chars().take(72).collect();
    suggestion.body = suggestion.body.trim().to_string();
    Ok(suggestion)
}

// ---- explain a commit ----

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitExplanation {
    pub summary: String,
    pub details: Vec<String>,
    pub risks: Vec<String>,
}

#[tauri::command(async)]
pub fn ai_explain_commit(state: State<'_, AppState>, hash: String, model: Option<String>) -> Result<CommitExplanation, String> {
    let root = active_repo(&state)?;
    validate_ref_arg(&hash)?;
    let show = git(&root, &["show", "--no-color", "--no-ext-diff", "--stat", "--patch", "--format=%H%nAuthor: %an%nDate: %ad%n%n%B", &hash])?;
    let schema = json!({
        "type": "object",
        "properties": {
            "summary": { "type": "string" },
            "details": { "type": "array", "items": { "type": "string" } },
            "risks": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["summary", "details", "risks"],
        "additionalProperties": false
    });
    let system = "You explain git commits to a developer reviewing history. `summary` is two or three plain \
sentences on what the commit does and why. `details` lists the notable changes, one short sentence each. \
`risks` lists anything a reviewer should double-check (behaviour changes, missing tests, edge cases); leave \
it empty when nothing stands out. Base every statement on the commit shown.";
    let value = call_claude(&model_or_default(model), "medium", system, &format!("<commit>\n{show}\n</commit>"), &schema)?;
    serde_json::from_value(value).map_err(|err| err.to_string())
}

// ---- resolve merge conflicts ----

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictResolution {
    pub index: usize,
    /// The text that replaces the whole conflict region, markers excluded.
    pub resolution: String,
    /// 0..1: how sure the model is that this keeps both sides' intent.
    pub confidence: f64,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictProposal {
    pub summary: String,
    pub resolutions: Vec<ConflictResolution>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConflictRegionInput {
    pub index: usize,
    pub ours: String,
    pub theirs: String,
    pub base: Option<String>,
}

pub(crate) fn conflict_prompt(path: &str, file: &str, regions: &[ConflictRegionInput]) -> String {
    let mut prompt = format!("File: {path}\n\nThe whole file, with conflict markers:\n<file>\n{file}\n</file>\n\nConflicts to resolve:\n");
    for region in regions {
        prompt.push_str(&format!("\n<conflict index=\"{}\">\n<ours>\n{}</ours>\n", region.index, region.ours));
        if let Some(base) = &region.base {
            prompt.push_str(&format!("<base>\n{base}</base>\n"));
        }
        prompt.push_str(&format!("<theirs>\n{}</theirs>\n</conflict>\n", region.theirs));
    }
    prompt
}

#[tauri::command(async)]
pub fn ai_resolve_conflicts(
    path: String,
    file: String,
    regions: Vec<ConflictRegionInput>,
    model: Option<String>,
) -> Result<ConflictProposal, String> {
    validate_repo_relative_path(&path)?;
    if regions.is_empty() {
        return Err("There are no conflicts in this file.".to_string());
    }
    let schema = json!({
        "type": "object",
        "properties": {
            "summary": { "type": "string" },
            "resolutions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "index": { "type": "integer" },
                        "resolution": { "type": "string" },
                        "confidence": { "type": "number" },
                        "explanation": { "type": "string" }
                    },
                    "required": ["index", "resolution", "confidence", "explanation"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["summary", "resolutions"],
        "additionalProperties": false
    });
    let system = "You resolve git merge conflicts. For each conflict, write the code that should replace the \
whole conflict region (without markers) so that both sides' intent survives: keep both changes when they are \
compatible, and combine them when they touch the same lines. Match the file's style and indentation, and end \
each resolution with a newline when the sides do. `confidence` is 0 to 1: high only when the combination is \
clearly right, low when the sides contradict each other or you had to guess. `explanation` says in one or two \
sentences what you kept from each side. Return one entry per conflict index given.";
    let value = call_claude(&model_or_default(model), "high", system, &conflict_prompt(&path, &file, &regions), &schema)?;
    let mut proposal: ConflictProposal = serde_json::from_value(value).map_err(|err| err.to_string())?;
    let expected: std::collections::HashSet<usize> = regions.iter().map(|region| region.index).collect();
    proposal.resolutions.retain(|resolution| expected.contains(&resolution.index));
    for resolution in &mut proposal.resolutions {
        resolution.confidence = resolution.confidence.clamp(0.0, 1.0);
    }
    if proposal.resolutions.len() != expected.len() {
        return Err(format!(
            "Claude resolved {} of {} conflicts; resolve the rest by hand or try again.",
            proposal.resolutions.len(),
            expected.len()
        ));
    }
    Ok(proposal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_uses_structured_output_effort_and_fallbacks() {
        let body = request_body(DEFAULT_MODEL, "low", "sys", "hi", &json!({"type": "object"}));
        assert_eq!(body["model"], "claude-opus-5-5");
        assert_eq!(body["output_config"]["effort"], "low");
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert_eq!(body["fallbacks"], "default");
        assert!(body.get("thinking").is_none(), "Opus 5.5 rejects a disabled thinking config");
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn parses_successful_json_answers() {
        let body = r#"{"stop_reason":"end_turn","content":[{"type":"thinking","thinking":""},{"type":"text","text":"{\"subject\":\"Add x\",\"body\":\"\"}"}]}"#;
        let value = parse_response(200, body).unwrap();
        assert_eq!(value["subject"], "Add x");
    }

    #[test]
    fn maps_errors_and_refusals_to_readable_messages() {
        assert!(parse_response(401, r#"{"error":{"message":"invalid x-api-key"}}"#).unwrap_err().contains("API key"));
        assert!(parse_response(429, "{}").unwrap_err().contains("Rate limited"));
        assert!(parse_response(529, r#"{"error":{"message":"Overloaded"}}"#).unwrap_err().contains("trouble"));
        assert!(parse_response(400, r#"{"error":{"message":"bad schema"}}"#).unwrap_err().contains("bad schema"));
        assert!(parse_response(200, r#"{"stop_reason":"refusal","content":[]}"#).unwrap_err().contains("declined"));
        assert!(parse_response(200, r#"{"stop_reason":"max_tokens","content":[]}"#).unwrap_err().contains("cut off"));
        assert!(parse_response(200, r#"{"stop_reason":"end_turn","content":[{"type":"text","text":"nope"}]}"#).is_err());
    }

    #[test]
    fn keys_and_models_are_validated() {
        assert!(valid_key("sk-ant-api03-AbC_123"));
        assert!(!valid_key("sk-ant\"\nheader = \"x: y"));
        assert!(!valid_key(""));
        assert_eq!(model_or_default(Some("claude-sonnet-5-5".into())), "claude-sonnet-5-5");
        assert_eq!(model_or_default(Some("bad model; rm".into())), DEFAULT_MODEL);
        assert_eq!(model_or_default(None), DEFAULT_MODEL);
    }

    #[test]
    fn prompts_carry_the_inputs() {
        let prompt = conflict_prompt(
            "a.rs",
            "x",
            &[ConflictRegionInput { index: 0, ours: "a\n".into(), theirs: "b\n".into(), base: Some("c\n".into()) }],
        );
        assert!(prompt.contains("<conflict index=\"0\">") && prompt.contains("<base>\nc\n</base>"));
        assert!(commit_prompt("+x", "feat: y", "main").contains("<diff>\n+x\n</diff>"));
    }
}
