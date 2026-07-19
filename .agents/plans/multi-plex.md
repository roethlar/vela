# Plan: multiple Plex servers (multi-Plex)

Status: **IN PROGRESS — all owner decisions in; Slices 1-3 complete, Slice 4
next.** Owner-reported 2026-07-18 (ISSUES.md).
Evidence below is from fresh main (post-5248fe6) tracing.

## Goal

Let Vela present more than one Plex server at a time, the way Jellyfin/Emby
already support multiple configured instances, without breaking the existing
single-Plex install, per-title overrides, or the machine-identity binding
guarantees in `source/plex.rs`.

## Evidence: where "exactly one Plex" is assumed today

- `src-tauri/src/lib.rs:19` — `pub const PLEX_SOURCE_ID: &str = "plex"`: one
  fixed id for the one Plex source.
- `src-tauri/src/lib.rs` (startup restore) — at most one `PlexSource` is
  rebuilt, from the config singletons `auth_token` + `client_identifier`;
  an https-only saved server is re-pinned via `set_server_manual`.
  Jellyfin/Emby restore from `cfg.sources` (a Vec) — already multi-instance.
- `src-tauri/src/config.rs:12-17` — account-level singletons: `auth_token`,
  `client_identifier`, `last_server_host/port/scheme`. One account, one
  remembered server.
- `src-tauri/src/config.rs:168-181` — `SourceConfig` is generic
  (id/kind/name/base_url/tokens), but `jellyfin::build_source` only accepts
  jellyfin/emby kinds; no Plex entry can live in `sources` today.
- `src-tauri/src/commands.rs:205` — `remove_source` refuses `PLEX_SOURCE_ID`
  by design; `commands.rs:229` (`unlink_plex`) clears the token singleton and
  removes the single registry entry, keeping `client_identifier` for re-link.
- `src-tauri/src/commands.rs` link flow (~975-993) — persists the singletons
  and upserts the one `PlexSource`.
- `src/lib/Settings.svelte:412` — the Connected tab branches on
  `s.kind === "plex"` to show a single account-level Disconnect; every other
  source gets a per-id Remove.

## Evidence: what is already multi-Plex-safe

- `kind_rank` / `detail_rank` (`commands.rs:1247,1258`) rank by **kind**, not
  id — N Plex backings all rank equally; no code change needed to merge them.
- `rank_backings` routes play/watch/detail identities through a
  source_id→kind map; it is id-agnostic and handles any number of Plex ids.
- Each `PlexSource` owns its own `PlexLibrary` and its own machine-identity
  binding (`plex.rs` `ensure_ready_*`); instances are self-contained.
- The merged data plane (dedup, overrides, watch edits) keys by source id and
  canonical item id; distinct Plex ids stay separated (prior read-only
  tracing, re-confirmed on current main).

## Hazard to carry into any design

Rediscovery on an **unpinned** instance may repoint the source at any server
on the account (`plex.rs` codex r7 commentary). With two sources on one
account this becomes cross-talk: both could land on the same server, or swap.
Every additional Plex source must be born pinned to a `machineIdentifier`,
and the reachability probe's acceptance of identifier-less discovery entries
must not apply to secondary sources.

## Owner decisions (answered 2026-07-19)

1. **Scope: multiple Plex accounts.** Each link adds one account bound to one
   server, exactly like today's single Plex — the link flow just becomes
   repeatable. An account with several servers can be linked again to add the
   second server as its own source. Every new source is born pinned to its
   `machineIdentifier` (hazard above).
2. **Re-key everything.** The legacy `"plex"` id goes away; every Plex source
   (including the existing one) gets a unique id minted at link time. A
   migration must re-key the existing config binding and sweep every
   persisted store that names `"plex"` (per-title overrides, playlists, any
   other stored source references — enumerate at implementation time).
3. **Credentials move into the source list.** Follows from #1: the one-slot
   account fields (`auth_token`/`client_identifier`/`last_server_*`) are
   retired by the migration; each Plex login is stored on its own `sources`
   entry with its own token, like Jellyfin/Emby.
4. **No account-wide disconnect.** Each Plex source gets its own per-row
   Remove in Settings, same as Jellyfin/Emby. `unlink_plex` and the
   `remove_source` Plex refusal are retired. Removing one source never
   touches another.

5. **Duplicates keep collapsing (answered 2026-07-19).** The same title on
   two Plex accounts shows as one library entry; the per-title override
   remains the escape hatch — identical to Plex+Jellyfin behavior today.

## Decision: which copy plays

**Owner decision: the user decides, in Settings.** Which copy of a
duplicated title plays is governed by a user-facing Settings control —
not by a hardcoded source order, not by runtime capability/bitrate
heuristics, and not by a play-time prompt that saves a default. The
existing per-title override remains the escape hatch above whatever the
setting says.

Rejected by owner: (a) stable added-order pick; (b) automatic
"best-copy" selection (HDR/resolution/bitrate/connection); (c)
first-play picker with remembered choice.

The exact shape of the Settings control is an implementation detail:
drafted in the collapse slice and shown to the owner for review before
build, not invented in this plan.

## Implementation slices

1. **Config foundation + migration.** Teach the restore path to build a
   `PlexSource` from a `sources` entry (a Plex sibling of
   `jellyfin::build_source`, machine-id pin included). One-shot migration:
   fold the legacy `auth_token`/`client_identifier`/`last_server_*` fields
   into a minted-id `sources` entry, then sweep every persisted `"plex"`
   reference (per-title overrides, playlists — enumerate stores during the
   slice) to the minted id. Legacy fields retired.
2. **Repeatable link flow.** Link command mints a fresh id per link and binds
   exactly one server, pinned at birth: auto-bind when the account has one
   reachable server, frontend picker when several. `unlink_plex` and the
   `remove_source` Plex refusal retired; removal goes through the normal
   per-id path.
3. **Settings.** Plex rows get per-row Remove like every other source; the
   `s.kind === "plex"` Disconnect branch goes away; link button stays and can
   be used repeatedly.
4. **Verification.** Unit coverage for migration + restore + link mint;
   e2e scenario with two mock Plex sources proving separation (independent
   remove) and collapse (shared title, one row, override works).

Each slice lands reviewable on the feature branch; Claude codereview gates
the merge per repo policy.

## Implementation log

- **Slice 1 — config foundation + migration: COMPLETE** (`a0c2d14`; live
  persistence guard `ef0bca4`). Plex credentials, saved endpoint, and stable
  machine identity now live on provider-neutral `sources` rows; startup builds
  every Plex row independently and rediscovery/identity learning updates only
  the matching row without blocking an async worker.
- The one-shot migration mints a non-legacy id, re-keys the config's last
  section, merged overrides, recents (including hierarchy/watch/detail/backing
  identities), Continue Watching tombstones, per-library sorts, and every Vela
  playlist item. A persisted cross-file marker makes a crash or unreadable
  playlist retry the same minted id; a missing playlist file stays missing.
- Twenty-two production mutations separately proved the startup hook,
  credential transfer and retirement, every routing-key family, playlist
  rewrite, retry/fail-closed behavior, idempotence, endpoint/credential/pin
  restore, invalid-pin refusal, exact-row binding updates, and the live
  identity-to-persistence handoff. Each failed its intended assertion and was
  restored from the committed implementation. Restored Rust 1.89/stable check,
  stable clippy, all Rust tests, and Cargo audit pass; audit reports only the
  repository's accepted warning-class notices.
- **Slice 2 — repeatable link flow: COMPLETE** (`64291bb`; decision/UI guard
  hardening `54fe020`). Every authorization mints a fresh `plex-{uuid}` source
  bound at birth to one identity-verified direct HTTPS machine. One reachable
  machine connects automatically; several pause on a name-only frontend picker
  while the token remains in a bounded, expiring backend session. Completed
  sessions are retained briefly for idempotent poll retries. The literal Plex
  id, account-wide unlink command, and Plex removal refusal are retired; normal
  per-id removal preserves every other source.
- Twenty-two production mutations separately proved fresh ID minting; token,
  device, endpoint, and machine-pin persistence; unpinned, non-HTTPS, and relay
  refusal; credential-free picker serialization; exact-machine selection;
  independent removal; session expiry and bounds; identifier-less and duplicate
  candidate rejection; wrong and missing identity rejection; zero/one/many
  reachable-server decisions; and both frontend choice handoffs. Each failed its
  intended assertion and was reverse-applied to the committed bytes. Restored
  Node/npm pins, clean install, npm audit/check/build, Rust 1.89/stable checks,
  clippy, all 167 Rust tests, and Cargo audit pass; both audits report zero known
  vulnerabilities and Cargo retains the accepted 17 warning-class notices.
- **Slice 3 — Settings per-row removal: COMPLETE** (`bfe1a2c`). The Connected
  list no longer treats Plex as an account singleton: every provider row calls
  normal removal with its exact source ID and displays Remove, while Link Plex
  remains available under Servers for repeat use. Three independent UI
  mutations (legacy hardcoded ID, account-wide unlink, and missing repeat-link
  action) failed the intended guard and restored exact; frontend check and
  production build pass.

## Non-goals (until decided otherwise)

No change to Jellyfin/Emby handling, no change to the kind ladder
(Plex-first), no local-family resurrection, no migration of
`docs/history/state-archive.md` records.
