-- Sample sb plugin: background work with sb.spawn.
-- Runs `du -sh` on the selected item without blocking the UI and shows the
-- result in the status line when it finishes.
--
-- Install:  cp duh.lua ~/.config/sb/plugins/

local M = {}

function M.entry(ctx)
    if not ctx.path then
        sb.notify("duh: nothing selected")
        return
    end
    sb.notify("duh: measuring " .. (ctx.name or ctx.path) .. "…")
    sb.spawn("du -sh " .. string.format("%q", ctx.path), function(r)
        if r.status == 0 then
            sb.notify((r.stdout:match("^%S+") or "?") .. "  " .. (ctx.name or ""))
        else
            sb.notify("duh failed: " .. r.stderr)
        end
    end)
end

return M
