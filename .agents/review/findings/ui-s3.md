# ui-s3: Motion and designed empty states

**Severity**: MEDIUM — abrupt navigation, incomplete reduced-motion handling,
and generic or misleading empty states made otherwise valid UI states feel
broken or unfinished.
**Status**: Primary Claude accepted with `guard_confirmed:true`; review closed
by owner ruling at Vela 0.1.55; version bump pending
**Branch**: `main` (approved Slice 3 implementation)
**Base**: `c28dbd24bf6f40f57e96e59146b5d9ffc064334d`
**Implementation**: `78b2f79` through
`841f7493be74122901a0c84fb0f66c1f21005c3e`
**Version**: 0.1.55 pending final Slice 3 bump

## Evidence

At the approved base, Vela had isolated hover and Settings animation but no
coherent navigation entrance, delayed child motion could survive reduced
motion, and several authoritative empty states were absent, generic, or
indistinguishable from loading and failure. The exact motion budget, empty-state
taxonomy, exclusions, and verification contract are owned by
`.agents/plans/ui-embellishments.md` Slice 3.

## Predicted observable failure

Details, seasons, and page navigation snap into place; cast items all enter at
once or accumulate unbounded delay; and reduced-motion users can still wait on
transition or animation delays. Empty Home, library, browse, search, person,
playlist, and episode surfaces either render blank space, expose the wrong
instruction, or appear while a request is still loading or has failed. A root
detail entrance can also create a transient document scrollbar and shift the
layout.

## What

Add one restrained motion vocabulary to the existing visual language and give
every authoritative zero-result surface a specific, useful empty state without
replacing compact or inline status.

## Approach

`EmptyState.svelte` owns the shared film/playlist illustration, one heading,
one hint, optional status semantics, and an optional action slot while each
parent owns placement and exact copy. Item and season detail, crumbs, cast,
cover-flow cards, buttons, episodes, and watched badges use short existing-ease
entrances or microinteractions. Motion caps are deterministic, reduced motion
suppresses duration and delay, and the content viewport contains entrance
geometry. A focused source contract and a real WebKit scenario cover every
taxonomy branch, exclusion, normal/reduced computed style, screenshots, and
viewport fit.

## Files changed

- `src/lib/EmptyState.svelte` and the page/detail/playlist Svelte surfaces —
  shared designed empties and exact loading/failure/compact-state precedence.
- `src/app.css`, `src/lib/ItemDetail.svelte`, and
  `src/lib/SeasonDetail.svelte` — bounded entrances, interaction feedback,
  reduced-motion suppression, and viewport containment.
- `tests/ui-motion.test.mjs` and `package.json` — canonical focused source
  contract.
- `tests/e2e/scenarios/uimotion.mjs` plus affected scenario copy references —
  real-app normal/reduced behavior, taxonomy, geometry, and screenshot proof.

## Guard proof

- Every authoritative empty category was mutated separately and failed its
  exact copy assertion: Welcome, navigable Home, no libraries, browse, dynamic
  search, person, playlist index, Vela playlist, server playlist, zero episodes,
  and choose-an-episode. Failure/loading exclusions were separately broken for
  Home, browse, both playlist families, and the season loading/error branches.
- Component structure and semantics were independently broken: decorative
  illustration, typed icon choice, single heading, optional status role, and
  parent-owned placement. Compact-menu, sidebar-metadata, and Settings inline
  copy exclusions also failed when replaced with the shared component.
- Detail/season entrances, both crumbs branches and keyed replay, Settings
  preservation, delay suppression, bounded poster/cast staggering, static cast
  behavior, flow-card will-change/grounding/easing, button press, episode hover,
  and watched-badge motion were each mutated separately and failed the intended
  focused assertion.
- Three first-written source guards were exposed as vacuous by mutation and
  repaired in separate commits: exact empty copy (`3af4602`), Home-hub stagger
  loop binding (`3f575aa`), and episode-loader branch binding (`66859ae`). The
  same mutations then failed for the intended reason and restored green.
- The real reduced-motion app was made noncompliant twice. Forcing the item
  detail animation to 120ms failed on the observed `0.12s` duration; forcing a
  flow-card transition to 120ms failed on its observed `0.12s` duration.
  Restoring the committed CSS byte-for-byte returned fresh reduced runs green.
- The existing `imagepolish` geometry guard caught a 1058-to-1068px detail-width
  change during entrance. A first stable-scrollbar-gutter repair (`b814d13`)
  remained red. Testing three isolated hypotheses showed that containing the
  animated `.content` viewport fixed the integration without weakening or
  delaying its assertion; `841f749` restored the scenario green.
- Restored verification at exact implementation head `841f749`: pinned Node
  26.5.0/npm 12.0.1, clean `npm ci`, zero npm vulnerabilities, 17 focused Node
  tests, zero Svelte diagnostics, production build, fresh normal and verified
  reduced-motion `uimotion` runs, and the complete fresh-binary Linux real-app
  suite 27/27. Final normal/reduced dark/light screenshots were inspected and
  the viewport-fit guards reported no overflow.

## Coder dispute (if any)

None.

## Known gaps

The owner is unavailable to playtest this track, by explicit ruling, so no
owner playtest is pending. The focused scenario uses Vela's hermetic Jellyfin
mock; Plex-only cast-headshot rendering remains source-contract coverage rather
than a live-server gate. Emby remains experimental under the existing release
ruling.

## Reviewer comments

**Primary implementation review — recorded 2026-07-17T01:34:25Z — accepted.**
Claude Code 2.1.212 (`claude-fable-5`, session
`d868f4c7-6e20-4a43-967b-d63b210c441b`) reviewed exact head
`6075f52bc79d11d3cf482b1b6e440127dd22127f` against base
`c28dbd24bf6f40f57e96e59146b5d9ffc064334d` in a verified detached worktree.
It received the neutral goal-first question and no primary verdict or suggested
finding. The parse-exact structured result carried both full SHAs,
`guard_confirmed:true`, verdict `accepted`, and no material finding.

The streamed tool transcript confirms that Claude independently changed the
production item-detail entrance from 200ms to 320ms. The focused motion contract
failed only the intended assertion (`item detail duration: 320 !== 200`) while
the other three tests remained green. Claude restored the production file from
the reviewed head, reran the contract 4/4 green and the complete focused Node
set 17/17 green, and confirmed an empty diff and status at the exact reviewed
SHA. The orchestrator verified the red and green output, exact head, and clean
worktree before removing it.

**Secondary-review ruling — recorded 2026-07-17 — waived.** Agy's bounded
headless probes could not produce a usable response because its command tool was
permission-denied even with the documented noninteractive override. No Agy
verdict was requested or counted. The owner directed the agent to stop using
Agy, ruled Fable the materially important reviewer, and removed the default
secondary-review gate. Claude's accepted primary verdict closes code review for
this slice.
