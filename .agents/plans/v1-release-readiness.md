# Plan: Vela 1.0 release readiness and publication

Status: **APPROVED — the owner directed the agent on 2026-07-20 to handle all
open release items, create the release, and publish it with `gh`.** This executes
the already-approved 2026-07-15 release decisions: unsigned native binaries,
an Arch artifact suitable for AUR, fail-closed artifact collection, experimental
Emby labeling, and disclosure of the accepted queued watch-edit race.

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
