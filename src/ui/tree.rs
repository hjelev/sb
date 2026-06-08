use std::{collections::HashMap, fs, io, path::PathBuf};

use crate::{App, SortMode};

pub(crate) struct TreeRow {
    pub(crate) entry: fs::DirEntry,
    pub(crate) prefix: String,
}

/// Recursion-invariant settings for [`walk_tree_rows`] (everything that stays
/// the same across the depth-first walk).
#[derive(Clone, Copy)]
struct TreeWalkConfig<'a> {
    include_hidden: bool,
    max_depth: Option<usize>,
    sort_mode: SortMode,
    folder_size_cache: Option<&'a HashMap<PathBuf, u64>>,
}

/// Recursion-invariant settings for [`walk_tree_rows_with_expansions`].
#[derive(Clone, Copy)]
struct TreeExpandConfig<'a> {
    include_hidden: bool,
    sort_mode: SortMode,
    folder_size_cache: Option<&'a HashMap<PathBuf, u64>>,
    expansion_levels: &'a HashMap<PathBuf, usize>,
}

pub(crate) fn collect_tree_rows(
    root: &PathBuf,
    include_hidden: bool,
    max_depth: Option<usize>,
    sort_mode: SortMode,
    folder_size_cache: Option<&HashMap<PathBuf, u64>>,
) -> io::Result<Vec<TreeRow>> {
    let mut rows = Vec::new();
    let mut ancestor_last = Vec::new();
    walk_tree_rows(
        root,
        &TreeWalkConfig {
            include_hidden,
            max_depth,
            sort_mode,
            folder_size_cache,
        },
        1,
        &mut ancestor_last,
        &mut rows,
    )?;
    Ok(rows)
}

pub(crate) fn collect_tree_rows_with_expansions(
    root: &PathBuf,
    include_hidden: bool,
    sort_mode: SortMode,
    folder_size_cache: Option<&HashMap<PathBuf, u64>>,
    expansion_levels: &HashMap<PathBuf, usize>,
) -> io::Result<Vec<TreeRow>> {
    let mut rows = Vec::new();
    let mut ancestor_last = Vec::new();
    walk_tree_rows_with_expansions(
        root,
        &TreeExpandConfig {
            include_hidden,
            sort_mode,
            folder_size_cache,
            expansion_levels,
        },
        0,
        &mut ancestor_last,
        &mut rows,
    )?;
    Ok(rows)
}

fn walk_tree_rows(
    dir: &PathBuf,
    config: &TreeWalkConfig,
    depth: usize,
    ancestor_last: &mut Vec<bool>,
    out: &mut Vec<TreeRow>,
) -> io::Result<()> {
    let &TreeWalkConfig {
        include_hidden,
        max_depth,
        sort_mode,
        folder_size_cache,
    } = config;
    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|res| res.ok())
        .filter(|entry| include_hidden || !crate::util::classify::is_hidden_entry(entry))
        .collect();

    App::sort_entries_by_mode(&mut entries, sort_mode, folder_size_cache);

    let total = entries.len();
    for (idx, entry) in entries.into_iter().enumerate() {
        let is_last = idx + 1 == total;
        let path = entry.path();
        let prefix = tree_prefix_compact(ancestor_last, is_last);
        let is_dir = path.is_dir();
        out.push(TreeRow {
            entry,
            prefix,
        });

        let should_descend = is_dir && max_depth.map(|limit| depth < limit).unwrap_or(true);
        if should_descend {
            ancestor_last.push(is_last);
            // best-effort: an unreadable subdirectory (e.g. permission denied)
            // is skipped so the rest of the tree still renders.
            let _ = walk_tree_rows(&path, config, depth + 1, ancestor_last, out);
            ancestor_last.pop();
        }
    }

    Ok(())
}

fn walk_tree_rows_with_expansions(
    dir: &PathBuf,
    config: &TreeExpandConfig,
    inherited_expand: usize,
    ancestor_last: &mut Vec<bool>,
    out: &mut Vec<TreeRow>,
) -> io::Result<()> {
    let &TreeExpandConfig {
        include_hidden,
        sort_mode,
        folder_size_cache,
        expansion_levels,
    } = config;
    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|res| res.ok())
        .filter(|entry| include_hidden || !crate::util::classify::is_hidden_entry(entry))
        .collect();

    App::sort_entries_by_mode(&mut entries, sort_mode, folder_size_cache);

    let total = entries.len();
    for (idx, entry) in entries.into_iter().enumerate() {
        let is_last = idx + 1 == total;
        let path = entry.path();
        let prefix = tree_prefix_compact(ancestor_last, is_last);
        let is_dir = path.is_dir();
        out.push(TreeRow { entry, prefix });

        if is_dir {
            let own_expand = expansion_levels.get(&path).copied().unwrap_or(0);
            let effective_expand = own_expand.max(inherited_expand);
            if effective_expand > 0 {
                ancestor_last.push(is_last);
                // best-effort: an unreadable subdirectory is skipped so the
                // rest of the tree still renders.
                let _ = walk_tree_rows_with_expansions(
                    &path,
                    config,
                    effective_expand.saturating_sub(1),
                    ancestor_last,
                    out,
                );
                ancestor_last.pop();
            }
        }
    }

    Ok(())
}

fn tree_prefix_compact(ancestor_last: &[bool], is_last: bool) -> String {
    // Keep root-level rows flat (no tree glyphs/spacer), and draw connectors only
    // within expanded subtrees.
    if ancestor_last.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for ancestor_is_last in ancestor_last.iter().skip(1) {
        if *ancestor_is_last {
            out.push_str("  ");
        } else {
            out.push_str("│ ");
        }
    }
    out.push_str(if is_last { "╰─" } else { "├─" });
    out
}
