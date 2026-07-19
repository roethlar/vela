# Plan: multiple Plex servers (multi-Plex)

Status: **DRAFT — awaiting owner decisions.** Owner-reported 2026-07-18
(ISSUES.md). Evidence below is from fresh main (post-5248fe6) tracing; no
implementation may start until the owner answers the decisions section.

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

## Owner decisions needed (plain English)

1. **What does "multi-Plex" mean for you?**
   (a) several servers under your one Plex account; (b) several Plex
   accounts; (c) both. — (a) is the cheapest useful step: discovery already
   lists every server the account can reach. (b)/(c) additionally need
   per-source tokens and a reworked link flow.
2. **Identity of the existing server.** Keep the current binding as id
   `"plex"` and mint `plex-<machineIdentifier>` ids only for added servers
   (no migration; persisted per-title overrides and watch keys that mention
   `"plex"` keep working), or re-key everything uniformly (needs a config
   and override migration). Recommendation: keep `"plex"`.
3. **Where credentials live.** Keep the account singletons and add a
   per-server binding list, or fold Plex entries into `sources` like
   Jellyfin/Emby (bigger config migration; changes what unlink means).
4. **Settings behavior.** With N servers: does account Disconnect drop all of
   them? Do added servers get per-row Remove? When you link an account that
   has several reachable servers, auto-add all of them or show a picker?
5. **Tie-breaking between Plex servers.** Today equal-rank backings fall back
   to registry order. Accept that, or do you want an explicit per-server
   priority control?

## Non-goals (until decided otherwise)

No change to Jellyfin/Emby handling, no change to the kind ladder
(Plex-first), no local-family resurrection, no migration of
`docs/history/state-archive.md` records.
