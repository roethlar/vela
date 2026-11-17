#!/bin/bash
# Probe M2, bisection variant: every draw cleared to solid red after mpv
# renders (probes/m2-vela/hook.py, M2_VARIANT): does Core Animation present
# what Vela draws at all?
export M2_VARIANT=red
exec bash "$(dirname "$0")/../m2-vela/run.sh"
