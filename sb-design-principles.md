# SB Design Principles

- always do the directory data capture in one pass
- build both display and selection rows from that same capture pass; avoid full second-pass parsing over all rows
- once you enter a directory cache everything + selection calculations so moving over files is smooth as butter
- leave all the colors and icons to the external source tool ls(that might change in the feature)
- when a row is selected, preserve source tool colors/icons instead of replacing with a decolored fallback
- make sure each new feature is independant and can work alone (plan to integrate many external tools like eza instead of ls, bat instead of less)
- keep the code as clean as possible
- if my requests are against any of these principles pls don't action them and tell me what principle I am about to break