# Probes

Throwaway programs that answer a question before product code is written,
run on GitHub's hosted macOS and Windows runners by the manual workflow
`.github/workflows/platform-probe.yml` (`gh workflow run platform-probe.yml
-f probe=<name> -f os=<windows-latest|macos-latest>`). Each probe is a
directory with a `run.sh`; whatever it writes to `$PROBE_OUT` is kept as the
run's artifact. Nothing here is built into Vela or shipped.

They exist for the macOS and Windows slices of the embedded player
(`.agents/plans/embedded-player.md`, amendment of 2026-11-16), whose log
records each probe's question and result. A probe is deleted once its
answer is in that log and product code has replaced it.

- `screenshot`: can a hosted runner show a window and take a picture of
  the screen? Every later visual check on these platforms depends on it.
- `m0-embed`: can macOS mpv `wid`/macvk draw beneath transparent Wry HTML, with `gpu-next` or `gpu`?
