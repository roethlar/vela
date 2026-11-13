# ps-2: the check that stream credentials stay off the command line does not see options as they are now written

**Severity**: LOW — the shipped code is correct and keeps stream
authentication in the owner-only include; what broke is one of the checks
that would notice if that stopped being true.
**Status**: In progress
**Branch**: —
**Commit**: (this commit)

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

(pending)

## Coder dispute (if any)

None.

## Known gaps

(pending)

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-seam.md`.
