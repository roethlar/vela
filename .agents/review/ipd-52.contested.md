# ipd-52: contested residue (2026-11-20)

Finding (iPadOS plan, first raised 2026-11-18, reopened in every round
since): a Jellyfin `Stopped` for an old play can clear the device's
session after a new play has started on it.

What the plan now does (`c9cfd9b`, and the paragraph added after round
3): a close not yet sent is dropped when a new play starts; a close
already sent is repeated until the server answers, each unanswered
attempt's connection closed first; only then does the new play report.

What the reviewer still holds (round 3, `ipados-r3.md`): closing a
connection cannot call back a request the server has already received;
its handler can run after the answered retry and after the new play's
`Started`, and then clears the new play's current item and play state.
The reviewer asks for a session of the new play's own that the server
recognises, or a server operation scoped to the old play.

Why the working agent does not take the repair further:
- Jellyfin 10.11 has no stop scoped to one play: its handler clears the
  session's current item whatever item the request names (the reviewer's
  own citation, `SessionManager.cs`).
- A session there belongs to the device and client, not to a play. A
  session of the new play's own means reporting under another device
  identity, which the access token is bound to, and which would add a
  device to the server for every such play. That is a change to how Vela
  identifies itself to a server, for all platforms, and it is unproven.
- What remains is narrow and does not lose data: a request received and
  not yet handled when a later request has already been answered. If it
  happens, the server's "now playing" for the device is empty until the
  new play's next progress report, seconds later. Neither item's
  position is lost, and nothing in Vela shows it.

Asked of the owner with the plans' approval: whether that residue is
accepted. If not, the next step is a probe against a real Jellyfin and a
real Emby of what a second device identity does to tokens and sessions,
before any design.
