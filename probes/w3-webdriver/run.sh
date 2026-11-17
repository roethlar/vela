#!/bin/bash
# Probe W3: why does the Windows end-to-end suite get no WebDriver session?
# The steps are PowerShell (run.ps1).
exec pwsh -NoProfile -NonInteractive -File "$(dirname "$0")/run.ps1"
