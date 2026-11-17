#!/bin/bash
# Probe M2, bisection variant: the layer without its contents format
# (probes/m2-vela/hook.py, M2_VARIANT).
export M2_VARIANT=nofmt
exec bash "$(dirname "$0")/../m2-vela/run.sh"
