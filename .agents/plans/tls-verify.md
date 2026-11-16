# Plan: the player checks the media server's certificate

## Status

APPROVED by the owner on 2026-11-14 (decision `.agents/decisions.md`, that
date: "The player checks the server's certificate, with a setting to turn
it off"). LANDED 2026-11-14 at `bc51d01`, its one slice whole. Not released.
The release note and the user documentation were written on 2026-11-15
(log, below); the version is 1.0.64's, which is the owner's to release.

## The fault

Found 2026-11-13 (`.agents/plans/embedded-player.slice6-notes.md`). The
system's mpv, up to 0.41, defaults to `tls-verify=no`, and Vela does not
set it. So mpv fetches an `https` stream, and sends the stream's sign-in
token with the request, without checking whom it is talking to. Vela's
own client, which browses the server, does check. The mpv that Vela ships
for the embedded player (0.42) checks by default, against its own TLS
library's trust store, which is not always the one Vela's client uses.

## What the owner chose

Of three ways put to them (check; leave it; check, with a setting to turn
it off) the owner chose the third.

## The slice

- The player is told to check the server's certificate, in both players:
  `--tls-verify=yes` for mpv's own window, the same option on the
  embedded player's mpv. It is the last word over the user's Advanced
  options and their own mpv configuration, like Vela's lifecycle options
  (`67536da`). Where mpv finds its trusted authorities is NOT set by Vela:
  a user whose server is signed by a private authority can still name it
  to mpv (`tls-ca-file`, in the Advanced options or their own
  configuration).
- A setting turns the check off: a new field in `config.json`, absent
  meaning on, parsed fail-closed like the others and kept through saves;
  and a control in Settings, in the player's section, off by default,
  worded so that the cost is plain (the sign-in token is then sent without
  knowing who receives it). With it off the player is told
  `--tls-verify=no`, also as the last word.
- When a play fails because the certificate was refused, the user is told
  so where Vela reports a play that did not start, in words that name the
  certificate and the setting. If that cannot be told apart reliably from
  other failures of mpv, the message for a play that ends at once says
  where to look. The implementer says which was possible and why.
- Old configuration files keep loading; the legacy fields are untouched.

## Not in this slice

A release note and a version: written when the next release is cut. What
the embedded player's mpv trusts beyond this option.

## Verification

The full check; end to end, the existing suite in both players (its mock
servers speak plain `http`, so they are not affected); one test that
shows the option on the command line in each state of the setting, and
one for the setting's round trip through the configuration. A real
`https` server with a refused certificate is not in the venue: say so.

## Implementation log

### 2026-11-14: landed at `bc51d01`

Written by codex as a coding task; read, amended, merged and checked by
the working agent.

- The setting is `mpv_tls_verify` in `config.json`: absent or null means
  check, only `false` turns checking off, any other value makes the file
  invalid like any bad settings file.
- The control is in Settings under "mpv player": a checkbox "Skip server
  certificate checks", off by default, its own "Save certificate setting"
  button, and a line saying what skipping costs and that a private
  authority can be named to mpv with `tls-ca-file` instead.
- `--tls-verify=yes` or `=no` is the last option on the external command
  line (after the stream-header include) and the last of the embedded
  player's options.
- A refused certificate cannot be told apart reliably from mpv's other
  early failures, and mpv can fail before Vela is connected to it. So the
  second way of the plan was taken: a play of an `https` stream, with
  checking on, that ends without having been seen to start (no file
  loaded, no position, no ordinary end reported) is reported where Vela
  reports a play that did not start, with words that name the certificate
  and the setting. The working agent narrowed this from codex's version,
  which showed it for every early end; other early ends are reported as
  before.
- Checks: the full check green (693 Rust tests, 549 Node tests). End to
  end, external: `playback`, `mpvargs`, `multiplex`, `surfaces`,
  `embeddedownconfig`, `windowsetting`, `smoke`, `sourcedeadend`, 8 of 8;
  embedded: `embeddedplay`, `mpvargs`, `embeddedownconfig`,
  `embeddedlatestart`, `playback`, 5 of 5.
- Not checked: a real `https` server whose certificate mpv refuses (none
  in the venue); the notice in a real window; Windows and macOS (not
  compiled here).

### 2026-11-15: the release note and the user documentation

Written without waiting for the release, under the owner's ruling of that
day that what is built is to be usable (`.agents/decisions.md`): a
paragraph in `docs/usage.md` ("Player behavior notes") and an entry under
"Unreleased", Fixes, in `RELEASE_NOTES.md`. Both say what changes for a
server whose certificate mpv does not accept and the two ways on. Nothing
of this plan is left for the agent.
