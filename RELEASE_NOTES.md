# Unreleased

## Changes

- Show a Plex, Jellyfin, or Emby mark wherever a server's name appears, so
  servers that share a name can be told apart.
- List server playlists directly under their heading, each with its server's
  mark; a server with no playlists to show takes no space, and the heading
  disappears when there are none.
- Emby's experimental status is now shown on its row in Settings.
- Within one run of Vela, open every video's mpv window the way the previous
  one was left: full screen or not, maximized or not, and on the same screen.
  This includes videos you start yourself, not only the next item of a
  playlist or Continue Playing. Custom mpv options such as `--fullscreen` now
  decide only how the first video of a run opens.
- Keep the mpv window's size from one video to the next as well. A video that
  opens full screen or maximized returns to that size when you leave full
  screen. The window's position is not carried. After the first video of a
  run, the carried size takes precedence over a custom `--geometry`.
- Add Settings → Player → Player window, which chooses how the first video
  opens after Vela starts: "As you last left it" (the default; quit from full
  screen and the next session opens full screen), "Full screen", or "Default"
  (mpv and your own options decide).
- With the Prefer Compatible source policy, choose the copy for the screen the
  video will open on, which is now the screen of the previous player window.
- Going back to an earlier Vela after playing anything in this version stops
  at Settings recovery. The player window is saved in `config.json` after the
  first video you play, even if you never open the new setting, and Vela
  1.0.58 to 1.0.60 refuse a settings file that holds it. Starting fresh
  settings there resets every setting except your server connections.

## Fixes

- On Windows, make mpv exit when a file finishes. It previously stayed open and
  idle, so Continue Playing and playlists never advanced and the finished
  item's watched state and resume position were not saved.
- Keep macOS AirPlay/HomePod video and audio synchronized by preferring mpv's
  AVFoundation audio output, with CoreAudio retained as fallback and expert
  overrides preserved.
- Update `h2` to 0.4.16 to address `RUSTSEC-2026-0258`, an unbounded empty DATA
  frame denial-of-service vulnerability.
- Update `rustls` to 0.23.45 to address `RUSTSEC-2026-0285`, TLS 1.3
  handshake messages accepted across encryption-level boundaries.
- Update `devalue` to 5.9.4 to address `GHSA-9rgm-9g3h-6x36` and related
  build-time serialization advisories.

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
