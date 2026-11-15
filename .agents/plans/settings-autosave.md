# Plan: Settings saves each change as it is made

## Status

APPROVED by the owner's request of 2026-11-15, verbatim: "all the Save
buttons in settings- do we need them? can we not either auto-save
settings changes or have one save button per page? these are making the
setting page difficult to use." Of the two ways the owner named, the
agent took the first (saving each change) and settled the details below
under the owner's working rule that implementation details are the
agent's to settle and record. LANDED 2026-11-15 at `5fc040f`; the log at
the end says what was built and checked.

## What is wrong

The Player tab of Settings has seven Save buttons, one per section:
playback source preference, Continue Playing, Video player, Player
window, the certificate setting, the mpv path, and the mpv options. A
choice does nothing until its own button is found and pressed, and a
choice left without it is dropped without a word when Settings closes.
The Appearance tab already applies a theme the moment it is chosen.

## Why saving each change, not one button per page

One button per page keeps the two faults in a smaller form: a change is
still not in effect until the button is reached, and closing Settings
with changes pending needs either a silent drop or a "discard changes?"
question. Saving each change has neither, and is what the Appearance tab
does.

## The rules (settled by the agent)

- **No Save buttons.** Every setting on the Player tab is saved when it
  is changed.
- **Choices** (lists, radio buttons, checkboxes) are saved the moment
  they change.
- **Text** (the mpv path, the Advanced mpv options) is saved when the
  field is left, and for the one-line path also on Enter. Not on every
  keystroke: half a path would be refused, and half an option line would
  be applied to the next video. Closing Settings with an edited text
  field saves it first; nothing typed is dropped silently.
- **Actions stay buttons**: Browse, Clear, the mpv install, Insert (a
  preset into the options, which is then saved), and everything on the
  Servers and Connected tabs. They do things; they are not Save buttons.
- **The user can see it happened.** Settings says "Saving…" while a
  write is on its way and "Saved" for a moment after. A save that is
  refused puts the control back to what is stored and says why, in that
  section. (As built: the status is one line in the dialog's header, not
  one per section; see the log.)
- **The order rules stay** (review finding `pw-7` and commit `729070d`):
  one write at a time per setting, each sent when the one before has been
  answered; a slow load never overwrites a newer choice; a late reply
  never replaces a newer one, also across closing and reopening.
- Settings that are stored together are still written together (the
  seven mpv options in one write; the source policy and the two display
  overrides in one), with the whole group as it stands when a member
  changes.
- No backend command changes.

## Verification

`npm run check`, `npm run build`; the tests of each control against the
stand-in backend, moved to the new way in; the three end-to-end
scenarios that pressed a Save button (`sourcepolicy`, `continueoff`,
`windowsetting`); a one-off drive of every control in the real window in
both players, as for `729070d`; then both suites in full.

## Paperwork on landing

`RELEASE_NOTES.md` (Unreleased); `.agents/decisions.md` (the rule for
every later setting); `.agents/state.md`; this file's log.

## Implementation log

### 2026-11-15: landed at `5fc040f`

Written by codex as a coding task; read, amended and checked by the
working agent.

- All seven Save buttons are gone. Choices save on change; the mpv path
  on leaving the field and on Enter; the Advanced mpv options on leaving
  the field; Insert saves at once; closing Settings saves an edited text
  field first, through queues that outlive the dialog.
- Rapid changes are all sent, in order (no coalescing); the last stands.
- The Player window choice and the certificate setting, which each had a
  mechanism of their own, now use the one the other controls use.
- **The agent's amendment.** Codex put a status line in each of the seven
  sections. For a control at the foot of the long mpv section that line
  was above the fold, and seven of them were noise. There is now one, in
  the dialog's header beside Close: "Saving…", "Saved" for two seconds,
  or "Not saved" while a section is showing a refused save.
- Checked: `npm run check` (564 tests) and the build. In the real window,
  by one-off scenarios not in the repository, in both players: every
  control changed and found in `config.json` within about 50 ms; text on
  leaving the field, on Enter and on closing; a path that is not mpv
  refused, not stored and said so; a reopened page showing everything;
  then 400 bursts of thirteen rapid changes to the grouped settings, with
  and without gaps: the last choice of every control stored and shown
  each time. `sourcepolicy`, `continueoff`, `windowsetting` and `smoke`
  pass in the external player, the first two in the embedded one.
- Not explained: the very first run of the one-off check in the embedded
  player timed out waiting for two display overrides to be stored. It did
  not happen again in 13 more runs or in the bursts.
- Both suites in full, at `2a954d0` (this change and the embedded
  player's track lists): external 48 of 48, embedded 51 of 51, parity
  verdict PASS.
