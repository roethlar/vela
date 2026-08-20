# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- 2026-08-20 macOS AirPlay synchronization diagnostic at
  `.agents/plans/macos-airplay-sync.md` is owner-approved and in progress;
  production edits remain unapproved. On the same AirPlay route, the
  credential-free stereo flash/click fixture stayed physically synchronized
  after startup with both forced CoreAudio and forced AVFoundation. CoreAudio
  swallowed the first click; AVFoundation swallowed about half of it. Both
  reported near-zero internal `avsync`. Installed Vela 1.0.60 has no saved
  advanced mpv args, own config, or autocrop override. Its real server-backed
  play reproduced roughly two seconds of physically late audio while IPC still
  reported CoreAudio, zero audio delay, and `avsync=0.006 ms`; unlike the local
  fixture, it was network MKV with 48 kHz E-AC-3 5.1 downmixed to stereo.
  Owner separately approved changing that live process to AVFoundation over
  IPC; mpv switched without an explicit reload or relaunch and physical sync
  became correct (`avsync=0.359 ms`). This proves the existing AVFoundation AO
  fixes the affected stream and eliminates any need for a guessed delay or
  AirPlay detector. Codec/channel-layout/input-path causality remains unproved
  but no longer blocks backend selection. Next evidence gate is whether
  AVFoundation follows built-in <-> AirPlay route changes during one play and
  remains correct on ordinary output; that system-output mutation awaits a
  separate owner `go`. Plezy `current-ao` is no longer required for root cause.
  The
  listener/polling/2,000 ms compensation design remains withdrawn. The two
  retained Claude verdicts over `32ef827f..9a79bb3` remain historical evidence
  at `.agents/review/openreview-2026-08-20-airplay-plan.md`; no verdict was
  discarded and no further Claude session is authorized.
- 2026-08-20 owner review-process constraint: never start more than one Claude
  session without explicit owner approval; never impose a low timeout, end a
  Claude process, or discard any verdict for any reason without explicit owner
  approval. A failed capability/schema check is reported with its verdict
  retained and does not authorize a retry.
- 2026-08-20 explicit local signed-build path is implemented at
  `scripts/build-signed-macos.sh`: macOS-only, Keychain-identity based, loads
  sibling `Certificates` API metadata without reading `.p12` passwords, fails
  closed on partial/unsafe credentials, delegates compilation to `build.sh`,
  then independently checks DMG/app Developer ID signatures, staple,
  Gatekeeper, and both universal architectures. Hermetic guards cover the
  success and pre-build refusal paths; removing only `spctl` failed its guard
  for the exact missing assessment, restored green. Real Apple submission and
  artifact evidence are machine-specific in `.agents/machines.md`.
- 2026-08-20 macOS 27 local-network failure under investigation: Jellyfin at
  `altiera:8096` is reachable from the host and answers its public endpoint,
  but the installed `/Applications/Vela.app` is unsigned/ad-hoc and fails its
  authentication transport request. A fresh locally signed, notarized,
  Gatekeeper-accepted universal build independently confirmed the packaged
  `Info.plist` still lacks Apple's `NSLocalNetworkUsageDescription`; Apple says
  apps that connect directly to local hosts should declare it. `altiera` also
  advertises four IPv6 addresses that refuse 8096 before its working IPv4
  addresses; direct-IP Vela isolation test is still pending. Local signing
  setup details live in `.agents/machines.md`; no product fix is approved yet.

- The 2026-08-05 README refresh split the old monolith: README.md is now
  end-user-facing with new real-library screenshots (`docs/images/vela-*.png`,
  server name scrubbed to "Plex"); deep user reference moved to
  `docs/usage.md`, build/dev/architecture to `docs/development.md`. The
  2026-07-25 transcoding-statement guards in `tests/transcoding-ui.test.mjs`
  were repointed from README.md to docs/usage.md with owner approval.
- Vela 1.0.60 is published as GitHub's Latest release from exact commit
  `10b968b583a44c4738ece324b5fcbb9d58276235` — the FIRST SIGNED release:
  the macOS universal DMG is Developer ID signed and notarized (spctl
  `Notarized Developer ID`), the Windows MSI + NSIS are Authenticode signed
  (Azure Trusted Signing); Linux and Arch artifacts remain unsigned by
  design. Its canonical workflow, checksum, local signature-inspection, and
  publication evidence lives in `.agents/plans/v1-release-readiness.md`.
- Product behavior remains as settled in `.agents/decisions.md`: Vela is a
  multi-server Plex/Jellyfin/experimental-Emby client, delegates HDR playback
  to external mpv, uses title-level watched state across duplicate copies, and
  offers Prefer Best, Prefer Compatible, Prefer Fastest Source, Ask Every Time,
  and per-title Play Version.

- JS toolchain pin moved to Node 26.7.0 / npm 12.0.2 (2026-08-18): `.node-version`, `packageManager`, and the four CI/release `npm install --global` sites. `engines.node` remains `>=26.5.0 <27`. The Linux E2E VM is still on the old user-local 26.5.0/12.0.1 tree and will fail the checker until that host is realigned.
- Release code signing merged at `b9c6199` (2026-08-07) shipped in 1.0.60.
  The wiring detail and durable tauri-cli signing facts live in
  `docs/history/state-archive.md` (2026-08-08 rotation). The merged
  `ci/release-code-signing` branch still exists locally and on `github`;
  deleting it is owner-gated.

## Next

- Resolve the macOS AirPlay plan's admitted review choices one owner decision at
  a time, beginning with bounded CoreAudio polling versus one app-lifetime
  listener. Then revise or review the plan only under explicit owner direction.
- Owner decision after direct-IP isolation: plan the macOS local-network fix
  (package the usage description, guard the built app metadata, and surface
  actionable transport causes instead of reqwest's shallow request error).

- Realign the Linux E2E VM from user-local Node 26.5.0/npm 12.0.1 to the new repo pin (26.7.0/12.0.2). Owner-gated; do not touch that host without a go.
- Launch marketing drafts are in-repo: GitHub social preview
  `docs/images/social-preview.png` (1280×640) and Reddit copy
  `docs/marketing/reddit-launch-post.md`. Still awaiting owner go to set the
  GitHub repo social image and to post on Reddit.
- Parked future directions, not current blockers: the migration-time one-shot
  Plex-to-Jellyfin/Emby watched-state copy; real Emby integration coverage; and
  a full frontend TLS multi-Plex rebind fixture if a second Plex server or
  suitable trusted mock becomes available.
- The rare queued watch-edit race remains an owner-accepted, disclosed 1.0
  limitation; its durable technical record is
  `.agents/plans/continue-watching-watch-state.md`.
## Blockers

- No known product blocker.
- The unrelated continuation/mpv and `refresh` E2E flakes remain recorded.
  They reproduced during the 1.0.59 full Linux run in `continueon`,
  `playverbs`, and `refresh` (36/39); the changed `sortpersist` scenario passed.

## Verification

- Canonical commands live in `.agents/repo-guidance.md` (Verification). Do not
  duplicate here.
- Linux live-server venue details live in `.agents/machines.md`.
- Release verification immutable artifact hashes live in
  `.agents/plans/v1-release-readiness.md`.

## Active Sources

- `AGENTS.md` and `.agents/repo-guidance.md`
- `.agents/decisions.md`
- `.agents/machines.md`
- `.agents/push-policy.md`
- `.agents/plans/library-sorting.md` and
  `.agents/plans/show-last-episode-sort.md` (landed sorting baseline)
- `.agents/plans/library-sort-direction.md` (landed at `c0d1412`, 1.0.59)
- `.agents/plans/v1-release-readiness.md` (published-release evidence)
- `README.md`, `RELEASE_NOTES.md`, `ISSUES.md`
- `docs/history/state-archive.md` for superseded state

## Unrecorded Repo Memory

- None known.
