//! AI-proposed folder reorganization ("Organize", `Ctrl+O`).
//!
//! Asks the same OpenAI-compatible chat-completions endpoint used for commit
//! messages (see [`crate::app_ai`]) to propose grouping a directory's
//! top-level entries into new or existing subfolders. The request runs on a
//! background worker thread and the result is delivered over an `mpsc`
//! channel, polled each frame by [`App::pump_ai_organize`]. Nothing on disk
//! changes until the user reviews the plan and presses Confirm.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;

use crate::app_ai::{post_chat_completions, provider_by_key};
use crate::util::background::{pump_once, spawn_worker};
use crate::{App, AppMode, OrganizeMove, OrganizePlan, OrganizePlanMsg};

/// Cap on the number of directory entries sent to the AI, so a huge directory
/// can't blow up the request body (mirrors `app_ai::MAX_DIFF_LINES`).
const MAX_ENTRIES: usize = 300;

/// Build the chat-completions request body for an organize-plan generation.
fn build_organize_request_body(
    model: &str,
    entries: &[(String, bool)],
    existing_folders: &[String],
) -> serde_json::Value {
    let listing = entries
        .iter()
        .map(|(name, is_dir)| format!("{}{}", name, if *is_dir { "/" } else { "" }))
        .collect::<Vec<_>>()
        .join("\n");
    let folders_hint = if existing_folders.is_empty() {
        "(none yet)".to_string()
    } else {
        existing_folders.join(", ")
    };

    serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You organize files into folders. Given a flat listing of a \
                    directory's top-level entries, reply with ONLY a single JSON object \
                    (no markdown, no explanation) of the shape:\n\
                    {\"folders\": [\"NewFolderName\", ...], \"moves\": [{\"name\": \"entry-name\", \"folder\": \"FolderName\"}]}\n\
                    Rules: only include entries in \"moves\" that should actually relocate — \
                    omit entries that are already well-placed. Each \"folder\" value must be a \
                    single path segment: no slashes, no \"..\", not empty. You may reuse one of \
                    the existing folders instead of proposing a new one. Never propose moving an \
                    entry into itself. List every folder you use (new or existing) in \"folders\" \
                    only if it does not already exist."
            },
            {
                "role": "user",
                "content": format!(
                    "Existing folders: {}\n\nTop-level entries (directories end with '/'):\n{}",
                    folders_hint, listing
                )
            }
        ],
        "temperature": 0.2,
        "max_tokens": 1000
    })
}

/// Strip a possible ```` ```json ... ``` ```` (or plain ```` ``` ```` ) fence
/// around a model response so `serde_json` can parse the inner object.
fn strip_json_fence(content: &str) -> &str {
    let trimmed = content.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    let rest = rest.strip_prefix("json").unwrap_or(rest);
    rest.strip_suffix("```").unwrap_or(rest).trim()
}

/// A path segment is safe to use as a single-component folder/entry name: no
/// separators, not empty, and not `.`/`..`.
fn is_safe_segment(s: &str) -> bool {
    !s.is_empty() && !s.contains('/') && !s.contains('\\') && s != "." && s != ".."
}

/// Extract, sanitize, and validate an [`OrganizePlan`] from a chat-completions
/// response. `valid_names` is the real set of entry names in the directory —
/// any `move.name` not in that set (a hallucinated source path) is dropped,
/// as is any folder name that isn't a safe single path segment.
fn parse_organize_plan(
    value: &serde_json::Value,
    valid_names: &HashSet<String>,
) -> Result<OrganizePlan, String> {
    let content = value["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "AI response missing message content".to_string())?;
    let json_str = strip_json_fence(content);
    let raw: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("AI returned invalid plan JSON: {}", e))?;

    let folders: Vec<String> = raw["folders"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .filter(|f| is_safe_segment(f))
                .collect()
        })
        .unwrap_or_default();

    let moves: Vec<OrganizeMove> = raw["moves"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let name = v["name"].as_str()?.to_string();
                    let folder = v["folder"].as_str()?.to_string();
                    Some((name, folder))
                })
                .filter(|(name, folder)| {
                    valid_names.contains(name) && is_safe_segment(folder) && folder != name
                })
                .map(|(name, folder)| OrganizeMove { name, folder })
                .collect()
        })
        .unwrap_or_default();

    if moves.is_empty() {
        return Err("AI proposed no changes".to_string());
    }

    // Only keep folders that are actually referenced by a surviving move, plus
    // ensure every referenced folder is present even if the model omitted it
    // from the `folders` array.
    let referenced: HashSet<&str> = moves.iter().map(|m| m.folder.as_str()).collect();
    let mut folders: Vec<String> = folders
        .into_iter()
        .filter(|f| referenced.contains(f.as_str()))
        .collect();
    for name in &referenced {
        if !folders.iter().any(|f| f == name) {
            folders.push((*name).to_string());
        }
    }

    Ok(OrganizePlan { folders, moves })
}

/// Perform the blocking HTTP request and parse the response. Runs on a worker thread.
fn generate_organize_plan(
    endpoint: &str,
    api_key: &str,
    model: &str,
    entries: &[(String, bool)],
    existing_folders: &[String],
    valid_names: &HashSet<String>,
) -> Result<OrganizePlan, String> {
    let body = build_organize_request_body(model, entries, existing_folders);
    let value = post_chat_completions(endpoint, api_key, body)?;
    parse_organize_plan(&value, valid_names)
}

impl App {
    /// Kick off a background AI organize-plan request for the given directory's
    /// top-level entries. Called right after entering `AppMode::Organize`.
    pub(crate) fn request_organize_plan(&mut self, dir: PathBuf) {
        if self.organize.rx.is_some() {
            self.set_status("organize plan already generating...");
            return;
        }
        let provider = provider_by_key(&self.ai.provider);
        let Some(api_key) = self.resolve_ai_api_key() else {
            self.set_status(format!(
                "no API key — set it in Settings (Tab) or export ${}",
                provider.env_var
            ));
            self.mode = AppMode::Browsing;
            return;
        };

        let read_dir = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) => {
                self.set_status(format!("cannot read directory: {}", e));
                self.mode = AppMode::Browsing;
                return;
            }
        };

        let mut entries: Vec<(String, bool)> = Vec::new();
        let mut existing_folders: Vec<String> = Vec::new();
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                existing_folders.push(name.clone());
            }
            entries.push((name, is_dir));
        }

        if entries.is_empty() {
            self.set_status("nothing to organize — directory is empty");
            self.mode = AppMode::Browsing;
            return;
        }
        if entries.len() > MAX_ENTRIES {
            self.set_status(format!(
                "directory too large to organize (max {} entries)",
                MAX_ENTRIES
            ));
            self.mode = AppMode::Browsing;
            return;
        }

        let valid_names: HashSet<String> = entries.iter().map(|(n, _)| n.clone()).collect();
        let endpoint = provider.endpoint.to_string();
        let model = self.resolve_ai_model();
        self.organize.work_dir = Some(dir);
        self.organize.plan = None;
        self.organize.scroll_offset = 0;
        self.organize.max_offset = 0;
        self.organize.button_focus = 0;
        self.set_status(format!("generating organize plan via {}...", provider.label));
        self.organize.rx = Some(spawn_worker(move |tx| {
            let result = generate_organize_plan(
                &endpoint,
                &api_key,
                &model,
                &entries,
                &existing_folders,
                &valid_names,
            );
            let _ = tx.send(match result {
                Ok(plan) => OrganizePlanMsg::Ok(plan),
                Err(err) => OrganizePlanMsg::Err(err),
            });
        }));
    }

    /// Poll the AI organize-plan channel. On success, stores the plan for
    /// review; on failure, drops back to browsing with a status message.
    pub(crate) fn pump_ai_organize(&mut self) {
        match pump_once(&mut self.organize.rx) {
            Some(OrganizePlanMsg::Ok(plan)) => {
                if self.mode == AppMode::Organize {
                    self.organize.plan = Some(plan);
                    self.organize.scroll_offset = 0;
                    self.organize.button_focus = 0;
                    self.set_status("organize plan ready — review and Confirm");
                }
            }
            Some(OrganizePlanMsg::Err(err)) => {
                self.set_status(err);
                if self.mode == AppMode::Organize {
                    self.mode = AppMode::Browsing;
                }
                self.organize.work_dir = None;
            }
            None => {}
        }
    }

    /// Cancel the Organize dialog without touching the filesystem.
    pub(crate) fn cancel_organize(&mut self) {
        self.organize.plan = None;
        self.organize.work_dir = None;
        self.organize.rx = None;
        self.mode = AppMode::Browsing;
        self.set_status("organize cancelled");
    }

    /// Apply the reviewed organize plan: create the proposed folders, then
    /// move each entry into its destination. Same-filesystem moves use a
    /// plain rename; the rare cross-device case falls back to a recursive
    /// copy followed by removing the source (mirrors the fallback already
    /// used by the paste queue in `app_transfer.rs`).
    pub(crate) fn apply_organize_plan(&mut self) {
        let Some(dir) = self.organize.work_dir.clone() else {
            self.mode = AppMode::Browsing;
            return;
        };
        let Some(plan) = self.organize.plan.take() else {
            self.mode = AppMode::Browsing;
            return;
        };

        for folder in &plan.folders {
            if !is_safe_segment(folder) {
                continue;
            }
            let _ = fs::create_dir_all(dir.join(folder));
        }

        let mut ok = 0usize;
        let mut failed = 0usize;
        for mv in &plan.moves {
            if !is_safe_segment(&mv.name) || !is_safe_segment(&mv.folder) {
                failed += 1;
                continue;
            }
            let src = dir.join(&mv.name);
            if !src.exists() {
                failed += 1;
                continue;
            }
            let dest_dir = dir.join(&mv.folder);
            if fs::create_dir_all(&dest_dir).is_err() {
                failed += 1;
                continue;
            }
            let dest = dest_dir.join(&mv.name);
            if dest.exists() {
                failed += 1;
                continue;
            }

            let moved = if fs::rename(&src, &dest).is_ok() {
                true
            } else {
                let (tx, _rx) = mpsc::channel();
                let mut copied = 0u64;
                Self::copy_path_with_progress(&src, &dest, &tx, &mut copied)
                    .and_then(|_| {
                        if src.is_dir() {
                            fs::remove_dir_all(&src)
                        } else {
                            fs::remove_file(&src)
                        }
                    })
                    .is_ok()
            };

            if moved {
                ok += 1;
            } else {
                failed += 1;
            }
        }

        self.organize.work_dir = None;
        self.mode = AppMode::Browsing;
        self.refresh_entries_or_status();
        if self.is_dual_panel_mode() {
            let _ = self.refresh_right_panel_entries();
        }
        if failed == 0 {
            self.set_status(format!("organize complete: {} item(s) moved", ok));
        } else {
            self.set_status(format!("organize finished: {} ok, {} failed", ok, failed));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_organize_plan_extracts_valid_plan() {
        let v = serde_json::json!({
            "choices": [{"message": {"content":
                "{\"folders\": [\"Images\"], \"moves\": [{\"name\": \"a.jpg\", \"folder\": \"Images\"}]}"
            }}]
        });
        let plan = parse_organize_plan(&v, &names(&["a.jpg", "b.txt"])).unwrap();
        assert_eq!(plan.folders, vec!["Images".to_string()]);
        assert_eq!(plan.moves.len(), 1);
        assert_eq!(plan.moves[0].name, "a.jpg");
        assert_eq!(plan.moves[0].folder, "Images");
    }

    #[test]
    fn parse_organize_plan_strips_markdown_fence() {
        let v = serde_json::json!({
            "choices": [{"message": {"content":
                "```json\n{\"folders\": [\"Docs\"], \"moves\": [{\"name\": \"a.pdf\", \"folder\": \"Docs\"}]}\n```"
            }}]
        });
        let plan = parse_organize_plan(&v, &names(&["a.pdf"])).unwrap();
        assert_eq!(plan.moves.len(), 1);
    }

    #[test]
    fn parse_organize_plan_errors_on_missing_content() {
        let v = serde_json::json!({"choices": []});
        assert!(parse_organize_plan(&v, &names(&[])).is_err());
    }

    #[test]
    fn parse_organize_plan_drops_hallucinated_names() {
        let v = serde_json::json!({
            "choices": [{"message": {"content":
                "{\"folders\": [\"X\"], \"moves\": [{\"name\": \"does-not-exist.txt\", \"folder\": \"X\"}]}"
            }}]
        });
        assert!(parse_organize_plan(&v, &names(&["real.txt"])).is_err());
    }

    #[test]
    fn parse_organize_plan_drops_unsafe_folder_names() {
        let v = serde_json::json!({
            "choices": [{"message": {"content":
                "{\"folders\": [\"../etc\"], \"moves\": [\
                    {\"name\": \"a.txt\", \"folder\": \"../etc\"}, \
                    {\"name\": \"b.txt\", \"folder\": \"Docs\"}\
                ]}"
            }}]
        });
        let plan = parse_organize_plan(&v, &names(&["a.txt", "b.txt"])).unwrap();
        assert_eq!(plan.moves.len(), 1);
        assert_eq!(plan.moves[0].name, "b.txt");
        assert!(!plan.folders.iter().any(|f| f == "../etc"));
    }

    #[test]
    fn is_safe_segment_rejects_traversal() {
        assert!(is_safe_segment("Docs"));
        assert!(!is_safe_segment(".."));
        assert!(!is_safe_segment("."));
        assert!(!is_safe_segment(""));
        assert!(!is_safe_segment("a/b"));
    }
}
