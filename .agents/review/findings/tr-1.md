# tr-1: Settings offers a quality control that playback ignores

**Severity**: HIGH — a shipped control promises server conversion and silently
does nothing, so the user believes they have addressed a stall they have not.
**Status**: Verified
**Branch**: none — repo policy is direct commits on `main`
**Commit**: `cdaa147` (version 1.0.16)

## Evidence

`src/lib/Settings.svelte` (Playback quality field, added in `9f87475`) at head
`b94fcd1`. The value round-trips through `set_mpv_advanced` into
`AppConfig::playback_quality`, and nothing reads it at play time: `play_item`
still resolves the stream exactly as before, and the whole capability model is
marked `#[allow(dead_code)]`.

## Predicted observable failure

Select "Convert to 720p HD — 2 Mbps", save, and play a high-bitrate title on a
constrained link or a machine that cannot decode it. Vela hands mpv the same
direct-stream URL as before, so the stall or decode failure is unchanged, with
no indication the setting did nothing.

## What

Slice 2 shipped the control and slice 3 wires it, so any build cut between them
carries a setting that lies. This is the same trap the marker work explicitly
avoided: `.agents/plans/skip-credits-intros-v2.md` slice 3 states "No Settings
controls in this slice, so no shipped UI offers a setting playback ignores."
Writing the transcoding plan I did not carry that rule across, and slice 2's
shape is wrong as a result — the defect is in the plan, not only in the code.

## Approach

Render the Playback quality control only once the play path honours it, gating
on a single constant that slice 3 removes along with the `#[allow(dead_code)]`
markers. The setting, its validation, and the command boundary all remain — only
the control is withheld, so nothing about slice 2's tested surface changes.

The plan's slice 2 is amended to say the control ships with the wiring, matching
the marker plan's rule.

## Files changed

- `src/lib/Settings.svelte` — gate the quality field.
- `.agents/plans/server-transcoding.md` — correct slice 2's shape.

## Guard proof

- `tests/transcoding-ui.test.mjs` — asserts the control sits inside the
  readiness gate while that gate is off, and that copy promising server
  conversion cannot ship ungated. Red-proven 2026-07-25 from the committed
  state: deleting the `{#if QUALITY_CONTROL_READY}` line failed the assertion
  "the quality field must be inside the readiness gate"; restoring passed.

  The test file also had to be ADDED to `package.json`'s `check` script, which
  names its test files explicitly — a new file under `tests/` is not picked up
  and would have gated nothing.

## Coder dispute (if any)

None on the defect. One scoping note: the severity depends on a release being
cut between slices, which the repo's per-slice version bumps make possible
rather than certain.

## Known gaps

None.

## Reviewer comments

`Reviewer: codex / (harness default model, default effort) / standard` — the
owner dispatched `codereview with codex, no model or effort specified`
(2026-07-25), so no model or reasoning-effort override was sent and codex's own
configured defaults applied. The harness cache still has no owner-confirmed
tier entry for codex.

Harness: codex MCP transport (`mcp__codex__codex`), read-only sandbox,
`approval-policy: never`. Reviewed head `b94fcd13ae2a6596937b57e6acdc622560e848e0`,
base `72e0f48f6c7ddeda603cea253951c4a93932e709` — both echoed back and matched
against the dispatched pins. Verdict: finding raised (HIGH), admitted at intake.
2026-07-25 UTC.

Reviewer text: "The change exposes a Playback quality selector whose help text
promises server conversion, but the selected value is only persisted and
returned to Settings; no stream-resolution or playback path reads it, while the
new provider capability code remains explicitly dead. This presents an
operational feature before any of its choices can affect playback."
