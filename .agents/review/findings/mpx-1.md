# mpx-1: support independent Plex server sources

**Severity**: MEDIUM — a second Plex server cannot coexist under the legacy
singleton model, so users lose independent credentials, routing, and removal.
**Status**: Verified
**Branch**: `fix/mpx-1-multi-plex`
**Commit**: implementation `a0c2d14` through `5e63462`; evidence/status
`c24c132`

## Evidence

At base `34ad47c`, `src-tauri/src/lib.rs`, `src-tauri/src/config.rs`, and
`src-tauri/src/commands.rs` restore, persist, link, and remove one literal
`plex` source. `src/lib/Settings.svelte` exposes account-wide Disconnect rather
than exact-row removal. The triggering condition is linking or restoring two
Plex machines that must keep different tokens and machine identifiers.

## Predicted observable failure

Without the fix, a second Plex link overwrites or aliases the singleton; two
machines cannot both restore, a shared title cannot retain independently routed
backings, and removing one server cannot leave the other live. The hermetic
Linux `multiplex` scenario detects those failures through the real Tauri app.

## What

Represent every Plex server as an independently identified source row, migrate
all persisted references away from the legacy literal ID, make authorization
repeatable and identity-pinned, and use exact source IDs for Settings removal.
Keep existing cross-source title collapse and per-title playback overrides.

## Approach

Legacy singleton fields migrate once into a minted `plex-{uuid}` row while all
config, recent, sort, tombstone, hierarchy, backing, and playlist references
are re-keyed under a crash-safe marker. Startup restores every row into an
independent `PlexSource`. Linking holds credentials in bounded backend-only
sessions until a reachable direct-HTTPS machine is identity-verified, then
persists a fresh row. Settings delegates every provider removal to the normal
exact-ID command. A two-machine TLS mock drives the full app through collapse,
both explicit overrides, and single-row removal.

## Files changed

- `src-tauri/src/config.rs` — per-source Plex credentials and complete legacy
  migration/re-keying.
- `src-tauri/src/lib.rs` — restore every configured Plex source.
- `src-tauri/src/commands.rs` — bounded repeatable link sessions, exact server
  selection, and exact-ID removal.
- `src-tauri/src/plex_library.rs` — identity-verified reachable server choices.
- `src-tauri/src/source/plex.rs`, `src-tauri/src/source/mod.rs` — per-row source
  construction, pin restore, and matching-row binding persistence.
- `src-tauri/src/playlists.rs` — source-reference migration support.
- `src/lib/Settings.svelte`, `src/routes/+page.svelte` — per-row removal and
  credential-free multi-server link selection.
- `tests/ui-foundation.test.mjs` — frontend link/removal contract guards.
- `tests/e2e/mockplex.mjs`, `tests/e2e/scenarios/multiplex.mjs`,
  `tests/e2e/run.mjs`, `tests/e2e/README.md` — two-machine real-app proof and
  scenario-scoped TLS trust.
- `.agents/plans/multi-plex.md`, `.agents/state.md`, `.agents/machines.md` —
  durable plan, evidence, status, and venue facts.

## Guard proof

- `tests/e2e/scenarios/multiplex.mjs` — two source rows restore with separate
  tokens and pins, collapse to one two-backed card, persist both explicit
  source overrides, and remove only Plex A while Plex B remains live. Four
  independent production mutations disabled all-row restore, override
  persistence, exact-row removal, and cross-source deduplication; each rebuilt
  app failed its intended assertion, restored exact, and the clean scenario
  plus full suite passed (1/1 and 30/30).
- Reviewer-accessible focused proof: mutate
  `remove_source_config` in `src-tauri/src/commands.rs` to retain no Plex rows,
  then run `cargo +stable test --locked
  commands::tests::removing_one_plex_source_preserves_the_other` from
  `src-tauri/`; it must fail. Restore exact head and rerun; it must pass.

## Coder dispute (if any)

Empty.

## Known gaps

The approved plan separately requires a Settings control that chooses the
default playback source for duplicate titles. That control was never drafted
or implemented; same-kind Plex ties still fall through to stable registry
order unless the existing per-title context-menu override applies. The plan and
state record this as an implementation blocker. The round-1 reviewer was asked
to grade the omission and accepted the recorded finding; that verdict does not
waive the owner-approved requirement or make the branch merge-ready.

## Reviewer comments

### Failed transport attempt — not a substantive review round

`Reviewer: claude / claude-opus-4-8 / xhigh / standard` (transcript-sourced;
owner requested Fable xhigh)

- Harness: Claude Code 2.1.215 MCP Workflow
- Reviewed head: `b90002aee37a6e483aa3cc69bceef41deef821f5`
- Base: `34ad47c628cf176f68ddfb0ace7138fae1ec2083`
- `guard_confirmed`: false
- Outcome: transport/provenance failure; no code verdict
- Timestamp: 2026-07-19T23:11:40Z

The MCP workflow ignored the requested Fable pin and its invocation transcript
resolved `claude-opus-4-8` at xhigh. Every `git` and `cargo` command then reached
an unanswerable approval gate, so the reviewer could not inspect the pinned diff
or execute the required red/restored-green proof. Its eventual payload reported
`reopened` and `guard_confirmed:false`, but also failed the strict result schema;
the orchestrator rejected it before verdict. A bounded direct CLI smoke probe
separately resolved `claude-fable-5` at xhigh, but the permission-granted CLI
transport is not authorized for this finding. The disposable worktree remained
clean at the exact reviewed SHA. This failed transport attempt does not consume
one of the owner's three substantive rounds.

### Exact-model MCP retry — not a substantive review round

`Reviewer: claude / claude-opus-4-8 / xhigh / standard` (initial invocation
transcript; owner pinned `claude-fable-5` exactly)

- Harness: Claude Code 2.1.215 MCP Workflow
- Reviewed head: `72628dee7245c5e905b5a75f8b6585d1894b644a`
- Base: `34ad47c628cf176f68ddfb0ace7138fae1ec2083`
- `guard_confirmed`: false
- Outcome: transport/provenance failure; no code verdict
- Timestamp: 2026-07-19T23:18:34Z

Retrying with the full `claude-fable-5` ID produced the same initial invocation
provenance (`claude-opus-4-8` at xhigh) and the first git command again reached
an unanswerable approval gate. The halted agent re-emitted a schema-valid
`reopened` payload with `guard_confirmed:false`; that re-emission itself ran as
Fable at high, not the requested xhigh, and performed no review. The
orchestrator rejected the dispatch before verdict. The disposable worktree
remained clean at the pinned head. This retry also does not consume a
substantive round.

### Substantive round 1 — accepted

`Reviewer: claude / claude-fable-5 / xhigh / standard`

- Harness: Claude Code 2.1.215 headless CLI
- Reviewed head: `c32a59bf70b80b80aac8a59177ffb37d0ba56dc4`
- Base: `34ad47c628cf176f68ddfb0ace7138fae1ec2083`
- `guard_confirmed`: true
- Verdict: accepted
- Timestamp: 2026-07-19T23:30:51Z

The invocation transcript confirms the exact requested model and xhigh effort
on the standard service tier. In its detached disposable worktree, Claude
changed exact-ID removal to delete every Plex row, observed the focused Rust
guard fail because zero rows remained, restored the original line, observed
the guard pass, and confirmed an empty status and diff at the reviewed SHA.
The orchestrator independently repeated the clean-tree, empty-diff, and exact-
SHA checks. Claude returned no comments after explicitly reviewing the known
Settings preference gap. This verdict verifies the recorded `mpx-1` finding;
it does not implement or waive that separately approved requirement, which
remains a merge blocker.
