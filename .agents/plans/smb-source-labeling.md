# Plan: SMB/SSH mounts as named sources (stop labeling them "Local")

Status: APPROVED 2026-07-04 (owner), with each plan's "proposed" defaults adopted. Covers the `ISSUES.md` entry
(Open - Owner-Reported 2026-07-04): SMB shares surface labeled "Local" in the
source chips and nav instead of being identified as SMB.

## Facts (confirmed by code reading, 2026-07-04)

- There are exactly four source kinds today: `plex`, `jellyfin`, `emby`,
  `local` (`src-tauri/src/source/mod.rs:84-85`). SMB and SSH have no source
  identity: their selected folders are flattened into plain `LocalFolder`s
  and merged into the single local source — boot merge
  `runtime_local_folders` (`src-tauri/src/lib.rs:394-409`), runtime twin
  `live_local_folders` (`src-tauri/src/commands.rs:995-1044`) — discarding
  their origin.
- The literal "Local" comes from `LocalSource::name()`
  (`src-tauri/src/source/local.rs:289-291`), stamped onto every section
  (`local.rs:315`), and hardcoded `SourceDto { id:"local", name:"Local" }`
  returns in the mount/add commands (`commands.rs:320, 505-509, 643, 921`).
- The identity we want already exists and is persisted but unused:
  `SmbMount.name` (defaults to `"{server}/{share}"`, `commands.rs:437`,
  `config.rs:56-76`) and `SshMount.name` (`config.rs:95-111`).
- Chips render one button per registered source (`get_sources`,
  `commands.rs:132-143`; `src/routes/+page.svelte:588-594`); nav tags come
  from `SectionDto.source_name` (`+page.svelte:601`, hub headers `:801`).
  Registry routing is `"<source_id>:<raw>"` (`mod.rs:159-166`), so multiple
  local-family sources route naturally.

## Design (proposed: one registered source per mount)

Parameterize `LocalSource` with identity instead of hardcoding it:

1. `LocalSource::new(id, name, kind, folders)`; the registry then holds a
   local family:
   - plain configured folders → `("local", "Local", "local")` (unchanged);
   - each SMB mount → `("smb-<mount.id>", mount.name, "smb", its selected
     folders)`;
   - each SSH mount → `("ssh-<mount.id>", mount.name, "ssh", its folder)`.
2. Rebuild both construction paths from the same grouping helper: boot
   (`lib.rs:100-110`) and `rebuild_local_locked`
   (`commands.rs:1065-1080`). Rebuild must replace the whole family:
   upsert current members and remove stale ids whose kind is
   local/smb/ssh (registry needs a small remove/retain operation —
   `upsert` alone can't drop an unmounted share).
3. Mount/add/remove commands return the real `SourceDto` for the affected
   source instead of the hardcoded "Local" one (`commands.rs:320, 505-509,
   643, 921`).
4. The `ssh_folder_ids` exclusion in the merges (`lib.rs:395`,
   `commands.rs:996`) becomes structural: SSH folders simply belong to their
   own source.
5. Frontend needs no structural change: chips gain one entry per mount with
   its human name; nav/hub tags read `movies · nagatha/media` instead of
   `movies · Local`. Optionally style by `kind` later.

Consequences to handle:
- Item keys for SMB/SSH items change prefix (`smb-<id>:<path>` instead of
  `local:<path>`). Local-family items carry no server watch state and the
  play queue is transient, so nothing durable stores the old keys; still,
  verify the queue and any in-flight browse tolerate a rebuild mid-session
  (they already must, since mounts can be removed today).
- Section keys embed folder ids already; per-mount sections shrink to that
  mount's folders (per-source view of a mount shows just its folders).
- This gives the library/All-view rework real per-source identity for its
  source-ranking phase (`.agents/plans/library-all-view-rework.md`).

Non-goals: no change to mount mechanics, credential handling, or the
narrow-roots validation; no nav redesign (that's the rework plan); no
Linux-specific behavior change.

## Verification

- Rust unit tests: grouping helper (config with plain folders + SMB mount +
  SSH mount → expected source family), rebuild replaces stale mount sources,
  mount command returns the mount's own SourceDto.
- Full CI set: `npm run check`, `npm run build`; from `src-tauri/`:
  `cargo check --locked`, `cargo clippy --all-targets --locked -- -D
  warnings`, `cargo test --locked`.
- Owner playtest: with a plain folder + an SMB share configured, chips read
  `All | Plex | <server> | <share name> | Local`; nav entries tag with the
  share name; browse + playback from the SMB share still work; unmounting
  removes its chip.

## Open points to settle at approval

1. Confirm per-mount sources over the minimal alternative (keep one "Local"
   source, only tag each section's `source_name` with its origin). The
   minimal version avoids key-prefix changes but keeps one merged "Local"
   chip, which only half-fixes the report.
2. Chip naming for the plain-folders source when mounts exist ("Local" vs
   "Folders").
3. Whether `kind` should surface in the UI (e.g. small SMB/SSH glyph) or
   name-only is enough for v1.
