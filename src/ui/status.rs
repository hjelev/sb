const KNOWN_STATUS_ICONS: [&str; 10] = ["", "󰜺", "󰱒", "", "", "", "󰍉", "󰖟", "", ""];

/// Returns true if a status message reads as a failure.
///
/// Shared by the footer renderer and the status-icon picker so the
/// "what counts as an error" rule lives in one place.
pub fn is_error_message(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("error") || lower.contains("failed") || lower.contains("not found")
}

pub fn status_icon_for_message(msg: &str) -> &'static str {
    let lower = msg.to_ascii_lowercase();
    if is_error_message(msg) || lower.contains("invalid") {
        ""
    } else if lower.contains("cancel") {
        "󰜺"
    } else if lower.starts_with("selected:") {
        "󰱒"
    } else if lower.contains("git")
        || lower.contains("commit")
        || lower.contains("branch")
        || lower.contains("tag")
    {
        ""
    } else if lower.contains("archive")
        || lower.contains("extract")
        || lower.contains("zip")
        || lower.contains("tar")
        || lower.contains("7z")
        || lower.contains("rar")
    {
        ""
    } else if lower.contains("copy")
        || lower.contains("paste")
        || lower.contains("clipboard")
        || lower.contains("transfer")
        || lower.contains("move")
    {
        ""
    } else if lower.contains("search") || lower.contains("find") || lower.contains("index") {
        "󰍉"
    } else if lower.contains("mount") || lower.contains("ssh") || lower.contains("rclone") {
        "󰖟"
    } else if lower.contains("created")
        || lower.contains("saved")
        || lower.contains("installed")
        || lower.contains("opened")
        || lower.contains("updated")
        || lower.contains("toggled")
        || lower.contains("complete")
    {
        ""
    } else {
        ""
    }
}

pub fn decorate_footer_message(msg: &str, nerd_font_active: bool) -> String {
    if !nerd_font_active {
        return msg.to_string();
    }

    let trimmed = msg.trim_start();
    if KNOWN_STATUS_ICONS
        .iter()
        .any(|icon| trimmed.starts_with(icon))
    {
        return msg.to_string();
    }

    format!("{} {}", status_icon_for_message(msg), msg)
}
