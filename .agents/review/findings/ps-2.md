# ps-2: the check that stream credentials stay off the command line does not see options as they are now written

**Severity**: LOW — the shipped code is correct and keeps stream
authentication in the owner-only include; what broke is one of the checks
that would notice if that stopped being true.
**Status**: In progress
**Branch**: —
**Commit**: `94d067e`

## Evidence

`tests/config-integrity.test.mjs` (at `b719a65`, line 315), in "mpv
authentication stays in a private include until its exact child is
reaped":

```js
assert.doesNotMatch(playback, /--http-header-fields=/);
```

Before Slice 1 an option was written as command-line text, so a regression
that passed the headers as an option had to contain that text. Since
`3e91c63` options are data (`LaunchArg::set("name", value)`), and the text
`--http-header-fields=` is produced only when the external launcher
renders them. The reviewer added
`LaunchArg::set("http-header-fields", ...)` filled from
`spec.http_headers` and all 14 checks in that file stayed green.

## Predicted observable failure

A change that puts stream authentication (Plex's `X-Plex-Token` header)
into mpv's argument list, where any program on the machine can read it
(`/proc/<pid>/cmdline`), passes `npm run check`.

## What

A source-text guard was carried across a refactor that changed how the
thing it looks for is spelled. It was re-anchored for the include option
and left as it was for the header option.

## Approach

The old assertion matched one spelling of the regression. Respelling the
assertion (`LaunchArg::set("http-header-fields"`) would repeat the mistake
for the next way an option can be written, so the check now rests on two
things that cannot be respelled, and on what the player is really started
with.

In `tests/config-integrity.test.mjs`, over the code compiled into the app
(each file up to its test module, comment lines removed) of `playback.rs`
and the three files of the player module:

1. every line that mentions `http_headers` is one of exactly three: the
   field's declaration, the "are there any" test, and the call that writes
   the include. A play's headers can therefore go nowhere else;
2. every line that mentions `http-header-fields`, in any letter case, is
   exactly one: the line that formats the include's content. mpv's header
   option can therefore not be set through `LaunchArg::set`,
   `LaunchArg::Option { .. }`, `LaunchArg::User` or text.

In Rust, `stream_auth_reaches_the_player_only_through_the_private_include`
(`src-tauri/src/player/external.rs`) builds a launch with two synthetic
headers, renders it with the external launcher, and requires that no
argument and no environment entry contains a header name, a header value
or the option name; that the include holds both headers; that the include's
path appears once, as `--include=<path>`; and that a launch without
headers names no include. The JavaScript test requires that Rust test to
exist.

## Files changed

- `tests/config-integrity.test.mjs` — `shippedCode`, `linesMentioning`;
  the three checks above replace the `--http-header-fields=` text match in
  "mpv authentication stays in a private include until its exact child is
  reaped"
- `src-tauri/src/player/external.rs` — the new test

## Guard proof

Run 2026-11-13 in a disposable worktree; each injection restored with
`git checkout` from the commit, then
`node --test tests/config-integrity.test.mjs` and
`cargo +stable test --locked --lib player::`. All injections are in
`build_launch` (`src-tauri/src/player/launch.rs`) unless noted. "JS" is
"mpv authentication stays in a private include until its exact child is
reaped"; "rendered" is the new Rust test; "recorded" is
`the_full_external_command_line_is_exactly_the_recorded_one`.

| Injection | JS | Rust |
| --- | --- | --- |
| The reviewer's mutation, on the commit BEFORE the fix (`1d7c405`) | pass (the finding) | recorded FAILS |
| The reviewer's mutation, on the fix | FAIL | rendered FAILS ("a stream header reached the player outside the include"); recorded FAILS |
| A second reader of the headers that sets no option | FAIL at the `http_headers` list (line 338) | pass |
| The header option set as `LaunchArg::set`, from a fixed value | FAIL at the option-name list (line 345) | rendered FAILS ("must be set by the include, never by an argument") |
| The header option as raw text, `--HTTP-Header-Fields=` | FAIL | rendered FAILS, same message |
| The Rust test renamed away (`external.rs`) | FAIL at the name check (line 349) | pass |
| A header value in the player's environment | FAIL | rendered FAILS |
| A header under another option name (`stream-lavf-o`) | FAIL | rendered FAILS |
| The include is written and not passed | FAIL (the existing include check) | rendered FAILS ("the include is named once") |
| The include is passed twice | pass | rendered FAILS, same message |
| An include option on a launch with no headers | pass | rendered FAILS; the minimal recorded test FAILS |
| Restored | PASS | PASS, 33 of 33 in `player::` |

The first row is worth keeping in mind when reading the severity: on the
commit the reviewer examined, the Rust recorded-command test did fail
under the reviewer's own mutation. The JavaScript check alone was blind.

## Coder dispute (if any)

None.

## Known gaps

- The two source-text lists are exact, so an innocent edit to one of those
  lines (a rename, a reformat) fails the test and has to update it. That
  is intended: those are the lines a reviewer should be made to look at.
- The check covers the code that describes and starts a launch
  (`playback.rs` and `player/`). What `commands.rs` puts into
  `PlaySpec.url` and `PlaySpec.http_headers` is covered by "Plex
  credentials never enter query strings or frontend artwork URLs", which
  this finding did not touch.
- Jellyfin and Emby stream URLs carry their token in the URL by the
  standing decision of 2026-05-23; this guard is about headers only.

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-seam.md`.

### Verification round 1, recorded 2026-11-13 06:37 UTC (this record is committed)

- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, called plainly; the host's codex config
  resolves to gpt-6-astra @ max. Thread
  `01a1d396-5ee8-74c2-9d30-04ac4f39db6b`, run in the disposable worktree
  `~/dev/vela-probe-wt` at the head SHA; the stream ends in
  `turn.completed`; dispatched 06:26 UTC, finished 06:33 UTC. Routing: T1
  no sensitive path in the diff, T2 severity LOW, T3 the recorded proof
  re-run by the orchestrator agrees.
- Reviewed head `94d067ed1344f1dd840f9354d177ac5204fbfc13`, base
  `1d7c405ae2ef88d5b46ef497e27375e005e3e96b` (both equal the dispatched
  pins).
- `guard_confirmed`: true. `capability_ok`: true.
- Verdict: **reopened**. Comments, verbatim:
  - src-tauri/src/player/external.rs:574 — command_envs omits inherited environment variables. Exporting the joined headers using std::env::set_var inside write_header_include left all 11 JavaScript checks and 33 Rust tests passing; a stub player confirmed it inherited the synthetic header. The JavaScript lists miss this because that helper uses the headers parameter. Check the child's effective environment.
  - tests/config-integrity.test.mjs:33 — The split assertion does not establish that the test module ends the file. Moving external.rs production items after its test module made shippedCode return an empty section while both suites passed. Other files supply every expected match, hiding the lost coverage. Reject empty production sections and ensure production code after test modules is scanned.

Orchestrator's acceptance: schema-valid payload, verdict in the enum, both
SHAs equal the pins, both booleans literally true. A valid `reopened`: not
accepted. Both comments are correct and were reproduced before repair (see
"Repair" below).

### Repair (round 2)

Both comments reproduced at `94d067e` before the repair: with the
reviewer's first mutation (`std::env::set_var` of the joined headers inside
the include writer), with the same through a copied variable, with a
function that reads `spec.http_headers` appended after `external.rs`'s test
module, and with that file's test module moved above its code, the
JavaScript file and all 33 `player::` tests passed.

What changed:

1. **The environment the player really gets.** The Rust test compared only
   what the command ADDS to the environment. A started program inherits
   all of Vela's own environment as well. The test now builds the effective
   environment (Vela's own as it stands after the launch was described,
   with the command's additions and removals applied) and looks for the
   header names, values and option name in every key and value of it. It
   also checks that what it looks at contains every one of Vela's own
   variables and every one the command adds, so the check cannot quietly
   shrink.
2. **Where shipped code is.** `shippedCode` took everything before a file's
   test module and assumed the module was last. It now requires that: after
   `mod tests {`, the only line starting in the first column is the closing
   brace. Code after the tests, or tests moved above the code, fail the
   test instead of going unread.
3. Two further source checks, not asked for, in the same test: no code that
   describes or starts a launch calls `set_var` or `remove_var`; and inside
   `player/launch.rs` every line that names the header list, the text it is
   joined into, or the include's content is one of nine recorded lines, so
   a copy of the credentials to anywhere else changes the list.

4. **The environment check runs alone.** Proving item 1 showed a hole in
   it: with the reviewer's mutation in place the Rust test still passed in
   one run of two. A test process has one environment for all of its
   tests, the mutation makes every test that describes a launch export its
   own headers into it under the same name, and another test's export
   could replace this test's before this test looked. The test now starts
   the test program again with only itself selected and does the check
   there, where nothing else runs; the outer test requires that exactly one
   test ran and passed.

Files: `tests/config-integrity.test.mjs`, `src-tauri/src/player/external.rs`
(the same two files the finding already declared). Commits: `bdecf65`,
`b84baeb` (removes one assertion of `bdecf65` that could not be shown to
bite), and the commit that adds item 4.
