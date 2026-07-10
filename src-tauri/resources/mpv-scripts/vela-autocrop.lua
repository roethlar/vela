--[[
Vela's auto-trigger companion to the stock mpv autocrop.lua (which Vela
loads UNMODIFIED, with its own auto mode disabled via autocrop-auto=no).

Why this exists: the stock script's auto mode treats auto_delay as a
POSITION in the file, so a resumed play (--start=<seconds>) skips the
delay and fires detection immediately at file-loaded — before hwdec is
established. Its hwdec guard then reads a stale "no", non-copy-back
hardware decode engages during the cropdetect window, no metadata is
gathered, and there is no retry (probe-confirmed; see
.agents/plans/autocrop-resume.md). Patching the stock file would fork it
("if we mod it, we own that fork" — owner ruling), so the trigger lives
here instead: wait a settle delay after EVERY load — fresh or resumed —
then invoke the stock script's own detection through its public binding,
the same entry point the user's Shift+C hits. By then hwdec is real and
the stock guard works.

Options (explicit identifier — mpv would otherwise derive
"vela_autocrop" from the filename and ignore the dashed form):
  --script-opts-append=vela-autocrop-delay=<seconds>
--]]
local options = {
    -- Settle delay after file-loaded before triggering detection. Long
    -- enough for the initial seek + hwdec init; short enough to feel
    -- automatic. The stock script adds its own detect_seconds on top.
    delay = 5,
}
require "mp.options".read_options(options, "vela-autocrop")

-- Observable load marker: auto mode silently degrades to the stock trigger
-- when this shim is missing, and on software/copy-back decode that path
-- still crops — so "cropping happened" cannot prove the shim resolved. The
-- E2E harness asserts this property instead (plan-review ac-r2).
mp.set_property_native("user-data/vela-autocrop/loaded", true)

local timer = nil

local function cancel()
    if timer then
        timer:kill()
        timer = nil
    end
end

-- Manual activity wins: stock auto is off, so if video-crop becomes
-- non-empty before the timer fires, the user cropped by hand (Shift+C).
-- Cancel — otherwise a user who crops and then UN-crops inside the delay
-- window would be re-cropped over their explicit undo (the stock script
-- kills its own pending timer on toggle for exactly this reason).
mp.observe_property("video-crop", "string", function(_, value)
    if timer and value ~= nil and value ~= "" then
        cancel()
    end
end)

mp.register_event("file-loaded", function()
    cancel()
    timer = mp.add_timeout(options.delay, function()
        timer = nil
        -- Fire only into a crop-less player: toggle would UNDO an
        -- existing crop (belt to the observer's braces).
        if mp.get_property("video-crop", "") == "" then
            mp.commandv("script-binding", "autocrop/toggle_crop")
        end
    end)
end)

mp.register_event("end-file", cancel)
