//! AI-generated git commit messages.
//!
//! Generates a one-line commit message from the working-tree diff via an
//! OpenAI-compatible chat-completions endpoint (Groq or GitHub Models). The
//! request runs on a background worker thread (see [`crate::util::background`])
//! and the result is delivered back over an `mpsc` channel, polled each frame by
//! [`App::pump_ai_commit`].

use std::sync::mpsc;

use std::time::{Duration, Instant};

use crate::util::background::spawn_worker;
use crate::{AiCommitMsg, AiKeyCheckMsg, AiKeyStatus, App, AppMode};

/// Wait this long after the last keystroke before validating the API key, so a
/// pasted or typed key is only tested once the user pauses.
const KEY_CHECK_DEBOUNCE: Duration = Duration::from_millis(600);

/// Cap on a single key-validation request so a stalled connection can't leak a
/// worker thread indefinitely.
const KEY_CHECK_TIMEOUT: Duration = Duration::from_secs(15);

/// A supported AI provider: its endpoint, default model, and the env var that
/// holds the API key when one is not set in the persisted config.
pub(crate) struct AiProvider {
    pub key: &'static str,
    pub label: &'static str,
    pub endpoint: &'static str,
    pub default_model: &'static str,
    pub env_var: &'static str,
}

pub(crate) const AI_PROVIDERS: &[AiProvider] = &[
    AiProvider {
        key: "groq",
        label: "Groq",
        endpoint: "https://api.groq.com/openai/v1/chat/completions",
        default_model: "llama-3.3-70b-versatile",
        env_var: "GROQ_API_KEY",
    },
    AiProvider {
        key: "github",
        label: "GitHub Models",
        endpoint: "https://models.github.ai/inference/chat/completions",
        default_model: "openai/gpt-4o-mini",
        env_var: "GITHUB_TOKEN",
    },
];

/// Look up a provider by its persisted key, defaulting to the first provider.
pub(crate) fn provider_by_key(key: &str) -> &'static AiProvider {
    AI_PROVIDERS
        .iter()
        .find(|p| p.key == key)
        .unwrap_or(&AI_PROVIDERS[0])
}

const MAX_DIFF_LINES: usize = 6000;
const MAX_DIFF_BYTES: usize = 100_000;

/// Bound a diff so a huge changeset can't blow up the request body. Truncates to
/// at most `MAX_DIFF_LINES` lines and `MAX_DIFF_BYTES` bytes (on a char
/// boundary), appending a marker when anything was dropped.
pub(crate) fn truncate_diff(diff: &str) -> String {
    let total_lines = diff.lines().count();
    let mut out: String = diff
        .lines()
        .take(MAX_DIFF_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    let mut truncated = total_lines > MAX_DIFF_LINES;

    if out.len() > MAX_DIFF_BYTES {
        let mut end = MAX_DIFF_BYTES;
        while end > 0 && !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        truncated = true;
    }

    if truncated {
        out.push_str("\n... [diff truncated]");
    }
    out
}

/// Validate an API key with a tiny authenticated request, classifying the
/// outcome by HTTP status. A 401/403 is a clear rejection; any other status
/// (success, bad model, rate limit, …) still means the credentials were
/// accepted. Transport errors are reported separately so they don't masquerade
/// as a rejected key.
fn check_api_key(endpoint: &str, api_key: &str, model: &str) -> Result<bool, String> {
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": "ping" }],
        "max_tokens": 1
    });
    match ureq::post(endpoint)
        .timeout(KEY_CHECK_TIMEOUT)
        .set("Authorization", &format!("Bearer {}", api_key))
        .set("Content-Type", "application/json")
        .send_json(body)
    {
        Ok(_) => Ok(true),
        Err(ureq::Error::Status(401, _)) | Err(ureq::Error::Status(403, _)) => Ok(false),
        Err(ureq::Error::Status(_, _)) => Ok(true),
        Err(e) => Err(format!("key check failed: {}", e)),
    }
}

/// Build the chat-completions request body for a commit-message generation.
fn build_request_body(model: &str, diff: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You write git commit messages. Reply with ONLY a single-line \
                    commit message in the imperative mood (max 72 characters). No quotes, \
                    no body, no explanation, no markdown."
            },
            {
                "role": "user",
                "content": format!("Write a commit message for this diff:\n\n{}", diff)
            }
        ],
        "temperature": 0.2,
        "max_tokens": 100
    })
}

/// Extract the assistant message content from a chat-completions response.
fn parse_commit_message(value: &serde_json::Value) -> Result<String, String> {
    let content = value["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "AI response missing message content".to_string())?;
    let msg = content
        .trim()
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .trim()
        .to_string();
    if msg.is_empty() {
        Err("AI returned an empty message".to_string())
    } else {
        Ok(msg)
    }
}

/// Perform the blocking HTTP request. Runs on a worker thread.
fn generate_commit_message(
    endpoint: &str,
    api_key: &str,
    model: &str,
    diff: &str,
) -> Result<String, String> {
    let body = build_request_body(model, diff);
    match ureq::post(endpoint)
        .set("Authorization", &format!("Bearer {}", api_key))
        .set("Content-Type", "application/json")
        .send_json(body)
    {
        Ok(resp) => {
            let value: serde_json::Value = resp
                .into_json()
                .map_err(|e| format!("invalid AI response: {}", e))?;
            parse_commit_message(&value)
        }
        Err(ureq::Error::Status(code, resp)) => {
            let detail: String = resp
                .into_string()
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            Err(format!("AI API error {}: {}", code, detail))
        }
        Err(e) => Err(format!("AI request failed: {}", e)),
    }
}

impl App {
    /// Resolve the API key: the persisted config value, or the provider's env
    /// var as a fallback. Returns `None` when neither is set.
    pub(crate) fn resolve_ai_api_key(&self) -> Option<String> {
        let key = self.ai_api_key.trim();
        if !key.is_empty() {
            return Some(key.to_string());
        }
        let provider = provider_by_key(&self.ai_provider);
        std::env::var(provider.env_var)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Resolve the model id: the configured value, or the provider default.
    pub(crate) fn resolve_ai_model(&self) -> String {
        let provider = provider_by_key(&self.ai_provider);
        let model = self.ai_model.trim();
        if model.is_empty() {
            provider.default_model.to_string()
        } else {
            model.to_string()
        }
    }

    /// Kick off a background AI commit-message generation from the current diff.
    /// On success [`pump_ai_commit`](Self::pump_ai_commit) prefills the commit
    /// input; on any failure it sets a status message instead.
    pub(crate) fn request_commit_message(&mut self) {
        if self.ai_commit_rx.is_some() {
            self.set_status("AI commit message already generating...");
            return;
        }
        let provider = provider_by_key(&self.ai_provider);
        let Some(api_key) = self.resolve_ai_api_key() else {
            self.set_status(format!(
                "no API key — set it in Settings (Tab) or export ${}",
                provider.env_var
            ));
            return;
        };
        let diff = self.collect_commit_diff();
        if diff.trim().is_empty() {
            self.set_status("no changes to summarize");
            return;
        }
        let endpoint = provider.endpoint.to_string();
        let model = self.resolve_ai_model();
        self.set_status(format!("generating commit message via {}...", provider.label));
        self.ai_commit_rx = Some(spawn_worker(move |tx| {
            let result = generate_commit_message(&endpoint, &api_key, &model, &diff);
            let _ = tx.send(match result {
                Ok(msg) => AiCommitMsg::Ok(msg),
                Err(err) => AiCommitMsg::Err(err),
            });
        }));
    }

    /// Cycle the Settings panel's provider selection and persist it.
    pub(crate) fn settings_cycle_provider(&mut self, forward: bool) {
        let n = AI_PROVIDERS.len();
        let idx = AI_PROVIDERS
            .iter()
            .position(|p| p.key == self.ai_provider)
            .unwrap_or(0);
        let next = if forward { (idx + 1) % n } else { (idx + n - 1) % n };
        self.ai_provider = AI_PROVIDERS[next].key.to_string();
        self.persist_ai_settings();
        self.set_status(format!("AI provider: {}", AI_PROVIDERS[next].label));
    }

    /// Append a character to the focused Settings text field (Model or API Key).
    pub(crate) fn settings_input_char(&mut self, c: char) {
        match self.settings_selected {
            1 => self.ai_model.push(c),
            2 => {
                self.ai_api_key.push(c);
                self.note_ai_key_edited();
            }
            _ => return,
        }
        self.persist_ai_settings();
    }

    /// Delete the last character of the focused Settings text field.
    pub(crate) fn settings_input_backspace(&mut self) {
        match self.settings_selected {
            1 => {
                self.ai_model.pop();
            }
            2 => {
                self.ai_api_key.pop();
                self.note_ai_key_edited();
            }
            _ => return,
        }
        self.persist_ai_settings();
    }

    /// Mark the API key as just-edited: clear any prior validation result (the
    /// ✓/✗ disappears the moment the key changes) and arm the debounce so the
    /// new value is re-checked once the user pauses.
    fn note_ai_key_edited(&mut self) {
        self.ai_key_status = AiKeyStatus::Unknown;
        self.ai_key_edit_at = Some(Instant::now());
    }

    /// Begin a background validation of the current API key — unless it's empty
    /// or unchanged since the last check. Called when focus leaves the key
    /// field, when typing pauses (debounce), and when the Settings panel opens.
    pub(crate) fn maybe_check_api_key(&mut self) {
        // Consume any pending debounce regardless of outcome.
        self.ai_key_edit_at = None;
        let key = self.ai_api_key.trim().to_string();
        if key.is_empty() {
            self.ai_key_status = AiKeyStatus::Unknown;
            self.ai_key_checked = None;
            return;
        }
        if self.ai_key_checked.as_deref() == Some(key.as_str()) {
            return; // already validated this exact value
        }
        self.ai_key_checked = Some(key.clone());
        self.ai_key_status = AiKeyStatus::Checking;
        let provider = provider_by_key(&self.ai_provider);
        let endpoint = provider.endpoint.to_string();
        let model = self.resolve_ai_model();
        // Replacing the receiver drops any in-flight check; its result is for an
        // older key and `pump_ai_key_check` would discard it anyway.
        self.ai_key_check_rx = Some(spawn_worker(move |tx| {
            let msg = match check_api_key(&endpoint, &key, &model) {
                Ok(valid) => AiKeyCheckMsg::Result { key, valid },
                Err(message) => AiKeyCheckMsg::Error { key, message },
            };
            let _ = tx.send(msg);
        }));
    }

    /// Fire the debounced key check after the user pauses, and poll the
    /// validation channel. Results for a since-changed key are discarded.
    pub(crate) fn pump_ai_key_check(&mut self) {
        if let Some(edited) = self.ai_key_edit_at {
            if edited.elapsed() >= KEY_CHECK_DEBOUNCE {
                self.maybe_check_api_key();
            }
        }
        let Some(rx) = self.ai_key_check_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(AiKeyCheckMsg::Result { key, valid }) => {
                self.ai_key_check_rx = None;
                if self.ai_api_key.trim() == key {
                    self.ai_key_status =
                        if valid { AiKeyStatus::Valid } else { AiKeyStatus::Invalid };
                }
            }
            Ok(AiKeyCheckMsg::Error { key, message }) => {
                self.ai_key_check_rx = None;
                if self.ai_api_key.trim() == key {
                    self.ai_key_status = AiKeyStatus::Unknown;
                    self.set_status(message);
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.ai_key_check_rx = None;
            }
        }
    }

    /// Persist the current AI settings (provider/model/key) to the config file.
    fn persist_ai_settings(&self) {
        let provider = self.ai_provider.clone();
        let model = self.ai_model.clone();
        let key = self.ai_api_key.clone();
        crate::util::config::SbPersistConfig::update(move |cfg| {
            cfg.ai_provider = provider;
            cfg.ai_model = model;
            cfg.ai_api_key = key;
        });
    }

    /// Poll the AI commit-message channel. On success, prefill the (still
    /// editable) commit-message input if the user is still entering one.
    pub(crate) fn pump_ai_commit(&mut self) {
        let Some(rx) = self.ai_commit_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(AiCommitMsg::Ok(text)) => {
                self.ai_commit_rx = None;
                if self.mode == AppMode::GitCommitMessage {
                    self.begin_input_edit(AppMode::GitCommitMessage, text);
                    self.set_status("AI message ready — edit and Enter to commit, or Ctrl+G to retry");
                }
            }
            Ok(AiCommitMsg::Err(err)) => {
                self.ai_commit_rx = None;
                self.set_status(err);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.ai_commit_rx = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_diff_passes_small_diffs_through() {
        let diff = "line one\nline two\n";
        assert_eq!(truncate_diff(diff), "line one\nline two");
    }

    #[test]
    fn truncate_diff_caps_long_diffs_by_line_count() {
        let diff = (0..MAX_DIFF_LINES + 50)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let out = truncate_diff(&diff);
        assert!(out.ends_with("... [diff truncated]"));
        // MAX_DIFF_LINES kept lines + the marker line.
        assert_eq!(out.lines().count(), MAX_DIFF_LINES + 1);
    }

    #[test]
    fn parse_commit_message_extracts_first_line() {
        let v = serde_json::json!({
            "choices": [{"message": {"content": "\"feat: add settings tab\"\nextra"}}]
        });
        assert_eq!(parse_commit_message(&v).unwrap(), "feat: add settings tab");
    }

    #[test]
    fn parse_commit_message_errors_on_missing_content() {
        let v = serde_json::json!({"choices": []});
        assert!(parse_commit_message(&v).is_err());
    }

    #[test]
    fn provider_lookup_defaults_to_groq() {
        assert_eq!(provider_by_key("github").key, "github");
        assert_eq!(provider_by_key("nonexistent").key, "groq");
    }
}
