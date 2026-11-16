#!/bin/bash
# Probe W2: does Vela's own embedded player work on Windows? The same probe as
# M2 (probes/m2-vela: the hook, the page script, the mock server), on a hosted
# Windows runner; the steps are PowerShell (run.ps1).
exec pwsh -NoProfile -NonInteractive -File "$(dirname "$0")/run.ps1"
