//! The plugin runtime: owns the main-thread Lua state, loads plugin module
//! tables, and runs `entry()`/hooks/spawn-callbacks with contained errors.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use mlua::{Lua, Table};

use crate::util::keymap::KeyCombo;

use super::api::{self, SpawnReq};
use super::{
    LoadedPlugin, PluginCtx, PluginEffect, PluginHooks, PluginMsg, PluginSource, PreviewerReg,
};

/// Which event hook to fire (see [`PluginRuntime::run_hook`]).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hook {
    Start,
    Cd,
    Select,
    Quit,
}

pub(crate) struct PluginRuntime {
    lua: Lua,
    pub plugins: Vec<LoadedPlugin>,
    /// Key → plugin name, resolved after the built-in keymap misses.
    pub keymap: HashMap<KeyCombo, String>,
    /// Last observed active-panel dir / selected path, for hook tick-diffing.
    pub last_dir: Option<PathBuf>,
    pub last_selected: Option<PathBuf>,
    /// Re-entrancy guard: set while hook effects are being applied so a hook's
    /// own `sb.cd` cannot recurse within the same tick.
    pub hooks_running: bool,
    spawn_tx: Sender<PluginMsg>,
    pub spawn_rx: Receiver<PluginMsg>,
    /// Lua callbacks for in-flight `sb.spawn` jobs, keyed by token.
    pending_callbacks: HashMap<u64, mlua::RegistryKey>,
    /// Number of in-flight `sb.spawn` jobs (with or without callbacks).
    active_spawns: usize,
    next_token: u64,
    /// Cached previewer registrations (rebuilt on load/enable changes).
    previewer_cache: Vec<PreviewerReg>,
    /// Set when `sb.confirm`/`sb.input` suspended and resumed the terminal
    /// during the last `entry()` call; the caller must `terminal.clear()`.
    terminal_dirty: bool,
}

impl PluginRuntime {
    /// Discover nothing, load nothing — an inert runtime (used before init
    /// and in tests that don't touch plugins).
    pub fn empty() -> Self {
        let (spawn_tx, spawn_rx) = channel();
        Self {
            lua: Lua::new(),
            plugins: Vec::new(),
            keymap: HashMap::new(),
            last_dir: None,
            last_selected: None,
            hooks_running: false,
            spawn_tx,
            spawn_rx,
            pending_callbacks: HashMap::new(),
            active_spawns: 0,
            next_token: 1,
            previewer_cache: Vec::new(),
            terminal_dirty: false,
        }
    }

    /// Take (and clear) whether `sb.confirm`/`sb.input` touched the terminal
    /// during the last `entry()` call.
    pub(crate) fn take_terminal_dirty(&mut self) -> bool {
        std::mem::take(&mut self.terminal_dirty)
    }

    /// Build a runtime from discovered sources: loads every enabled plugin,
    /// runs its `setup()`, and applies persisted key bindings. Load errors
    /// are recorded per-plugin, never fatal.
    pub fn init(
        sources: Vec<PluginSource>,
        disabled: &[String],
        bindings: &HashMap<String, String>,
    ) -> Self {
        let mut rt = Self::empty();
        for source in sources {
            let enabled = !disabled.iter().any(|d| d == &source.name);
            rt.plugins.push(LoadedPlugin {
                source,
                enabled,
                has_entry: false,
                preview_exts: Vec::new(),
                hooks: PluginHooks::default(),
                module_key: None,
                last_error: None,
                bound_key: None,
            });
        }
        for idx in 0..rt.plugins.len() {
            if rt.plugins[idx].enabled {
                rt.load_plugin(idx);
            }
        }
        for (name, combo_str) in bindings {
            if let Some(combo) = KeyCombo::parse(combo_str)
                && let Some(p) = rt.plugins.iter_mut().find(|p| &p.source.name == name)
            {
                p.bound_key = Some(combo);
            }
        }
        rt.rebuild_indexes();
        rt
    }

    /// (Re)load one plugin's script: evaluate `main.lua` to a module table,
    /// inspect its capabilities, and run `setup()` if present. Clears any
    /// previous error/module on re-load. Call [`Self::rebuild_indexes`] after.
    pub fn load_plugin(&mut self, idx: usize) {
        let (script, name) = {
            let p = &mut self.plugins[idx];
            if let Some(key) = p.module_key.take() {
                let _ = self.lua.remove_registry_value(key);
            }
            p.last_error = None;
            p.has_entry = false;
            p.preview_exts.clear();
            p.hooks = PluginHooks::default();
            (p.source.script.clone(), p.source.name.clone())
        };
        let result: Result<(), String> = (|| {
            let code = std::fs::read_to_string(&script).map_err(|e| e.to_string())?;
            let module: Table = self
                .lua
                .load(&code)
                .set_name(format!("@{}", name))
                .eval()
                .map_err(|e| e.to_string())?;
            let p = &mut self.plugins[idx];
            p.has_entry = module.get::<mlua::Function>("entry").is_ok();
            p.hooks = PluginHooks {
                start: module.get::<mlua::Function>("on_start").is_ok(),
                cd: module.get::<mlua::Function>("on_cd").is_ok(),
                select: module.get::<mlua::Function>("on_select").is_ok(),
                quit: module.get::<mlua::Function>("on_quit").is_ok(),
            };
            if module.get::<mlua::Function>("peek").is_ok()
                && let Ok(preview) = module.get::<Table>("preview")
                && let Ok(exts) = preview.get::<Vec<String>>("exts")
            {
                p.preview_exts = exts
                    .into_iter()
                    .map(|e| e.trim_start_matches('.').to_ascii_lowercase())
                    .filter(|e| !e.is_empty())
                    .collect();
            }
            let setup = module.get::<mlua::Function>("setup");
            let key = self
                .lua
                .create_registry_value(module)
                .map_err(|e| e.to_string())?;
            self.plugins[idx].module_key = Some(key);
            if let Ok(setup) = setup {
                setup.call::<()>(()).map_err(|e| e.to_string())?;
            }
            Ok(())
        })();
        if let Err(msg) = result {
            self.plugins[idx].last_error = Some(msg);
        }
    }

    /// Rebuild the key-binding map and previewer cache from plugin state.
    /// Disabled or errored plugins contribute nothing.
    pub fn rebuild_indexes(&mut self) {
        self.keymap.clear();
        self.previewer_cache.clear();
        for p in &self.plugins {
            if !p.enabled || p.module_key.is_none() {
                continue;
            }
            if p.has_entry
                && let Some(combo) = p.bound_key
            {
                self.keymap.entry(combo).or_insert_with(|| p.source.name.clone());
            }
            if !p.preview_exts.is_empty() {
                self.previewer_cache.push(PreviewerReg {
                    plugin: p.source.name.clone(),
                    script: p.source.script.clone(),
                    exts: p.preview_exts.clone(),
                });
            }
        }
    }

    pub fn resolve_key(&self, combo: KeyCombo) -> Option<&str> {
        self.keymap.get(&combo).map(String::as_str)
    }

    pub fn previewer_regs(&self) -> Vec<PreviewerReg> {
        self.previewer_cache.clone()
    }

    /// Whether any plugin registered the given hook (lets the tick skip
    /// snapshot/diff work entirely when nothing listens).
    pub fn wants_hook(&self, hook: Hook) -> bool {
        self.plugins.iter().any(|p| {
            p.enabled
                && p.module_key.is_some()
                && match hook {
                    Hook::Start => p.hooks.start,
                    Hook::Cd => p.hooks.cd,
                    Hook::Select => p.hooks.select,
                    Hook::Quit => p.hooks.quit,
                }
        })
    }

    pub fn has_pending_spawns(&self) -> bool {
        self.active_spawns > 0
    }

    /// Run `entry()` of the named plugin. Returns the queued effects, or the
    /// error message (also recorded on the plugin).
    pub fn run_entry(
        &mut self,
        name: &str,
        ctx: &PluginCtx,
    ) -> Result<Vec<PluginEffect>, String> {
        let idx = self
            .plugins
            .iter()
            .position(|p| p.source.name == name)
            .ok_or_else(|| format!("no plugin named '{}'", name))?;
        {
            let p = &self.plugins[idx];
            if !p.enabled {
                return Err(format!("plugin '{}' is disabled", name));
            }
            if p.module_key.is_none() {
                return Err(p
                    .last_error
                    .clone()
                    .unwrap_or_else(|| format!("plugin '{}' failed to load", name)));
            }
            if !p.has_entry {
                return Err(format!("plugin '{}' has no entry()", name));
            }
        }
        let result = self.call_module_fn(idx, "entry", ctx, true, true);
        if let Err(msg) = &result {
            self.plugins[idx].last_error = Some(msg.clone());
        }
        result
    }

    /// Fire a hook on every plugin that registered it; per-plugin errors are
    /// recorded, effects from all plugins are concatenated in plugin order.
    pub fn run_hook(&mut self, hook: Hook, ctx: &PluginCtx) -> Vec<PluginEffect> {
        let fn_name = match hook {
            Hook::Start => "on_start",
            Hook::Cd => "on_cd",
            Hook::Select => "on_select",
            Hook::Quit => "on_quit",
        };
        let mut all = Vec::new();
        for idx in 0..self.plugins.len() {
            let wants = {
                let p = &self.plugins[idx];
                p.enabled
                    && p.module_key.is_some()
                    && match hook {
                        Hook::Start => p.hooks.start,
                        Hook::Cd => p.hooks.cd,
                        Hook::Select => p.hooks.select,
                        Hook::Quit => p.hooks.quit,
                    }
            };
            if !wants {
                continue;
            }
            match self.call_module_fn(idx, fn_name, ctx, true, false) {
                Ok(mut effects) => all.append(&mut effects),
                Err(msg) => self.plugins[idx].last_error = Some(msg),
            }
        }
        all
    }

    /// Invoke `module.<fn_name>(ctx)` with a fresh effects queue and `sb`
    /// table; drains queued spawn requests into worker threads afterwards.
    fn call_module_fn(
        &mut self,
        idx: usize,
        fn_name: &str,
        ctx: &PluginCtx,
        allow_spawn: bool,
        allow_terminal: bool,
    ) -> Result<Vec<PluginEffect>, String> {
        let plugin_name = self.plugins[idx].source.name.clone();
        let plugin_dir = plugin_dir_of(&self.plugins[idx].source);
        let effects = Rc::new(RefCell::new(Vec::new()));
        let spawns = allow_spawn.then(|| Rc::new(RefCell::new(Vec::<SpawnReq>::new())));
        let term_flag = allow_terminal.then(|| Rc::new(Cell::new(false)));

        let call = || -> mlua::Result<()> {
            let key = self.plugins[idx]
                .module_key
                .as_ref()
                .expect("checked by caller");
            let module: Table = self.lua.registry_value(key)?;
            let func: mlua::Function = module.get(fn_name)?;
            let sb = api::build_sb_table(
                &self.lua,
                &plugin_name,
                plugin_dir,
                effects.clone(),
                spawns.clone(),
                term_flag.clone(),
            )?;
            let ctx_t = api::ctx_table(&self.lua, ctx)?;
            self.lua.globals().set("sb", sb)?;
            func.call::<()>(ctx_t)
        };
        let result = call().map_err(|e| e.to_string());

        if let Some(reqs) = spawns {
            for req in reqs.take() {
                self.start_spawn(&plugin_name, req);
            }
        }
        if let Some(flag) = term_flag {
            self.terminal_dirty |= flag.get();
        }
        result.map(|()| effects.take())
    }

    /// Start one `sb.spawn` job on a worker thread; the result comes back
    /// through the shared channel drained by `poll_spawn`.
    fn start_spawn(&mut self, plugin_name: &str, req: SpawnReq) {
        let token = self.next_token;
        self.next_token += 1;
        if let Some(cb) = req.callback
            && let Ok(key) = self.lua.create_registry_value(cb)
        {
            self.pending_callbacks.insert(token, key);
        }
        self.active_spawns += 1;
        let tx = self.spawn_tx.clone();
        let plugin = plugin_name.to_string();
        let cmd = req.cmd;
        std::thread::spawn(move || {
            let output = std::process::Command::new("sh").arg("-c").arg(&cmd).output();
            let msg = match output {
                Ok(out) => PluginMsg::SpawnDone {
                    token,
                    plugin,
                    status: out.status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                },
                Err(e) => PluginMsg::SpawnDone {
                    token,
                    plugin,
                    status: -1,
                    stdout: String::new(),
                    stderr: e.to_string(),
                },
            };
            let _ = tx.send(msg);
        });
    }

    /// Non-blocking poll of finished `sb.spawn` jobs.
    pub fn poll_spawn(&mut self) -> Option<PluginMsg> {
        match self.spawn_rx.try_recv() {
            Ok(msg) => {
                self.active_spawns = self.active_spawns.saturating_sub(1);
                Some(msg)
            }
            Err(_) => None,
        }
    }

    /// Invoke the stored Lua callback for a finished spawn (if any) with a
    /// `{ status, stdout, stderr }` table; returns its queued effects.
    pub fn run_spawn_callback(
        &mut self,
        token: u64,
        plugin: &str,
        status: i32,
        stdout: &str,
        stderr: &str,
    ) -> Result<Vec<PluginEffect>, String> {
        let Some(key) = self.pending_callbacks.remove(&token) else {
            return Ok(Vec::new());
        };
        let idx = self.plugins.iter().position(|p| p.source.name == plugin);
        let plugin_dir = idx
            .map(|i| plugin_dir_of(&self.plugins[i].source))
            .unwrap_or_default();
        let effects = Rc::new(RefCell::new(Vec::new()));
        let spawns = Rc::new(RefCell::new(Vec::<SpawnReq>::new()));
        let call = || -> mlua::Result<()> {
            let cb: mlua::Function = self.lua.registry_value(&key)?;
            let sb = api::build_sb_table(
                &self.lua,
                plugin,
                plugin_dir,
                effects.clone(),
                Some(spawns.clone()),
                None,
            )?;
            self.lua.globals().set("sb", sb)?;
            let res = self.lua.create_table()?;
            res.set("status", status)?;
            res.set("stdout", stdout)?;
            res.set("stderr", stderr)?;
            cb.call::<()>(res)
        };
        let result = call().map_err(|e| e.to_string());
        let _ = self.lua.remove_registry_value(key);
        for req in spawns.take() {
            self.start_spawn(plugin, req);
        }
        if let Err(msg) = &result
            && let Some(i) = idx
        {
            self.plugins[i].last_error = Some(msg.clone());
        }
        result.map(|()| effects.take())
    }
}

/// The directory `sb.plugin_dir()` reports: the plugin's own directory for
/// the `<name>/main.lua` form, or the plugins root for flat files.
fn plugin_dir_of(source: &PluginSource) -> PathBuf {
    source
        .script
        .parent()
        .map(PathBuf::from)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_plugin(dir: &std::path::Path, name: &str, body: &str) -> PluginSource {
        let script = dir.join(format!("{}.lua", name));
        std::fs::write(&script, body).unwrap();
        PluginSource {
            name: name.to_string(),
            script,
        }
    }

    fn ctx() -> PluginCtx {
        PluginCtx {
            cwd: PathBuf::from("/tmp"),
            selected: Some(PathBuf::from("/tmp/file.txt")),
            files: vec![PathBuf::from("/tmp/file.txt")],
            panel: "left",
            prev_dir: None,
        }
    }

    #[test]
    fn entry_effects_are_collected_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        let src = write_plugin(
            tmp.path(),
            "demo",
            r#"
            local M = {}
            function M.entry(ctx)
                sb.notify("hi " .. ctx.name)
                sb.cd("/tmp")
                sb.refresh()
            end
            return M
            "#,
        );
        let mut rt = PluginRuntime::init(vec![src], &[], &HashMap::new());
        let effects = rt.run_entry("demo", &ctx()).unwrap();
        assert_eq!(
            effects,
            vec![
                PluginEffect::Status("hi file.txt".to_string()),
                PluginEffect::Cd(PathBuf::from("/tmp")),
                PluginEffect::RefreshDir,
            ]
        );
    }

    #[test]
    fn lua_error_is_contained_and_recorded() {
        let tmp = tempfile::tempdir().unwrap();
        let src = write_plugin(
            tmp.path(),
            "boom",
            "local M = {}\nfunction M.entry(ctx) error('kaboom') end\nreturn M",
        );
        let mut rt = PluginRuntime::init(vec![src], &[], &HashMap::new());
        let err = rt.run_entry("boom", &ctx()).unwrap_err();
        assert!(err.contains("kaboom"), "got: {err}");
        assert!(rt.plugins[0].last_error.as_deref().unwrap().contains("kaboom"));
    }

    #[test]
    fn load_error_is_recorded_not_fatal() {
        let tmp = tempfile::tempdir().unwrap();
        let src = write_plugin(tmp.path(), "broken", "this is not lua ((");
        let rt = PluginRuntime::init(vec![src], &[], &HashMap::new());
        assert!(rt.plugins[0].last_error.is_some());
        assert!(rt.plugins[0].module_key.is_none());
    }

    #[test]
    fn disabled_plugin_is_not_loaded_or_bound() {
        let tmp = tempfile::tempdir().unwrap();
        let src = write_plugin(
            tmp.path(),
            "off",
            "local M = {}\nfunction M.entry() end\nreturn M",
        );
        let mut bindings = HashMap::new();
        bindings.insert("off".to_string(), "ctrl+t".to_string());
        let rt = PluginRuntime::init(vec![src], &["off".to_string()], &bindings);
        assert!(rt.plugins[0].module_key.is_none());
        assert!(rt.plugins[0].last_error.is_none());
        assert!(rt.keymap.is_empty());
    }

    #[test]
    fn capabilities_and_bindings_are_inspected() {
        let tmp = tempfile::tempdir().unwrap();
        let src = write_plugin(
            tmp.path(),
            "caps",
            r#"
            local M = {}
            M.preview = { exts = { "TXT", ".md" } }
            function M.entry() end
            function M.peek() end
            function M.on_cd() end
            return M
            "#,
        );
        let mut bindings = HashMap::new();
        bindings.insert("caps".to_string(), "ctrl+t".to_string());
        let rt = PluginRuntime::init(vec![src], &[], &bindings);
        let p = &rt.plugins[0];
        assert!(p.has_entry);
        assert_eq!(p.preview_exts, ["txt", "md"]);
        assert!(p.hooks.cd && !p.hooks.start);
        let combo = KeyCombo::parse("ctrl+t").unwrap();
        assert_eq!(rt.resolve_key(combo), Some("caps"));
        assert_eq!(rt.previewer_regs().len(), 1);
    }

    #[test]
    fn setup_runs_once_at_load() {
        let tmp = tempfile::tempdir().unwrap();
        let marker = tmp.path().join("marker");
        let src = write_plugin(
            tmp.path(),
            "init",
            &format!(
                "local M = {{}}\nfunction M.setup() io.open('{}', 'w'):close() end\nreturn M",
                marker.display()
            ),
        );
        let _rt = PluginRuntime::init(vec![src], &[], &HashMap::new());
        assert!(marker.exists());
    }

    #[test]
    fn hook_effects_concatenate_across_plugins() {
        let tmp = tempfile::tempdir().unwrap();
        let a = write_plugin(
            tmp.path(),
            "a",
            "local M = {}\nfunction M.on_cd(ctx) sb.notify('a:' .. ctx.cwd) end\nreturn M",
        );
        let b = write_plugin(
            tmp.path(),
            "b",
            "local M = {}\nfunction M.on_cd(ctx) sb.notify('b') end\nreturn M",
        );
        let mut rt = PluginRuntime::init(vec![a, b], &[], &HashMap::new());
        assert!(rt.wants_hook(Hook::Cd));
        assert!(!rt.wants_hook(Hook::Quit));
        let effects = rt.run_hook(Hook::Cd, &ctx());
        assert_eq!(
            effects,
            vec![
                PluginEffect::Status("a:/tmp".to_string()),
                PluginEffect::Status("b".to_string()),
            ]
        );
    }

    #[test]
    fn spawn_delivers_result_and_callback_effects() {
        let tmp = tempfile::tempdir().unwrap();
        let src = write_plugin(
            tmp.path(),
            "bg",
            r#"
            local M = {}
            function M.entry(ctx)
                sb.spawn("echo hello", function(r)
                    sb.notify("got: " .. r.stdout)
                end)
            end
            return M
            "#,
        );
        let mut rt = PluginRuntime::init(vec![src], &[], &HashMap::new());
        let effects = rt.run_entry("bg", &ctx()).unwrap();
        assert!(effects.is_empty());
        assert!(rt.has_pending_spawns());
        // Wait for the worker to report back.
        let msg = loop {
            if let Some(m) = rt.poll_spawn() {
                break m;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let PluginMsg::SpawnDone { token, plugin, status, stdout, stderr } = msg;
        assert_eq!(status, 0);
        let effects = rt
            .run_spawn_callback(token, &plugin, status, &stdout, &stderr)
            .unwrap();
        assert_eq!(
            effects,
            vec![PluginEffect::Status("got: hello\n".to_string())]
        );
        assert!(!rt.has_pending_spawns());
    }

    #[test]
    fn confirm_and_input_are_unavailable_from_hooks() {
        let tmp = tempfile::tempdir().unwrap();
        let src = write_plugin(
            tmp.path(),
            "asks_in_hook",
            "local M = {}\nfunction M.on_cd() sb.confirm('really?') end\nreturn M",
        );
        let mut rt = PluginRuntime::init(vec![src], &[], &HashMap::new());
        assert!(rt.wants_hook(Hook::Cd));
        // run_hook swallows per-plugin errors into last_error rather than
        // propagating them, so drive it through and check the recorded error.
        let _ = rt.run_hook(Hook::Cd, &ctx());
        let err = rt.plugins[0].last_error.as_deref().unwrap();
        assert!(err.contains("sb.confirm is not available here"), "got: {err}");
    }

    #[test]
    fn example_git_commit_plugin_loads_cleanly() {
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/plugins/git-commit.lua");
        let src = PluginSource {
            name: "git-commit".to_string(),
            script,
        };
        let rt = PluginRuntime::init(vec![src], &[], &HashMap::new());
        let p = &rt.plugins[0];
        assert!(p.last_error.is_none(), "load error: {:?}", p.last_error);
        assert!(p.has_entry);
    }

    #[test]
    fn json_encode_decode_round_trips_and_is_available_everywhere() {
        let tmp = tempfile::tempdir().unwrap();
        let src = write_plugin(
            tmp.path(),
            "json",
            r#"
            local M = {}
            function M.entry(ctx)
                local encoded = sb.json_encode({ a = 1, b = "two", c = { 3, 4 } })
                local decoded = sb.json_decode(encoded)
                sb.notify(decoded.b .. ":" .. decoded.c[2])
            end
            function M.on_start(ctx)
                -- Pure helpers must also work from hooks (no terminal needed).
                sb.notify(sb.json_encode({ ok = true }))
            end
            return M
            "#,
        );
        let mut rt = PluginRuntime::init(vec![src], &[], &HashMap::new());
        let effects = rt.run_entry("json", &ctx()).unwrap();
        assert_eq!(effects, vec![PluginEffect::Status("two:4".to_string())]);
        assert!(rt.wants_hook(Hook::Start));
        let effects = rt.run_hook(Hook::Start, &ctx());
        assert_eq!(effects, vec![PluginEffect::Status("{\"ok\":true}".to_string())]);
    }
}
