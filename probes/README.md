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
- `w0-embed`: compare mpv in a tao HWND versus a child HWND beneath a transparent WebView2 page on Windows.
- `m1-render`: can macOS mpv's OpenGL render API draw in a CAOpenGLLayer beneath transparent Wry HTML, with accelerated or software OpenGL?
- `m2-vela`: does Vela's own embedded player work on a Mac? A debug Vela
  with a probe-only hook (`hook.py`, applied on the runner, never committed)
  plays the end-to-end suite's mock Jellyfin clip in its own window, twice,
  driven by a script in its page (`drive.js`), with screenshots; the set
  comes from CI's newest successful run on main. Hosted Macs give the
  software OpenGL renderer and no EDR screen, so mpv gets `--profile=fast`
  (as the probe's own Advanced option) to keep up. The hook also prints
  counters for the macOS display chain and, at start, why a settings file
  was unavailable and (Windows) the ACL the privacy check read.
  `m2-nofmt`, `m2-nocs` and `m2-red` are bisection variants of it
  (`M2_VARIANT`: no contents format; no colour space or EDR flag; every
  draw cleared to red).
- `w2-vela`: the same on a hosted Windows runner (`run.ps1`; M2's hook,
  page script and mock server), where the end-to-end suite cannot yet start
  a WebDriver session. Hosted Windows draws Direct3D 11 with WARP.
- `libmpv-macos`: build the pinned macOS 11+ universal set on `macos-latest`, load/play/audit both architectures (Intel under Rosetta), and retain build/configure/check logs, MANIFEST and every dylib's `otool -L`. Budget estimate: 30–60 minutes cold, unmeasured; the workflow allows 170 minutes.
