# ps-2: the check that stream credentials stay off the command line does not see options as they are now written

**Severity**: LOW — the shipped code is correct and keeps stream
authentication in the owner-only include; what broke is one of the checks
that would notice if that stopped being true.
**Status**: Open
**Branch**: —
**Commit**: —

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

(pending)

## Files changed

(pending)

## Guard proof

(pending)

## Coder dispute (if any)

None.

## Known gaps

(pending)

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-player-seam.md`.
