# ec-4: an include inside a profile the user selects is applied after Vela's protected options

**Severity**: MEDIUM — the user's file can then set `idle`, after which mpv never ends at the end of a file and the play's final bookkeeping and Continue Playing never run. It needs own-configuration mode, a profile containing an include, and that profile selected in the Advanced options.
**Status**: Verified
**Branch**: —
**Commit**: `85b38ff`, `f80bf0c`, `1b7b6b4`

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

### Verification round 1, recorded 2026-11-13 09:17 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d425-a4f4-7510-88a7-a37547400fe0`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 09:03 UTC, finished 09:13 UTC.
- Reviewed head `85b38ff70741727f467233cb8b9d7abf9b59b9d2`, base
  `e66a928eda55b107743487560d35468b395f5a2a` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/player/embedded/options.rs:134 — User content remains reachable through Vela's own profile option. Enable own configuration, define [gpu-hq] with include=/path/idle-yes.conf containing idle=yes, and leave Advanced options empty. Vela's profile=gpu-hq then shares a file with idle=once. Through the actual build_launch → steps → configure path and shipped libmpv 0.42, I measured idle=yes while vo and IPC verification passed, preserving ec-4's predicted EOF/bookkeeping failure. Isolate profile applications regardless of argument origin and guard this case.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. A valid `reopened`: not
accepted. The comment is correct, and it shows the repair drew its line in
the wrong place. The rule "every line of the user's is a step of its own"
assumed that only the user's lines can bring in the user's content. Vela's
own `profile=gpu-hq` can too: a profile of that name in the user's own
`mpv.conf` is the same profile to mpv, and Vela's line selecting it sits in
one file with Vela's protected options. No rule about whose line it is can
be right; the repair makes every option a step of its own.

### Repair (round 2), `f80bf0c` and `1b7b6b4`

`options::steps` now makes every option of a launch a step of its own,
Vela's as well as the user's, in the order of the list; an option that
includes a file is that inclusion, as before. There is no longer a rule
about whose line it is, because none can be right: a profile is whatever
the user's configuration says it is, whoever selects it. Each step is
applied by its own call into mpv (`session.rs::configure`, unchanged), so
whatever an option brings in has been applied before the next one is.

Files: `src-tauri/src/player/embedded/options.rs`; and the E2E scenarios
`tests/e2e/scenarios/embeddedownconfig.mjs` (new: the reviewer's case),
`tests/e2e/scenarios/embeddedplay.mjs`, `tests/e2e/README.md`.

Guard proof for round 2, run 2026-11-13 at `ddf475f` in a disposable
worktree: `cargo +stable test --locked --lib embedded::options`, and the two
E2E scenarios under the headless compositor against the built library set.

| Injection | Rust | `embeddedownconfig` (the reviewer's case) | `embeddedplay` (a profile the user selects) |
| --- | --- | --- | --- |
| The original defect: options share files, split only at an include | FAIL, four tests | FAIL: "Vela's own idle setting stands, whatever the profile Vela selects has been made to include" | FAIL |
| The first repair, which was reopened: only the user's lines are steps of their own | FAIL: `a_profile_is_never_in_one_file_with_the_options_after_it`, `no_step_holds_more_than_one_option_and_none_is_lost_or_moved`, and the order test | FAIL, the same assertion | PASS, rightly: that case it had closed |
| Unchanged | PASS (10) | PASS | PASS |

The E2E check committed with `f80bf0c` did not bite for the reopened case:
with user options standing between Vela's defaults and Vela's protected
options, the first repair's code passed it. The reviewer's case has nothing
in the Advanced options. `1b7b6b4` adds that case as its own scenario, and
the table above is from after it.

### Verification round 2 (repair-delta redispatch), recorded 2026-11-13 17:46 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
  escalated: T5 (ceiling)
- Harness: codex-cli 0.167.2, called plainly, in a fresh session (the
  host's codex config resolves to gpt-6-astra @ max, the ceiling). Thread
  `01a1d44e-9ab2-78f2-aa0d-e2427f2a4986`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 09:48 UTC, finished 09:53 UTC.
- Reviewed head `1b7b6b42721ab6d785b1b32848ab3d21121e7c61`, base
  `31eacad98f252a1bec7ca26df5e4a5e7cef668f7` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **accepted**. No comments.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. Accepted. The fix is
on `main`; the finding is closed.
