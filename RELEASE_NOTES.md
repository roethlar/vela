# Vela 1.0.0

Vela 1.0 is the first public release of the HDR-first desktop client for Plex,
Jellyfin, and experimental Emby. It keeps the library in a focused native app
and hands playback to your installed mpv, preserving mpv's codec, GPU,
tone-mapping, and HDR capabilities.

## Highlights

- Browse, search, and play across multiple media-server connections from one
  desktop interface on macOS, Windows, and Linux.
- Merge duplicate titles across servers and choose them with **Prefer Best**,
  **Prefer Compatible**, **Prefer Fastest Source**, or **Ask Every Time**.
  **Play Version** supplies a per-title override in automatic modes.
- Keep title-level watched state aligned across every currently connected copy,
  including after a natural playback completion.
- Resume from the Continue Watching cover-flow, curate it from item menus, and
  automatically continue through TV episodes or the rendered carousel.
- Create cross-server Vela playlists and browse server-owned playlists without
  modifying them.
- Use native display detection and manual display overrides for compatible
  source selection. Windows HDR playback was validated on a real HDR system.
- Choose from eleven themes, including a literal-black OLED palette, with
  restrained motion and progressive artwork reveal.

## Downloads

The release includes:

- macOS universal DMG (Apple Silicon and Intel)
- Windows NSIS setup executable and MSI
- Linux AppImage, Debian package, and RPM
- Arch Linux package suitable for local installation and AUR publication
- `SHA256SUMS` covering every release artifact

Vela and its installers are currently unsigned. macOS Gatekeeper or Windows
SmartScreen may require an explicit approval. mpv is intentionally not bundled;
install mpv 0.38 or newer separately, or use Vela's first-run installation help.

## Server status

- **Plex:** primary and most deeply exercised integration, including live-server
  browse, detail, playback, completion, watch-state, scan, offline-error, and
  recovery checks.
- **Jellyfin:** supported and previously exercised against a real server; the
  complete protocol and playback paths also run in the Linux real-app suite.
- **Emby:** experimental. It shares the Jellyfin-family implementation but has
  not yet been tested against a real Emby server.

## Known limitations

- A rare queued watch-edit interleaving can temporarily hide an item from
  Continue Watching or lose a sub-threshold local resume stamp. It requires a
  slow or failing edit, a second queued edit on another item, and a play of that
  second item before its edit begins. Playing the item again repairs the state.
- The backend multi-Plex rebind path is exercised with two isolated Plex mocks,
  but the frontend section-identity comparison does not have a full TLS Plex
  end-to-end fixture. This is an inspection-only coverage edge, not a known
  single-server defect.
- Plex playback requires a reachable direct HTTPS server connection; Vela does
  not default to Plex Relay for HDR streams.
- HDR output ultimately depends on mpv, the GPU backend, display, and operating
  system color-management path.

Full setup, configuration, privacy, and build details are in the
[README](https://github.com/roethlar/vela#readme).
