# Vela 1.0.60

Vela 1.0.60 ships signed release binaries: the macOS app is Developer ID
signed and notarized, and the Windows installers are Authenticode signed.

## Highlights

- Install on macOS without a Gatekeeper approval prompt — the universal DMG
  (Apple Silicon and Intel) is Developer ID signed and notarized by Apple.
- Install on Windows without a SmartScreen warning — both the MSI and the NSIS
  installer are Authenticode signed through Azure Trusted Signing.
- Read a refreshed end-user README with real-library screenshots; the deep
  user reference moved to `docs/usage.md` and the build/dev documentation to
  `docs/development.md`.
- Pick up dependency security fixes (postcss, SvelteKit, nanoid).

## Downloads

Native packages are attached for macOS, Windows, Debian/Ubuntu, RPM-based
Linux, Arch Linux, and AppImage users. `SHA256SUMS` covers every promised
package.

The macOS and Windows artifacts are signed; the Linux packages and the Arch
package are unsigned, as before. mpv 0.38 or newer remains a separate required
installation.

## Server status

- **Plex:** primary and most deeply exercised integration.
- **Jellyfin:** supported.
- **Emby:** experimental; it shares the Jellyfin-family implementation but has
  not been verified against a real Emby server.

## Known limitations

The rare queued watch-edit race disclosed in 1.0.0 remains an accepted
limitation. Full setup and privacy details are in the
[README](https://github.com/roethlar/vela#readme).

[Changes since 1.0.59](https://github.com/roethlar/vela/compare/v1.0.59...v1.0.60)
