-- Sample sb plugin: a key-bindable command.
-- Creates a timestamped file in the current directory, refreshes the
-- listing, selects the new file, and shows a status message.
--
-- Install:  cp touch-notify.lua ~/.config/sb/plugins/
-- Run:      `;` then `:touch-notify`, or bind a key in the Plugins panel (P).

local M = {}

function M.entry(ctx)
    local name = os.date("touched-%H%M%S.txt")
    local f = io.open(ctx.cwd .. "/" .. name, "w")
    if not f then
        sb.notify("touch-notify: cannot write in " .. ctx.cwd)
        return
    end
    f:close()
    sb.refresh()
    sb.select(name)
    sb.notify("created " .. name)
end

return M
