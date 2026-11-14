# Codereview 2026-11-14 — the player window work (mpv window state, size, and the Player window setting)

Outcome: three codereview codex hunts over `f1e0c9c..9815e10`, one per
slice of `.agents/plans/mpv-window-geometry.md`: **findings** in each,
seven candidates in all, seven admitted (`pw-1` to `pw-6` MEDIUM, `pw-7`
LOW), none declined.

## Dispatch record

- Owner's dispatch word, verbatim (2026-11-12, the session goal):
  "codereview slices with codex". This is the generation half: a defect
  hunt over a pinned landed range.
- The three slices landed as ONE commit (the plan's log says why), so the
  three hunts have the same range. Each was given one slice as its
  mandate, told that the other two are reviewed separately, and told to
  report a defect where its slice meets another.
- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, CLI transport, headless one-shot, stdin
  closed: `codex exec --sandbox workspace-write --json --output-schema
  <schema> -o <outfile> <prompt>`. Called plainly, with no model and no
  effort argument, per `.agents/machines.md`. The `--json` stream names no
  model; the host's `~/.codex/config.toml` reads `model = "gpt-6-astra"`,
  `model_reasoning_effort = "max"`, so that is what a plain call resolves
  to on `continuum`.
- Run from `~/dev/vela-probe-wt`, a detached `git worktree` of this clone
  checked out at the head SHA, and declared disposable in the prompt; one
  hunt at a time.
- Tier: standard. T1 evaluated against the range before dispatch: no
  changed path matches a sensitive-path glob; the repo has no
  `.agents/review/sensitive-paths` override.
- **Slice 1** (every play inherits the last observed window state): thread `01a1d8fb-78ad-7930-a26b-1d732fe9afc7`; the event stream ends
  in `turn.completed`; dispatched 2026-11-14 07:35 UTC, finished
  07:44 UTC. Verdict `findings`, 2.
- **Slice 2** (the windowed size is carried): thread `01a1d903-e3f5-77f0-b071-fd0c783f33b4`; the event stream ends
  in `turn.completed`; dispatched 2026-11-14 07:44 UTC, finished
  07:52 UTC. Verdict `findings`, 1.
- **Slice 3** (the three-way setting and the saved window): thread `01a1d90b-53fc-77f1-9a6f-9935e6aa9faf`; the event stream ends
  in `turn.completed`; dispatched 2026-11-14 07:52 UTC, finished
  08:01 UTC. Verdict `findings`, 4.
- A first dispatch of the three, made a minute earlier with a base one
  commit too early (`058a900`, which put an unrelated documentation commit
  inside the range), was stopped by the orchestrator about a minute into
  the first hunt and replaced by these. It had produced no verdict.
- The prompts said that window position is left out on purpose, that the
  embedded player must neither receive window arguments nor contribute a
  window, that four behaviours recorded in the plan's log are the owner's
  to judge, and that no mpv window can be drawn in the reviewer's sandbox,
  so the two end-to-end scenarios are to be read.

## Reviewed range

- base: `f1e0c9cce251f7555d714ad85341667e899f832e`
- head: `9815e101825671509b6c196c1bb0213d5cdfa888`

One commit, `9815e10`.

## Acceptance (computed by the orchestrator)

For each of the three: the payload matches the schema; `verdict` is
`findings` and `findings` is non-empty; `reviewed_sha` and `base_sha` equal
the dispatched pins; `capability_ok` is literally `true`. Accepted as
valid findings verdicts.

Payloads, verbatim.

Slice 1:

```json
{"verdict":"findings","capability_ok":true,"reviewed_sha":"9815e101825671509b6c196c1bb0213d5cdfa888","base_sha":"f1e0c9cce251f7555d714ad85341667e899f832e","findings":[{"title":"Revalidate the compatibility display after stream resolution","evidence":"src-tauri/src/commands.rs:4912 samples the display for ranking at :5189, before stream resolution. Window placement samples it again at :5446. If the previous mpv moves screens or delivers a late display event during resolution, these snapshots disagree. A temporary Rust probe reproduced selection of 2160p for DP-1 followed by placement on 1080p HDMI-1.","predicted_failure":"A manual replacement opens on the newly observed screen with a copy ranked for the old screen, producing an inappropriate resolution or HDR choice and potentially unnecessary bandwidth use.","severity":"MEDIUM","better_approach":"Compare the ranking target with the post-resolution display snapshot and reselect/re-resolve when it changes. Keep window inheritance sampling after resolution."},{"title":"Choose the compatibility display using the upcoming player","evidence":"src-tauri/src/commands.rs:4893 accepts every previous external-player record without considering the upcoming player, which is resolved later at :5389. In a run initialized for embedding, switch the player setting from external playback on DP-1 back to embedded playback with Vela on HDMI-1. Ranking retains DP-1, while src-tauri/src/player/launch.rs:913 correctly omits external placement options. A temporary Rust probe confirmed this mismatch.","predicted_failure":"The embedded play renders on Vela's screen but Prefer Compatible selects resolution/HDR for the previous external window's screen. This regresses manual embedded launches, which previously used Vela's display.","severity":"MEDIUM","better_approach":"Use Vela's display for an upcoming embedded play and inherit the external display only for an upcoming external play. Reconsider the target if fallback changes the effective player."}]}
```

Slice 2:

```json
{"verdict":"findings","capability_ok":true,"reviewed_sha":"9815e101825671509b6c196c1bb0213d5cdfa888","base_sha":"f1e0c9cce251f7555d714ad85341667e899f832e","findings":[{"title":"Invalid dimension reports preserve an invented window size","evidence":"src-tauri/src/playback.rs:160-180 combines each dimension with the previous complementary dimension, then leaves the candidate untouched when validation fails. After confirming 800x450, reporting width=1024 followed by height=89 makes snapshot() return 1024x450. A second Rust reproduction using width=1024, height=0, width=0 also returned 1024x450. Both reproductions failed their assertions.","predicted_failure":"The next play receives --geometry=1024x450 even though that size was never reported as a complete pair. An invalid resize or shutdown sequence overwrites the valid carried size and can persist across restarts in Remember mode. Playback continues, but window-size continuity is corrupted.","severity":"MEDIUM","better_approach":"Track coherent dimension pairs, for example through osd-dimensions, and retain the last valid pair when an invalid report arrives. Add regression tests starting from an existing size, with valid and invalid dimensions arriving in either order."}]}
```

Slice 3:

```json
{"verdict":"findings","capability_ok":true,"reviewed_sha":"9815e101825671509b6c196c1bb0213d5cdfa888","base_sha":"f1e0c9cce251f7555d714ad85341667e899f832e","findings":[{"title":"Exit performs an unbounded synchronous config write","evidence":"src-tauri/src/lib.rs:514 calls remember_window_on_exit directly from the Tauri exit callback. src-tauri/src/commands.rs:4307-4311 synchronously loads and updates config; storage.rs:396-414 waits on both locks and performs filesystem writes without a deadline. A probe holding config.lock confirmed that this call returns only after the lock is released.","predicted_failure":"Quitting with a changed window freezes the event loop until config persistence finishes. A stalled config writer or filesystem can prevent Vela from exiting indefinitely.","severity":"MEDIUM","better_approach":"Perform persistence on a blocking worker with a bounded shutdown protocol. Keep the event loop responsive and allow exit when the persistence deadline expires."},{"title":"Captured window snapshots can overwrite newer saved state","evidence":"src-tauri/src/commands.rs:5740-5750 checks the latest session before acquiring config's write locks, then writes the captured tuple without rechecking. An interleaving probe captured play A, published and saved play B, then resumed A's update: A overwrote B. The exit path at commands.rs:4322-4338 likewise overwrote a newer final observation saved by that play's end callback.","predicted_failure":"Remember mode can persist an older play's window or an earlier snapshot of the current play. The next run consequently opens with stale fullscreen, screen or size values.","severity":"MEDIUM","better_approach":"Check the session identity and sample its observation inside the serialized config mutation. Coordinate end and exit persistence so an earlier snapshot cannot replace a later one, without holding the observation lock across filesystem work."},{"title":"An embedded play makes the first-window setting apply again","evidence":"src-tauri/src/commands.rs:7104 replaces the sole window record for every successful play, including embedded plays. opening_window at commands.rs:4349-4355 treats that embedded record as no previous external window. A probe using fullscreen mode, an external play left windowed, then an embedded play returned fullscreen=true for the next external play.","predicted_failure":"An external–embedded–external sequence discards the earlier external window and reapplies the setting to a window that is not the run's first external window.","severity":"MEDIUM","better_approach":"Preserve the last external window separately from the latest-play metadata. Consult first_window_of_run only when no external window has been published during this run."},{"title":"A failed setting save leaves the unsaved mode selected","evidence":"src/lib/Settings.svelte:664 binds the selection directly to playerWindowMode, while the rejection handler at lines 428-431 only sets err. Executing the save function with a rejected setter left 'fullscreen' displayed although the backend still held 'remember'.","predicted_failure":"After a failed save, the control continues showing a mode the backend does not hold. Playback follows the previous mode until a successful save; an error is shown, so this is a UI consistency defect rather than data corruption.","severity":"LOW","better_approach":"Restore or reload the last confirmed mode after rejection. Alternatively, explicitly distinguish the unsaved selection from the active backend value."}]}
```

## Intake

Each candidate was checked against the code by the coordinating agent
before it was admitted.

| Candidate | From | Verdict | Why |
| --- | --- | --- | --- |
| Revalidate the compatibility display after stream resolution | Slice 1 | ADMITTED as `pw-1` | Read: `playback_compatibility_target` samples the record before resolution; the launch samples it again after. Before this work the same gap existed for an automatic successor; every play now inherits, so every play has it. |
| Choose the compatibility display using the upcoming player | Slice 1 | ADMITTED as `pw-2` | Read: `compatibility_display` looks at the record's player, not at the coming play's. |
| Invalid dimension reports preserve an invented window size | Slice 2 | ADMITTED as `pw-3` | Read: `WindowTracker::observe_size` forms the candidate from the new dimension and the old other one, and returns without touching the candidate when the pair is not usable. |
| Exit performs an unbounded synchronous config write | Slice 3 | ADMITTED as `pw-4` | Read: `lib.rs` calls `remember_window_on_exit` directly in the exit path; it calls `config::load_config` and `config::update`. The line before it, the wait for embedded plays, has a two-second limit; this has none. |
| Captured window snapshots can overwrite newer saved state | Slice 3 | ADMITTED as `pw-5` | Read: both the end-of-play path and the exit path capture first and write afterwards without looking again. |
| An embedded play makes the first-window setting apply again | Slice 3 | ADMITTED as `pw-6` | Read: `opening_window` falls back to the setting whenever the one record is not an external play's. The plan's log said the setting applies again only "after embedded plays only"; this is an external play, then an embedded one, then an external one. |
| A failed setting save leaves the unsaved mode selected | Slice 3 | ADMITTED as `pw-7` | Read: `savePlayerWindowMode` sets the value from the backend's answer on success and leaves it as selected on failure. |

Per-finding records: `.agents/review/findings/pw-1.md` to `pw-7.md`.

## Notes

- All seven are in product code that is on `main` as Vela 1.0.64 and has
  not been released or tagged.
- `pw-1`, `pw-2` and `pw-6` are in what the work added beyond the plan
  (the Prefer Compatible screen; the rule that an embedded play hands on
  no window). `pw-2` and `pw-6` need the embedded player, which is hidden.
- `pw-4` and `pw-5` are in the save at exit, which was itself a repair
  made after the helpers' attack on the slice.
- None of the seven was among the 24 findings the helpers raised before
  landing.
- Not covered by the reviewer: a real mpv window (none can be drawn in its
  sandbox), and the end-to-end suite, which it read.
