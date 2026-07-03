# Plan: Letterbox/IMAX crop — detect during first playback, correct once

Status: DRAFT — not approved for implementation. The Phase 0 spike gates the
design; no product code until the spike has run and the owner approves this
plan. Decision context: `.agents/decisions.md` 2026-07-03 (direction) and
2026-06-28 (evidence: detection findings, live `video-crop` D-state wedge).

## Target behavior

- First-ever play of a file: launch immediately, uncropped. A sampled
  bar-detection scan runs concurrently against the same stream. When it
  completes, apply exactly one correction to the running player and persist
  the crop in a per-file cache.
- Every later play: cache hit → crop goes into mpv's launch arguments;
  nothing ever changes mid-play.
- Never clip real picture: the crop is the union bounding box of picture
  across samples (equivalently: the largest-picture scene wins).

## Phase 0 — render-zoom safety spike (owner at the machine)

Goal: prove whether render-only properties applied once over IPC avoid the
VO-reconfigure D-state wedge that `video-crop` triggered twice on the
gpu-next/Vulkan/Wayland/HDR stack.

Protocol:
1. Launch mpv by hand with Vela's real arg set (`--no-config`,
   `--vo=gpu-next,gpu`, `--profile=gpu-hq`, `--gpu-api=vulkan`,
   `--hwdec=auto`, `--target-colorspace-hint=yes`, `--hdr-compute-peak=yes`,
   `--input-ipc-server=/tmp/spike.sock`) on the known problem file (Plex 4K
   HDR, 2:1 picture in a 16:9 container; correct crop `3840x1920+0+120`).
2. Over the IPC socket, while playing and again while paused, set in turn:
   `video-zoom` (small steps then the target value), `panscan` 0→1,
   `video-align-y` −1→1. Repeat each transition a few times.
3. After each command: confirm video responds, mpv stays interactive, and
   `ps -o stat= -p <mpv pid>` never shows `D`. A wedge is the failure signal
   (known shape: uninterruptible I/O; even SIGKILL waits on it).
4. Judge the visual: is a one-time zoom event mid-logo acceptable on screen?
5. Do NOT touch `video-crop` — its failure mode is already established.

Outcome A (no wedge): the one-time correction is applied in place via
`video-zoom`/`panscan` (+ `video-align-y` for asymmetric bars). Note panscan
crops symmetrically; if a file's bars are asymmetric beyond what align can
compensate, fall back to Outcome B behavior for that file.

Outcome B (wedge or unusable visuals): the correction relaunches mpv at the
current position with `--video-crop=WxH+X+Y` in launch args — a fresh launch
never travels the live-crop path. (~1–2s visible restart, once per file,
first play only.)

## Phase 1 — detection engine and cache

- Sampler: a second, windowless mpv instance (`--vo=image` at reduced
  resolution into a private temp dir), so Vela gains no new hard dependency;
  remote streams reuse the same auth-header include mechanism playback uses.
  Decode N stratified timestamps across the runtime (proposed N=64, skipping
  the first/last ~2% for studio cards and credit fades), hard wall-clock cap.
- Per-frame analysis: per-row/per-column mean luma with a PQ-aware black
  threshold — mpv `cropdetect` and `osd-dimensions` are known-misleading on
  HDR (2026-06-28 evidence); per-decoded-row brightness is the proven signal.
  Union the per-sample content boxes; snap the result outward to the nearest
  standard aspect (1.33/1.78/1.85/2.0/2.2/2.35–2.40/2.76) within tolerance so
  a missed widest-scene sample can't clip picture; skip cropping entirely when
  bars are under ~2% of the frame dimension.
- Cache: `crop_cache.json` beside the config (atomic save, owner-only perms,
  same defensive persistence rules as config): key = source id + item key +
  part identity, value = crop rect + scan parameters version.
- Wiring: cache hit at `play_by_key` → extend `PlaySpec` with the crop launch
  arg. Miss → spawn the sampler off the async workers (existing
  `spawn_blocking` discipline; no shared locks held); on completion, apply
  the Phase 0 mechanism only if the same item is still playing, then persist.

## Phase 2 — integration details

- Cancel the scan when playback stops or the queue advances.
- v1 skips split-file (`edl://`) media and applies no crop there.
- Local/SMB/SSH files use the same sampler on paths; Jellyfin/Emby streams
  work once their header parity lands (or by passing their tokened URL to the
  sampler as-is, matching current exposure).
- A Settings toggle to disable the feature entirely.

## Verification

- Unit tests: detection math on synthetic frames (SDR + PQ-shaped values),
  snapping/tolerance behavior, cache round-trip, PlaySpec crop arg assembly.
- Manual playtest matrix: the known 2:1-in-16:9 HDR file (crop matches
  `3840x1920+0+120`), a uniform 2.39 file, a full-frame 16:9 file (no crop),
  a 4:3 file, plus a mid-file aspect-change file if available (IMAX-style).
- Watch for wedge regressions during every manual run (`ps -o stat=`).

## Open points to settle at approval

1. Sampler N and wall-clock cap values.
2. Outcome B only: relaunch automatically when the scan lands, or gate on a
   keypress ("reframe now")?
3. Whether the Settings toggle ships in v1 or later.
