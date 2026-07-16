# ui-s2: Media image loading polish

**Severity**: MEDIUM — media art could pop into place, disappear into a blank
frame on failure, or retain stale loaded state when an image URL changed.
**Status**: Verified — primary Claude and owner-authorized independent Agy
accepted with `guard_confirmed:true`; version bump pending
**Branch**: `main` (approved Slice 2 implementation)
**Commit**: `830cabda963bb96ffa1eb525c5cc08a80f246def` plus focused E2E selector fix
`c22a07edc075576804cec3e7d1ca9f493eb436ef`

## Evidence

At base `e98220e`, Vela's media `<img>` templates appeared as soon as the
webview painted decoded content, several surfaces hid failed elements with
component-local flags or inline styles, and a failed URL could poison later
content rendered through the same component. The authorized behavior and
surface taxonomy are recorded in `.agents/plans/ui-embellishments.md` Slice 2.

## Predicted observable failure

Slow poster and backdrop requests visibly pop rather than revealing in the
existing frame. A failed poster or episode still leaves an empty or visibly
broken surface instead of a title-bearing fallback. Reusing an image node for a
new URL can display the old loaded state, while a cached image whose event fired
before listener attachment can remain transparent. Reduced-motion users can
inherit the full reveal duration.

## What

Give every media-art image one source-aware, DOM-preserving opacity reveal and
an always-present fixed-frame underlay. Keep the Plex QR as the sole functional
image exception, while retaining asynchronous decoding on all runtime images.

## Approach

`src/lib/imageReveal.ts` owns load, error, cached-image, changed-source, stale
work, and cleanup behavior without replacing the owning `<img>`. Global CSS
owns only the 180ms opacity transition and absolute cover geometry. Nine media
templates use that primitive over title, film-icon, or themed-surface underlays;
the QR keeps meaningful alternative text and no fade. The Jellyfin mock and
Linux scenario provide deterministic held, released, successful, failed,
theme, and reduced-motion witnesses without timing sleeps.

## Files changed

- `src/lib/imageReveal.ts:18`, `src/app.css:472` — source-aware reveal action
  and shared opacity/cover primitives.
- `src/routes/+page.svelte:2097`, `src/lib/ItemDetail.svelte:105`,
  `src/lib/SeasonDetail.svelte:237`, `src/lib/PlaylistsView.svelte:275`, and
  `src/lib/ServerPlaylistView.svelte:137` — nine media-art integrations plus
  the QR decoding exception.
- `tests/image-reveal.test.mjs:88`, `package.json` — focused action, CSS,
  inventory, and fallback contract in the canonical frontend check.
- `tests/e2e/mockjf.mjs:80`, `tests/e2e/scenarios/imagepolish.mjs:358` —
  deterministic image controller and real WebKit held/released/failure/theme/
  reduced-motion coverage.

## Guard proof

- Ordinary reveal: changing the successful load from class-add to class-remove
  failed `imageReveal reveals only a successful nonzero-width load and hides an
  error`; restoring the committed action returned it green.
- Source/cache lifecycle: removing synchronous source reset and, separately,
  removing the cached microtask check failed their exact focused tests;
  restoring each returned both orderings green.
- Inventory: removing async decoding from the QR and, separately, removing the
  playlist reveal action failed the exact 10-image/9-media contract; restoring
  each returned it green.
- Underlays: deleting the episode-row `.noart` and, separately, restoring an
  inline visibility-hide handler failed the fallback taxonomy/obsolete-handler
  contract; restoring each returned it green.
- CSS: making loaded opacity zero and, separately, changing 180ms to 800ms
  failed the shared primitive contract; restoring each returned it green.
- Reduced motion: an injected `180ms !important` exception was copied to the
  Linux venue and freshly built. `imagepolish` failed on the held grid poster's
  observed `0.18s` transition. Restoring the committed CSS byte-for-byte and
  rebuilding returned the reduced-motion run green.
- The first fresh Linux run exposed a nested season-card selector assumption;
  the real label starts with the show name. Selecting the rendered season
  metadata instead fixed the integration, and the focused normal run passed.
- Restored verification: pinned Node/npm assertion, clean `npm ci`, zero npm
  vulnerabilities, canonical frontend check (13 Node tests, zero Svelte
  diagnostics), production build, focused normal and reduced-motion fresh-build
  Linux runs, and the complete Linux real-app suite 26/26. Six final dark/light
  held, loaded, and failed-state screenshots were inspected at the exact head.

## Coder dispute (if any)

None.

## Known gaps

The owner is unavailable to playtest this track, by explicit ruling. Jellyfin
and Emby do not expose the rich cast detail needed to render Plex headshots in
the hermetic scenario, so the complete source contract plus Svelte compilation
own that surface. No production test hook or live Plex gate was added. The test
image is intentionally a deterministic 1x1 PNG; DOM and server witnesses prove
successful decode/reveal while screenshots own geometry and fallback quality.

## Reviewer comments

**Primary implementation review — recorded 2026-07-16T23:08:18Z — accepted.**
Claude Code 2.1.211 (`claude-fable-5`) reviewed exact head
`0ccb269cc765eaa38d40ea93f8e61845500a6aa3` against base
`e98220e9d4ce10f400fd50f1c67cd31b19db6ef8` in a disposable worktree. Its
schema-valid result carried the exact SHAs, `guard_confirmed:true`, verdict
`accepted`, and no material comments. The reviewer reported an independent
production-regression mutation, expected focused failure, restoration, and
green guard. The orchestrator independently confirmed that only the clean
primary worktree remained after review.

**Independent secondary review — recorded 2026-07-16T23:23:19Z — blocked;
no verdict counted.** Grok 0.2.101 (`grok-4.5`) was dispatched against the same
exact head and base without access to the primary verdict. The first CLI-created
worktree silently opened the newer documentation head and correctly returned
`invalid`; that attempt was discarded. In a verified detached worktree, one
schema-valid `accepted` response claimed `guard_confirmed:true`, but its exported
transcript contained no tool calls, so the orchestrator discarded it rather
than accepting an unperformed proof.

Fresh agentic sessions then read the governing record, exact range, action,
CSS, source contract, media integrations, and E2E scenario and passed the
focused baseline. Grok's headless runner repeatedly ended with
`stopReason: Cancelled` before completing a mutation. One later session
constructed a combined mutation/red/restore/green command, but Bash rejected
its unquoted final parentheses at parse time; neither red nor green artifact
was created. Subsequent context degraded into a nonexistent-file
hallucination. `grok models` confirmed that `grok-4.5` is the only installed
Grok model. The detached worktree remained clean at the reviewed head and was
removed. Fail-closed result: there is no independent-secondary acceptance, so
the Grok attempts do not count as review acceptance.

**Independent substitute review — recorded 2026-07-16T23:34:14Z — accepted.**
On the owner's explicit instruction, Agy 1.1.3 using `Gemini 3.1 Pro (High)`
replaced Grok as the independent secondary reviewer. Agy reviewed the same
exact head `0ccb269cc765eaa38d40ea93f8e61845500a6aa3` against base
`e98220e9d4ce10f400fd50f1c67cd31b19db6ef8` in a verified detached worktree
that predates both reviewer verdict records. Its runtime log confirms the
requested model selection and conversation
`df1bae8e-0900-40a1-8075-f2ae275b9ae2`.

Agy independently mutated the production reveal path and ran the focused
contract. The red run failed three load/source/cache assertions because valid
images no longer revealed; after restoring from the reviewed head, all seven
focused tests passed. The final worktree was empty at the exact reviewed SHA.
Its parse-exact result carried both full SHAs, `guard_confirmed:true`, verdict
`accepted`, and no material comments. The orchestrator verified the red and
green logs, model-resolution log, clean worktree, and exact head before removing
the disposable worktree. This acceptance satisfies the independent-secondary
gate in lieu of the discarded Grok attempts.
