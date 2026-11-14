# Plan: the player checks the media server's certificate

## Status

APPROVED by the owner on 2026-11-14 (decision `.agents/decisions.md`, that
date: "The player checks the server's certificate, with a setting to turn
it off"). Being built as one slice, by codex as a coding task, reviewed
and landed by the working agent. This section and the log at the end say
how far it has landed.

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

(nothing landed yet)
