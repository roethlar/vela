# Codereview 2026-11-17 — Slices N1 and W1 (NVIDIA offered; end-to-end on weston)

The embedded player plan's amendment "2026-11-17 (2)", approved by the
owner the same day: one codex hunt per slice, as the amendment asks.

## N1: Vela's window offered on NVIDIA's proprietary driver

Outcome: a codereview codex hunt over `d7b8642..daa182c`: **clean**, no
findings.

- Why: the range lands product code the working agent wrote (the start-up
  decision's NVIDIA refusal removed: `startup.rs`, `select.rs`; `main.rs`'s
  comment; the documents' way back if Vela's window fails there).
- Reviewer: codex / host default / host default / standard, called plainly
  as in `codereview-2026-11-16-platforms.md`, in a disposable worktree at
  the head SHA (`~/dev/vela-agent-keep/wt/rv-final-n1`); schema-valid,
  `capability_ok` true, both pins echoed. Brief: the scratchpad's
  `review-final/add-n1.py`, slice `n1`. That script also took back the X11
  wording an earlier slice (`fix15`, overlay mode) had put into the
  brief's description of Vela.

## W1: Linux's end-to-end runs on weston only

Outcome: a codereview codex hunt over `daa182c..9d44c5e`: **findings**,
one candidate, admitted (LOW).

- Why: the range lands test-harness code the working agent wrote (the
  venue in `tests/e2e/run.mjs`, `playerfallback`'s new premise,
  `markerbutton`'s video output, the harness test that pins it).
- Reviewer: as for N1, in `~/dev/vela-agent-keep/wt/rv-final-w1`;
  schema-valid, `capability_ok` true, both pins echoed. Brief:
  `review-final/add-w1.py`, slice `w1`.

| ID   | Severity | Finding (one line)                                              | Status | Commit |
|------|----------|------------------------------------------------------------------|--------|--------|
| w1-1 | LOW      | Two comments in `display.rs` still call the end-to-end venue Xvfb | `[x]` | the commit after `9d44c5e` |

### w1-1

Reviewer, verbatim, evidence: "src-tauri/src/display.rs:578-579 and
:1636-1637 still describe the current E2E venue as Xvfb with an empty
WAYLAND_DISPLAY. tests/e2e/run.mjs:419 now sets a nonempty Weston socket
for both Linux suites." Predicted failure: "For ordinary runs from a
Wayland session, these comments incorrectly describe compositor-output
discovery as skipped, although platform_outputs now queries Weston. This
misdocuments the path exercised by E2E; the defect is documentation-only."
Verified: both comments say so, and the working agent's own search for
Xvfb had covered `tests/`, the docs and the scripts but not `src-tauri/`.
Admitted.

Repaired: both comments drop the venue; the empty-display check stays, as
the reviewer advised. `src-tauri/src/player/launch.rs`'s "probed with mpv
0.41 on Xvfb" is left: it records where a behaviour was observed, not
what the venue is.
