# ec-4: an include inside a profile the user selects is applied after Vela's protected options

**Severity**: MEDIUM — the user's file can then set `idle`, after which mpv never ends at the end of a file and the play's final bookkeeping and Continue Playing never run. It needs own-configuration mode, a profile containing an include, and that profile selected in the Advanced options.
**Status**: In progress
**Branch**: —
**Commit**: `85b38ff`

## Evidence

Reviewer, verbatim: "src-tauri/src/player/embedded/options.rs:101-108 separates direct include options but batches profile selection with Vela's protected options. With own mpv.conf defining [example] containing include=/path/override.conf, Advanced --profile=example, and override.conf containing idle=yes, a headless libmpv 0.42 probe of the same include sequence returned idle=yes while vo and input-ipc-server retained Vela's values."

## Predicted observable failure

Reviewer, verbatim: "The startup verification passes, but EOF leaves mpv running indefinitely. Final playback bookkeeping and continuation do not complete."

Reviewer's suggested direction, verbatim: "Finish applying user options and their nested profiles before applying Vela's protected tail in a separate step. Test the effective values through libmpv, including profile-contained includes."

## What

The embedded player applies a launch's options as included option files. It was known (measured in Slice 3) that mpv applies an `include` written inside such a file after the rest of that file, so a direct include was already a step of its own. A profile is a second way in: `--profile=NAME` in the Advanced options, where the user's profile contains an `include`. The profile line shared an options file with the options Vela puts after the user's (`idle=once` among them), and the profile's include was applied after them. The user's file could then set `idle=yes`, and mpv would never end at the end of a file: no final check-in, no Continue Playing. The two options Vela reads back after start-up (`vo`, `input-ipc-server`) happened not to be the ones affected, so the start-up check passed.

## Approach

`options::steps` no longer puts any line of the user's into a file with anything else: every user line is a step of its own, applied by its own call into mpv, so whatever it brings in has been applied before the next option in the list is. Vela's own options next to each other still share a step. The rule deliberately does not name `profile`: which options can pull in an include is mpv's business and can grow. Measured against the shipped library (`mpv 0.42`) before and after: with the profile and Vela's tail in one file, `idle` ended as `yes`; with the profile in a step of its own, `once`.

## Files changed

- `src-tauri/src/player/embedded/options.rs` — `steps`; the module notes; two new tests and two updated.
- `tests/e2e/scenarios/embeddedplay.mjs` — the scenario's Advanced options now include a file that defines such a profile and select it; the existing check of `options/idle` in the running player is the proof through the real library.

## Guard proof

Run 2026-11-13 at `85b38ff` in a disposable worktree; `cargo +stable test --locked --lib embedded::options`, and the `embeddedplay` E2E scenario under the headless compositor against the built library set.

| Injection | Result |
| --- | --- |
| The original defect: a user line shares a file with what follows it | Rust FAIL: `a_line_of_the_users_is_a_step_of_its_own`, `no_step_holds_an_option_of_the_users_together_with_one_of_velas`, and the two order tests |
| The user's line is a step of its own, but Vela's options before it are applied after it | Rust FAIL, three tests |
| The user's line starts a new file but shares it with what follows | Rust FAIL, three tests |
| Control: the same result written another way | Rust PASS, rightly |
| The original defect, through the real library | E2E FAIL: "Vela's own idle setting stands after everything the user's options bring in" |
| Unchanged, through the real library | E2E PASS |
| Unchanged | Rust PASS (10) |

## Coder dispute (if any)

## Known gaps

- A profile with `profile-cond` is applied by mpv whenever its condition becomes true, at any time during the play, and can set any option then. That is true of the external player as well and is not something the order of a launch can govern. (For the three HDR target options it is finding `hd-2`.)
- The user's own `mpv.conf` (own-configuration mode) is applied before all of Vela's options, in calls of its own, so an include or profile inside it is finished before Vela's first option; unchanged and not re-measured here beyond the Slice 3 measurement.
- The E2E scenario runs on a developer's machine only; CI has no compositor. The unit tests are what CI holds.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `053cb36..0a63799` with `a02db80`, 2026-11-13 08:09
UTC (`.agents/review/codereview-2026-11-13-embedded-core.md`). Reviewer's
title: "Includes inside user profiles override the protected option tail". Reviewer's severity: MEDIUM.
