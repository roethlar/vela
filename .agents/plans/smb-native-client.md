# Plan: Native SMB client + loopback streaming proxy (drop Linux mount dependency)

Status: APPROVED 2026-07-04 (owner), as drafted. Process: implement via
`.agents/playbooks/reviewloop.md` with `codex` as the reviewer harness — one
slice ↔ one review unit ↔ one recorded verdict; merge to main stays
owner-gated.

Owner direction (2026-07-04): the GVfs/KIO-FUSE dependency is unacceptable —
"if Vela cannot make the connection itself without the underlying OS mount,
it's worthless." Vela must speak SMB natively on Linux. This supersedes the
2026-05-23 "Keep Linux SMB user-space only by default" decision's *mechanism*
(resolve desktop-session FUSE mounts) while preserving its *constraint* (no
root, no privileged CIFS mounts). A superseding entry goes to
`.agents/decisions.md` when this plan is approved.

## Facts (confirmed by code reading and host checks, 2026-07-04)

- Today SMB is mount-plumbing only: `src-tauri/src/smb.rs` never speaks SMB.
  On Linux it hunts for readable GVfs (`/run/user/<uid>/gvfs/smb-share:…`) or
  KIO-FUSE (`/run/user/<uid>/kio-fuse-*/smb/…`) directories, optionally
  nudging `gio mount` (bounded, non-interactive, `smb.rs:436-443`). With
  neither gvfs nor an active KIO mapping present, mounting fails with the
  "install/enable kio-fuse or gvfs-fuse" error (owner hit this on Arch/KDE;
  the target server also refuses anonymous access, so the non-interactive
  `gio mount` path can never succeed there).
- Selected SMB folders are flattened into the local-family source:
  `smb_runtime_folders` maps each `SmbFolder` (path relative to the share
  root) onto `<mountpoint>/<path>` `LocalFolder`s (`src-tauri/src/lib.rs:428`),
  served by `source/local.rs` (std::fs walking + `listing_cache.rs` +
  `metadata.rs`). Playback hands mpv a plain file path.
- mpv cannot play `smb://` URLs on the owner's system (Arch ffmpeg is built
  without libsmbclient; verified via `mpv --list-protocols`). So native
  browsing alone is not enough — playback needs either a real file path or an
  HTTP stream. mpv already plays HTTP with custom headers via
  `StreamResolution { url, http_headers }` (`source/mod.rs:100-106`), the
  same plumbing Plex uses; HTTP Range seeking is mpv-native.
- `libsmbclient.so` is present on the owner's machine (ships in Arch's
  `smbclient` package, already installed). tokio (full features) is already a
  dependency (`src-tauri/Cargo.toml:24`); no HTTP *server* dependency exists.
- `SmbMount` persists id/name/server/share/username/password/domain/
  mountpoint/folders (`config.rs:68-86`). `SmbFolder.path` is relative to the
  share root, so existing configs translate to native SMB paths without loss;
  only `mountpoint` becomes vestigial.
- macOS (`mount_smbfs`) and Windows (`net use`) mounts work rootless via the
  OS today and are not part of the owner's complaint.

## Decision summary

Linux SMB switches to a native in-process client: browse/list/search via
libsmbclient (through the `pavao` crate), playback via a localhost-only HTTP
Range proxy that translates mpv's byte-range requests into SMB reads. The
Linux mount-hunting path (gvfs/kio candidates, `gio mount`, boot remount) is
removed — native becomes the only Linux SMB path. macOS and Windows keep
their existing OS-mount flows in this plan; going native there is a separate
follow-up decision.

## Design

### 1. SMB client layer (`src-tauri/src/smb_client.rs`)

- Wrap libsmbclient via `pavao` (Rust bindings; sync API). All calls run
  under `spawn_blocking`, honoring the existing "no blocking work on async
  workers / across shared locks" practice.
- Operations: connect/auth check, list directory (names + kind + size +
  mtime), stat, and positioned reads (open, seek/pread, size) for streaming.
- One client context per logical use (listing pass, stream); libsmbclient
  contexts are not treated as thread-safe. Credentials come from the existing
  `SmbMount` config fields; never logged (2026-05-23 token-handling decision
  stands).

### 2. Filesystem provider abstraction in the local family

- Extract the directory-walking/stat surface `source/local.rs` uses into a
  small provider trait (list_dir/stat/read for artwork-sidecar checks), with
  the std::fs implementation preserving current behavior byte-for-byte.
- Add an SMB provider backed by `smb_client`. The local-family pipeline —
  sections from folders, listing cache, metadata extraction, watch state,
  per-mount named sources (2026-07-04 labeling plan) — is reused, not
  duplicated. This is the main refactor risk; it lands as its own
  no-behavior-change slice before any SMB use of it.

### 3. Loopback streaming proxy (`src-tauri/src/stream_proxy.rs`)

- Minimal hand-rolled HTTP/1.1 responder on `tokio::net::TcpListener`, bound
  to `127.0.0.1` on an OS-assigned port, started lazily on first SMB
  playback. No new dependencies; mpv is the only intended client.
- Supports `GET`/`HEAD` with `Accept-Ranges: bytes`, single-range `Range`
  requests (206/416), `Content-Length`, and connection close/keep-alive
  enough for mpv seeking.
- URL shape: `http://127.0.0.1:<port>/<token>` where `<token>` is an
  unguessable 128-bit random handle minted per `resolve_stream` call and
  mapped in-memory to the SMB target. The proxy serves only registered
  tokens — no path component, no directory serving, no traversal surface.
  Tokens carry no credentials; the URL appearing in mpv's title/stats is
  harmless, and `playback.rs` already sets an explicit media title.
- Reads stream through `spawn_blocking` SMB pread calls; a dropped client
  connection aborts the read loop. No request/URL logging.

### 4. Source and command wiring

- `resolve_stream` for SMB-backed items registers the file with the proxy
  and returns the loopback URL with empty headers; everything else in
  `playback.rs` is unchanged.
- `mount_smb` becomes a connect-and-verify step (auth + list share root)
  with the same command surface where practical; `list_smb_directories`
  lists via the native client. The add-SMB UI error copy drops the
  gvfs/kio-fuse instructions.
- Boot: no remount pass for SMB on Linux; sources register directly from
  config. Existing configs migrate transparently (`SmbFolder.path` is
  already share-relative; `mountpoint` is ignored on Linux, retained in the
  struct for macOS/Windows).
- `smb.rs` keeps only the macOS/Windows mount code; Linux mount-hunting,
  `gio` nudging, and the boot remount task are deleted.

### 5. Packaging and docs

- `Cargo.toml`: add `pavao`. Build needs libsmbclient headers
  (`smbclient` on Arch; `libsmbclient-dev` on Debian-family CI if any).
- `packaging/arch/PKGBUILD`: add `smbclient` to `depends`.
- Tauri Linux bundle (deb/rpm/AppImage under `src-tauri/bundle/linux/` and
  `tauri.conf` bundle deps): declare the libsmbclient runtime dependency.
- README/ISSUES updated; superseding decision recorded.

## Security invariants (carry-forward)

- No root, no privileged mounts — unchanged, now trivially true.
- Proxy binds loopback only; unguessable per-stream tokens; no token or
  credential logging; serves only exact registered files.
- Credentials remain in Vela's config under the existing owner-only-perms,
  atomic-save, fail-closed regime; native SMB removes the old "credentials
  in mount process arguments" exposure on platforms that had it.
- Listing/search/playback stay inside configured share folders (same
  root-scoping rule as local folders, applied to SMB paths).

## Slices (one commit each, in order)

1. `smb_client.rs` wrapper + `pavao` dependency; unit tests for path/URI
   mapping. `list_smb_directories` + add/verify flow go native (browsing a
   share works with no mount present).
2. Provider-trait refactor of `source/local.rs` — pure refactor, verified
   no-behavior-change (full CI set green, listing cache intact).
3. Native SMB listing: SMB provider + per-mount sources served natively on
   Linux; listing cache covers SMB as before.
4. `stream_proxy.rs` + SMB `resolve_stream` + playback integration; unit
   tests for Range parsing and the token registry (guard-proof rule: revert
   fix, watch tests fail).
5. Delete Linux mount machinery from `smb.rs`/`lib.rs`; migrate boot path;
   update UI error copy.
6. Packaging (`PKGBUILD`, bundle deps), README/ISSUES, decision entry,
   `state.md` handoff.

## Verification

- Per slice: `npm run check`, `npm run build`, and from `src-tauri/`:
  `cargo check --locked`, `cargo clippy --all-targets --locked -D warnings`,
  `cargo test --locked`.
- Manual playtest against the owner's real share (`10.1.10.206/media`,
  credentialed): add source with no gvfs/kio present, browse folders, play a
  movie, seek forward/back, resume, and confirm watch-state updates.
- Packaging slice: `npm run build:arch` and confirm the package declares
  `smbclient`.

## Risks / open points

- `pavao`/libsmbclient FFI is the biggest unknown (context lifecycle,
  error mapping, large-file pread behavior). Slice 1 proves it against the
  real NAS before anything depends on it.
- Hand-rolled HTTP is deliberately minimal; if mpv needs more than
  single-range GET/HEAD in practice, revisit with a real server crate
  rather than growing the hand-rolled one.
- Windows builds must not try to compile pavao (gate the dependency and
  module `cfg(all(unix))` or narrower) — mac/win keep OS mounts.
- Seek-heavy playback multiplies SMB round-trips; if seeking stutters on
  the LAN, add a modest readahead buffer in the proxy (noted, not built
  up front).
