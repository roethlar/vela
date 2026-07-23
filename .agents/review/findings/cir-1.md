# cir-1: pre-split invalid settings can erase the only live connection copy

**Severity**: MEDIUM — a user can be promised preserved server authorization,
then lose every live connection and need to reauthorize after settings
recovery.
**Status**: Resolved by owner; follow-up review waived
**Branch**: not started
**Commit**: `e08ce44`

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

Owner ruling 2026-07-23 rejects partial salvage. Revision 4 adds an explicit
pre-split invalid state, but never parses or validates a connection subsection
from the damaged combined file. Its blocking copy says that creating fresh
settings requires reconnecting servers and offers **Rename and create new
settings** or **Exit Vela**.

Recovery privately renames the entire legacy file, installs fresh settings,
leaves connections empty/absent, and routes to reconnection. Exit writes
nothing. The promise that settings recovery preserves connections is rendered
only when a separate valid `connections.json` already exists.

## Files changed

- `.agents/plans/config-integrity-recovery.md`
- `.agents/decisions.md`
- `.agents/state.md`
- this finding record

## Guard proof

Required with the eventual plan repair:

- seed a pre-split combined config with one unknown settings key and a complete
  valid source set; the UI must disclose reconnection, rename the complete file,
  install fresh settings, extract no source row, and route to reconnect;
- repeat with malformed JSON and with an invalid source row; both take the same
  whole-file path without a preservation promise;
- seed post-split invalid settings beside valid connections; only that branch
  promises and proves byte-identical connection preservation;
- activate Exit in each fault state and prove neither durable file changes;
- reverting the pre-/post-split discriminator or adding connection extraction
  from invalid input must fail the corresponding guard.

## Coder dispute

None. The candidate is **ADMITTED**: it cites the contradictory plan/code
states, predicts a deterministic user-visible authorization loss, and its
MEDIUM severity is justified by recoverable-but-disruptive credential loss.

## Known gaps

The product ruling and plan repair are complete. On 2026-07-23 the owner
declined the proposed follow-up external review and then explicitly directed
implementation to proceed. No clean follow-up verdict is claimed.

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

Owner disposition, 2026-07-23: revision 4 resolves the finding by renaming the
whole invalid combined file and requiring reconnection, with no partial
salvage. The owner declined a follow-up Claude review and explicitly activated
implementation.
