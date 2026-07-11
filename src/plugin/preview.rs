//! Plugin previewers: match a file to a registered plugin and run its
//! `peek()` on the preview worker thread.
//!
//! The main-thread Lua is `!Send`, so the worker builds a throwaway Lua per
//! call, re-evaluates the plugin script, and calls `peek(ctx)` on the module
//! table it returns. `setup()` is intentionally not run here — previewers
//! must be self-contained.

use std::path::Path;

use mlua::{Lua, Table, Value};

use super::PreviewerReg;

/// Pick the previewer for `path`: first registration listing the exact
/// (lowercased) extension wins; `"*"` catch-alls are only consulted when no
/// exact match exists.
pub(crate) fn match_previewer<'a>(
    regs: &'a [PreviewerReg],
    path: &Path,
) -> Option<&'a PreviewerReg> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    regs.iter()
        .find(|r| r.exts.iter().any(|e| e == &ext))
        .or_else(|| regs.iter().find(|r| r.exts.iter().any(|e| e == "*")))
}

/// Run `peek()` in a fresh Lua. Returns the preview lines and optional
/// footer. `peek` may return a `{ lines = {...}, footer = str }` table or a
/// plain string (split on newlines).
pub(crate) fn run_peek(
    reg: &PreviewerReg,
    path: &Path,
    max_lines: usize,
) -> Result<(Vec<String>, Option<String>), String> {
    let run = || -> mlua::Result<(Vec<String>, Option<String>)> {
        let lua = Lua::new();
        let code = std::fs::read_to_string(&reg.script)
            .map_err(|e| mlua::Error::runtime(e.to_string()))?;
        let module: Table = lua
            .load(&code)
            .set_name(format!("@{}", reg.plugin))
            .eval()?;
        let peek: mlua::Function = module.get("peek")?;
        let plugin_dir = reg
            .script
            .parent()
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        let sb = super::api::build_sb_preview_table(&lua, &reg.plugin, plugin_dir)?;
        lua.globals().set("sb", sb)?;
        let ctx = lua.create_table()?;
        ctx.set("path", path.to_string_lossy().into_owned())?;
        ctx.set(
            "ext",
            path.extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default(),
        )?;
        ctx.set("max_lines", max_lines)?;
        let result: Value = peek.call(ctx)?;
        coerce_peek_result(result, max_lines)
    };
    run().map_err(|e| e.to_string())
}

fn coerce_peek_result(
    value: Value,
    max_lines: usize,
) -> mlua::Result<(Vec<String>, Option<String>)> {
    match value {
        Value::String(s) => {
            let text = s.to_string_lossy();
            let lines = text
                .lines()
                .take(max_lines)
                .map(str::to_string)
                .collect();
            Ok((lines, None))
        }
        Value::Table(t) => {
            let mut lines: Vec<String> = t.get::<Option<Vec<String>>>("lines")?.unwrap_or_default();
            lines.truncate(max_lines);
            let footer: Option<String> = t.get("footer")?;
            Ok((lines, footer))
        }
        Value::Nil => Ok((Vec::new(), None)),
        other => Err(mlua::Error::runtime(format!(
            "peek() must return a string or a table, got {}",
            other.type_name()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn reg(plugin: &str, exts: &[&str], script: PathBuf) -> PreviewerReg {
        PreviewerReg {
            plugin: plugin.to_string(),
            script,
            exts: exts.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn exact_extension_beats_catch_all_regardless_of_order() {
        let regs = vec![
            reg("any", &["*"], PathBuf::new()),
            reg("txt", &["txt"], PathBuf::new()),
        ];
        let hit = match_previewer(&regs, Path::new("/a/file.TXT")).unwrap();
        assert_eq!(hit.plugin, "txt");
        let hit = match_previewer(&regs, Path::new("/a/file.bin")).unwrap();
        assert_eq!(hit.plugin, "any");
        assert!(match_previewer(&regs, Path::new("/a/noext")).is_none());
    }

    #[test]
    fn run_peek_table_and_string_results() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("count.lua");
        std::fs::write(
            &script,
            r#"
            local M = {}
            M.preview = { exts = { "txt" } }
            function M.peek(ctx)
                local n = 0
                for _ in io.lines(ctx.path) do n = n + 1 end
                return { lines = { "first" }, footer = n .. " lines" }
            end
            return M
            "#,
        )
        .unwrap();
        let target = tmp.path().join("data.txt");
        std::fs::write(&target, "a\nb\nc\n").unwrap();
        let r = reg("count", &["txt"], script);
        let (lines, footer) = run_peek(&r, &target, 100).unwrap();
        assert_eq!(lines, ["first"]);
        assert_eq!(footer.as_deref(), Some("3 lines"));
    }

    #[test]
    fn run_peek_error_is_contained() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("bad.lua");
        std::fs::write(
            &script,
            "local M = {}\nfunction M.peek() error('nope') end\nreturn M",
        )
        .unwrap();
        let r = reg("bad", &["txt"], script);
        let err = run_peek(&r, Path::new("/tmp/x.txt"), 10).unwrap_err();
        assert!(err.contains("nope"), "got: {err}");
    }

    #[test]
    fn effect_api_is_rejected_in_previewers() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("naughty.lua");
        std::fs::write(
            &script,
            "local M = {}\nfunction M.peek(ctx) sb.cd('/tmp') end\nreturn M",
        )
        .unwrap();
        let r = reg("naughty", &["txt"], script);
        let err = run_peek(&r, Path::new("/tmp/x.txt"), 10).unwrap_err();
        assert!(err.contains("not available in previewers"), "got: {err}");
    }
}
