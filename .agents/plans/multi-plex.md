# Plan: multiple Plex servers (multi-Plex)

Status: **DRAFT — core decisions answered 2026-07-19; one question open.**
Owner-reported 2026-07-18 (ISSUES.md). Evidence below is from fresh main
(post-5248fe6) tracing; no implementation until the open question is
answered.

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

## Open question

**Duplicates across Plex accounts.** Vela already collapses the same title
found on several servers into one row and picks a default copy to play
(per-title override wins). With two Plex accounts holding the same movie:
(a) keep that behavior — one row, app picks the copy, override per title; or
(b) show the title once per Plex account (breaks with how Plex+Jellyfin
merge today). Awaiting owner answer; the "tie-breaking" question is moot
unless (a).

## Non-goals (until decided otherwise)

No change to Jellyfin/Emby handling, no change to the kind ladder
(Plex-first), no local-family resurrection, no migration of
`docs/history/state-archive.md` records.
