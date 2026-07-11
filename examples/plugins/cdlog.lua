-- Sample sb plugin: event hooks + per-plugin state directory.
-- Appends every directory change (and session start/end) to a log file in
-- the plugin's data dir (~/.config/sb/plugin-data/cdlog/log).
--
-- Install:  cp cdlog.lua ~/.config/sb/plugins/

local M = {}

local function log(line)
    local f = io.open(sb.data_dir() .. "/log", "a")
    if f then
        f:write(os.date("%Y-%m-%d %H:%M:%S "), line, "\n")
        f:close()
    end
end

function M.on_start(ctx)
    log("start " .. ctx.cwd)
end

function M.on_cd(ctx)
    log((ctx.prev or "?") .. " -> " .. ctx.cwd)
end

function M.on_quit(ctx)
    log("quit " .. ctx.cwd)
end

return M
