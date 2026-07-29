# Vela 1.0.59

Vela 1.0.59 makes every existing library sort reversible with an independent
ascending or descending direction.

## Highlights

- Choose what to sort by from a direction-neutral dropdown, then toggle the
  adjacent boxed `↑` or `↓` control to reverse the order.
- Change the sort field without losing the selected direction, or change the
  direction without losing the field.
- Restore each source library's complete sort choice after restart. The merged
  **All** view remains session-only, matching its existing behavior.
- Use the arrow control from a keyboard or assistive technology through its
  dynamic direction label and tooltip.

## Downloads

Native packages are attached for macOS, Windows, Debian/Ubuntu, RPM-based Linux,
Arch Linux, and AppImage users. `SHA256SUMS` covers every promised package.

Vela and its installers are unsigned, so macOS Gatekeeper or Windows
SmartScreen may require explicit approval. mpv 0.38 or newer remains a separate
required installation.

## Server status

- **Plex:** primary and most deeply exercised integration.
- **Jellyfin:** supported.
- **Emby:** experimental; it shares the Jellyfin-family implementation but has
  not been verified against a real Emby server.

## Known limitations

The rare queued watch-edit race disclosed in 1.0.0 remains an accepted
limitation. Full setup and privacy details are in the
[README](https://github.com/roethlar/vela#readme).

[Changes since 1.0.58](https://github.com/roethlar/vela/compare/v1.0.58...v1.0.59)
