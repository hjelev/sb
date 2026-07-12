-- Sample sb plugin: reproduces sb's built-in Ctrl+g git workflow
-- (dirty-check -> diff/status + confirm -> AI-assisted commit message ->
-- commit+push with rebase-retry -> optional tag+push) as an external
-- plugin, using sb.confirm_shell/sb.confirm/sb.input for the interactive
-- steps and a single sb.run for the actual git work so it plays out in one
-- continuous live terminal view. sb.confirm_shell renders the diff via
-- delta when it's on PATH, falling back to plain colored diff otherwise.
--
-- AI commit messages reuse whatever provider/model/API key/auto-commit
-- setting is already configured in sb's own Settings panel (Tab) — read
-- straight from ~/.config/sb/config, same as the built-in `G` workflow —
-- and shell out to `curl` (an OpenAI-compatible chat-completions request),
-- since plugin Lua has no built-in HTTP client. Requires `curl` on PATH and
-- an API key (Settings, or $GROQ_API_KEY / $GITHUB_TOKEN); otherwise this
-- step is skipped and you type the message manually, exactly like before.
--
-- Install:  cp git-commit.lua ~/.config/sb/plugins/
-- Bind a key to it from the Plugins panel (P, then b), or run it with
-- `;` then `:git-commit`.

local M = {}

local AI_PROVIDERS = {
    groq = {
        endpoint = "https://api.groq.com/openai/v1/chat/completions",
        default_model = "llama-3.3-70b-versatile",
        env_var = "GROQ_API_KEY",
    },
    github = {
        endpoint = "https://models.github.ai/inference/chat/completions",
        default_model = "openai/gpt-4o-mini",
        env_var = "GITHUB_TOKEN",
    },
}

local function shq(s)
    return "'" .. s:gsub("'", "'\\''") .. "'"
end

local function shell(cmd)
    local h = io.popen(cmd .. " 2>&1")
    if not h then return "", 1 end
    local out = h:read("*a") or ""
    local ok = h:close()
    return out, ok and 0 or 1
end

local function git(dir, args)
    return shell("git -C " .. shq(dir) .. " " .. args)
end

-- Mirrors sb's own config lookup (src/util/config.rs): $XDG_CONFIG_HOME/sb
-- or ~/.config/sb.
local function config_path()
    local base = os.getenv("XDG_CONFIG_HOME")
    if not base or base == "" then
        base = (os.getenv("HOME") or "") .. "/.config"
    end
    return base .. "/sb/config"
end

-- Reads the same ai_provider/ai_model/ai_api_key_<provider>/ai_auto_commit
-- lines the built-in workflow persists from the Settings panel.
local function read_ai_config()
    local cfg = { provider = "groq", model = "", api_keys = {}, auto_commit = false }
    local f = io.open(config_path(), "r")
    if not f then return cfg end
    for line in f:lines() do
        local k, v = line:match("^%s*([%w_]+)%s*=%s*(.-)%s*$")
        if k == "ai_provider" and v and v ~= "" then
            cfg.provider = v
        elseif k == "ai_model" then
            cfg.model = v or ""
        elseif k == "ai_auto_commit" then
            cfg.auto_commit = (v == "true")
        elseif k and k:match("^ai_api_key_") then
            cfg.api_keys[k:sub(#"ai_api_key_" + 1)] = v
        end
    end
    f:close()
    return cfg
end

-- Mirrors sb's built-in truncate_diff (src/app_ai.rs): cap at 6000 lines /
-- 100000 bytes so a huge changeset can't blow up the request body.
local function truncate_diff(diff)
    local lines, n = {}, 0
    for line in (diff .. "\n"):gmatch("([^\n]*)\n") do
        n = n + 1
        if n > 6000 then
            lines[#lines + 1] = "... [diff truncated]"
            return table.concat(lines, "\n")
        end
        lines[#lines + 1] = line
    end
    local out = table.concat(lines, "\n")
    if #out > 100000 then
        return out:sub(1, 100000) .. "\n... [diff truncated]"
    end
    return out
end

-- Generates a one-line commit message via an OpenAI-compatible
-- chat-completions endpoint, same prompt/shape as the built-in workflow.
-- Returns the message, or nil + a reason to show the user.
local function ai_generate_commit_message(dir, cfg)
    if os.execute("command -v curl >/dev/null 2>&1") ~= true then
        return nil, "curl not found"
    end
    local provider = AI_PROVIDERS[cfg.provider] or AI_PROVIDERS.groq
    local api_key = cfg.api_keys[cfg.provider]
    if not api_key or api_key == "" then
        api_key = os.getenv(provider.env_var)
    end
    if not api_key or api_key == "" then
        return nil, "no API key (set in Settings or export " .. provider.env_var .. ")"
    end
    local model = (cfg.model ~= "" and cfg.model) or provider.default_model

    local diff = git(dir, "diff HEAD")
    local untracked = (git(dir, "ls-files --others --exclude-standard"):gsub("%s+$", ""))
    if untracked ~= "" then
        diff = diff .. "\n\n# Untracked files:\n"
        for name in untracked:gmatch("[^\n]+") do
            diff = diff .. "# " .. name .. "\n"
        end
    end
    diff = truncate_diff(diff)
    if diff:match("^%s*$") then
        return nil, "no changes to summarize"
    end

    local body = sb.json_encode({
        model = model,
        messages = {
            {
                role = "system",
                content = "You write git commit messages. Reply with ONLY a single-line "
                    .. "commit message in the imperative mood (max 72 characters). No quotes, "
                    .. "no body, no explanation, no markdown.",
            },
            { role = "user", content = "Write a commit message for this diff:\n\n" .. diff },
        },
        temperature = 0.2,
        max_tokens = 100,
    })

    local bodyfile = os.tmpname()
    local bf = io.open(bodyfile, "w")
    bf:write(body)
    bf:close()

    -- Header (with the API key) goes through a curl config file rather than
    -- a -H argv, so the key never shows up in the process list.
    local curlcfg = os.tmpname()
    local cf = io.open(curlcfg, "w")
    cf:write('header = "Content-Type: application/json"\n')
    cf:write('header = "Authorization: Bearer ' .. api_key:gsub('"', '\\"') .. '"\n')
    cf:write('data-binary = "@' .. bodyfile .. '"\n')
    cf:close()

    local out = shell(string.format("curl -s -X POST -K %s %s", shq(curlcfg), shq(provider.endpoint)))
    os.remove(bodyfile)
    os.remove(curlcfg)

    local ok, resp = pcall(sb.json_decode, out)
    if not ok or type(resp) ~= "table" then
        return nil, "AI response parse failed"
    end
    if resp.error then
        return nil, tostring(type(resp.error) == "table" and resp.error.message or resp.error)
    end
    local content = resp.choices and resp.choices[1]
        and resp.choices[1].message and resp.choices[1].message.content
    if not content then
        return nil, "AI response missing content"
    end
    local first = content:match("^[^\n]*") or content
    first = first:gsub('^%s*"?%s*', ""):gsub('%s*"?%s*$', "")
    if first == "" then
        return nil, "AI returned an empty message"
    end
    return first
end

function M.entry(ctx)
    local dir = ctx.cwd

    local status_out, code = git(dir, "status --porcelain")
    if code ~= 0 then
        sb.notify("git-commit: not a git repository")
        return
    end
    if status_out == "" then
        sb.notify("git-commit: repository is clean")
        return
    end

    local delta_ok = os.execute("command -v delta >/dev/null 2>&1")
    local diff_flags = delta_ok
        and "-c core.pager=delta -c delta.side-by-side=true -c delta.features=side-by-side"
        or "-c color.ui=always"
    local show_cmd = string.format(
        "git -C %s %s diff && git -C %s status",
        shq(dir), diff_flags, shq(dir)
    )
    if not sb.confirm_shell(show_cmd, "Commit these changes?") then
        sb.notify("git-commit: cancelled")
        return
    end

    local ai_cfg = read_ai_config()
    local want_ai = ai_cfg.auto_commit or sb.confirm("Generate commit message with AI?")
    local ai_msg
    if want_ai then
        local ai_err
        ai_msg, ai_err = ai_generate_commit_message(dir, ai_cfg)
        if not ai_msg then
            sb.notify("git-commit: AI unavailable (" .. ai_err .. ") — enter manually")
        end
    end

    local raw = sb.input("commit message (add --amend to amend+force-push)", ai_msg or "")
    if not raw or raw == "" then
        sb.notify("git-commit: cancelled (empty message)")
        return
    end
    local amend = false
    local words = {}
    for token in raw:gmatch("%S+") do
        if token == "--amend" then
            amend = true
        else
            words[#words + 1] = token
        end
    end
    local message = table.concat(words, " ")

    local want_tag = sb.confirm("Create and push a tag after a successful push?")
    local tag
    if want_tag then
        local prefill = git(dir, "describe --tags --abbrev=0"):match("^%S+") or "v0.1.0"
        tag = sb.input("tag name", prefill)
        want_tag = tag ~= nil and tag ~= ""
    end

    local push = amend and "push origin HEAD -f" or "push origin HEAD"
    local retry = amend and "" or " || (git pull --rebase && git push origin HEAD)"
    local cmd = string.format(
        "set -e; git add --all && git commit -m %s%s && (git %s%s)",
        shq(message), amend and " --amend" or "", push, retry
    )
    if want_tag then
        cmd = cmd .. string.format(" && git tag %s && git push origin %s", shq(tag), shq(tag))
    end

    sb.run(cmd)
    sb.refresh()
end

return M
