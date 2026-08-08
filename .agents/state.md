# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- The 2026-08-05 README refresh split the old monolith: README.md is now
  end-user-facing with new real-library screenshots (`docs/images/vela-*.png`,
  server name scrubbed to "Plex"); deep user reference moved to
  `docs/usage.md`, build/dev/architecture to `docs/development.md`. The
  2026-07-25 transcoding-statement guards in `tests/transcoding-ui.test.mjs`
  were repointed from README.md to docs/usage.md with owner approval.
- Vela 1.0.59 is published as GitHub's Latest release from exact commit
  `59919210f5e4d2b8b5547acd6b2c7324509286ce`. Its canonical workflow,
  artifact, checksum, package-inspection, and publication evidence lives in
  `.agents/plans/v1-release-readiness.md`.
- Product behavior remains as settled in `.agents/decisions.md`: Vela is a
  multi-server Plex/Jellyfin/experimental-Emby client, delegates HDR playback
  to external mpv, uses title-level watched state across duplicate copies, and
  offers Prefer Best, Prefer Compatible, Prefer Fastest Source, Ask Every Time,
  and per-title Play Version.

- Release code signing is wired into `.github/workflows/release.yml` on branch
  `ci/release-code-signing` — **pushed and PROVEN GREEN by dispatch run
  31228844221 (2026-08-07)**: macOS universal bundle signed and notarized
  through tauri-cli with every assertion passing; Windows MSI + NSIS signed
  during bundling via Tauri's `bundle > windows > signCommand` hook (CI-only
  `src-tauri/tauri.windows.conf.json`, injected because tauri-action builds
  and uploads in one action), signature assertions passing. Both paths gated
  on non-empty secrets, so a secretless run stays unsigned and green; Linux
  and Arch artifacts are never signed. Two fixes rode the proving run:
  `79e9635` (npm audit lockfile bumps — two advisories published after the
  last audit run were blocking the whole pipeline) and a repo-secret
  correction (`AZURE_SIGNING_ACCOUNT` is `roethlar-app-signing`; the first
  run 403'd on a wrong account name, no workflow change needed). **The
  branch is unmerged — merging into master is owner-gated and is the last
  step before a tagged release ships signed.** Durable facts learned:
  tauri-cli tests the `APPLE_*` variables for presence only (an
  empty-but-defined secret reads as "please sign"); it staples the `.app`
  while signing but never notarizing the `.dmg` that wraps it; a failed
  notarization SUBMISSION fails the build — only notarize-auth LOOKUP
  failures are downgraded to a warning (and with the full API-key triple
  exported, lookup cannot fail).

## Next

- The first signed release will make README.md ("Release binaries are
  unsigned") and RELEASE_NOTES.md stale. Both are accurate for 1.0.59 as
  published, so they were deliberately left alone; update them with the release
  that actually ships signed.
- Launch marketing drafts are in-repo and unpushed: GitHub social preview
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
