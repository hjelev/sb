# SB Design Principles

- always do the directory data capture in one pass
- build both display and selection rows from that same capture pass; avoid full second-pass parsing over all rows
- Integrations are external tools used by lsy.
- Integration behavior and formatting must be configurable via environment variables.
- discovered means command -v tool succeeds.
- Discovered integrations are enabled by default, and can be toggled at runtime
- once you enter a directory cache everything including selection calculations 
- Leave colors and icons to the active source backend
- when a row is selected, render it with reverse-video highlight using the plain (decolored) text; ANSI stripping is required to prevent color codes from breaking selection rendering
- Keep implementations simple and maintainable; avoid duplicated logic paths
- make a new readme-lsy.md file that describes lsy
- don't modify sb and readme.md file, only use them as reference when needed or asked to look at a feature
- make everything configurable via env. variables and keep a clean map of the defaults of these variables
- make sure all tools used for gathering system data like ls and lsd are configured to output maximum information, colors and icons
- No per-item runtime processing in scrolling, rendering, or selection hot paths.
- if my requests are against any of these principles pls don't action them and tell me what principle I am about to break