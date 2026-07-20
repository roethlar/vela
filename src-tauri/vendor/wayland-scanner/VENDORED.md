# Vendored wayland-scanner

This directory contains the source of crates.io `wayland-scanner` 0.31.10,
published from Smithay/wayland-rs commit
`a3d7927d87799b2955bf491b51c7c2a3a82da661`. The downloaded crate archive had
SHA-256
`9c324a910fd86ebdc364a3e61ec1f11737d3b1d6c273c0239ee8ff4bc0d24b4a`.

The intentional changes are the two upstream quick-xml compatibility updates:
`Cargo.toml` moves from quick-xml 0.39 to 0.41 (and similar 2 to 3 for parity),
and `parse.rs` uses `xml10_content`. Upstream made the parser/API adjustment in
commit `ec2d932855593d48aa83c76820f3efbcfea86d39` and the security update in
`d07c4f91f28b42e5a485823ffd9d8d5a210b1053`. This avoids RUSTSEC-2026-0194 and
RUSTSEC-2026-0195 without adopting the unreleased, API-breaking Wayland
client/runtime changes that followed the 0.31.10 release.
