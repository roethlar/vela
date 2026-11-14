# pw-3: a width and a height that were never reported together can be carried as the window's size

**Severity**: MEDIUM — width and height arrive as two reports. Each new one is paired with the other's previous value, and when the pair that results is out of bounds the earlier, mixed pair is kept. After 800x450, a report of width 1024 followed by a height that is refused leaves 1024x450, which the window never was; the next play opens at that size, and in the default choice it is saved for the next run.
**Status**: In progress
**Branch**: —
**Commit**: `2dd9eb7`

## Evidence

Reviewer, verbatim: "src-tauri/src/playback.rs:160-180 combines each dimension with the previous complementary dimension, then leaves the candidate untouched when validation fails. After confirming 800x450, reporting width=1024 followed by height=89 makes snapshot() return 1024x450. A second Rust reproduction using width=1024, height=0, width=0 also returned 1024x450. Both reproductions failed their assertions."

## Predicted observable failure

Reviewer, verbatim: "The next play receives --geometry=1024x450 even though that size was never reported as a complete pair. An invalid resize or shutdown sequence overwrites the valid carried size and can persist across restarts in Remember mode. Playback continues, but window-size continuity is corrupted."

Reviewer's suggested direction, verbatim: "Track coherent dimension pairs, for example through osd-dimensions, and retain the last valid pair when an invalid report arrives. Add regression tests starting from an existing size, with valid and invalid dimensions arriving in either order."

## What

The defect is as the reviewer describes. In `src-tauri/src/playback.rs`,
`WindowTracker` kept the last `osd-width` and `osd-height` as separate
fields, and each new report was paired with the other's last value
(`observe_size(Some(width), self.height)`). An out-of-bounds pair returned
early, but by then a mixed pair could already have become the candidate.
After 800x450, a width of 1024 made the candidate 1024x450, and a refused
height of 89 (or a height of 0 and then a width of 0) left that invented
size in place. The next play opened at 1024x450, a size the window never
was, and with the setting that remembers the window it was saved for the
next run.

The helper who attacked the fix found nothing more to repair here. It ran
the reviewer's two sequences through the real observation, both as the
old separate reports and as the new single report, and each time the
size stayed 800x450.

## Approach

The reader observes mpv's `osd-dimensions` (observe id 7) in place of
`osd-width` and `osd-height` (ids 7 and 8). `osd-dimensions` is one report
that holds the width and the height together, so a size is only ever a
pair mpv sent. `WindowChange::Width` and `WindowChange::Height` are
replaced by `Size(w, h)`. The parser takes it only when `data` is an
object whose `w` and `h` are both whole numbers that fit a `u32`. The
tracker no longer keeps a half size between reports. An unusable size
returns before touching anything, so the candidate, its time and the
confirmed size all stay as they were. Consumers still match on the
property's name, never on the id.

The helper checked the property against the system's mpv 0.41 first
(`--no-config`, an IPC socket, a test source, `--vo=x11` on a private
Xvfb, and also `--vo=null` and idle with no file). The event mpv sends is
`{"event":"property-change","id":7,"name":"osd-dimensions","data":{"w":800,"h":450,"par":1.000000,"aspect":1.777778,"mt":0,"mb":0,"ml":0,"mr":0}}`.
It is sent at once with `w=0, h=0`, then with 800x450 when the window
opens, again on every resize, on fullscreen on and on fullscreen off, each
time just before the matching `osd-width` and `osd-height` events. With
`--vo=null` and with no file it reports only 0x0, which the bounds still
refuse. The attacking helper repeated the probe on its own Xvfb and got
the same results.

Only `playback.rs` changed. The embedded player's IPC reader is the same
code, and the end-to-end helpers already read `osd-dimensions` from it.

What a user gets: the size carried to the next play, and saved for the
next run when the setting remembers it, is always a size mpv reported in
one report.

Made by a helper agent (Claude Opus 5.5) on a local branch, attacked by a
second, confirmed by another on the merged tree, and read and landed by
the coordinating agent, one commit per helper commit. The attack found
nothing to repair here. Its one finding about this code, that a
fullscreen change less than a second after a resize loses the new size,
is older behaviour: the rule that drops the candidate when a flag turns
on (`if enabled { self.candidate = None; }`) was already at
`playback.rs:156-157` at the base `663fe52`, and this fix changes nothing
about the flags but one line of a doc comment. Nothing was changed for
it.

## Files changed

- `src-tauri/src/playback.rs` (`WindowTracker`, `WindowChange`,
  `window_property_change`, the subscription in `spawn_position_reader`,
  doc comments, unit tests)

## Guard proof

In `playback::tests` (`src-tauri/src/playback.rs`), the tracker tests in
its `windowed_size` module and two tests beside them:

- `an_unusable_report_after_a_size_leaves_that_size`: the reviewer's two
  sequences in their one-report form, and the other unusable sizes, after
  a confirmed size and after a candidate not yet confirmed, leave 800x450.
- `usable_and_unusable_reports_in_either_order`: from an existing size,
  the usable report is taken whole.
- `a_size_outside_the_bounds_is_not_recorded`,
  `a_reported_size_becomes_the_candidate` (renamed from
  `a_reported_size_becomes_the_candidate_only_when_both_dimensions_are_known`,
  since a half size no longer exists),
  `a_play_hands_on_the_size_it_was_opened_at_until_it_reports_its_own`
  and `the_observation_confirms_on_any_later_event`: the older rules
  (confirmation after a second, discard when a flag turns on, ignore while
  one is on, bounds, the opened-at hand-on, 0x0), rewritten to the new
  property.
- `window_property_parser_accepts_only_named_typed_change_events`: the
  parser accepts the object mpv sends and rejects `osd-width` and
  `osd-height`, a missing half, and every other JSON type. It used to
  require that `osd-width` gives `Width(1280)` and `osd-height` gives
  `Height(720)`; that is the behaviour the finding removes.
- `the_reader_subscribes_to_the_window_size_and_records_it`: the real
  reader, over a Unix socket, is asked for `osd-dimensions` and records a
  size through it; a report with height 0 after it changes nothing.

Red-proofs, each restored from the commit afterwards:

- An unusable report's usable half paired with the confirmed size's other
  half (the reviewer's defect): `an_unusable_report_after_a_size_leaves_that_size`
  failed with `[(1024, 89)] left: Some(((1024, 450), ..)) right: None`,
  and `usable_and_unusable_reports_in_either_order` failed.
- An unusable report drops the candidate: the first test failed
  (candidate `None`, expected `Some(((800,450), t0))`), and so did
  `usable_and_unusable_reports_in_either_order`,
  `a_size_outside_the_bounds_is_not_recorded` and, through the real
  reader, `the_reader_subscribes_to_the_window_size_and_records_it`
  (`None`, expected 1024x576).
- An unusable report restarts the candidate's time: the first test failed
  (the same size, a later `Instant`).
- A missing `h` read as 0: the parser test failed on `{"w":1280}`, `left:
  Some(Size(1280, 0)) right: None`.
- Non-negative fractional numbers accepted: the parser test failed on
  `{"w":1280.5,"h":720}`, `left: Some(Size(1280, 720)) right: None`. The
  first version of this injection failed on the negative case instead, the
  wrong reason, and was tightened.
- The reader asks for `osd-width` instead (the wiring): the reader test
  failed at "the reader asks for osd-dimensions" (a read timeout).
- The parser matches `"osd-dimension"` (the wiring): the parser test,
  `a_play_hands_on_the_size_it_was_opened_at_until_it_reports_its_own`,
  `the_observation_confirms_on_any_later_event` and the reader test
  failed.
- The attacking helper, with injections of its own: an unusable size
  clamped into the bounds failed 5 tests, the reader test among them; an
  unusable report confirming the pending candidate early failed
  `an_unusable_report_after_a_size_leaves_that_size`; the observation
  dropping `Size` changes (the wiring) failed 3, the reader test among
  them; `w` and `h` swapped failed 4; the usable width of an unusable
  report merged into the confirmed size failed with left `Some((1024,
  450))`. Subscribing to `osd-width` and `osd-height` as well stayed
  green, which is correct because the parser rejects them.
- The confirming helper, on the merged tree: an unusable report's width
  paired with the kept height: `an_unusable_report_after_a_size_leaves_that_size`
  failed with left `Some(((1024, 450), ..))`, right `None`, the
  reviewer's invented 1024x450, and
  `usable_and_unusable_reports_in_either_order` failed with left
  `(1280,576)`, right `(1024,576)`.

`playlistplay`, which checks the 0x0 report through the real reader, passed
in mpv's own window and in the embedded player at this commit.

On the merged tree (the confirming helper's trial merge of both branches
onto `main` at `efbf30c`; `main` has since moved by commits that touch
none of these files), the full check was green apart from the two
recorded audit exits (`npm audit` exits 1 on known advisories, `cargo
audit` exits 101 because it is not installed): 641 Rust tests passed (10
ignored), and of 413 Node tests 412 passed and 1 was skipped, an older
skip. Both end-to-end suites were run with a fresh build: external 46/48
and embedded 49/50, the only failures being the recorded ones
(`embeddedownconfig` in the external suite, `pagefail` in both), and `npm
run e2e:verdict` gave "parity verdict: PASS". `playlistplay` passed in
both players.

Nothing here rests on source-text checks only: the reader test drives the
real reader over a socket.

At landing (2026-11-14) the ten commits of this review's fixes were put on
a branch cut from `main` in the helpers' order. Each passed `npm run check`
and `cargo +stable test --locked` on its own before the next was added,
and the full check was green on the last (641 Rust tests, 424 Node tests;
the two audit commands exit as recorded). The product code and the two
player-window test files there were byte for byte the tree the confirming
helper had run end to end.

## Coder dispute (if any)

## Known gaps

- The plan `.agents/plans/mpv-window-geometry.md` still names `osd-width`
  and `osd-height` (lines 82, 93, 103, 187, 219 and 398) and needs a note
  that the reader now observes `osd-dimensions`.
- Only X11 on Xvfb and `--vo=null` were probed. No HiDPI or Wayland report
  of `osd-dimensions` was checked.
- Older behaviour, kept: a size set less than a second before fullscreen
  or maximize turns on is dropped by the existing discard rule, so the
  size from before it is carried on instead. Real mpv sends the fullscreen
  flag before the fullscreen size, so the attacking helper suggested the
  discard may only be needed for maximize under a window manager; that was
  not checked.
- An older test in the same module,
  `an_external_play_reports_its_end_without_waiting_for_the_eof_watcher`,
  failed once under load in the attacking helper's run and passed in every
  run after. This fix does not touch it; its panic text was not captured.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `f1e0c9c..9815e10` with Slice 2 of the plan as its
mandate, 2026-11-14
(`.agents/review/codereview-2026-11-14-player-window.md`). Reviewer's
title: "Invalid dimension reports preserve an invented window size". Reviewer's severity: MEDIUM.
