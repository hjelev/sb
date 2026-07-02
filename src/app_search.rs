use std::{
    collections::HashSet,
    env, fs,
    path::PathBuf,
};

use regex::{Regex, RegexBuilder};

use crate::{
    App, AppMode, InternalSearchCandidatesMsg, InternalSearchContentLimits, InternalSearchContentMsg,
    InternalSearchPattern, InternalSearchResult, InternalSearchScope, PathFilterMode,
    PathInputFilter,
};
use crate::util::background::{pump_once, spawn_worker};

impl App {
    pub(crate) fn build_path_filter_regex(filter: &PathInputFilter) -> Result<Regex, String> {
        let pattern = match filter.mode {
            PathFilterMode::Prefix => format!("^(?:{})", filter.pattern),
            PathFilterMode::Suffix => format!("(?:{})$", filter.pattern),
            PathFilterMode::Contains => filter.pattern.clone(),
        };

        RegexBuilder::new(&pattern)
            .case_insensitive(true)
            .build()
            .map_err(|e| e.to_string())
    }

    pub(crate) fn entry_name_matches_path_filter(name: &str, filter_regex: &Regex) -> bool {
        filter_regex.is_match(name)
    }

    pub(crate) fn collect_internal_search_candidates(
        root: &PathBuf,
        max_items: usize,
    ) -> (Vec<PathBuf>, HashSet<PathBuf>) {
        let mut out: Vec<PathBuf> = Vec::new();
        let mut symlinks: HashSet<PathBuf> = HashSet::new();
        let mut stack: Vec<PathBuf> = vec![root.clone()];

        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };

            for entry in entries.flatten() {
                let name = crate::util::classify::entry_name(&entry);
                if crate::util::classify::is_hidden_name(&name) {
                    continue;
                }

                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file()
                    && let Ok(rel) = path.strip_prefix(root) {
                        // The entry's own type (no follow) is already known from
                        // readdir; remember symlinks so rendering needn't stat.
                        if entry.file_type().map(|t| t.is_symlink()).unwrap_or(false) {
                            symlinks.insert(rel.to_path_buf());
                        }
                        out.push(rel.to_path_buf());
                    }

                if out.len() >= max_items {
                    break;
                }
            }

            if out.len() >= max_items {
                break;
            }
        }

        out.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
        (out, symlinks)
    }

    pub(crate) fn fuzzy_score_and_ranges(candidate: &str, query: &str) -> Option<(i64, Vec<(usize, usize)>)> {
        if query.is_empty() {
            return Some((0, Vec::new()));
        }

        let cand_chars: Vec<char> = candidate.chars().collect();
        let query_chars: Vec<char> = query.chars().collect();
        let mut q_idx = 0usize;
        let mut last_match: Option<usize> = None;
        let mut score = 0i64;
        let mut matched_char_indices: Vec<usize> = Vec::new();

        for (c_idx, ch) in cand_chars.iter().enumerate() {
            if q_idx >= query_chars.len() {
                break;
            }

            if ch.eq_ignore_ascii_case(&query_chars[q_idx]) {
                score += 5;
                if c_idx == 0 || matches!(cand_chars[c_idx - 1], '/' | '_' | '-' | ' ') {
                    score += 8;
                }
                if let Some(prev) = last_match
                    && c_idx == prev + 1 {
                        score += 12;
                    }
                last_match = Some(c_idx);
                matched_char_indices.push(c_idx);
                q_idx += 1;
            }
        }

        if q_idx != query_chars.len() {
            return None;
        }

        let mut byte_offsets: Vec<usize> = candidate.char_indices().map(|(idx, _)| idx).collect();
        byte_offsets.push(candidate.len());

        let mut ranges: Vec<(usize, usize)> = Vec::new();
        for idx in matched_char_indices {
            if idx + 1 < byte_offsets.len() {
                ranges.push((byte_offsets[idx], byte_offsets[idx + 1]));
            }
        }
        let merged = Self::merge_byte_ranges(ranges);

        Some((score - candidate.chars().count() as i64, merged))
    }

    pub(crate) fn merge_byte_ranges(mut ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
        if ranges.is_empty() {
            return ranges;
        }
        ranges.sort_by_key(|(start, _)| *start);
        let mut merged: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
        for (start, end) in ranges {
            if let Some((_, last_end)) = merged.last_mut()
                && start <= *last_end {
                    *last_end = (*last_end).max(end);
                    continue;
                }
            merged.push((start, end));
        }
        merged
    }

    pub(crate) fn parse_regex_query(query: &str) -> Option<(String, bool)> {
        let trimmed = query.trim();
        if let Some(rest) = trimmed.strip_prefix("re:") {
            return Some((rest.trim().to_string(), true));
        }

        if trimmed.starts_with('/') {
            let closing = trimmed.rfind('/').unwrap_or(0);
            if closing > 0 {
                let pattern = &trimmed[1..closing];
                if pattern.is_empty() {
                    return Some((String::new(), true));
                }
                let flags = &trimmed[closing + 1..];
                let case_insensitive = flags.contains('i');
                if flags.chars().all(|c| c == 'i') {
                    return Some((pattern.to_string(), case_insensitive));
                }
            }
        }

        None
    }

    pub(crate) fn literal_match_ranges_ascii(text: &str, needle: &str) -> Vec<(usize, usize)> {
        let query_chars: Vec<char> = needle.chars().collect();
        if query_chars.is_empty() {
            return Vec::new();
        }

        let text_chars: Vec<(usize, char)> = text.char_indices().collect();
        if query_chars.len() > text_chars.len() {
            return Vec::new();
        }

        let mut out: Vec<(usize, usize)> = Vec::new();
        let mut i = 0usize;
        while i + query_chars.len() <= text_chars.len() {
            let mut matched = true;
            for (j, qch) in query_chars.iter().enumerate() {
                let tch = text_chars[i + j].1;
                if !tch.eq_ignore_ascii_case(qch) {
                    matched = false;
                    break;
                }
            }

            if matched {
                let start = text_chars[i].0;
                let end_idx = i + query_chars.len();
                let end = if end_idx < text_chars.len() {
                    text_chars[end_idx].0
                } else {
                    text.len()
                };
                out.push((start, end));
                i += query_chars.len();
            } else {
                i += 1;
            }
        }

        out
    }

    pub(crate) fn refresh_internal_search_filename_results(&mut self, query: &str) {
        if let Some(regex) = self.search.regex.as_ref() {
            let mut matched: Vec<(usize, usize, usize, String, InternalSearchResult)> = Vec::new();
            for rel in &self.search.candidates {
                let rel_str = rel.to_string_lossy().into_owned();
                let ranges = Self::merge_byte_ranges(
                    regex
                        .find_iter(&rel_str)
                        .map(|m| (m.start(), m.end()))
                        .collect(),
                );
                if let Some((first_start, _)) = ranges.first() {
                    matched.push((
                        *first_start,
                        rel_str.chars().count(),
                        rel_str.len(),
                        rel_str,
                        InternalSearchResult::Filename {
                            rel_path: rel.clone(),
                            match_ranges: ranges,
                        },
                    ));
                }
            }

            matched.sort_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| a.1.cmp(&b.1))
                    .then_with(|| a.2.cmp(&b.2))
                    .then_with(|| a.3.cmp(&b.3))
            });

            self.search.results = matched.into_iter().map(|(_, _, _, _, item)| item).collect();
            return;
        }

        let mut scored: Vec<(i64, usize, String, InternalSearchResult)> = Vec::new();
        for rel in &self.search.candidates {
            let rel_str = rel.to_string_lossy().into_owned();
            if let Some((score, ranges)) = Self::fuzzy_score_and_ranges(&rel_str, query) {
                scored.push((
                    score,
                    rel_str.chars().count(),
                    rel_str,
                    InternalSearchResult::Filename {
                        rel_path: rel.clone(),
                        match_ranges: ranges,
                    },
                ));
            }
        }

        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(&b.2))
        });

        self.search.results = scored.into_iter().map(|(_, _, _, item)| item).collect();
    }

    pub(crate) fn internal_search_content_limits() -> InternalSearchContentLimits {
        let parse_env_usize = |key: &str, default: usize| {
            env::var(key)
                .ok()
                .and_then(|v| v.trim().parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(default)
        };

        InternalSearchContentLimits {
            max_files: parse_env_usize("SB_SEARCH_CONTENT_MAX_FILES", 20_000),
            max_hits: parse_env_usize("SB_SEARCH_CONTENT_MAX_HITS", 2_000),
            max_file_bytes: parse_env_usize("SB_SEARCH_CONTENT_MAX_FILE_BYTES", 2 * 1024 * 1024),
        }
    }

    pub(crate) fn adjust_internal_search_content_limit(&mut self, increase: bool, fast: bool) {
        let factor = if fast { 10usize } else { 1usize };

        let (current, step, min_value) = match self.search.limits_selected {
            0 => (
                self.search.content_limits.max_files,
                500usize.saturating_mul(factor),
                100usize,
            ),
            1 => (
                self.search.content_limits.max_hits,
                100usize.saturating_mul(factor),
                50usize,
            ),
            _ => (
                self.search.content_limits.max_file_bytes,
                (256usize * 1024).saturating_mul(factor),
                64usize * 1024,
            ),
        };

        let new_value = if increase {
            current.saturating_add(step)
        } else {
            current.saturating_sub(step).max(min_value)
        };

        match self.search.limits_selected {
            0 => self.search.content_limits.max_files = new_value,
            1 => self.search.content_limits.max_hits = new_value,
            _ => self.search.content_limits.max_file_bytes = new_value,
        }

        if self.search.scope == InternalSearchScope::Content {
            self.refresh_internal_search_results();
        }
    }

    pub(crate) fn reset_internal_search_content_limits_to_defaults(&mut self) {
        self.search.content_limits = Self::internal_search_content_limits();
        if self.search.scope == InternalSearchScope::Content {
            self.refresh_internal_search_results();
        }
    }

    pub(crate) fn build_internal_search_limit_note(
        limits: InternalSearchContentLimits,
        scanned_candidates: usize,
        files_limit_hit: bool,
        large_files_skipped: usize,
        hits_limit_hit: bool,
    ) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if files_limit_hit {
            parts.push(format!("scanned first {} files", limits.max_files));
        }
        if hits_limit_hit {
            parts.push(format!("capped at {} matches", limits.max_hits));
        }
        if large_files_skipped > 0 {
            parts.push(format!(
                "skipped {} file(s) > {}",
                large_files_skipped,
                Self::format_size(limits.max_file_bytes as u64)
            ));
        }
        if parts.is_empty() {
            return None;
        }

        Some(format!(
            "Limits: {} (candidates: {})",
            parts.join("; "),
            scanned_candidates
        ))
    }

    pub(crate) fn run_internal_search_content_query(
        current_dir: PathBuf,
        candidates: Vec<PathBuf>,
        pattern: InternalSearchPattern,
        limits: InternalSearchContentLimits,
    ) -> (Vec<InternalSearchResult>, Option<String>) {
        let regex = match &pattern {
            InternalSearchPattern::Regex {
                pattern,
                case_insensitive,
            } => RegexBuilder::new(pattern)
                .case_insensitive(*case_insensitive)
                .build()
                .ok(),
            InternalSearchPattern::Literal(_) => None,
        };

        let mut out: Vec<InternalSearchResult> = Vec::new();
        let mut large_files_skipped = 0usize;
        let mut files_limit_hit = false;
        let mut hits_limit_hit = false;

        for (idx, rel) in candidates.iter().enumerate() {
            if idx >= limits.max_files {
                files_limit_hit = true;
                break;
            }

            let abs = current_dir.join(rel);
            if !abs.is_file() || Self::is_binary_file(&abs) {
                continue;
            }

            let Ok(meta) = fs::metadata(&abs) else {
                continue;
            };
            if meta.len() as usize > limits.max_file_bytes {
                large_files_skipped += 1;
                continue;
            }

            let Ok(bytes) = fs::read(&abs) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);

            for (line_idx, line) in text.lines().enumerate() {
                let ranges = match (&pattern, regex.as_ref()) {
                    (InternalSearchPattern::Regex { .. }, Some(re)) => Self::merge_byte_ranges(
                        re.find_iter(line).map(|m| (m.start(), m.end())).collect(),
                    ),
                    (InternalSearchPattern::Literal(query), _) => {
                        Self::literal_match_ranges_ascii(line, query)
                    }
                    _ => Vec::new(),
                };

                if ranges.is_empty() {
                    continue;
                }

                out.push(InternalSearchResult::Content {
                    rel_path: rel.clone(),
                    line_number: line_idx + 1,
                    line_text: line.to_string(),
                    match_ranges: ranges,
                });

                if out.len() >= limits.max_hits {
                    hits_limit_hit = true;
                    break;
                }
            }

            if hits_limit_hit {
                break;
            }
        }

        let note = Self::build_internal_search_limit_note(
            limits,
            candidates.len(),
            files_limit_hit,
            large_files_skipped,
            hits_limit_hit,
        );

        (out, note)
    }

    pub(crate) fn cancel_internal_search_content_request(&mut self) {
        self.search.content_request_id = self.search.content_request_id.wrapping_add(1);
        self.search.content_rx = None;
        self.search.content_pending = false;
    }

    pub(crate) fn refresh_internal_search_content_results_async(
        &mut self,
        query: &str,
        regex_pattern: Option<(String, bool)>,
    ) {
        if query.is_empty() {
            self.cancel_internal_search_content_request();
            self.search.results.clear();
            self.search.content_limit_note = None;
            return;
        }

        let limits = self.search.content_limits;
        let request_id = self.search.content_request_id.wrapping_add(1);
        self.search.content_request_id = request_id;
        self.search.content_pending = true;
        self.search.content_limit_note = Some(format!(
            "Limits: files <= {}, hits <= {}, file <= {}",
            limits.max_files,
            limits.max_hits,
            Self::format_size(limits.max_file_bytes as u64),
        ));

        let current_dir = self.left.dir.clone();
        let candidates = self.search.candidates.clone();
        let pattern = if let Some((pattern, case_insensitive)) = regex_pattern {
            InternalSearchPattern::Regex {
                pattern,
                case_insensitive,
            }
        } else {
            InternalSearchPattern::Literal(query.to_string())
        };

        self.search.content_rx = Some(spawn_worker(move |tx| {
            let (results, limit_note) =
                App::run_internal_search_content_query(current_dir, candidates, pattern, limits);
            let _ = tx.send(InternalSearchContentMsg::Finished {
                request_id,
                results,
                limit_note,
            });
        }));
    }

    pub(crate) fn pump_internal_search_content_progress(&mut self) {
        if self.search.content_rx.is_none() {
            return;
        }
        match pump_once(&mut self.search.content_rx) {
            Some(InternalSearchContentMsg::Finished {
                request_id,
                results,
                limit_note,
            }) => {
                if request_id == self.search.content_request_id {
                    self.search.results = results;
                    self.search.content_limit_note = limit_note;
                    self.search.content_pending = false;
                    if self.search.results.is_empty() {
                        self.search.selected = 0;
                    } else {
                        self.search.selected = self
                            .search.selected
                            .min(self.search.results.len() - 1);
                    }
                }
            }
            None => {
                // Worker disconnected without sending (receiver dropped).
                if self.search.content_rx.is_none() {
                    self.search.content_pending = false;
                }
            }
        }
    }

    pub(crate) fn refresh_internal_search_results(&mut self) {
        let query = self.input_buffer.trim().to_string();
        self.search.regex_mode = false;
        self.search.regex = None;
        self.search.regex_error = None;

        let mut compiled_regex: Option<Regex> = None;

        let parsed_regex = Self::parse_regex_query(&query);

        if let Some((pattern, case_insensitive)) = parsed_regex.as_ref() {
            self.search.regex_mode = true;

            let regex = RegexBuilder::new(pattern)
                .case_insensitive(*case_insensitive)
                .build();

            let Ok(regex) = regex else {
                self.cancel_internal_search_content_request();
                self.search.results.clear();
                self.search.selected = 0;
                self.search.content_limit_note = None;
                self.search.regex_error = Some("invalid regex".to_string());
                return;
            };
            compiled_regex = Some(regex);
        }

        self.search.regex = compiled_regex;

        match self.search.scope {
            InternalSearchScope::Filename => {
                self.cancel_internal_search_content_request();
                self.search.content_limit_note = None;
                self.refresh_internal_search_filename_results(&query);
            }
            InternalSearchScope::Content => {
                self.refresh_internal_search_content_results_async(&query, parsed_regex);
            }
        }

        if self.search.results.is_empty() {
            self.search.selected = 0;
        } else {
            self.search.selected = self
                .search.selected
                .min(self.search.results.len() - 1);
        }
    }

    pub(crate) fn cancel_internal_search_candidate_scan(&mut self) {
        self.search.candidates_scan_id = self.search.candidates_scan_id.wrapping_add(1);
        self.search.candidates_rx = None;
        self.search.candidates_pending = false;
    }

    pub(crate) fn start_internal_search_candidate_scan(&mut self) {
        const INTERNAL_SEARCH_MAX_ITEMS: usize = 20_000;

        self.cancel_internal_search_candidate_scan();
        self.search.candidates_truncated = false;
        self.search.candidates.clear();
        self.search.candidate_symlinks.clear();
        self.search.results.clear();
        self.search.selected = 0;

        self.search.candidates_scan_id = self.search.candidates_scan_id.wrapping_add(1);
        let scan_id = self.search.candidates_scan_id;
        self.search.candidates_pending = true;

        let root = self.left.dir.clone();
        self.search.candidates_rx = Some(spawn_worker(move |tx| {
            let (candidates, symlinks) =
                App::collect_internal_search_candidates(&root, INTERNAL_SEARCH_MAX_ITEMS);
            let truncated = candidates.len() >= INTERNAL_SEARCH_MAX_ITEMS;
            let _ = tx.send(InternalSearchCandidatesMsg::Finished {
                scan_id,
                candidates,
                symlinks,
                truncated,
            });
        }));
    }

    pub(crate) fn pump_internal_search_candidates_progress(&mut self) {
        if self.search.candidates_rx.is_none() {
            return;
        }
        match pump_once(&mut self.search.candidates_rx) {
            Some(InternalSearchCandidatesMsg::Finished {
                scan_id,
                candidates,
                symlinks,
                truncated,
            }) => {
                if scan_id == self.search.candidates_scan_id {
                    self.search.candidates = candidates;
                    self.search.candidate_symlinks = symlinks;
                    self.search.candidates_truncated = truncated;
                    self.search.candidates_pending = false;
                    self.refresh_internal_search_results();

                    if self.search.candidates.is_empty() {
                        self.set_status("search: no files found");
                    } else if self.search.candidates_truncated {
                        self.set_status("search indexed first 20000 files");
                    } else if self.status_message == "search: indexing files asynchronously..." {
                        self.status_message.clear();
                    }
                }
            }
            None => {
                // Worker disconnected without sending (receiver dropped).
                if self.search.candidates_rx.is_none() {
                    self.search.candidates_pending = false;
                }
            }
        }
    }

    pub(crate) fn start_internal_search(&mut self) {
        self.start_internal_search_with_scope(InternalSearchScope::Filename);
    }

    pub(crate) fn start_internal_search_with_scope(&mut self, scope: InternalSearchScope) {
        self.search.selected = 0;
        self.search.scope = scope;
        self.search.content_limit_note = None;
        self.search.limits_menu_open = false;
        self.search.limits_selected = 0;
        self.panel_tab = 1;
        self.begin_input_edit(AppMode::InternalSearch, String::new());
        self.start_internal_search_candidate_scan();
        self.refresh_internal_search_results();
        self.set_status("search: indexing files asynchronously...");
    }

    pub(crate) fn toggle_internal_search_scope(&mut self) {
        self.search.scope = match self.search.scope {
            InternalSearchScope::Filename => InternalSearchScope::Content,
            InternalSearchScope::Content => InternalSearchScope::Filename,
        };
        if self.search.scope == InternalSearchScope::Filename {
            self.search.limits_menu_open = false;
        }
        self.search.selected = 0;
        self.refresh_internal_search_results();
    }

    pub(crate) fn selected_internal_search_path(&self) -> Option<PathBuf> {
        let result = self.search.results.get(self.search.selected)?;
        let rel = match result {
            InternalSearchResult::Filename { rel_path, .. } => rel_path,
            InternalSearchResult::Content { rel_path, .. } => rel_path,
        };
        Some(self.left.dir.join(rel))
    }
}
