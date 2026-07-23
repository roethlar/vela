# cir-1: pre-split invalid settings can erase the only live connection copy

**Severity**: MEDIUM — a user can be promised preserved server authorization,
then lose every live connection and need to reauthorize after settings
recovery.
**Status**: Open
**Branch**: not started
**Commit**: pending

## Evidence

- `.agents/plans/config-integrity-recovery.md:244` requires the one-time split
  to leave an otherwise invalid combined config untouched.
- `.agents/plans/config-integrity-recovery.md:250` requires every settings field
  and active source to validate before `connections.json` is written.
- `.agents/plans/config-integrity-recovery.md:435` promises on the settings
  recovery screen that connections are already stored separately and will not
  change.
- `src-tauri/src/config.rs:14` currently owns the legacy Plex singleton
  credentials and `src-tauri/src/config.rs:71` owns every active `sources`
  record. Before the split succeeds, `connections.json` does not hold another
  copy.

Trigger: launch the first split-capable build with no `connections.json` and a
combined 1.0.0 `config.json` whose active source records are valid but whose
settings portion contains an unknown or invalid value.

## Predicted observable failure

Strict pre-migration validation rejects the combined file before creating
`connections.json`. The UI nevertheless says connections are separate and
unchanged. If the user clicks **Back up and create new settings**, recovery
replaces the only live file containing the provider tokens with defaults. The
exact backup survives, but the source registry is empty and Plex/Jellyfin/Emby
must be reauthorized.

## What

The plan's post-split recovery guarantee is correct only after the connection
split has completed. Its migration and UI state machine has no pre-split invalid
branch, so the same settings-recovery action makes a false promise during the
upgrade window.

## Approach

No repair is authorized yet. The recommended root fix is to add an explicit
pre-split invalid state. After the user requests recovery and Vela creates the
exact private combined-file backup, syntactically parseable input can validate
the complete legacy connection block independently from settings. If every
connection is strictly valid, atomically create `connections.json` from that
whole block before installing fresh settings; do not load the invalid settings
or salvage individual source rows.

Malformed JSON or any invalid/unknown connection row cannot be separated
without guessing. That branch must warn before confirmation that connections
cannot be preserved and reauthorization will be required. The alternative is
to use that warning for every pre-split invalid file, as the reviewer proposed,
but that gives up the owner's stated no-reauthorization goal even when the
connection block is independently valid.

## Files changed

- None; review intake only.

## Guard proof

Required with the eventual plan repair:

- seed a pre-split combined config with one unknown settings key and a complete
  valid source set; explicit recovery must create the exact combined backup,
  create private valid `connections.json`, reset only settings, and restore all
  sources without relinking;
- seed the same state with malformed JSON and separately with an invalid source
  row; the UI must make no preservation promise and must disclose
  reauthorization before the recovery action;
- reverting the production branch discriminator or independent connection-block
  validation must fail the corresponding guard.

## Coder dispute

None. The candidate is **ADMITTED**: it cites the contradictory plan/code
states, predicts a deterministic user-visible authorization loss, and its
MEDIUM severity is justified by recoverable-but-disruptive credential loss.

## Known gaps

The review request authorized assessment and recording, not a plan repair.
Whether independently valid connections may be moved from an otherwise invalid
combined file during explicit recovery remains an owner ruling.

The reviewer launch denied one optional `git diff --stat` Bash call despite the
launch-scoped grant. The reviewer still completed a 25-turn read-only inspection
using repository read/search tools; the exact pins and schema-valid finding
were independently checked by the orchestrator. No clean acceptance is inferred
from this review.

## Reviewer comments

Reviewer: claude / `claude-opus-4-8` (inline, session-only) / max / frontier
(owner-selected competitive review).

Claude Code 2.1.218, CLI transport, exact reviewed head
`bf3730a14105465f6f5a7edd6e3fd326acd57132`, base
`7a4b5b02cb7287559944cab7246d2a4dd0c5c5d2`, verdict `findings`, UTC
2026-07-23T12:42:46Z. The JSON envelope exited zero, resolved the requested
model exactly, matched both pins, and contained one finding.

Reviewer finding: pre-split invalid config routes to settings recovery, which
loses every connection while the UI promises the opposite. The reviewer
recommended distinguishing absence of `connections.json`, warning that
reauthorization is required, and limiting the preserved-connections guarantee
to post-split state.
