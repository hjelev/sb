use super::*;

pub(crate) fn handle_internal_search_key(app: &mut App, key: KeyEvent) -> io::Result<KeyDispatchOutcome> {
    match key.code {
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.search.scope == InternalSearchScope::Content {
                app.search.limits_menu_open = !app.search.limits_menu_open;
            }
        }
        KeyCode::Esc if app.search.limits_menu_open => {
            app.search.limits_menu_open = false;
        }
        KeyCode::Enter if app.search.limits_menu_open => {
            app.search.limits_menu_open = false;
        }
        KeyCode::Up if app.search.limits_menu_open => {
            app.search.limits_selected = app.search.limits_selected.saturating_sub(1);
        }
        KeyCode::Down if app.search.limits_menu_open => {
            app.search.limits_selected = (app.search.limits_selected + 1).min(2);
        }
        KeyCode::Left if app.search.limits_menu_open => {
            app.adjust_internal_search_content_limit(false, key.modifiers.contains(KeyModifiers::SHIFT));
        }
        KeyCode::Right if app.search.limits_menu_open => {
            app.adjust_internal_search_content_limit(true, key.modifiers.contains(KeyModifiers::SHIFT));
        }
        KeyCode::Char('-') if app.search.limits_menu_open => {
            app.adjust_internal_search_content_limit(false, key.modifiers.contains(KeyModifiers::SHIFT));
        }
        KeyCode::Char('+') if app.search.limits_menu_open => {
            app.adjust_internal_search_content_limit(true, key.modifiers.contains(KeyModifiers::SHIFT));
        }
        KeyCode::Char('=') if app.search.limits_menu_open => {
            app.adjust_internal_search_content_limit(true, key.modifiers.contains(KeyModifiers::SHIFT));
        }
        KeyCode::Char('r') if app.search.limits_menu_open => {
            app.reset_internal_search_content_limits_to_defaults();
        }
        KeyCode::Backspace | KeyCode::Delete | KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End
            if app.search.limits_menu_open =>
        {
        }
        KeyCode::Char(_)
            if app.search.limits_menu_open
                && !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
        }
        KeyCode::Esc => {
            app.cancel_internal_search_candidate_scan();
            app.cancel_internal_search_content_request();
            app.clear_input_edit();
            app.mode = AppMode::Browsing;
        }
        KeyCode::BackTab => {
            app.cancel_internal_search_candidate_scan();
            app.cancel_internal_search_content_request();
            app.panel_tab = 0;
            app.help_scroll_offset = 0;
            app.mode = AppMode::Help;
        }
        KeyCode::Tab => {
            app.cancel_internal_search_candidate_scan();
            app.cancel_internal_search_content_request();
            app.panel_tab = 2;
            app.refresh_bookmarks_cache();
            app.mode = AppMode::Bookmarks;
        }
        KeyCode::Enter => {
            let selected_path = app.selected_internal_search_path();
            app.cancel_internal_search_candidate_scan();
            app.cancel_internal_search_content_request();
            app.clear_input_edit();
            app.mode = AppMode::Browsing;
            if let Some(path) = selected_path
                && let Some(parent) = path.parent() {
                    app.try_enter_dir(parent.to_path_buf());
                    if let Some(name) = path.file_name() {
                        app.select_entry_named(&name.to_string_lossy());
                    }
                }
        }
        KeyCode::Up => {
            app.search.selected = app.search.selected.saturating_sub(1);
        }
        KeyCode::Down => {
            let max_idx = app.search.results.len().saturating_sub(1);
            app.search.selected = (app.search.selected + 1).min(max_idx);
        }
        KeyCode::PageUp => {
            app.search.selected = app.search.selected.saturating_sub(10);
        }
        KeyCode::PageDown => {
            let max_idx = app.search.results.len().saturating_sub(1);
            app.search.selected = (app.search.selected + 10).min(max_idx);
        }
        KeyCode::Backspace => {
            app.input_backspace();
            app.refresh_internal_search_results();
        }
        KeyCode::Delete => {
            app.input_delete();
            app.refresh_internal_search_results();
        }
        KeyCode::Left => app.input_move_left(),
        KeyCode::Right => app.input_move_right(),
        KeyCode::Home => {
            app.input_move_home();
        }
        KeyCode::End => {
            app.input_move_end();
        }
        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_internal_search_scope();
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.input_insert_char(c);
            app.refresh_internal_search_results();
        }
        _ => {}
    }
    Ok(KeyDispatchOutcome::Ok)
}

