# Plan: Vela 1.0 release readiness and publication

Status: **COMPLETE — Vela 1.0.0 was published on 2026-07-20.** The owner
directed the agent to handle all open release items, create the release, and
publish it with `gh`. This executed the already-approved 2026-07-15 release
decisions: unsigned native binaries, an Arch artifact suitable for AUR,
fail-closed artifact collection, experimental Emby labeling, and disclosure of
the accepted queued watch-edit race.

## Goal

Publish Vela 1.0.0 from a clean, fully verified `main` only after every known
release blocker is closed and every promised native artifact is proven present.
The public GitHub release must contain release notes and installable macOS,
Windows, Linux, and Arch outputs. No signing or notarization is introduced.

## Authority and boundaries

- The owner's 2026-07-20 instruction authorizes the code fixes, release
  workflow changes, version bump, tag push, draft creation, artifact upload,
  and final GitHub publication in this plan.
- Existing provider credentials and live-server access may be used only through
  the approved live-E2E rails in `.agents/machines.md`. Every touched server
  state must be restored on success, failure, and interruption.
- Do not add a second Plex server, bundle mpv, add signing credentials, claim
  stable Emby support, publish telemetry, or reopen settled playback behavior.
- Each implementation finding is one commit. Every new guard is independently
  regressed, must fail for the intended reason, then is restored and rerun.
- A failed workflow, missing artifact, failed smoke, or failed restoration stops
  publication. Never force-push, rewrite history, or publish a partial release.

## Slice 1 — close the native macOS wrapper defect

Fix `scripts/build.sh --native` under macOS Bash 3 without changing the default
universal build or the Linux/Arch dispatch. Extend the existing build-script
guard to execute the empty-native-argument path under the repository's static
harness. Prove the guard by restoring the unsafe empty-array expansion, observe
the exact failure, restore the fix, then run the focused guard and wrapper
syntax checks. Commit this slice alone.

## Slice 2 — make release artifact production fail closed

Update `.github/workflows/release.yml` and its static guard so that:

- macOS must produce its universal DMG;
- Linux must produce AppImage, deb, and rpm artifacts;
- Windows must produce MSI and NSIS artifacts;
- an Arch Linux job builds a native `.pkg.tar.zst` from the tracked PKGBUILD as
  a non-root user with the pinned Node/npm contract;
- each promised artifact type has its own fail-closed assertion/upload path;
- a final inventory job cannot succeed unless every platform artifact was
  downloaded and recognized;
- tagged runs continue to create only a draft release; publication remains a
  deliberate `gh` step after inspection.

The guard must separately fail when missing-artifact handling is weakened,
when the Arch job is removed, and when any promised artifact class disappears.
Validate workflow syntax with `actionlint` when available and run the focused
guard. Commit this slice alone.

## Slice 3 — close live and documentation release gates

Run the existing live Plex/Jellyfin coverage first. Close the real-Plex natural
completion gate using the approved live harness: snapshot one safe episode's
played/resume state, drive a clean EOF through the real app, verify the completed
episode disappears, Plex reports it watched, `only-tv` starts the next episode,
and the successor appears without manual Refresh, then restore the episode state
on every exit path. If the current harness cannot do this safely, add the
smallest test-only leg and restoration rail needed; do not weaken the exact
behavior. Re-run the stopped-server error leg if the durable record still
requires it.

Reconcile `ISSUES.md` and `.agents/state.md` against landed evidence: close or
reclassify stale Open headings, update the current version, and retain only
genuinely open/nonblocking edges. Add release notes that explicitly disclose
the rare accepted queued watch-edit race, unsigned packages, external mpv
requirement, experimental Emby support, and the inspection-only multi-Plex
rebind frontend edge. Commit code/test changes separately from documentation.

## Slice 4 — release graphics and exact 1.0.0 candidate

Capture credential-free screenshots from the real application using hermetic
fixtures. Add only assets that show current Vela UI and contain no third-party
library artwork, server names, tokens, or user data. Produce the documented
launch/social graphic from those safe assets and reference the primary
screenshot in the README or release notes.

Bump every version surface once from 0.1.62 to 1.0.0 using the repository bump
script. Run the complete canonical macOS command set, fresh Linux real-app E2E,
Linux packaging, Windows native compiler/clippy/tests and installer build, and
the Arch package build. Run a manual `workflow_dispatch` release build from the
exact candidate commit and download its artifacts. Verify expected filenames,
embedded versions where available, nonzero sizes, and SHA-256 checksums. Install
or inspect each artifact on its available native venue. Commit the version bump
alone after all code/docs slices.

## Slice 5 — tag, inspect, and publish

Require green GitHub CI on the exact 1.0.0 candidate. Create annotated tag
`v1.0.0`, push the candidate and tag to the configured remotes under the
owner's explicit publication authorization, and wait for the tagged Release
workflow. Download and inspect the tagged artifacts; attach the Arch artifact
and checksum manifest with `gh` if the native action did not attach them.

Use `gh` to verify the draft's title, notes, tag/target commit, and complete
asset inventory. Publish the draft only after all checks pass. Confirm the
public release and asset list through `gh release view`. Do not create or
publish a replacement tag if any earlier step fails.

## Completion evidence

Record exact commits, guard red proofs, canonical/native test results, live
state restoration, workflow run URLs, artifact filenames/checksums, tag target,
and published release URL in this plan and `.agents/state.md`. The release is
complete only when the repository is clean, every configured remote points to
the intended commit/tag, GitHub reports the release published, and no live
fixture or service was left mutated.

### Landed release slices

- Approved plan: `88d19d1`.
- macOS Bash 3 native-wrapper repair: `404c5ec`.
- Fail-closed native/Arch release workflow: `056679d`; nested artifact
  inventory repair: `bb3ba1b`; self-excluding checksum manifest: `06df681`;
  explicit least-privilege tagged bundle permission for future tags:
  `9f97355`.
- Safe real-Plex completion coverage: `1dbf8da`; deterministic Plex startup
  readiness: `954a98e`.
- Release notes/docs: `a57a5ad` and `8f29aa0`; 1.0.0 version surfaces:
  `d2ebb54`; safe screenshots and launch graphic: `cf0503b`.
- Every new release/live/build guard was independently regressed and failed
  for its intended reason before restoration: Bash 3 empty arrays, missing
  handling, Arch removal, all seven artifact classes, nested inventory,
  live-fixture registration/startup/restore/refresh behaviors, checksum
  self-inclusion, and tagged bundle write permission.

### Exact verification

- Candidate/tag commit: `06df6812d7fe81185213778669fcaa87680ac83b`.
  Exact GitHub CI passed in
  `https://github.com/roethlar/vela/actions/runs/29725629732`; the successful
  exact-commit release rehearsal, including non-root Arch and all seven native
  artifact assertions, is
  `https://github.com/roethlar/vela/actions/runs/29725629650`.
- macOS canonical verification passed: exact Node/npm, clean install and zero
  npm vulnerabilities, frontend checks/build, Rust 1.89 and stable checks,
  warning-free clippy, 205 Rust tests, and zero Rust vulnerabilities. The local
  universal DMG verified, reported version 1.0.0, and contained `x86_64` and
  `arm64` binaries.
- The exact Linux VM run passed all 31 real-app scenarios. Native ARM64 deb and
  rpm bundles built; the deb reported package `vela`, version 1.0.0, and
  architecture `arm64`. VM-local `cargo-audit` installation was unavailable
  because its 1.7 GB tmpfs filled, so the exact macOS audit and exact GitHub
  audit are the authoritative Rust vulnerability proofs.
- The real-Plex completion run proved clean EOF, Plex watched state, automatic
  `only-tv` continuation, recents replacement, and UI refresh without a manual
  reload. The stopped-server edit/scan/restart leg also passed. The touched
  movie and two episodes were restored to unwatched/zero, credentials were
  deleted, and the Plex service/watchdog were restored. The current host had no
  saved Jellyfin source (`source_count 0`), so no new live-Jellyfin run was
  possible; the prior real-server proof and exact hermetic paths remain the
  recorded coverage.
- The exact 1.0.0 NSIS installer checksum matched on `netwatch-01`, replaced
  0.1.62 with exit code zero, and left both executable metadata and uninstall
  registration at 1.0.0. Windows HDR had already been owner-confirmed on that
  native venue.

### Published artifacts

The uploaded draft was downloaded back through `gh`; its seven-line manifest
verified every package, did not include itself, and the downloaded DMG passed
`hdiutil verify`. Package metadata reported Arch `vela` 1.0.0-1 x86_64 and
Debian `vela` 1.0.0 amd64.

```text
94ee8221ded684b3c27bfdede62e85997f8d47920315bda30b27c51a8693ba97  vela-1.0.0-1-x86_64.pkg.tar.zst
05466f35e660d8f1a3810c2026a01906ece0b4bbe8c2de00e232db0e0a184e23  Vela_1.0.0_amd64.AppImage
02166e9b7cbb3c02cf17280e992cb721c0130dbb0df487ebf12f52bd1e48c8ef  Vela_1.0.0_amd64.deb
88c0a0db0d148d761b96453cf1ccb6d5254793accf94f78854c94177f97b5cc6  Vela-1.0.0-1.x86_64.rpm
b9d6e4ec0fca1d56a0eb9424d2a16391a85c2bf8a943d1c71bb2d08ddd6da852  Vela_1.0.0_universal.dmg
e460e7f123796290a2acce141f1a424249efc3debc0e6b349a89470d82783d0f  Vela_1.0.0_x64_en-US.msi
79ae56fc3f57b19dd9924466e03b8d0a8f6b8b83e7fe01ca11ecd3d7a180fd0b  Vela_1.0.0_x64-setup.exe
```

Annotated tag `v1.0.0` peels to the candidate commit above. GitHub's initial
tag push exposed that the immutable tag's bundle jobs lacked release-write
permission; no release was published by those failed runs. The cause is fixed
durably on `main` at `9f97355`. Publication then used `gh` with the already
successful exact-commit rehearsal artifacts, followed by a full GitHub
download/checksum round trip. Repository workflow defaults were restored and
verified at `read`.

GitHub reports the release non-draft, non-prerelease, Latest, targeted at the
exact candidate, with eight uploaded assets. Published URL:
`https://github.com/roethlar/vela/releases/tag/v1.0.0`.

## Maintenance release: Vela 1.0.58

Vela 1.0.58 was published on 2026-07-27 from exact commit
`791ff479de29fa264f367b32b1c06dbee00160fa`. Annotated tag `v1.0.58` has tag
object `80982e7b97a599240e8af3893d486fe688d45db9` and peels to that exact commit.
GitHub CI passed on the candidate in
`https://github.com/roethlar/vela/actions/runs/30277549893`.

The tagged release workflow ran at
`https://github.com/roethlar/vela/actions/runs/30278423592`. Its npm audit,
macOS universal DMG, Linux AppImage/deb/rpm, Windows MSI/NSIS, and non-root Arch
jobs all passed. The inventory job recognized all seven promised package
classes and generated `SHA256SUMS`, then failed only while attaching the Arch
package and manifest: that checkout-free job invoked `gh release upload`
without `--repo`, so `gh` could not resolve a repository. Under Slice 5's
manual-attachment rail, the two workflow artifacts were downloaded, verified,
and uploaded with an explicit repository selector. It did not weaken the
1.0.58 artifact proof.

The permanent automation repair landed at `3fa2858`: the checkout-free
inventory job now supplies `--repo "$GITHUB_REPOSITORY"`, and the existing
static release-workflow guard requires that selector on the upload command.
Deleting only the selector made the focused guard fail for that missing
contract; restoring the committed workflow returned all three focused checks
to green. `actionlint` was not installed locally. Exact-commit GitHub CI passed
in `https://github.com/roethlar/vela/actions/runs/30280854039`.

The manifest matched the downloaded Arch package and GitHub's immutable digests
for the other six promised packages:

```text
6bd44d765841d4c7cfae23d4f49e663b9e8fe3a32ad49f7ba8f0f80fbb3635bc  vela-1.0.58-1-x86_64.pkg.tar.zst
7278ed90c868fc1af1577039084e0e16b02bb87ae8d50feb3fdef6c2556d975a  Vela_1.0.58_amd64.AppImage
f4d1b17a76a001cd70341b8558a88d0205f1833db5ad6376fca9cdfaee7d74fd  Vela_1.0.58_amd64.deb
356cdb598525dcedb075e0fad11d031eda7cf6a5445638a9479c6b26f5e34e85  Vela-1.0.58-1.x86_64.rpm
40b610c46b7651044c90382479ef6db899c65244410f57cebf25dc2e44bcddca  Vela_1.0.58_universal.dmg
769d095bb4420196207d6eb53dfd5161957609a8118b3c3729781122e0679332  Vela_1.0.58_x64_en-US.msi
36bce683b8d8bdd5db8df12c4a2bd93975aa5fa5d3de689949fb23b3cc8cfc0a  Vela_1.0.58_x64-setup.exe
```

The published release has nine nonempty assets: those seven packages,
`SHA256SUMS` (`d6894d4cedd80163c702e5bb31be28feedef6371142fa5ce3bd6ef804f37b399`),
and the extra universal app archive
`Vela_1.0.58_universal.app.tar.gz`
(`8a116fdd35202939ed8434efbd301b00adf80d71dd3fcbbef338e87a77d0cd74`).
The manually attached Arch package and manifest were downloaded back from the
public release; the manifest was byte-identical and verified the package.
GitHub reports the release non-draft, non-prerelease, and Latest, with title
`Vela 1.0.58`, the exact target above, and publication timestamp
`2026-07-27T15:24:03Z`:
`https://github.com/roethlar/vela/releases/tag/v1.0.58`.

No VM execution was required for this publication; the exact-commit GitHub CI
and tagged release workflow supplied the release evidence.

## Maintenance release: Vela 1.0.59

Vela 1.0.59 was published from exact commit
`59919210f5e4d2b8b5547acd6b2c7324509286ce`. Annotated tag `v1.0.59` has tag
object `fb0ca13acd1112dbfb65e17d98baa7b1f2836f80` and peels to that exact commit.
GitHub CI passed on the candidate in
`https://github.com/roethlar/vela/actions/runs/30420572850`.

The tagged release workflow passed in
`https://github.com/roethlar/vela/actions/runs/30420740712`: npm audit, macOS
universal DMG, Linux AppImage/deb/rpm, Windows MSI/NSIS, non-root Arch, and the
final inventory job were all green. The inventory job found all seven promised
package classes, generated `SHA256SUMS`, and attached the Arch package and
manifest successfully through the repaired explicit-repository path.

A fresh draft download matched GitHub's immutable digests for all nine
nonempty assets. The seven-line manifest verified every promised package:

```text
f04c9ebb7d82d5eb09b5a7c0e837382b9107d33000c3a4babbb22f2c4ef3f837  vela-1.0.59-1-x86_64.pkg.tar.zst
7d2d320ac6222aeac09c7ab665aaa936b9b292e22b76659c47658bf606759ccc  Vela_1.0.59_amd64.AppImage
34bca1e29f78c4942ddf551092038d3b8672d7975810ba05f411dce98b070edb  Vela_1.0.59_amd64.deb
fb2e3804a23fdae91f125077a4d4a68e64dc2e6969d02cf67a439f3f86e1bc79  Vela-1.0.59-1.x86_64.rpm
072d1cfa3b31e114bf0e33f457eba6e6919e0b35e9db0186d6f969a261864f20  Vela_1.0.59_universal.dmg
9005f3aeaf0c17d1b5acd985699dbd9b4ac4c53640f07aee03479365aa13962d  Vela_1.0.59_x64_en-US.msi
b4dad4631e628c4166367c262ce0cedbbbe3ddb510579a7fb6dc2f08a906a9c5  Vela_1.0.59_x64-setup.exe
```

`SHA256SUMS` itself has digest
`f07d583ae6549d13bd8692dccd4bfb7f2bfcf9616792541158f79971c237ff49`;
the extra `Vela_1.0.59_universal.app.tar.gz` has digest
`b1bec643f88cc438bc55f37fa8d71e7285a8b79920072a3ddd894f6c96872c66`.
`hdiutil verify` accepted the downloaded DMG. Both the DMG and universal app
archive reported version/build 1.0.59 and contained x86_64 and arm64 binaries.
Debian control metadata reported `vela` 1.0.59 amd64; Arch metadata reported
`vela` 1.0.59-1 x86_64. File inspection identified the remaining downloads as
the expected x86_64 AppImage, RPM, x64 MSI, and NSIS installer types.

GitHub reports release database ID `361499610`, title `Vela 1.0.59`, exact
target above, non-draft, non-prerelease, and Latest, with publication timestamp
`2026-07-29T04:06:32Z`. The anonymous release page returned HTTP 200:
`https://github.com/roethlar/vela/releases/tag/v1.0.59`.

No VM execution was required for publication; the owner-confirmed product
playtest, exact-commit GitHub CI, and successful tagged release workflow supplied
the release evidence.

## Maintenance release: Vela 1.0.60 (first signed release)

Vela 1.0.60 was published on 2026-08-08 from exact commit
`10b968b583a44c4738ece324b5fcbb9d58276235` — the first release shipping SIGNED
macOS and Windows artifacts, after the `ci/release-code-signing` merge at
`b9c6199`. Annotated tag `v1.0.60` has tag object
`a36c36ba1698f170c21b65ffd91e7b22032a7d7a` and peels to that exact commit.

The tagged release workflow passed in
`https://github.com/roethlar/vela/actions/runs/31240332521`: npm audit, macOS
universal DMG, Linux AppImage/deb/rpm, Windows MSI/NSIS, non-root Arch, and
the final inventory job were all green. Both signing assertion steps ran and
passed: `Require a signed and notarized macOS bundle` (codesign deep/strict,
Developer ID authority, stapler validate, spctl) and `Require signed Windows
installers` (Authenticode `Valid` on the MSI and NSIS). Linux and Arch
artifacts are unsigned by design.

A fresh draft download matched GitHub's immutable digests for all nine
nonempty assets. The seven-line manifest verified every promised package:

```text
d8bc9c1b192fb519ef98f3d70d4947b5028f655a364741ae381a568efac02aeb  vela-1.0.60-1-x86_64.pkg.tar.zst
f84df70c82f2b78611f69ec39507685cb32cc63c24b7dbb58b5db1cd24f38ff1  Vela_1.0.60_amd64.AppImage
5e8fdaa54b98255708dbad523db0bd260ca62a942beb27f04d29d9da775ecceb  Vela_1.0.60_amd64.deb
27edf4e9c5c897fb1c0b2b98e19a7f6fefb771de66879f130ffe08e6b37b8109  Vela-1.0.60-1.x86_64.rpm
a8a86be32a526418ec6e4be52db57ad8ce420afdfdc41fc433337b982bf279a1  Vela_1.0.60_universal.dmg
2e9239bcc01433c2dcf8b492a1a3cb1f0200aebcd77f696f562114de006590fa  Vela_1.0.60_x64_en-US.msi
a62dc7768a9118fc119c24f7f5930ba5bdec3678773d877e4b6e2cc71c322dfb  Vela_1.0.60_x64-setup.exe
```

`SHA256SUMS` itself has digest
`c4a58ebad21b08a70b5e508e3b163ec6c586715714433c4c12b36a8f273794dd`;
the extra `Vela_1.0.60_universal.app.tar.gz` has digest
`289470fdf5327a347a56dc3a8613098fc7ea04478e06247539f8df090d91c449`.

Local macOS inspection of the downloaded DMG: `hdiutil verify` VALID; the DMG
is `Developer ID Application: MICHAEL COELHO (27R2KCAHN7)` signed and
satisfies its designated requirement; the bundled app passes
`codesign --verify --deep --strict`, `xcrun stapler validate` (notarization
ticket stapled), and `spctl --assess` reports `accepted` with
`source=Notarized Developer ID`; it reports version 1.0.60 and contains
x86_64 and arm64 binaries. Windows Authenticode validity is evidenced by the
in-run assertion above (no Authenticode verifier on the macOS dev host).

GitHub reports release database ID `367113525`, title `Vela 1.0.60`, exact
target above, non-draft, non-prerelease, and Latest, with publication
timestamp `2026-08-08T05:03:46Z`. The anonymous release page returned HTTP
200: `https://github.com/roethlar/vela/releases/tag/v1.0.60`. The release
notes body is the `RELEASE_NOTES.md` content at the tag.

Local pre-tag verification ran the full repo set with the pinned toolchain
(Node 26.5.0/npm 12.0.1 from a checksum-verified official archive, staged in
`/tmp` because the dev host's Homebrew Node had drifted to 26.7.0/12.0.2):
toolchain assertion, `npm ci`, `npm audit` (0 vulnerabilities),
svelte-check (0/0), production build, `cargo +1.89.0 check --locked`,
`cargo +stable check --locked`, `clippy --all-targets --locked -D warnings`,
`cargo +stable test --locked`, and `cargo audit` (only the standing allowed
RUSTSEC-2024-0429 notice) — all green.
