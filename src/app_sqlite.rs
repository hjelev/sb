use std::{io, path::PathBuf, process::Command};

use crate::{App, AppMode};

impl App {
    pub(crate) fn sqlite_quote_ident(name: &str) -> String {
        format!("\"{}\"", name.replace('"', "\"\""))
    }

    pub(crate) fn sqlite_query_rows(
        path: &PathBuf,
        sql: &str,
        with_header: bool,
    ) -> io::Result<Vec<Vec<String>>> {
        let mut cmd = Command::new("sqlite3");
        cmd.args([
            "-readonly",
            "-batch",
            "-separator",
            "\x1f",
            "-nullvalue",
            "NULL",
        ]);
        if with_header {
            cmd.arg("-header");
        } else {
            cmd.arg("-noheader");
        }
        cmd.arg(path);
        cmd.arg(sql);
        let out = cmd.output()?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let msg = if stderr.is_empty() {
                "sqlite3 query failed".to_string()
            } else {
                format!("sqlite3 query failed: {}", stderr)
            };
            return Err(io::Error::other(msg));
        }

        let stdout = String::from_utf8_lossy(&out.stdout);
        let rows = stdout
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.split('\x1f').map(|s| s.to_string()).collect::<Vec<String>>())
            .collect::<Vec<Vec<String>>>();
        Ok(rows)
    }

    pub(crate) fn sqlite_query_box_lines(path: &PathBuf, sql: &str) -> io::Result<Vec<String>> {
        let out = Command::new("sqlite3")
            .args(["-readonly", "-batch", "-header", "-box"])
            .arg(path)
            .arg(sql)
            .output()?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let msg = if stderr.is_empty() {
                "sqlite3 query failed".to_string()
            } else {
                format!("sqlite3 query failed: {}", stderr)
            };
            return Err(io::Error::other(msg));
        }

        let stdout = String::from_utf8_lossy(&out.stdout);
        Ok(stdout.lines().map(|line| line.to_string()).collect())
    }

    pub(crate) fn sqlite_list_tables(path: &PathBuf) -> io::Result<Vec<String>> {
        let rows = Self::sqlite_query_rows(
            path,
            "SELECT name FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' ORDER BY name;",
            false,
        )?;
        let mut tables = rows
            .into_iter()
            .filter_map(|row| row.first().cloned())
            .filter(|name| !name.trim().is_empty())
            .collect::<Vec<String>>();
        tables.sort();
        tables.dedup();
        Ok(tables)
    }

    pub(crate) fn refresh_sqlite_preview_rows(&mut self) {
        self.db_preview.output_lines.clear();
        self.db_preview.error = None;

        let Some(path) = self.db_preview.path.clone() else {
            return;
        };
        let Some(table_name) = self.db_preview.tables.get(self.db_preview.selected).cloned() else {
            return;
        };

        let quoted_table = Self::sqlite_quote_ident(&table_name);
        let sql = format!("SELECT * FROM {} LIMIT {};", quoted_table, self.db_preview.row_limit);
        match Self::sqlite_query_box_lines(&path, &sql) {
            Ok(lines) => {
                self.db_preview.output_lines = lines;
            }
            Err(err) => {
                self.db_preview.error = Some(err.to_string());
            }
        }
    }

    pub(crate) fn begin_sqlite_preview(&mut self, db_path: PathBuf) {
        self.db_preview.path = Some(db_path.clone());
        self.db_preview.tables.clear();
        self.db_preview.selected = 0;
        self.db_preview.output_lines.clear();
        self.db_preview.error = None;

        match Self::sqlite_list_tables(&db_path) {
            Ok(tables) => {
                self.db_preview.tables = tables;
                if self.db_preview.tables.is_empty() {
                    self.db_preview.error = Some("No tables/views found in this database".to_string());
                } else {
                    self.refresh_sqlite_preview_rows();
                }
            }
            Err(err) => {
                self.db_preview.error = Some(err.to_string());
            }
        }

        self.mode = AppMode::DbPreview;
    }

    pub(crate) fn switch_sqlite_preview_table(&mut self, delta: isize) {
        if self.db_preview.tables.is_empty() {
            return;
        }
        let last = self.db_preview.tables.len().saturating_sub(1) as isize;
        let next = (self.db_preview.selected as isize + delta).clamp(0, last) as usize;
        if next != self.db_preview.selected {
            self.db_preview.selected = next;
            self.refresh_sqlite_preview_rows();
        }
    }
}
