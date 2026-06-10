use crate::EntryRenderCache;
use ratatui::{prelude::*, widgets::Cell};

pub(crate) fn panel_size_width(cache: &[EntryRenderCache], show_size: bool) -> usize {
    if show_size {
        cache.iter()
            .map(|entry| entry.size_col.trim().chars().count())
            .max()
            .unwrap_or(1)
            .max(1)
    } else {
        1
    }
}

pub(crate) fn panel_percent_total(cache: &[EntryRenderCache], show_pct: bool) -> Option<u64> {
    if show_pct && cache.iter().all(|entry| entry.size_bytes.is_some()) {
        Some(
            cache.iter()
                .filter_map(|entry| entry.size_bytes)
                .fold(0u64, |acc, size| acc.saturating_add(size)),
        )
    } else {
        None
    }
}

pub(crate) fn format_percent_col(total_bytes: Option<u64>, entry_bytes: Option<u64>, width: usize) -> String {
    match (total_bytes, entry_bytes) {
        (Some(total), Some(entry_size)) if total > 0 => {
            let pct = (entry_size as f64 * 100.0) / (total as f64);
            format!("{:>width$}", format!("{:.0}%", pct), width = width)
        }
        _ => format!("{:>width$}", "-", width = width),
    }
}

pub(crate) fn panel_name_width(
    term_w: u16,
    show_size: bool,
    size_width: usize,
    show_pct: bool,
    pct_width: usize,
    show_date: bool,
    date_width: usize,
) -> usize {
    (term_w as usize)
        .saturating_sub(
            (if show_size { size_width } else { 0 })
                + (if show_pct { pct_width } else { 0 })
                + (if show_date { date_width } else { 0 }),
        )
        .max(1)
}

/// The optional size/percent/date metric columns for one file-list row.
///
/// Each column is `Some` only when it should render. The percent column is
/// nested under the size column (it only appears when size is also shown) and
/// reuses the size column's style.
pub(crate) struct MetricColumns {
    /// `(width, style)` for the size column.
    pub(crate) size: Option<(usize, Style)>,
    /// `(width, total_bytes_for_pct)` for the percent column.
    pub(crate) pct: Option<(usize, Option<u64>)>,
    /// Style for the (pre-formatted) date column.
    pub(crate) date: Option<Style>,
}

pub(crate) fn push_metric_cells(cells: &mut Vec<Cell>, entry_cache: &EntryRenderCache, cols: &MetricColumns) {
    if let Some((size_width, size_style)) = cols.size {
        let size_col = format!("{:>width$}", entry_cache.size_col.trim(), width = size_width);
        cells.push(Cell::from(Span::styled(size_col, size_style)));
        if let Some((pct_width, total_for_pct)) = cols.pct {
            let pct_col = format_percent_col(total_for_pct, entry_cache.size_bytes, pct_width);
            cells.push(Cell::from(Span::styled(pct_col, size_style)));
        }
    }
    if let Some(date_style) = cols.date {
        cells.push(Cell::from(Span::styled(entry_cache.date_col.clone(), date_style)));
    }
}

pub(crate) fn push_metric_constraints(
    constraints: &mut Vec<Constraint>,
    show_size: bool,
    size_width: usize,
    show_pct: bool,
    pct_width: usize,
    show_date: bool,
    date_width: usize,
) {
    if show_size {
        constraints.push(Constraint::Length(size_width as u16));
    }
    if show_pct {
        constraints.push(Constraint::Length(pct_width as u16));
    }
    if show_date {
        constraints.push(Constraint::Length(date_width as u16));
    }
}
