use super::*;

pub(crate) fn run_tui_body(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> io::Result<()>
{
    let mut deferred_key: Option<KeyEvent> = None;
    let hostname = hostname::get().map(|h| h.to_string_lossy().into_owned()).unwrap_or_else(|_| "host".to_string());
    let user = env::var("USER").unwrap_or_else(|_| "user".to_string());

    loop {
        // Capture in-flight async work *before* pumping: a pump may apply (and
        // then clear) a channel this iteration, so the frame that shows the
        // final result must still draw. One trailing idle frame is harmless.
        let had_async_work = app.has_active_async_work();
        let clock_changed = app.refresh_header_clock_if_needed();
        app.pump_archive_progress();
        app.pump_copy_total_prescan();
        app.pump_copy_progress();
        app.pump_download_progress();
        app.pump_folder_size_progress();
        app.pump_recursive_mtime_progress();
        app.pump_current_dir_total_size_progress();
        app.pump_selected_total_size_progress();
        app.pump_git_info();
        app.request_git_info_for_current_dir_once();
        app.pump_notes_progress();
        app.pump_right_notes_progress();
        app.pump_internal_search_candidates_progress();
        app.pump_internal_search_content_progress();
        app.pump_preview_progress();
        app.request_preview_for_selected();
        let text_input_cursor = matches!(
            app.mode,
            AppMode::PathEditing
                | AppMode::FolderFilter
                | AppMode::DownloadInput
                | AppMode::DownloadNaming
                | AppMode::Renaming
                | AppMode::PasteRenaming
                | AppMode::NewFile
                | AppMode::NewFolder
                | AppMode::ArchiveCreate
                | AppMode::NoteEditing
                | AppMode::CommandInput
                | AppMode::GitCommitMessage
                | AppMode::GitTagInput
                | AppMode::InternalSearch
                | AppMode::BookmarkEditing
        );
        // Skip the whole render (cursor update + draw) on idle iterations where
        // nothing changed. `needs_redraw` is set by event handling (below) and by
        // init; `clock_changed`/`had_async_work` cover time- and channel-driven
        // updates that arrive without user input.
        let should_draw = app.needs_redraw || clock_changed || had_async_work;
        if should_draw {
        if text_input_cursor {
            execute!(terminal.backend_mut(), SetCursorStyle::BlinkingBar)?;
        } else {
            execute!(terminal.backend_mut(), SetCursorStyle::DefaultUserShape)?;
        }
        terminal.draw(|f| {
            // Footer pill hit-zones are rebuilt every frame. Overlay footers
            // append during render_overlays and the main footer appends in
            // render_footer, so clear once here before any of them run.
            app.footer_shortcut_zones.clear();
            let theme = *ui::theme::theme_spec(app.active_theme);
            f.render_widget(
                Block::default().style(
                    Style::default()
                        .bg(theme.bg_panel)
                        .fg(theme.text_normal),
                ),
                f.size(),
            );
            let footer_height: u16 = if app.is_preview_mode() || app.is_dual_panel_mode() { 1 } else { 2 };
            let header_rows: u16 = if app.is_preview_mode() || app.is_dual_panel_mode() { 1 } else { 2 };
            let chunks = Layout::default()
                .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
                .split(f.size());
            // Pre-calculate if scrollbar will be visible for header alignment
            let scrollbar_in_main = {
                let table_area_height = chunks[0].height.saturating_sub(header_rows);
                let needs_scroll = app.left.entries.len() > table_area_height as usize;
                let table_area_width = if app.is_preview_mode() {
                    (chunks[0].width * 33 / 100).max(1)
                } else if app.is_dual_panel_mode() {
                    (chunks[0].width * 50 / 100).max(1)
                } else {
                    chunks[0].width
                };
                app.mode_shows_main_scrollbar() && table_area_width > 2 && needs_scroll
            };
            let ctx = RenderCtx {
                theme,
                main: chunks[0],
                footer: chunks[1],
                header_rows,
                scrollbar_in_main,
            };
            render_header(f, app, &ctx, &user, &hostname);
            let tl = render_table(f, app, &ctx);
            render_scrollbar_and_preview(f, app, &ctx, &tl);
            render_overlays(f, app, &ctx);
            render_footer(f, app, &ctx);
        })?;
            app.needs_redraw = false;
        }


        // After ratatui has drawn, overlay native protocol image in the preview pane
        // for terminals that support in-pane rendering.
        let native_protocol = App::terminal_image_protocol().0;
        let native_pane_supported = matches!(
            native_protocol,
            crate::integration::probe::TerminalImageProtocol::Kitty
                | crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                | crate::integration::probe::TerminalImageProtocol::Sixel
        );
        if native_pane_supported && app.is_preview_mode() {
            if let (Some(area), Some(png), Some((rgb, iw, ih))) = (
                app.preview_native_area,
                app.preview_image_png.as_ref(),
                app.preview_image_rgb.as_ref(),
            ) {
                let fit = App::fit_native_image_area(area, *iw, *ih);
                let path_key = app
                    .preview_target_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "<no-path>".to_string());
                let draw_key = format!(
                    "{}|{}x{}|{}:{}:{}:{}",
                    path_key, iw, ih, fit.x, fit.y, fit.width, fit.height
                );

                if app.preview_native_last_key.as_deref() != Some(draw_key.as_str()) {
                    match native_protocol {
                        crate::integration::probe::TerminalImageProtocol::Kitty => {
                            let _ = App::clear_kitty_pane_image(crate::app_images::KITTY_IMAGE_ID_PREVIEW);
                            let _ = App::emit_kitty_pane(
                                png,
                                *iw,
                                *ih,
                                fit.x,
                                fit.y,
                                fit.width,
                                fit.height,
                                crate::app_images::KITTY_IMAGE_ID_PREVIEW,
                            );
                        }
                        crate::integration::probe::TerminalImageProtocol::Iterm2Inline => {
                            // Use the full preview pane bounds so clearing removes
                            // remnants from previously larger images.
                            let _ = App::emit_iterm2_pane(
                                png,
                                area.x,
                                area.y,
                                area.width,
                                area.height,
                            );
                        }
                        crate::integration::probe::TerminalImageProtocol::Sixel => {
                            // Pass the full pane area: emit_sixel_pane handles its
                            // own pixel-aware sizing and clears stale content first.
                            let _ = App::emit_sixel_pane(
                                rgb,
                                *iw,
                                *ih,
                                area.x,
                                area.y,
                                area.width,
                                area.height,
                            );
                        }
                        _ => {}
                    }
                    app.preview_native_last_key = Some(draw_key);
                }
            } else if app.preview_native_last_key.is_some() {
                // Switched from image -> non-image (folder/text/etc.): clear once.
                match native_protocol {
                    crate::integration::probe::TerminalImageProtocol::Kitty => {
                        let _ = App::clear_kitty_pane_image(crate::app_images::KITTY_IMAGE_ID_PREVIEW);
                    }
                    crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                    | crate::integration::probe::TerminalImageProtocol::Sixel => {
                        if let Some(area) = app.preview_native_area {
                            let _ = App::clear_preview_pane_area(
                                area.x,
                                area.y,
                                area.width,
                                area.height,
                            );
                        }
                    }
                    _ => {}
                }
                app.preview_native_last_key = None;
            }
        } else if app.preview_native_last_key.is_some() {
            // Preview disabled (or no longer native pane): clear once and stop tracking.
            match native_protocol {
                crate::integration::probe::TerminalImageProtocol::Kitty => {
                    let _ = App::clear_kitty_pane_image(crate::app_images::KITTY_IMAGE_ID_PREVIEW);
                }
                crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                | crate::integration::probe::TerminalImageProtocol::Sixel => {
                    if let Some(area) = app.preview_native_area {
                        let _ = App::clear_preview_pane_area(
                            area.x,
                            area.y,
                            area.width,
                            area.height,
                        );
                    }
                }
                _ => {}
            }
            app.preview_native_last_key = None;
        }

        // After ratatui has drawn, overlay the native-protocol help logo when
        // the Help overlay is open, unscrolled, and the terminal supports it.
        if app.mode == AppMode::Help && native_pane_supported {
            if let (Some(area), Some((png, iw, ih))) =
                (app.help_logo_native_area, ui::panels::help_logo_png_bytes_and_dims())
            {
                let fit = App::fit_native_image_area(area, iw, ih);
                let draw_key = format!("{}x{}|{}:{}:{}:{}", iw, ih, fit.x, fit.y, fit.width, fit.height);

                if app.help_logo_native_last_key.as_deref() != Some(draw_key.as_str()) {
                    match native_protocol {
                        crate::integration::probe::TerminalImageProtocol::Kitty => {
                            let _ = App::clear_kitty_pane_image(crate::app_images::KITTY_IMAGE_ID_HELP_LOGO);
                            let _ = App::emit_kitty_pane(
                                png,
                                iw,
                                ih,
                                fit.x,
                                fit.y,
                                fit.width,
                                fit.height,
                                crate::app_images::KITTY_IMAGE_ID_HELP_LOGO,
                            );
                        }
                        crate::integration::probe::TerminalImageProtocol::Iterm2Inline => {
                            let _ = App::emit_iterm2_pane(png, fit.x, fit.y, fit.width, fit.height);
                        }
                        crate::integration::probe::TerminalImageProtocol::Sixel => {
                            if let Some((rgb, rw, rh)) = ui::panels::help_logo_rgb_for_sixel(
                                ui::theme::theme_spec(app.active_theme).bg_panel,
                            ) {
                                let _ = App::emit_sixel_pane(
                                    &rgb, rw, rh, fit.x, fit.y, fit.width, fit.height,
                                );
                            }
                        }
                        _ => {}
                    }
                    app.help_logo_native_last_key = Some(draw_key);
                }
                app.help_logo_native_last_area = Some(area);
            } else if app.help_logo_native_last_key.is_some() {
                // Logo no longer drawable (scrolled away, mode changed mid-frame): clear once.
                match native_protocol {
                    crate::integration::probe::TerminalImageProtocol::Kitty => {
                        let _ = App::clear_kitty_pane_image(crate::app_images::KITTY_IMAGE_ID_HELP_LOGO);
                    }
                    crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                    | crate::integration::probe::TerminalImageProtocol::Sixel => {
                        if let Some(area) = app.help_logo_native_last_area {
                            let _ = App::clear_preview_pane_area(area.x, area.y, area.width, area.height);
                        }
                    }
                    _ => {}
                }
                app.help_logo_native_last_key = None;
                app.help_logo_native_last_area = None;
            }
        } else if app.help_logo_native_last_key.is_some() {
            // Left Help mode (or protocol unsupported): clear once and stop tracking.
            match native_protocol {
                crate::integration::probe::TerminalImageProtocol::Kitty => {
                    let _ = App::clear_kitty_pane_image(crate::app_images::KITTY_IMAGE_ID_HELP_LOGO);
                }
                crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                | crate::integration::probe::TerminalImageProtocol::Sixel => {
                    if let Some(area) = app.help_logo_native_last_area {
                        let _ = App::clear_preview_pane_area(area.x, area.y, area.width, area.height);
                    }
                }
                _ => {}
            }
            app.help_logo_native_last_key = None;
            app.help_logo_native_last_area = None;
        }

        let mut next_key: Option<KeyEvent> = deferred_key.take();
        if next_key.is_none() && event::poll(Duration::from_millis(80))? {
            match event::read()? {
                Event::Key(key) => {
                    next_key = Some(key);
                }
                Event::Mouse(mouse) => {
                    let is_scroll = matches!(
                        mouse.kind,
                        crossterm::event::MouseEventKind::ScrollUp
                            | crossterm::event::MouseEventKind::ScrollDown
                    );
                    if is_scroll {
                        // Drain queued scroll events so one wheel tick = one move
                        while event::poll(Duration::from_millis(0))? {
                            match event::read()? {
                                Event::Mouse(m)
                                    if m.kind == mouse.kind => {}
                                other => {
                                    // Non-scroll event — put it back via deferred handling isn't
                                    // possible, so just process it immediately before the scroll.
                                    if let Event::Mouse(m2) = other {
                                        let area = terminal.size()?;
                                        if let Some(simulated_key) = app.handle_mouse_event(m2, area) {
                                            deferred_key = Some(simulated_key);
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    let area = terminal.size()?;
                    if let Some(simulated_key) = app.handle_mouse_event(mouse, area) {
                        deferred_key = Some(simulated_key);
                    }
                    app.needs_redraw = true;
                    continue;
                }
                _ => {}
            }
        }

        if let Some(key) = next_key {
            // Any dispatched key may mutate state; repaint on the next iteration.
            app.needs_redraw = true;
            match key_dispatch::handle_app_key_event(terminal, app, key, &mut deferred_key)? {
                key_dispatch::KeyDispatchOutcome::Quit => break,
                key_dispatch::KeyDispatchOutcome::ContinueLoop => continue,
                key_dispatch::KeyDispatchOutcome::Ok => {}
            }
        }
    }

    Ok(())

}

