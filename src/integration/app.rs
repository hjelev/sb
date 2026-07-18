use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    process::Command,
};

use crate::util::tui::{self, ResumeMode};
use crate::{integration, App, AppMode, ArchiveKind};

use super::{catalog, probe, rows::IntegrationRow};

impl App {
    pub(crate) fn integration_catalog() -> Vec<catalog::IntegrationSpec> {
        catalog::integration_catalog()
    }

    pub(crate) fn integration_brew_package(key: &str) -> Option<&'static str> {
        catalog::integration_brew_package(key)
    }

    pub(crate) fn brew_command_path() -> Option<String> {
        let (found, path) = Self::integration_probe("brew");
        if found {
            return Some(path);
        }

        let mut candidates: Vec<PathBuf> = Vec::new();
        #[cfg(target_os = "macos")]
        {
            candidates.push(PathBuf::from("/opt/homebrew/bin/brew"));
            candidates.push(PathBuf::from("/usr/local/bin/brew"));
        }
        #[cfg(target_os = "linux")]
        {
            candidates.push(PathBuf::from("/home/linuxbrew/.linuxbrew/bin/brew"));
            if let Ok(home) = env::var("HOME") {
                candidates.push(PathBuf::from(home).join(".linuxbrew/bin/brew"));
            }
        }

        for candidate in candidates {
            if candidate.exists() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }

        None
    }

    pub(crate) fn clear_integration_install_prompt(&mut self) {
        self.integration.install_key = None;
        self.integration.install_package = None;
        self.integration.install_brew_path = None;
    }

    pub(crate) fn begin_integration_install_prompt_for_selected(&mut self) {
        if self.integration.rows_cache.is_empty() {
            self.set_status("no integration selected");
            return;
        }

        let Some(row) = self.integration.rows_cache.get(self.integration.selected).cloned() else {
            self.set_status("invalid integration selection");
            return;
        };

        if row.key == "__all_optional__" {
            self.set_status("select a specific integration to install");
            return;
        }

        if row.required {
            self.set_status("required integration cannot be installed from here");
            return;
        }

        if row.available {
            self.set_status(format!("{} is already available", row.label));
            return;
        }

        let Some(package) = Self::integration_brew_package(&row.key) else {
            self.set_status(format!("no brew package mapping for {}", row.label));
            return;
        };

        self.integration.install_key = Some(row.key);
        self.integration.install_package = Some(package.to_string());
        self.integration.install_brew_path = Self::brew_command_path();
        self.confirm_integration_install_button_focus = 0;
        self.mode = AppMode::ConfirmIntegrationInstall;
        self.set_status("confirm integration install");
    }

    pub(crate) fn show_brew_setup_guidance(&mut self) -> io::Result<()> {
        let _tui = tui::suspend(ResumeMode::Cleared)?;

        println!("Homebrew was not found on this system.");
        println!();
        println!("Install Homebrew first, then retry from Integrations:");
        println!(
            "  /bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
        );
        println!();
        println!("After install, verify with: brew --version");
        println!();
        println!("Press Enter to return to shell buddy...");
        let _ = io::stdout().flush();
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);

        drop(_tui);
        Ok(())
    }

    pub(crate) fn confirm_integration_install(&mut self) -> io::Result<()> {
        let Some(key) = self.integration.install_key.clone() else {
            self.mode = AppMode::Integrations;
            self.set_status("no pending integration install");
            return Ok(());
        };
        let Some(package) = self.integration.install_package.clone() else {
            self.mode = AppMode::Integrations;
            self.set_status("no pending integration package");
            return Ok(());
        };

        let brew_path = self
            .integration.install_brew_path
            .clone()
            .or_else(Self::brew_command_path);

        if brew_path.is_none() {
            self.show_brew_setup_guidance()?;
            self.mode = AppMode::Integrations;
            self.clear_integration_install_prompt();
            self.refresh_integration_rows_cache();
            self.set_status("brew not found; setup instructions shown");
            return Ok(());
        }

        let brew = brew_path.unwrap_or_default();

        let _tui = tui::suspend(ResumeMode::Cleared)?;

        println!("Installing integration '{}' with Homebrew", key);

        let mut install_steps: Vec<Vec<String>> = Vec::new();
        #[cfg(target_os = "macos")]
        {
            if key == "archivemount" || key == "fuse-zip" {
                install_steps.push(vec![
                    "install".to_string(),
                    "--cask".to_string(),
                    "macfuse".to_string(),
                ]);
            }
        }
        if key == "mmdflux" {
            install_steps.push(vec![
                "tap".to_string(),
                "kevinswiber/mmdflux".to_string(),
            ]);
        }
        install_steps.push(vec!["install".to_string(), package.clone()]);

        let mut failed_step: Option<String> = None;
        for step in install_steps {
            let pretty = step.join(" ");
            println!("$ {} {}", brew, pretty);
            let status = Command::new(&brew)
                .args(step.iter().map(|s| s.as_str()))
                .status();

            match &status {
                Ok(s) => {
                    if let Some(code) = s.code() {
                        println!("\n[exit code: {}]", code);
                    } else {
                        println!("\n[process terminated by signal]");
                    }
                    if !s.success() {
                        failed_step = Some(pretty);
                        break;
                    }
                }
                Err(e) => {
                    println!("\n[failed to execute brew: {}]", e);
                    failed_step = Some(pretty);
                    break;
                }
            }
        }

        println!("\nPress Enter to return to shell buddy...");
        let _ = io::stdout().flush();
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);

        drop(_tui);

        match failed_step {
            None => {
                self.set_integration_enabled(&key, true);
                self.set_status(format!("installed {} with brew", package));
            }
            Some(step) => {
                self.set_status(format!("brew install failed: {}", step));
            }
        }

        self.refresh_integration_rows_cache();
        self.mode = AppMode::Integrations;
        self.clear_integration_install_prompt();
        Ok(())
    }

    pub(crate) fn integration_probe(cmd: &str) -> (bool, String) {
        probe::integration_probe(cmd)
    }

    pub(crate) fn integration_availability_and_detail(key: &str) -> (bool, String) {
        probe::integration_availability_and_detail(key)
    }

    pub(crate) fn integration_support_and_detail(key: &str) -> (bool, bool, String) {
        probe::integration_support_and_detail(key)
    }

    pub(crate) fn terminal_image_protocol() -> (probe::TerminalImageProtocol, String) {
        probe::terminal_image_protocol()
    }

    pub(crate) fn integration_enabled(&self, key: &str) -> bool {
        if Self::integration_catalog()
            .iter()
            .any(|s| s.key == key && s.required)
        {
            true
        } else {
            self.integration.overrides.get(key).copied().unwrap_or(true)
        }
    }

    pub(crate) fn integration_active(&self, key: &str) -> bool {
        let (available, _) = Self::integration_availability_and_detail(key);
        self.integration_enabled(key) && available
    }

    pub(crate) fn set_integration_enabled(&mut self, key: &str, enabled: bool) {
        if Self::integration_catalog()
            .iter()
            .any(|s| s.key == key && s.required)
        {
            return;
        }
        self.integration.overrides.insert(key.to_string(), enabled);
    }

    pub(crate) fn set_all_optional_integrations(&mut self, enabled: bool) {
        for spec in Self::integration_catalog().iter().filter(|s| !s.required) {
            let (available, partially_supported, _) =
                Self::integration_support_and_detail(spec.key);
            if !available && !partially_supported {
                continue;
            }
            self.integration.overrides
                .insert(spec.key.to_string(), enabled);
        }
    }

    pub(crate) fn all_optional_integrations_enabled(&self) -> bool {
        Self::integration_catalog()
            .iter()
            .filter(|s| !s.required)
            .all(|s| self.integration_enabled(s.key))
    }

    pub(crate) fn integration_rows(&self) -> Vec<IntegrationRow> {
        integration::rows::build_integration_rows(
            self.all_optional_integrations_enabled(),
            Self::integration_catalog(),
            |key| self.integration_enabled(key),
            |key| {
                let (available, partially_supported, _) =
                    Self::integration_support_and_detail(key);
                (available, partially_supported)
            },
        )
    }

    pub(crate) fn refresh_integration_rows_cache(&mut self) {
        let mut rows = self.integration_rows();
        let query = self.integration.search_query.trim().to_lowercase();
        if !query.is_empty() {
            rows.retain(|row| {
                row.key != "__all_optional__"
                    && (row.label.to_lowercase().contains(&query)
                        || row.category.to_lowercase().contains(&query)
                        || row.description.to_lowercase().contains(&query))
            });
        }
        self.integration.rows_cache = rows;
        let len = self.integration.rows_cache.len();
        if len == 0 {
            self.integration.selected = 0;
        } else if self.integration.selected >= len {
            self.integration.selected = len - 1;
        }
    }

    pub(crate) fn reset_integration_search(&mut self) {
        self.integration.search_active = false;
        self.integration.search_query.clear();
    }

    pub(crate) fn seven_zip_tool() -> Option<String> {
        probe::seven_zip_tool()
    }

    pub(crate) fn rar_tool() -> Option<String> {
        probe::rar_tool()
    }

    pub(crate) fn bat_tool() -> Option<String> {
        probe::bat_tool()
    }

    pub(crate) fn can_extract_archive(&self, path: &PathBuf) -> bool {
        match Self::archive_kind(path) {
            Some(ArchiveKind::Zip) => {
                self.integration_enabled("zip") && Self::integration_probe("unzip").0
            }
            Some(ArchiveKind::Tar) => self.integration_active("tar"),
            Some(ArchiveKind::SevenZip) => {
                self.integration_enabled("7z") && Self::seven_zip_tool().is_some()
            }
            Some(ArchiveKind::Rar) => self.integration_enabled("rar") && Self::rar_tool().is_some(),
            None => false,
        }
    }
}
