//! Download flow: URL-input parsing, file-name derivation, job spawning,
//! and progress pumping. Extracted from main.rs (impl App).

use crate::util;
use crate::util::background::drain_channel;
use crate::{App, AppMode, DownloadProgressMsg};

impl App {
    pub(crate) fn begin_download_input(&mut self) {
        if self.transfer.download_rx.is_some() {
            self.set_status("download already in progress");
            return;
        }

        self.transfer.download_pending_url = None;
        self.transfer.download_pending_name = None;
        self.transfer.download_resume_input = None;
        self.begin_input_edit(AppMode::DownloadInput, String::new());
    }

    fn parse_download_input(raw: &str) -> Result<(String, Option<String>), String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("enter a URL to download".to_string());
        }

        let (url, file_name) = if let Some(rest) = trimmed.strip_prefix('"') {
            let Some(end_quote) = rest.find('"') else {
                return Err("quoted URL is missing a closing quote".to_string());
            };
            let url = rest[..end_quote].trim().to_string();
            let remainder = rest[end_quote + 1..].trim();
            let file_name = if remainder.is_empty() {
                None
            } else {
                Some(remainder.to_string())
            };
            (url, file_name)
        } else if let Some(split_at) = trimmed.find(char::is_whitespace) {
            let url = trimmed[..split_at].trim().to_string();
            let remainder = trimmed[split_at..].trim();
            let file_name = if remainder.is_empty() {
                None
            } else {
                Some(remainder.to_string())
            };
            (url, file_name)
        } else {
            (trimmed.to_string(), None)
        };

        if url.is_empty() {
            return Err("enter a URL to download".to_string());
        }
        if !url.contains("://") {
            return Err("URL must include a scheme like https://".to_string());
        }

        Ok((url, file_name))
    }

    fn download_url_host(url: &str) -> Option<String> {
        let authority_and_path = url.split_once("://")?.1;
        let authority = authority_and_path
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default();
        let host_port = authority.rsplit('@').next().unwrap_or(authority);
        let host = if let Some(rest) = host_port.strip_prefix('[') {
            rest.split(']').next().unwrap_or_default().trim().to_string()
        } else {
            host_port.split(':').next().unwrap_or_default().trim().to_string()
        };

        if host.is_empty() {
            None
        } else {
            Some(host)
        }
    }

    fn download_url_file_name(url: &str) -> Option<String> {
        let authority_and_path = url.split_once("://")?.1;
        let path_and_more = authority_and_path.split_once('/').map(|(_, tail)| tail)?;
        let path = path_and_more
            .split(['?', '#'])
            .next()
            .unwrap_or_default();
        let name = path.rsplit('/').find(|segment| !segment.is_empty())?;
        if name == "." || name == ".." {
            None
        } else {
            Some(name.to_string())
        }
    }

    fn validate_download_file_name(name: &str) -> Result<String, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("download name cannot be empty".to_string());
        }
        if trimmed == "." || trimmed == ".." {
            return Err("download name cannot be . or ..".to_string());
        }
        if trimmed.contains('/') || trimmed.contains('\\') {
            return Err("download name cannot contain path separators".to_string());
        }
        Ok(trimmed.to_string())
    }

    fn queue_download_request(&mut self, url: String, file_name: String, resume_input: String) {
        self.transfer.download_pending_url = Some(url.clone());
        self.transfer.download_pending_name = Some(file_name.clone());
        self.transfer.download_resume_input = Some(resume_input);

        if self.left.dir.join(&file_name).exists() {
            self.clear_input_edit();
            self.mode = AppMode::ConfirmDownloadOverwrite;
            self.set_status(format!("target exists: overwrite {}?", file_name));
            return;
        }

        self.start_download_job(url, file_name);
    }

    pub(crate) fn submit_download_input(&mut self) {
        let resume_input = self.input_buffer.trim().to_string();
        let (url, explicit_name) = match Self::parse_download_input(&resume_input) {
            Ok(parsed) => parsed,
            Err(message) => {
                self.set_status(message);
                return;
            }
        };

        if let Some(name) = explicit_name {
            match Self::validate_download_file_name(&name) {
                Ok(file_name) => self.queue_download_request(url, file_name, resume_input),
                Err(message) => self.set_status(message),
            }
            return;
        }

        if let Some(name) = Self::download_url_file_name(&url) {
            match Self::validate_download_file_name(&name) {
                Ok(file_name) => self.queue_download_request(url, file_name, resume_input),
                Err(message) => self.set_status(message),
            }
            return;
        }

        let Some(host_name) = Self::download_url_host(&url) else {
            self.set_status("could not derive a file name from URL");
            return;
        };

        self.transfer.download_pending_url = Some(url);
        self.transfer.download_pending_name = None;
        self.transfer.download_resume_input = Some(resume_input);
        self.begin_input_edit(AppMode::DownloadNaming, host_name);
        self.set_status("edit download name and press Enter");
    }

    pub(crate) fn submit_download_name(&mut self) {
        let Some(url) = self.transfer.download_pending_url.clone() else {
            self.mode = AppMode::Browsing;
            self.clear_input_edit();
            self.set_status("download target is missing");
            return;
        };

        match Self::validate_download_file_name(&self.input_buffer) {
            Ok(file_name) => {
                let resume_input = format!("\"{}\" {}", url, file_name);
                self.queue_download_request(url, file_name, resume_input);
            }
            Err(message) => self.set_status(message),
        }
    }

    pub(crate) fn cancel_download_overwrite(&mut self) {
        let resume_input = self.transfer.download_resume_input.clone().unwrap_or_default();
        self.begin_input_edit(AppMode::DownloadInput, resume_input);
        self.transfer.download_pending_name = None;
        self.set_status("download overwrite cancelled");
    }

    fn preferred_download_tool(&self) -> Option<&'static str> {
        if self.integration_active("wget") {
            Some("wget")
        } else if self.integration_active("curl") {
            Some("curl")
        } else {
            None
        }
    }

    pub(crate) fn start_download_job(&mut self, url: String, file_name: String) {
        if self.transfer.download_rx.is_some() {
            self.set_status("download already in progress");
            return;
        }

        let Some(tool) = self.preferred_download_tool() else {
            self.status_tool_not_found("wget/curl");
            return;
        };

        let output_path = self.left.dir.join(&file_name);
        self.transfer.download_active_name = file_name.clone();
        self.transfer.download_pending_url = None;
        self.transfer.download_pending_name = None;
        self.transfer.download_resume_input = None;
        self.clear_input_edit();
        self.mode = AppMode::Browsing;
        self.set_status(format!("downloading {} via {}", file_name, tool));

        self.transfer.download_rx = Some(util::background::spawn_worker(move |tx| {
            let result = util::command::CommandBuilder::download_with_progress(tool, &url, &output_path, |hint| {
                let _ = tx.send(DownloadProgressMsg::Status(hint.to_string()));
            });

            let _ = tx.send(DownloadProgressMsg::Finished { file_name, result });
        }));
    }

    /// Drains all pending download messages for this frame.
    ///
    /// `Disconnected` only means the sender is gone **and** the queue is empty; any `Finished`
    /// message was already delivered as `Ok(Finished)` in this same drain loop (never skip the
    /// `finished` block by returning early on `Disconnected`).
    pub(crate) fn pump_download_progress(&mut self) {
        if self.transfer.download_rx.is_none() {
            return;
        }

        let mut finished: Option<(String, Result<(), String>)> = None;
        let mut latest_status: Option<String> = None;
        for msg in drain_channel(&mut self.transfer.download_rx) {
            match msg {
                DownloadProgressMsg::Status(s) => latest_status = Some(s),
                DownloadProgressMsg::Finished { file_name, result } => {
                    finished = Some((file_name, result));
                }
            }
        }
        // Queue empty and all senders dropped. If the worker exited normally, we already
        // received `Finished` above; otherwise `finished` stays empty.
        let channel_closed = self.transfer.download_rx.is_none();

        if let Some((file_name, result)) = finished {
            self.transfer.download_rx = None;
            self.transfer.download_active_name.clear();
            self.refresh_entries_or_status();
            self.sync_inactive_panel_if_same_dir();
            match result {
                Ok(()) => {
                    self.select_entry_named(&file_name);
                    self.set_status(format!("downloaded {}", file_name));
                }
                Err(error) => {
                    self.set_status(format!("download failed for {}: {}", file_name, error));
                }
            }
            return;
        }

        if channel_closed {
            self.transfer.download_active_name.clear();
            self.transfer.download_rx = None;
            self.set_status("download worker disconnected");
            return;
        }

        if let Some(s) = latest_status {
            let name = self.transfer.download_active_name.clone();
            if name.is_empty() {
                self.set_status(format!("downloading: {}", s));
            } else {
                self.set_status(format!("downloading {} — {}", name, s));
            }
        }
    }
}
