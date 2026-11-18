//! Core application types (extracted from main.rs).

use std::collections::HashMap;
use std::path::PathBuf;

use ratatui::prelude::Color;

/// Tool/display options for building a file preview, threaded from the UI state
/// into the background preview worker.
#[derive(Clone, Copy)]
pub(crate) struct PreviewBuildOptions {
    pub(crate) use_bat: bool,
    pub(crate) use_file: bool,
    pub(crate) use_resvg: bool,
    pub(crate) use_timg: bool,
    pub(crate) use_pdftotext: bool,
    pub(crate) use_glow: bool,
    pub(crate) use_doxx: bool,
    pub(crate) use_xleak: bool,
    pub(crate) use_sqlite3: bool,
    pub(crate) use_sox: bool,
    pub(crate) use_mmdflux: bool,
    pub(crate) use_links: bool,
    pub(crate) use_hexyl: bool,
    pub(crate) use_zip_list: bool,
    pub(crate) use_tar_list: bool,
    pub(crate) use_7z_list: bool,
    pub(crate) use_rar_list: bool,
    pub(crate) pane_cols: u16,
    pub(crate) pane_rows: u16,
    pub(crate) show_icons: bool,
    pub(crate) nerd_font_active: bool,
    pub(crate) theme_id: crate::ui::theme::ThemeId,
    pub(crate) filename_color_mode: FilenameColorMode,
}

/// How file (not folder) names are colored in the list. Folders are never affected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilenameColorMode {
    /// File names match their icon color (default, original behavior).
    Full,
    /// Normal file names use the theme's normal text color; status colors
    /// (executable, symlink, archive, broken link, age-encrypted) are kept.
    Less,
    /// All file names use the theme's normal text color; only icons stay colored.
    White,
}

impl FilenameColorMode {
    /// Cycle Full → Less → White → Full.
    pub(crate) fn next(self) -> Self {
        match self {
            FilenameColorMode::Full => FilenameColorMode::Less,
            FilenameColorMode::Less => FilenameColorMode::White,
            FilenameColorMode::White => FilenameColorMode::Full,
        }
    }

    /// Stable key used for persistence.
    pub(crate) fn as_key(self) -> &'static str {
        match self {
            FilenameColorMode::Full => "full",
            FilenameColorMode::Less => "less",
            FilenameColorMode::White => "white",
        }
    }

    /// Parse a persisted key, defaulting to `Full` for unknown values.
    pub(crate) fn from_key(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "less" => FilenameColorMode::Less,
            "white" => FilenameColorMode::White,
            _ => FilenameColorMode::Full,
        }
    }

    /// Short label shown in the Themes panel toggle row.
    pub(crate) fn label(self) -> &'static str {
        match self {
            FilenameColorMode::Full => "Full",
            FilenameColorMode::Less => "Less",
            FilenameColorMode::White => "White",
        }
    }
}

pub(crate) struct ArchiveMount {
    pub(crate) archive_path: PathBuf,
    pub(crate) mount_path: PathBuf,
    pub(crate) return_dir: PathBuf,
    pub(crate) archive_name: String,
}

#[derive(Clone)]
pub(crate) struct SshHost {
    pub(crate) alias: String,
    pub(crate) hostname: String,
    pub(crate) user: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) identity_file: Option<String>,
}

#[derive(Clone)]
pub(crate) enum RemoteEntry {
    Ssh(SshHost),
    Rclone {
        name: String,
        rtype: String,
    },
    ArchiveMount {
        archive_name: String,
        mount_path: PathBuf,
    },
    LocalMount {
        name: String,
        mount_path: PathBuf,
        source: String,
    },
}

impl RemoteEntry {
    pub(crate) fn alias(&self) -> &str {
        match self {
            RemoteEntry::Ssh(h) => &h.alias,
            RemoteEntry::Rclone { name, .. } => name,
            RemoteEntry::ArchiveMount { archive_name, .. } => archive_name,
            RemoteEntry::LocalMount { name, .. } => name,
        }
    }
}

pub(crate) struct SshMount {
    pub(crate) host_alias: String,
    pub(crate) mount_path: PathBuf,
    pub(crate) return_dir: PathBuf,
    pub(crate) remote_label: String,
    pub(crate) remote_root: String,
    pub(crate) remote_os_icon: Option<(&'static str, Color)>,
}

/// Branch name, dirty flag, and an optional (tag name, commits-ahead) pair.
pub(crate) type GitInfo = (String, bool, Option<(String, u64)>);

/// Borrowed view of [`GitInfo`], returned by cache readers that don't need to
/// clone the strings.
pub(crate) type GitInfoRef<'a> = (&'a str, bool, Option<(&'a str, u64)>);

pub(crate) struct GitInfoCache {
    pub(crate) path: PathBuf,
    pub(crate) info: Option<GitInfo>,
}

pub(crate) enum CopyProgressMsg {
    TotalBytes(u64),
    CopiedBytes(u64),
    Finished(Result<(), String>),
}

pub(crate) enum DownloadProgressMsg {
    Status(String),
    Finished {
        file_name: String,
        result: Result<(), String>,
    },
}

pub(crate) enum ArchiveProgressMsg {
    TotalBytes(u64),
    Progress(u64),
    Finished(Result<String, String>),
}

pub(crate) enum FolderSizeMsg {
    EntrySize(u64, PathBuf, u64),
    Finished(u64),
}

pub(crate) enum SelectedTotalSizeMsg {
    Finished(u64, u64),
}

pub(crate) enum CurrentDirTotalSizeMsg {
    Finished(u64, u64),
}

pub(crate) enum RecursiveMtimeMsg {
    EntryMtime(u64, PathBuf, u64),
    Finished(u64),
}

pub(crate) enum NotesLoadMsg {
    Finished(u64, PathBuf, HashMap<String, String>),
}

/// Result of a background AI commit-message generation request.
pub(crate) enum AiCommitMsg {
    Ok(String),
    Err(String),
}

/// Validation state of the AI API key shown in the Settings panel.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum AiKeyStatus {
    /// Not checked (no key, or the key changed since the last check).
    #[default]
    Unknown,
    /// A validation request is in flight.
    Checking,
    /// The provider accepted the key.
    Valid,
    /// The provider rejected the key (bad/expired token).
    Invalid,
}

/// Result of a background AI API-key validation request. Carries the key that
/// was checked so a result for a since-changed key can be discarded.
pub(crate) enum AiKeyCheckMsg {
    /// The check completed: `valid` is true when the provider authenticated it.
    Result { key: String, valid: bool },
    /// The check could not be performed (network/transport error).
    Error { key: String, message: String },
}

/// A single proposed relocation within an [`OrganizePlan`]: move the entry
/// named `name` (a top-level entry of the organized directory) into `folder`
/// (a single path segment, new or existing).
pub(crate) struct OrganizeMove {
    pub(crate) name: String,
    pub(crate) folder: String,
}

/// An AI-proposed reorganization of a directory's top-level entries.
pub(crate) struct OrganizePlan {
    /// Folder names to create (if they don't already exist) before applying `moves`.
    pub(crate) folders: Vec<String>,
    pub(crate) moves: Vec<OrganizeMove>,
}

/// Result of a background AI organize-plan generation request.
pub(crate) enum OrganizePlanMsg {
    Ok(OrganizePlan),
    Err(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArchiveKind {
    Zip,
    Tar,
    SevenZip,
    Rar,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortMode {
    NameAsc,
    NameDesc,
    ExtensionAsc,
    SizeAsc,
    SizeDesc,
    ModifiedNewest,
    ModifiedOldest,
}

#[derive(Clone)]
pub(crate) enum PathFilterMode {
    Prefix,
    Suffix,
    Contains,
}

#[derive(Clone)]
pub(crate) struct PathInputFilter {
    pub(crate) mode: PathFilterMode,
    pub(crate) pattern: String,
}

impl SortMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            SortMode::NameAsc => "Name (A-Z)",
            SortMode::NameDesc => "Name (Z-A)",
            SortMode::ExtensionAsc => "Extension (A-Z)",
            SortMode::SizeAsc => "Size (Small-Large)",
            SortMode::SizeDesc => "Size (Large-Small)",
            SortMode::ModifiedNewest => "Modified (Newest)",
            SortMode::ModifiedOldest => "Modified (Oldest)",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppMode {
    Browsing,
    PathEditing,
    FolderFilter,
    DbPreview,
    CommandInput,
    GitCommitMessage,
    GitTagInput,
    InternalSearch,
    NoteEditing,
    DownloadInput,
    DownloadNaming,
    Renaming,
    PasteRenaming,
    NewFile,
    NewFolder,
    ArchiveCreate,
    ConfirmExtract,
    ConfirmDownloadOverwrite,
    ConfirmIntegrationInstall,
    Help,
    ConfirmDelete,
    Organize,
    Bookmarks,
    BookmarkEditing,
    ConfirmDeleteBookmark,
    Integrations,
    Themes,
    SortMenu,
    SshPicker,
    Settings,
    Shortcuts,
    Plugins,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum InternalSearchScope {
    Filename,
    Content,
}

pub(crate) enum InternalSearchResult {
    Filename {
        rel_path: PathBuf,
        match_ranges: Vec<(usize, usize)>,
    },
    Content {
        rel_path: PathBuf,
        line_number: usize,
        line_text: String,
        match_ranges: Vec<(usize, usize)>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PreviewPaneFocus {
    #[default]
    Folder,
    Preview,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum ViewMode {
    #[default]
    Normal,
    Preview,
    DualPanel,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DualPanelSide {
    #[default]
    Left,
    Right,
}

/// An icon prefix carved off the front of a folder-preview line so it can be
/// colored independently of the file name (icon stays colored while the name
/// follows the filename-color mode).
#[derive(Clone, Copy)]
pub(crate) struct PreviewIconSpan {
    /// Byte length of the icon prefix within the line.
    pub(crate) len: usize,
    /// Icon color (kept regardless of the filename-color mode).
    pub(crate) fg: Option<Color>,
}

#[derive(Clone, Copy)]
pub(crate) enum PreviewLineKind {
    Plain,
    Styled {
        fg: Option<Color>,
        bold: bool,
        dim: bool,
        /// When set, the line is split into a colored icon span plus the name
        /// span (which uses `fg`). `None` renders the whole line with `fg`.
        icon: Option<PreviewIconSpan>,
    },
}

/// Components for rendering the header-right disk summary. `disk_segment` is
/// drawn as a horizontal progress bar (background filled by `used_fraction`).
pub(crate) struct DiskHeaderInfo {
    /// Folder-size prefix incl. trailing separator, e.g. "📂 3.4G | ".
    pub(crate) folder_segment: String,
    /// The used/total label without a percentage, e.g. "💾 246G / 915G".
    pub(crate) disk_segment: String,
    /// Used fraction in 0.0..=1.0; `None` when total/free is unknown.
    pub(crate) used_fraction: Option<f64>,
}

#[derive(Clone, Copy)]
pub(crate) struct InternalSearchContentLimits {
    pub(crate) max_files: usize,
    pub(crate) max_hits: usize,
    pub(crate) max_file_bytes: usize,
}

pub(crate) enum InternalSearchPattern {
    Regex {
        pattern: String,
        case_insensitive: bool,
    },
    Literal(String),
}

pub(crate) enum InternalSearchContentMsg {
    Finished {
        request_id: u64,
        results: Vec<InternalSearchResult>,
        limit_note: Option<String>,
    },
}

pub(crate) enum InternalSearchCandidatesMsg {
    Finished {
        scan_id: u64,
        candidates: Vec<PathBuf>,
        /// Subset of `candidates` that are symlinks (checked during the walk).
        symlinks: std::collections::HashSet<PathBuf>,
        truncated: bool,
    },
}

/// Cheap change detector for a previewed path: modification time + size.
/// Editing a file (or adding/removing entries in a directory) changes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileStamp {
    pub modified: Option<std::time::SystemTime>,
    pub len: u64,
}

impl FileStamp {
    pub(crate) fn of(path: &std::path::Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        Some(Self {
            modified: meta.modified().ok(),
            len: meta.len(),
        })
    }
}

/// Loaded preview text, styled line kinds, and an optional footer, as cached
/// per-path in [`PreviewCache`].
pub(crate) type PreviewCacheEntry = (Vec<String>, Vec<PreviewLineKind>, Option<String>);

/// Bounded least-recently-used cache of text previews keyed by path. Each
/// entry holds up to a few hundred ANSI lines, so an unbounded map would grow
/// for the whole session as the user browses.
pub(crate) struct PreviewCache {
    map: std::collections::HashMap<PathBuf, (FileStamp, PreviewCacheEntry, u64)>,
    /// Monotonic use counter; the entry with the smallest value is evicted.
    tick: u64,
    capacity: usize,
}

impl Default for PreviewCache {
    fn default() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }
}

impl PreviewCache {
    pub(crate) const DEFAULT_CAPACITY: usize = 256;

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            map: std::collections::HashMap::new(),
            tick: 0,
            capacity: capacity.max(1),
        }
    }

    /// Cached preview for `path` if it was built for `stamp`; marks it as
    /// recently used. A stale entry (file changed) is dropped.
    pub(crate) fn get(
        &mut self,
        path: &std::path::Path,
        stamp: FileStamp,
    ) -> Option<PreviewCacheEntry> {
        self.tick += 1;
        let tick = self.tick;
        match self.map.get_mut(path) {
            Some((cached_stamp, entry, used)) if *cached_stamp == stamp => {
                *used = tick;
                Some(entry.clone())
            }
            Some(_) => {
                self.map.remove(path);
                None
            }
            None => None,
        }
    }

    pub(crate) fn insert(&mut self, path: PathBuf, stamp: FileStamp, entry: PreviewCacheEntry) {
        self.tick += 1;
        if !self.map.contains_key(&path) && self.map.len() >= self.capacity {
            // O(capacity) scan, only on inserts of new paths: fine for a few
            // hundred entries and avoids pulling in an LRU crate.
            if let Some(oldest) = self
                .map
                .iter()
                .min_by_key(|(_, (_, _, used))| *used)
                .map(|(p, _)| p.clone())
            {
                self.map.remove(&oldest);
            }
        }
        self.map.insert(path, (stamp, entry, self.tick));
    }

    pub(crate) fn clear(&mut self) {
        self.map.clear();
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }
}

pub(crate) enum PreviewContentMsg {
    Ready {
        request_id: u64,
        path: PathBuf,
        lines: Vec<String>,
        line_kinds: Vec<PreviewLineKind>,
        footer: Option<String>,
        image_rgb: Option<(Vec<u8>, u32, u32)>,
        /// `image_rgb` PNG-encoded off the UI thread (kitty/iTerm2 panes).
        image_png: Option<Vec<u8>>,
    },
    Failed {
        request_id: u64,
        path: PathBuf,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(len: u64) -> FileStamp {
        FileStamp {
            modified: None,
            len,
        }
    }

    fn entry(text: &str) -> PreviewCacheEntry {
        (vec![text.to_string()], vec![PreviewLineKind::Plain], None)
    }

    #[test]
    fn preview_cache_evicts_least_recently_used() {
        let mut cache = PreviewCache::with_capacity(2);
        cache.insert("a".into(), stamp(1), entry("a"));
        cache.insert("b".into(), stamp(1), entry("b"));
        // Touch "a" so "b" becomes the oldest.
        assert!(cache.get(std::path::Path::new("a"), stamp(1)).is_some());
        cache.insert("c".into(), stamp(1), entry("c"));
        assert_eq!(cache.len(), 2);
        assert!(cache.get(std::path::Path::new("b"), stamp(1)).is_none());
        assert!(cache.get(std::path::Path::new("a"), stamp(1)).is_some());
        assert!(cache.get(std::path::Path::new("c"), stamp(1)).is_some());
    }

    #[test]
    fn preview_cache_drops_stale_entries() {
        let mut cache = PreviewCache::with_capacity(4);
        cache.insert("a".into(), stamp(1), entry("old"));
        assert!(cache.get(std::path::Path::new("a"), stamp(2)).is_none());
        assert_eq!(cache.len(), 0);
        // Re-inserting an existing path never evicts another entry.
        cache.insert("x".into(), stamp(1), entry("x"));
        cache.insert("x".into(), stamp(2), entry("x2"));
        assert_eq!(cache.len(), 1);
        let (lines, _, _) = cache.get(std::path::Path::new("x"), stamp(2)).unwrap();
        assert_eq!(lines, vec!["x2".to_string()]);
    }
}
