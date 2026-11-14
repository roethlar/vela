--[[
Vela's black-bar cropping for a player inside Vela's own process.

Nothing here is upstream mpv code. Vela-authored, MIT, like the rest of the
repo — see PROVENANCE.md.

What it does: on request, it finds the picture inside the black bars with
FFmpeg's `bbox` filter, then crops to it with mpv's `video-crop`. A second
request removes the crop. In automatic mode it measures once by itself, a
settle delay after playback starts (fresh and resumed plays alike).

Options (explicit identifier: mpv would otherwise derive "vela_crop" from the
filename and silently ignore the dashed form Vela passes):
  --script-opts-append=vela-crop-auto=yes|no   (default no: crop only on request)
  --script-opts-append=vela-crop-delay=<seconds>  (default 5)

The default for `auto` is "no" on purpose: a launch that lost its option must
never turn a crop the user asks for into one that happens by itself.

Binding: `script-binding vela_crop/toggle`. Removes a crop in effect, else
measures and crops. Ignored while a measurement runs.

State, for whoever shows a crop control: `user-data/vela-crop/state` is one
of `off`, `measuring`, `on`.

It refuses to measure, and leaves the picture alone, for an image, for a file
with too little left to play, for a pixel format it cannot name, and for HDR
video (PQ or HLG transfer, Dolby Vision, or no video parameters to tell):
changing `video-crop` while HDR plays has wedged the player before, and here
the player is Vela itself. HDR is refused with a short message on screen.

Degrade, never disrupt: anything that goes wrong leaves the picture uncropped
and the player as it was.
--]]

local settings = {
    auto = false,
    -- Seconds between the start of playback and the automatic measurement.
    delay = 5,
}
require "mp.options".read_options(settings, "vela-crop")

-- How long boxes are collected, counted from the first one that arrives.
local GATHER_SECONDS = 1
-- The longest a measurement may take from the filter's insertion. Switching
-- the decoder can hold back the first box for a while.
local MEASURE_CAP_SECONDS = 4
-- The bbox filter's label, which also names its metadata property.
local FILTER_LABEL = "vela-crop"
local STATE_PROPERTY = "user-data/vela-crop/state"

-- What this script has changed, so that it can put it back.
local cropped = false
local measuring = false
-- The `hwdec` value to restore after a measurement, or nil when untouched.
local saved_hwdec = nil
local filter_inserted = false
-- The union of every box seen in this measurement, or nil before the first.
local union = nil
local frame_w, frame_h = nil, nil
local gather_timer = nil
local cap_timer = nil
local delay_timer = nil
-- Automatic mode: measure as soon as playback is no longer paused.
local waiting_for_unpause = false
local delay_started = false

local function publish(state)
    mp.set_property_native(STATE_PROPERTY, state)
end

local function kill(timer)
    if timer then
        timer:kill()
    end
    return nil
end

-- Bits per sample for the pixel formats mpv names, nil for anything else.
local EIGHT_BIT = {
    nv12 = true, nv21 = true, nv16 = true, nv24 = true, nv42 = true,
    yuv410p = true, yuv411p = true, yuv420p = true, yuv422p = true,
    yuv440p = true, yuv444p = true,
    yuvj420p = true, yuvj422p = true, yuvj440p = true, yuvj444p = true,
    gray = true,
}
local SEMI_PLANAR_HIGH = {
    p010 = 10, p012 = 12, p016 = 16,
    p210 = 10, p212 = 12, p216 = 16,
    p410 = 10, p412 = 12, p416 = 16,
}

local function bit_depth(format)
    if type(format) ~= "string" then
        return nil
    end
    if EIGHT_BIT[format] then
        return 8
    end
    if SEMI_PLANAR_HIGH[format] then
        return SEMI_PLANAR_HIGH[format]
    end
    local planar = format:match("^yuvj?4[0-4][0-4]p(%d+)$") or format:match("^gray(%d+)$")
    local depth = tonumber(planar)
    if depth and depth >= 8 and depth <= 16 then
        return depth
    end
    return nil
end

local function is_hdr(params)
    local transfer = params.gamma
    if transfer == "pq" or transfer == "hlg" then
        return true
    end
    if params.colormatrix == "dolbyvision" then
        return true
    end
    -- A Dolby Vision stream can arrive with an ordinary transfer tag.
    return mp.get_property_native("current-tracks/video/dolby-vision-profile") ~= nil
end

-- Why a measurement cannot be made now, or nil when it can. Returns the
-- message to show as the second value when the user should be told.
local function refusal()
    local track = mp.get_property_native("current-tracks/video")
    if type(track) ~= "table" or track.image or track.albumart then
        return "no video"
    end
    local remaining = mp.get_property_number("playtime-remaining")
    if not remaining or remaining < GATHER_SECONDS + 1 then
        return "too little left to play"
    end
    local params = mp.get_property_native("video-params")
    if type(params) ~= "table" then
        return "no video parameters"
    end
    if is_hdr(params) then
        return "HDR", "Cropping is not available for HDR video"
    end
    local format = params["hw-pixelformat"] or params.pixelformat
    if not bit_depth(format) then
        return "unknown pixel format"
    end
    if type(params.w) ~= "number" or type(params.h) ~= "number" or params.w < 2 or params.h < 2 then
        return "no frame size"
    end
    return nil
end

-- Defined below; the metadata observer and the timers end a measurement.
local finish_measurement

local function on_metadata(_, metadata)
    if not measuring or type(metadata) ~= "table" then
        return
    end
    local x1 = tonumber(metadata["lavfi.bbox.x1"])
    local y1 = tonumber(metadata["lavfi.bbox.y1"])
    local x2 = tonumber(metadata["lavfi.bbox.x2"])
    local y2 = tonumber(metadata["lavfi.bbox.y2"])
    if not (x1 and y1 and x2 and y2) or x2 < x1 or y2 < y1 then
        return
    end
    if union then
        union.x1 = math.min(union.x1, x1)
        union.y1 = math.min(union.y1, y1)
        union.x2 = math.max(union.x2, x2)
        union.y2 = math.max(union.y2, y2)
        return
    end
    union = { x1 = x1, y1 = y1, x2 = x2, y2 = y2 }
    -- The window opens with the first box.
    gather_timer = mp.add_timeout(GATHER_SECONDS, function()
        gather_timer = nil
        finish_measurement()
    end)
end

-- Take the filter out and give the decoder back what it had. Safe to call in
-- any state.
local function undo_measurement()
    measuring = false
    gather_timer = kill(gather_timer)
    cap_timer = kill(cap_timer)
    mp.unobserve_property(on_metadata)
    if filter_inserted then
        mp.commandv("vf", "remove", "@" .. FILTER_LABEL)
        filter_inserted = false
    end
    if saved_hwdec then
        mp.set_property("hwdec", saved_hwdec)
        saved_hwdec = nil
    end
    union = nil
end

-- The rectangle to crop to, or nil when the boxes do not describe bars worth
-- removing. Edges are widened to even pixel positions, inside the frame.
local function crop_rectangle()
    if not union then
        return nil
    end
    local left = union.x1 - union.x1 % 2
    local top = union.y1 - union.y1 % 2
    local right = math.min(frame_w, union.x2 + 1 + (union.x2 + 1) % 2)
    local bottom = math.min(frame_h, union.y2 + 1 + (union.y2 + 1) % 2)
    local w = right - left
    local h = bottom - top
    w = w - w % 2
    h = h - h % 2
    if w >= frame_w and h >= frame_h then
        return nil -- the whole frame: no bars
    end
    if w * 2 < frame_w or h * 2 < frame_h then
        return nil -- less than half the frame: not bars, a dark scene
    end
    return string.format("%dx%d+%d+%d", w, h, left, top)
end

finish_measurement = function()
    if not measuring then
        return
    end
    local rectangle = crop_rectangle()
    local switched = saved_hwdec ~= nil
    if not rectangle then
        undo_measurement()
        publish("off")
        return
    end
    -- This order matters. Changing `video-crop` alone can stop playback in
    -- its tracks; what follows it (the filter's removal, the decoder's change
    -- back, or else a seek to where playback already is) gets it going again.
    mp.set_property("video-crop", rectangle)
    cropped = true
    undo_measurement()
    if not switched then
        mp.command("no-osd seek 0 relative+exact")
    end
    publish("on")
end

local function start_measurement()
    local reason, message = refusal()
    if reason then
        if message then
            mp.osd_message(message, 3)
        end
        return
    end
    local params = mp.get_property_native("video-params")
    local depth = bit_depth(params["hw-pixelformat"] or params.pixelformat)
    frame_w, frame_h = params.w, params.h
    measuring = true
    union = nil
    publish("measuring")

    -- bbox sees nothing when frames stay on the GPU: mpv drops the filter
    -- without a word. Copy-back and software decoding hand it real frames.
    local current = mp.get_property("hwdec-current")
    if current and current ~= "" and current ~= "no" and not current:match("%-copy$") then
        saved_hwdec = mp.get_property("hwdec")
        mp.set_property("hwdec", "no")
    end

    -- bbox compares raw samples with min_val, so the threshold for "black"
    -- grows with the bit depth.
    local min_val = math.floor(24 * 2 ^ (depth - 8))
    local filter = string.format("@%s:lavfi=[bbox=min_val=%d]", FILTER_LABEL, min_val)
    if not mp.commandv("vf", "add", filter) then
        undo_measurement()
        publish("off")
        return
    end
    filter_inserted = true
    mp.observe_property("vf-metadata/" .. FILTER_LABEL, "native", on_metadata)
    -- No box at all by then means the filter is not working: nothing is cropped.
    cap_timer = mp.add_timeout(MEASURE_CAP_SECONDS, function()
        cap_timer = nil
        finish_measurement()
    end)
end

local function cancel_automatic()
    delay_timer = kill(delay_timer)
    waiting_for_unpause = false
end

local function toggle()
    if measuring then
        return
    end
    cancel_automatic()
    if cropped then
        mp.set_property("video-crop", "")
        cropped = false
        publish("off")
        return
    end
    start_measurement()
end

local function automatic_measurement()
    delay_timer = nil
    if measuring or cropped then
        return
    end
    if mp.get_property_native("pause") then
        -- No frames reach the filter while paused; wait for playback.
        waiting_for_unpause = true
        return
    end
    start_measurement()
end

local function on_pause(_, paused)
    if paused == false and waiting_for_unpause then
        waiting_for_unpause = false
        automatic_measurement()
    end
end

local function on_playback_restart()
    if not settings.auto or delay_started then
        return
    end
    delay_started = true
    local delay = tonumber(settings.delay) or 5
    if delay < 0 then
        delay = 0
    end
    delay_timer = mp.add_timeout(delay, automatic_measurement)
end

-- A file that ends, or a new one that starts, leaves nothing of a measurement
-- behind.
local function cleanup()
    cancel_automatic()
    local was_measuring = measuring
    undo_measurement()
    delay_started = false
    if was_measuring then
        publish(cropped and "on" or "off")
    end
end

-- A new file starts uncropped: the last file's rectangle is not its picture.
-- In automatic mode it is then measured on its own.
local function new_file()
    cleanup()
    if cropped then
        mp.set_property("video-crop", "")
        cropped = false
        publish("off")
    end
end

publish("off")
mp.add_key_binding(nil, "toggle", toggle)
mp.observe_property("pause", "bool", on_pause)
mp.register_event("playback-restart", on_playback_restart)
mp.register_event("end-file", cleanup)
mp.register_event("start-file", new_file)
