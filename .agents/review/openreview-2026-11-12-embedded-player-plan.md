# Openreview 2026-11-12 — embedded player plan

Outcome: openreview codex (gpt-6-astra @ max, grade not declared; inline,
session-only) over `c692305..cf35ddf`: **acceptable_with_changes**.
Material changes: a startup-safe idle policy for the embedded player;
session-scoped player-view lifecycle events; allow-listed bundled-script
actions for skip and manual autocrop. None is adopted yet: each waits for
the owner's ruling.

## Dispatch record

- Owner's dispatch word, verbatim: "see if you can do a review with codex.
  you might need to su michael to get it to load since the settings aren't
  in the root homedir. do it when there's something to review, whatever
  that is." The agent chose openreview because the thing to review was a
  plan.
- Harness: codex-cli 0.167.2, CLI transport, headless one-shot, stdin
  closed, run from the repo root on host `continuum` as user `michael`
  (the orchestrating session was root):
  `codex exec --sandbox workspace-write --json --output-schema <schema>
  -o <outfile> <prompt>`.
- Model and effort: no override was passed. The host's
  `~/.codex/config.toml` reads `model = "gpt-6-astra"`,
  `model_reasoning_effort = "max"`. The `--json` event stream carries no
  model field, so this records the configured pair, not a
  transcript-confirmed one.
- Grade: none declared for this pair; no machine-local cache was consulted.
- Thread `01a1d2ee-a1c5-78f2-8b82-a45c64972792`; exit 0; 26 tool calls
  (through the host's `ptk` shell hook), 5 web searches.

## Reviewed range

- base: `c692305648d79616eb3a7b3e563eeec385d2e198`
- head: `cf35ddfa8c25013002f0348f3b3eb0bcd4021325`

The range is the single commit that recorded the both-players ruling and
drafted `.agents/plans/embedded-player.md`. The prompt was the playbook's
neutral question plus the coordinates, the isolation rules and the verdict
schema; nothing about the plan's reasoning was supplied.

## Acceptance (computed by the orchestrator)

Exit 0; payload matches the schema; `verdict` in the enum; `reviewed_sha`
and `base_sha` equal the dispatched SHAs; `capability_ok` literally `true`;
`material_changes` non-empty with `acceptable_with_changes`. Accepted. The
main worktree was unchanged afterwards and `git worktree list` showed only
the main tree.

## Verdict payload

- goal: "Record the decision to keep external mpv and plan an embedded libmpv player, initially for Linux/Wayland, preserving existing playback behavior while supporting video inside Vela."
- recommended_approach: "Keep the shared launch description, separate launchers, and existing JSON-IPC trackers. Validate embedded startup and shutdown first, define session-scoped view lifecycle events, and route bundled-script controls through their existing handlers. Gate implementation on the outstanding owner decisions and release on behavioral parity."
- comparison: "The architecture fits the repository and avoids duplicating playback tracking. The probe-first rollout is appropriate, but the specified startup, view lifecycle, and skip-control contracts need correction. This range changes documentation only. The diff check and four focused existing tests passed; isolated libmpv and Lua probes confirmed the startup and skip-state issues. Full application builds and E2E were not run. The disposable worktree was removed."
- material_changes:
  - "Give embedded playback a startup-safe idle policy while preserving shutdown at EOF."
  - "Separate session-scoped player-view lifecycle events from existing watch-state refresh notifications."
  - "Add allow-listed bundled-script actions that preserve marker state and manual autocrop controls."

## Findings intake

Three candidates, three admitted, none declined. Each is a defect in the
plan's text, so its fix is a plan revision, which waits for the owner's
ruling on the matching material change.

### ep-1 (HIGH) — Keep libmpv alive until the first loadfile

- evidence: .agents/plans/embedded-player.md:150 initializes libmpv before attaching the render surface and sending loadfile; .agents/plans/embedded-player.md:170 specifies idle=no. A local libmpv 0.41 probe using that sequence received shutdown before loadfile.
- predicted failure: Embedded startup can terminate while the render surface is being prepared, before playback begins.
- better approach: Use embedded idle=once, which waits initially and exits after playback, while retaining external idle=no. This matches the documented [mpv idle option](https://raw.githubusercontent.com/mpv-player/mpv/v0.41.0/DOCS/man/options.rst). The corrected probe survived delayed loading and then emitted end-file and shutdown; make both behaviors explicit acceptance tests.
- intake: ADMITTED. Reproduced by the orchestrator with the spike program: `stage1 -o idle=no` rendered 0 frames and ended at once; `stage1 -o idle=once` played normally (97 frames in 4 s).

### ep-2 (HIGH) — Do not use playback-ended to close the embedded view

- evidence: .agents/plans/embedded-player.md:267 brackets the view with player-opened and existing playback-ended events. src-tauri/src/commands.rs:5105 emits playback-ended without a session ID; src-tauri/src/lib.rs:284 emits it again after advance_playlist at src-tauri/src/lib.rs:257.
- predicted failure: A completed video's delayed refresh can close the successor's embedded view during playlist advancement or replacement playback. Closing the view can also wait unnecessarily for server bookkeeping.
- better approach: Emit player-opened/player-closed events carrying sessionId from the player lifecycle. Ignore stale-session events and retain playback-ended for watch-state refresh. Test a successor opening before its predecessor's delayed notifications arrive.
- intake: ADMITTED. Checked: `playback-ended` is emitted at `src-tauri/src/commands.rs` (in `on_end`, payload `{sourceId, itemKey}`, no session id) and again in the dispatcher in `src-tauri/src/lib.rs` after `advance_playlist`.

### ep-3 (MEDIUM) — Invoke the marker script's skip handler instead of seeking directly

- evidence: .agents/plans/embedded-player.md:277 specifies seek-absolute for the HTML skip button. src-tauri/resources/mpv-scripts/vela-markers.lua:155 implements activate_skip, which marks the range consumed and clears its active state before seeking. A Lua probe showed that a clamped raw seek leaves the marker active, while activate_skip clears it.
- predicted failure: When a seek lands inside the same marker range, the embedded skip button remains active instead of being dismissed, violating the promised parity.
- better approach: Add an allow-listed skip-active-marker action that invokes the script's existing binding. Separate marker state from OSD drawing when suppressing its button, preserve Space-to-skip behavior, and test clamped seeks through the HTML control.
- intake: ADMITTED. Checked: `vela-markers.lua` defines `activate_skip` and exposes it as the `activate-skip` key binding; the reviewer's Lua probe of the clamped-seek case was not re-run by the orchestrator.

## Limits of this review

It reviewed a plan; no product code exists. The reviewer said it ran the
diff check, four existing tests and isolated libmpv and Lua probes, and did
not build the app or run E2E.
