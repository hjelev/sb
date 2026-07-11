//! The `sb` API table exposed to plugin Lua code.
//!
//! Functions never touch `App` directly: immediate helpers answer from
//! captured data, everything else pushes a [`PluginEffect`] into a queue that
//! is applied after the Lua call returns. `sb.spawn` requests are likewise
//! collected and started by the runtime once the call completes.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use mlua::{Lua, Table, Value};

use super::{PluginCtx, PluginEffect};

/// A background command queued by `sb.spawn(cmd, on_done?)`.
pub(crate) struct SpawnReq {
    pub cmd: String,
    pub callback: Option<mlua::Function>,
}

/// Convert a [`PluginCtx`] snapshot into the `ctx` table passed to `entry()`
/// and hooks.
pub(crate) fn ctx_table(lua: &Lua, ctx: &PluginCtx) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("cwd", ctx.cwd.to_string_lossy().into_owned())?;
    if let Some(sel) = &ctx.selected {
        t.set("path", sel.to_string_lossy().into_owned())?;
        if let Some(name) = sel.file_name() {
            t.set("name", name.to_string_lossy().into_owned())?;
        }
    }
    let files = lua.create_table()?;
    for (i, f) in ctx.files.iter().enumerate() {
        files.set(i + 1, f.to_string_lossy().into_owned())?;
    }
    t.set("files", files)?;
    t.set("panel", ctx.panel)?;
    if let Some(prev) = &ctx.prev_dir {
        t.set("prev", prev.to_string_lossy().into_owned())?;
    }
    Ok(t)
}

/// Build the full `sb` table for `entry()`/hook/spawn-callback calls on the
/// main-thread Lua. `spawns` collects `sb.spawn` requests; pass `None` to
/// reject them (hooks could allow them, but v1 keeps spawn entry-only if the
/// runtime chooses so).
pub(crate) fn build_sb_table(
    lua: &Lua,
    plugin_name: &str,
    plugin_dir: PathBuf,
    effects: Rc<RefCell<Vec<PluginEffect>>>,
    spawns: Option<Rc<RefCell<Vec<SpawnReq>>>>,
) -> mlua::Result<Table> {
    let t = build_pure_helpers(lua, plugin_name, plugin_dir)?;

    macro_rules! effect_fn {
        ($lua_name:literal, |$($arg:ident : $ty:ty),*| $effect:expr) => {{
            let fx = effects.clone();
            t.set(
                $lua_name,
                lua.create_function(move |_, ($($arg,)*): ($($ty,)*)| {
                    fx.borrow_mut().push($effect);
                    Ok(())
                })?,
            )?;
        }};
    }

    effect_fn!("notify", |msg: String| PluginEffect::Status(msg));
    effect_fn!("cd", |path: String| PluginEffect::Cd(PathBuf::from(path)));
    effect_fn!("select", |name: String| PluginEffect::SelectName(name));
    effect_fn!("mark", |names: Vec<String>| PluginEffect::MarkNames(names));
    effect_fn!("clipboard_set", |paths: Vec<String>| {
        PluginEffect::ClipboardSet(paths.into_iter().map(PathBuf::from).collect())
    });
    effect_fn!("edit", |path: String| PluginEffect::EditPath(PathBuf::from(path)));
    effect_fn!("view", |path: String| PluginEffect::ViewPath(PathBuf::from(path)));
    effect_fn!("run", |cmd: String| PluginEffect::RunShellWait(cmd));
    // Zero-argument effect functions (the macro's arg list can't match `||`).
    for (name, effect) in [
        ("refresh", PluginEffect::RefreshDir),
        ("clear_marks", PluginEffect::ClearMarks),
    ] {
        let fx = effects.clone();
        t.set(
            name,
            lua.create_function(move |_, ()| {
                fx.borrow_mut().push(effect.clone());
                Ok(())
            })?,
        )?;
    }

    match spawns {
        Some(reqs) => {
            t.set(
                "spawn",
                lua.create_function(
                    move |_, (cmd, callback): (String, Option<mlua::Function>)| {
                        reqs.borrow_mut().push(SpawnReq { cmd, callback });
                        Ok(())
                    },
                )?,
            )?;
        }
        None => {
            t.set(
                "spawn",
                lua.create_function(|_, _: mlua::MultiValue| -> mlua::Result<()> {
                    Err(mlua::Error::runtime("sb.spawn is not available here"))
                })?,
            )?;
        }
    }

    Ok(t)
}

/// Build the reduced `sb` table for previewer (`peek`) calls, which run on a
/// worker thread with a throwaway Lua: only the pure helpers exist; effect
/// functions raise a descriptive error.
pub(crate) fn build_sb_preview_table(
    lua: &Lua,
    plugin_name: &str,
    plugin_dir: PathBuf,
) -> mlua::Result<Table> {
    let t = build_pure_helpers(lua, plugin_name, plugin_dir)?;
    for name in [
        "notify", "cd", "refresh", "select", "mark", "clear_marks",
        "clipboard_set", "edit", "view", "run", "spawn",
    ] {
        t.set(
            name,
            lua.create_function(move |_, _: mlua::MultiValue| -> mlua::Result<Value> {
                Err(mlua::Error::runtime(format!(
                    "sb.{} is not available in previewers",
                    name
                )))
            })?,
        )?;
    }
    Ok(t)
}

/// Helpers that answer immediately from captured data (safe everywhere).
fn build_pure_helpers(
    lua: &Lua,
    plugin_name: &str,
    plugin_dir: PathBuf,
) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set(
        "version",
        lua.create_function(|_, ()| Ok(env!("CARGO_PKG_VERSION")))?,
    )?;
    let dir_str = plugin_dir.to_string_lossy().into_owned();
    t.set(
        "plugin_dir",
        lua.create_function(move |_, ()| Ok(dir_str.clone()))?,
    )?;
    let name = plugin_name.to_string();
    t.set(
        "data_dir",
        lua.create_function(move |_, ()| {
            let dir = super::discovery::plugin_data_dir(&name);
            std::fs::create_dir_all(&dir).map_err(mlua::Error::runtime)?;
            Ok(dir.to_string_lossy().into_owned())
        })?,
    )?;
    Ok(t)
}
