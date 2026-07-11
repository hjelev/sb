-- Sample sb plugin: a custom previewer.
-- Takes over the preview pane for .txt/.log files: shows the first lines
-- with a line-count footer. `peek` runs on a worker thread in a throwaway
-- Lua state, so keep it self-contained (no sb.* effects, no setup state).
--
-- Install:  cp linecount.lua ~/.config/sb/plugins/

local M = {}

M.preview = { exts = { "txt", "log" } }

function M.peek(ctx)
    local lines, total = {}, 0
    for line in io.lines(ctx.path) do
        total = total + 1
        if total <= ctx.max_lines then
            lines[total] = string.format("%4d  %s", total, line)
        end
    end
    return { lines = lines, footer = total .. " lines (linecount plugin)" }
end

return M
