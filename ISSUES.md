# Issue Queue

## Open - Owner-Reported (2026-07-04)

Owner-observed on macOS against a live Plex server, with screenshot evidence.
Recorded as reported; untriaged, no code investigation yet. Other backends
unchecked.

- Continue Watching does not refresh after playback. A video watched to
  completion or partway is only reflected in the Home hubs after an app
  restart.

- Card watch state is stale after playback. Progress bars and played
  checkmarks do not update after a video is finished or its resume position
  moves in either direction, until restart. Plausibly the same root cause as
  the hub-refresh item (no metadata refresh after playback ends) — unverified;
  both recorded as observed.

- Rows mix poster and content-frame artwork. The same row renders 2:3 posters
  next to 16:9 episode thumbnails at different heights (e.g. a movie poster
  beside an episode thumb in Continue Watching). The owner finds this
  distracting; a row should present one consistent artwork shape.

## Kimi-K2.6 Review Triage (2026-05-23)

Review triage from the Kimi-K2.6 report against `vela-foundation` on 2026-05-23.
Items here are verified or worth tracking. Severity is adjusted from the report
where the original claim was overstated.

> **Status (addressed 2026-05-23):** All P0, P1, and P2 items below are
> implemented, with two deliberate exceptions noted inline: `serde-xml-rs` is
> kept (it's used for nested Plex XML; migration deferred, not dropped) and
> `bundle.targets = "all"` is left as-is (Tauri's `all` is host-native, not a
> cross-compile failure). The 5 pre-existing Plex dead-code warnings are now
> silenced with targeted `#[allow(dead_code)]`, so `clippy -D warnings` is clean
> and CI enforces it. Not-runtime-verified: the CSP needs confirming against a
> release build (it doesn't apply to the Vite dev server).

### P0 - Fix Before Merge

- Move blocking OS/process work out of async command bodies.
  `mount_smb`, `unmount_smb`, and `play_item` call OS mount/unmount or child
  process wait/spawn paths from async Tauri commands. Use
  `tauri::async_runtime::spawn_blocking` or make the commands synchronous.

- Fail closed on Plex stream preflight errors.
  `src-tauri/src/source/plex.rs` only rejects `404` during `HEAD` preflight.
  Non-2xx statuses and request errors should surface before launching mpv.

- Add HTTP status checks before parsing Plex XML.
  Several Plex library requests parse response bodies without
  `error_for_status()`, producing confusing XML parse errors for HTTP failures.

- Percent-encode Jellyfin/Emby stream and poster URL query values.
  `stream_url` and `poster_url` interpolate ids, tags, device ids, session ids,
  and tokens into URLs directly. Build these URLs with `url::Url` or
  `url::form_urlencoded`.

- Add keys to dynamic Svelte `{#each}` blocks.
  Source, section, hub, item, crumb, sort, folder, and SMB rows should use stable
  keys to avoid stale DOM state after source switches and list refreshes.

- Restore a restrictive Tauri CSP.
  `tauri.conf.json` currently has `"csp": null`. Add a CSP that allows the app,
  Tauri asset URLs, needed image sources, and no unnecessary script sources.

- Tighten local file exposure through the asset protocol.
  Folder and SMB commands currently expand the asset protocol scope to any path
  passed by the webview. Keep the intentional local-media behavior, but reduce
  XSS blast radius with stricter command validation or a narrower file-serving
  strategy.

### P1 - Security and Reliability Hardening

- Put mpv IPC sockets in a private runtime directory.
  The Unix IPC path is predictable under `/tmp`. Use a per-app private directory
  with owner-only permissions and a random path component.

- Replace the Plex PIN XML attribute helper with a real XML parser.
  The current helper is small and probably fine for Plex's current response, but
  the project already uses `quick-xml`; parse the PIN response with it instead
  of string searching.

- Remove `serde-xml-rs` if practical.
  The project already uses `quick-xml`, and some Plex paths switched to manual
  streaming because `serde-xml-rs` did not fit nested Plex XML well. Migrate the
  remaining struct parses before dropping the dependency. Treat any vulnerability
  claim as unverified until `cargo audit` is available.

- Add `rust-version` to `src-tauri/Cargo.toml`.
  The branch uses APIs such as `Result::inspect_err`; declare the supported Rust
  floor explicitly.

- Revisit `bundle.targets = "all"`.
  Native packaging should be platform-specific unless CI is prepared to build all
  Tauri bundle formats on every OS.

- Add release profile settings.
  Consider size-oriented release defaults such as LTO, `panic = "abort"`,
  fewer codegen units, and strip settings after confirming they do not hurt debug
  symbol needs.

- Bound and decouple metadata cache writes.
  The metadata cache grows indefinitely and writes the full JSON file while
  holding its map lock. Add an eviction policy and avoid long disk writes under
  the lock.

- Render the QR code without raw `{@html}` where possible.
  The current SVG is backend-generated, so this is low risk, but an `<img>` data
  URI or sanitized SVG keeps the UI safer if the data path changes later.

### P2 - UX, Accessibility, and Maintenance

- Fix Settings modal accessibility.
  Remove `role="button"` from the backdrop, move focus into the dialog on open,
  trap focus while open, and restore focus on close.

- Track and clear frontend timers.
  Store timeout ids for clipboard reset and Plex link polling; clear them when
  superseded or on component destroy.

- Add poster image fallback handling.
  Server and online poster URLs should fall back to the no-art placeholder on
  load failure.

- Add a playback option for borderless mpv windows.
  Expose a Vela setting that passes `--border=no` / `--no-border` when spawning
  mpv, so users can remove window-manager decorations from the playback window.

- Add CI.
  Minimum checks: `cargo check`, `cargo clippy --all-targets`, `cargo test`,
  `npm run check`, and dependency auditing once tooling is installed.

### Not Queued From The Report

- SMB credentials in process arguments: already documented and accepted as a
  local-only exposure for this branch. Reopen only if the threat model changes.

- Progress bar hidden when `viewOffsetMs === 0`: hiding a zero-progress bar is
  acceptable UI behavior.

- EDL parser empty URL segment: current EDL strings are generated internally from
  non-empty Plex part URLs; not a practical issue unless external EDL input is
  introduced.

- Broad component/CSS refactors and magic-number cleanup: useful later, but not
  merge-blocking for the current branch.
