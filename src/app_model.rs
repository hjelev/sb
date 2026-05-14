//! Core application types (extracted from main.rs).

use std::collections::HashMap;
use std::path::PathBuf;

use ratatui::prelude::Color;

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
    Rclone { name: String, rtype: String },
    ArchiveMount { archive_name: String, mount_path: PathBuf },
    LocalMount { name: String, mount_path: PathBuf, source: String },
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
    pub(crate) _host_alias: String,
    pub(crate) mount_path: PathBuf,
    pub(crate) return_dir: PathBuf,
    pub(crate) remote_label: String,
    pub(crate) remote_root: String,
    pub(crate) remote_os_icon: Option<(&'static str, Color)>,
}

pub(crate) struct GitInfoCache {
    pub(crate) path: PathBuf,
    pub(crate) info: Option<(String, bool, Option<(String, u64)>)>,
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
    Bookmarks,
    Integrations,
    SortMenu,
    SshPicker,
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreviewPaneFocus {
    Folder,
    Preview,
}

#[derive(Clone, Copy)]
pub(crate) enum PreviewLineKind {
    Plain,
    Styled {
        fg: Option<Color>,
        bold: bool,
        dim: bool,
    },
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
        truncated: bool,
    },
}

pub(crate) enum PreviewContentMsg {
    Ready {
        request_id: u64,
        path: PathBuf,
        lines: Vec<String>,
        line_kinds: Vec<PreviewLineKind>,
        footer: Option<String>,
        image_rgb: Option<(Vec<u8>, u32, u32)>,
    },
    Failed {
        request_id: u64,
        path: PathBuf,
        message: String,
    },
}
