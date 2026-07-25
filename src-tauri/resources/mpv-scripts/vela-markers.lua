--[[
Vela's intro / credits / commercial skip control.

Vela resolves the marker ranges the user's own media server publishes for the
item being played, writes them to a private per-launch payload file, and hands
this script the path on the CHILD PROCESS ENVIRONMENT ONLY. The path never
travels in mpv's script-opts list: that list is comma-split, and Windows and
user paths do not survive it (the same lesson the auth-include path taught).

Nothing here is upstream mpv code. Vela-authored, MIT, like the rest of the
repo — see PROVENANCE.md.

Options (explicit identifier: mpv would otherwise derive "vela_markers" from
the filename and silently ignore the dashed form Vela passes):
  --script-opts-append=vela-markers-intro-policy=off|button|autoskip
  --script-opts-append=vela-markers-credits-policy=off|button|autoskip
  --script-opts-append=vela-markers-commercial-policy=off|button|autoskip

Vela always passes all three explicitly. The "off" defaults below are therefore
unreachable in a real launch; they exist so that loading this script by hand,
or with a policy Vela never sent, cannot make the player seek without being
told to. An unrecognized value is likewise treated as off — the player is the
wrong place to guess, and Vela's settings layer has already rejected anything
invalid before launch.

Degrade, never disrupt: a missing, unreadable, or unparseable payload leaves
this script inert. It must never crash mpv or block playback.
--]]
local utils = require "mp.utils"

local options = {
    ["intro-policy"] = "off",
    ["credits-policy"] = "off",
    ["commercial-policy"] = "off",
}
require "mp.options".read_options(options, "vela-markers")

local KINDS = {
    intro = { option = "intro-policy", label = "Skip Intro" },
    credits = { option = "credits-policy", label = "Skip Credits" },
    commercial = { option = "commercial-policy", label = "Skip Commercials" },
}

local MOUSE_SECTION = "vela-markers-mouse"
local SPACE_BINDING = "vela-markers-space"

local markers = {}
-- The marker currently drawn on screen, and the one whose skip already fired.
-- `consumed` clears only when a position OUTSIDE it is observed, which is what
-- stops an autoskip from firing again when mpv clamps the seek back inside the
-- range, while still letting a deliberate seek back in re-arm the button.
local armed = nil
local consumed = nil
local button_visible = false
local overlay = nil

local function policy_for(kind)
    local entry = KINDS[kind]
    if not entry then
        return "off"
    end
    local value = options[entry.option]
    if value == "button" or value == "autoskip" then
        return value
    end
    return "off"
end

-- Read the payload, then remove it immediately whether or not it parses. A
-- crash before this point can leave only owner-private marker timings behind,
-- which the next launch's prefix prune collects.
local function load_payload()
    local path = os.getenv("VELA_MARKERS_PAYLOAD")
    if not path or path == "" then
        return {}
    end
    local file = io.open(path, "r")
    if not file then
        return {}
    end
    local body = file:read("*a")
    file:close()
    os.remove(path)
    if not body or body == "" then
        return {}
    end
    local parsed = utils.parse_json(body)
    if type(parsed) ~= "table" or type(parsed.markers) ~= "table" then
        return {}
    end
    -- Vela normalized these already; re-check anyway, because this arrived as
    -- a file and a half-written one must not produce a nonsense seek.
    local loaded = {}
    for index, entry in ipairs(parsed.markers) do
        if type(entry) == "table"
            and KINDS[entry.kind]
            and type(entry.start_ms) == "number"
            and type(entry.end_ms) == "number"
            and entry.end_ms > entry.start_ms
        then
            loaded[#loaded + 1] = {
                key = index,
                kind = entry.kind,
                start_s = entry.start_ms / 1000,
                end_s = entry.end_ms / 1000,
            }
        end
    end
    return loaded
end

local function osd_size()
    local dimensions = mp.get_property_native("osd-dimensions")
    if type(dimensions) ~= "table" then
        return nil
    end
    local w, h = dimensions.w, dimensions.h
    if type(w) ~= "number" or type(h) ~= "number" or w < 1 or h < 1 then
        return nil
    end
    return w, h
end

-- The hitbox is computed first and the box is DRAWN to it, rather than drawing
-- text and guessing its extent afterwards: ASS gives no text metrics, and a
-- button whose clickable area disagrees with its pixels is worse than no
-- button. OSD space is window pixels, so these coordinates are also what the
-- E2E harness clicks.
local function button_rect(w, h)
    local width = math.max(200, math.floor(w * 0.20))
    local height = math.max(40, math.floor(h * 0.07))
    local right = w - math.floor(w * 0.03)
    local bottom = h - math.floor(h * 0.07)
    return {
        x1 = right - width,
        y1 = bottom - height,
        x2 = right,
        y2 = bottom,
    }
end

local function clear_button()
    if overlay then
        overlay:remove()
    end
    if button_visible then
        mp.remove_key_binding(SPACE_BINDING)
        mp.input_disable_section(MOUSE_SECTION)
    end
    button_visible = false
    mp.set_property_native("user-data/vela-markers/button-bounds", {})
    mp.set_property_native("user-data/vela-markers/active", "")
end

local function activate_skip()
    local marker = armed
    if not marker then
        return
    end
    -- Consume BEFORE seeking: a landing clamped back inside the range must not
    -- re-show the button we just dismissed.
    consumed = marker.key
    armed = nil
    clear_button()
    mp.commandv("seek", tostring(marker.end_s), "absolute+exact")
end

-- Returns whether the button actually reached the screen. The caller latches
-- `armed` only on success, so a tick that arrives before the video output has
-- published its dimensions simply retries on the next one instead of latching a
-- button nobody can see.
local function draw_button(marker)
    local w, h = osd_size()
    if not w then
        return false
    end
    if not overlay then
        overlay = mp.create_osd_overlay("ass-events")
    end
    local rect = button_rect(w, h)
    overlay.res_x = w
    overlay.res_y = h
    local label = KINDS[marker.kind].label
    local font = math.max(16, math.floor((rect.y2 - rect.y1) * 0.40))
    -- ASS colours are BGR. A dim panel with a light label, drawn as one filled
    -- rectangle plus one centred text event.
    overlay.data = table.concat({
        string.format(
            "{\\an7\\pos(0,0)\\bord0\\shad0\\1c&H1A1A1A&\\1a&H40&\\p1}m %d %d l %d %d l %d %d l %d %d{\\p0}",
            rect.x1, rect.y1, rect.x2, rect.y1, rect.x2, rect.y2, rect.x1, rect.y2
        ),
        string.format(
            "\n{\\an5\\pos(%d,%d)\\bord0\\shad0\\1c&HFFFFFF&\\fs%d}%s   (Space)",
            math.floor((rect.x1 + rect.x2) / 2),
            math.floor((rect.y1 + rect.y2) / 2),
            font,
            label
        ),
    })
    overlay:update()

    -- Bind the click to this exact rectangle so every other click in the window
    -- stays mpv's, and take SPACE only while the button is on screen so normal
    -- pause behaviour resumes the moment it clears.
    mp.input_define_section(
        MOUSE_SECTION,
        "MBTN_LEFT script-binding " .. mp.get_script_name() .. "/activate-skip",
        "force"
    )
    mp.input_enable_section(MOUSE_SECTION, "allow-vo-dragging+allow-hide-cursor")
    mp.set_mouse_area(rect.x1, rect.y1, rect.x2, rect.y2, MOUSE_SECTION)
    if not button_visible then
        mp.add_forced_key_binding("SPACE", SPACE_BINDING, activate_skip)
    end
    button_visible = true
    mp.set_property_native("user-data/vela-markers/button-bounds", rect)
    mp.set_property_native("user-data/vela-markers/active", marker.kind)
    return true
end

local function marker_at(position)
    for _, marker in ipairs(markers) do
        if position >= marker.start_s and position < marker.end_s then
            if policy_for(marker.kind) ~= "off" then
                return marker
            end
        end
    end
    return nil
end

local function on_position(_, position)
    if type(position) ~= "number" or #markers == 0 then
        return
    end
    local marker = marker_at(position)
    if not marker then
        -- Outside every actionable range: this is the observation that lets a
        -- consumed marker be armed again on a later seek back into it.
        consumed = nil
        if armed or button_visible then
            armed = nil
            clear_button()
        end
        return
    end
    if consumed and consumed ~= marker.key then
        consumed = nil
    end
    if consumed == marker.key then
        return
    end
    local policy = policy_for(marker.kind)
    if policy == "autoskip" then
        consumed = marker.key
        armed = nil
        clear_button()
        mp.commandv("seek", tostring(marker.end_s), "absolute+exact")
        mp.osd_message("Skipped " .. marker.kind, 1.5)
        return
    end
    if armed ~= marker and draw_button(marker) then
        armed = marker
    end
end

local function reset()
    armed = nil
    consumed = nil
    clear_button()
end

markers = load_payload()

-- Observable load marker. Vela injects this script only when it has at least
-- one usable marker, so the E2E harness asserts this property rather than
-- inferring the script resolved from a seek that may have other causes.
mp.set_property_native("user-data/vela-markers/loaded", #markers > 0)
mp.set_property_native("user-data/vela-markers/active", "")
mp.set_property_native("user-data/vela-markers/button-bounds", {})

if #markers > 0 then
    mp.add_key_binding(nil, "activate-skip", activate_skip)
    mp.observe_property("time-pos", "number", on_position)
    -- Re-render against the new geometry so the drawn box and its hitbox stay
    -- the same rectangle when the window is resized or fullscreened.
    mp.observe_property("osd-dimensions", "native", function()
        if armed then
            draw_button(armed)
        end
    end)
    mp.register_event("end-file", reset)
end
