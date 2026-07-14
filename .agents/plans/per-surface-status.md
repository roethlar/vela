# Plan: failures report on the surface that owns them

Status: **APPROVED** (owner, 2026-07-14). Two decisions taken in the owner's words:

1. *"Own surfaces"* — the play queue, the mpv setup bar and the open detail page each
   report their own failures. The top banner keeps only VIEW-scoped ones.
2. *"Its own line"* — a failed watch-state edit (mark watched / unwatched / remove from
   Continue Watching) reports on its own action line, **not** on the view banner. Same
   treatment the library scan already has.

Decision record: `.agents/decisions.md`, 2026-07-14.

## Problem

`src/routes/+page.svelte` has ONE error banner (`errorParts` → the `$derived` `error`
string → `div.error`) written by surfaces with four different lifetimes:

| writer | surface | lives until |
| --- | --- | --- |
| listing / refresh / search / sections | the view | the next load of that view |
| watch-state edit (`setWatched`, `removeFromContinue`) | an action | the next edit |
| queue mutations (`playNext`, `addToQueue`, `queueJumpTo`, `queueRemove`, `queueClearAll`) | the queue drawer + chip | the next queue action, or the drawer closing |
| `installMpv` | the mpv setup bar | the next install attempt |
| `play` from an open detail | the detail page | the detail closing |

Because the code could not say which failure belonged to which surface, every clear was
either **too wide** (silently erasing a failure the user still needed) or **too narrow**
(stranding a diagnostic over a view that no longer exists).

The `library-refresh-scan` code-review loop found this EIGHT rounds running — r18
(publish), r19 (ordering), r20 (retract), r21 (dedup), r22 (`setError`), r23 (a `linking`
flag), r24 (`setError(null)` again). Several doors were opened by the fix for the previous
one. Full evidence: `.agents/plans/library-refresh-scan.md` `## Code review log`, r17-r24.

**The interim model** (`da99a46`) gives each part an `owner: "view" | "queue" | "mpv" |
"detail"` and clears per owner. It is coherent and currently correct. **This plan replaces
it** — it is not a base to build on. Three of its four owners have no automated guard and
cannot get one (see Verification).

## Precedent to copy exactly

The library scan already works this way, from r15 of the same loop. Read it first:

- `scanStatus = $state<{ text: string; failed: boolean } | null>` + `scanStatusOwner` +
  `scanStatusTimer` in `+page.svelte`.
- Rendered as `div.scanerror` (failure) or `div.notice` (success), NEVER `div.error`.
- A SUCCESS auto-clears on a 4s timer, and only the owning attempt may clear it
  (`if (scanStatusOwner === attempt) clearScanStatus()`), so an older attempt's timer
  cannot wipe a newer attempt's status.
- A FAILURE stays until the next scan — "unlike the acknowledgement, a failure is not
  something to tidy away on a timer".
- An attempt counter (`scanAttempt`) invalidates in-flight outcomes when the source list
  changes (`onSourcesChanged` bumps it), so a scan cannot publish over an unrelated source.

Every surface below follows that shape: `{ text, failed } | null` + an attempt counter +
success-auto-clears / failure-persists.

## Slices

Each slice is independently shippable and independently verifiable. Commit each as it
lands. Run the full check set (`.agents/repo-guidance.md` Verification) per slice.

### Slice 1 — the watch-state edit gets its own line

The highest-value slice: this writer caused most of the loop's defects.

- Add `editStatus = $state<{ text: string; failed: boolean } | null>` and `editAttempt`,
  mirroring `scanStatus`/`scanAttempt`.
- `setWatched` and `removeFromContinue` publish there, never through `setError`/`addError`.
- Render it next to the scan's status (same slot family, `div.scanerror` / `div.notice`),
  NOT in `div.error`.
- `onSourcesChanged` bumps `editAttempt` and clears the status — an edit in flight when a
  source is removed must not publish over an unrelated source (the r16-3 rule, applied).
- **`rootSig` gates TWO things. Exactly one of them dies.** (Corrected against the code
  before implementing — the first draft of this plan claimed the whole gate could go, and
  it cannot.)
  - **The PUBLISH gate goes** (`if (rootSig() !== myRoot) return;` before reporting). An
    action's outcome does not care which view is on screen — the scan already publishes
    regardless. This is the gate that caused the defects, and deleting it IS the fix.
  - **The REPAINT gate stays** (`if (rootSig() !== myRoot) { heal; return; }`). It decides
    whether to re-enter the CURRENT root, and if the user walked to another library, a
    repaint resets their grid to page one and throws away their scroll. That harm is real
    (r22-2) and independent of where the failure is reported.
  - The win is not that the gate disappears; it is that its **blast radius collapses**. Get
    it wrong now and the cost is an unnecessary reload, not a silently lost failure.
- **KEEP the heal.** The backend curates recents/tombstones BEFORE the server call and
  rolls back on failure (`src-tauri/src/commands.rs` `set_watched`), so a Home load inside
  that window captures a transient lie. The catch must still re-fetch the watch state
  (`hubs = []`, and `loadHome` when `authenticated && mode === "home"`) — guarded by
  pagefail case 14. This is orthogonal to where the failure is REPORTED.
- Guards: pagefail cases 4-14 currently assert the edit's failure lands (or does not land)
  on `div.error`. Rewrite them against the new surface. **Most of them will simplify or
  collapse** — cases 5, 7, 8, 9, 12, 13 exist purely because the edit shared the banner.
  Keep case 14 (the heal) and case 11's page-failure half.

### Slice 2 — the queue drawer reports its own

- `queueStatus` + `queueAttempt`, rendered INSIDE the drawer (`aside.drawer`) and, when the
  drawer is closed, on the queue chip (`button.queuechip`) as a small failure dot — the
  failure must remain discoverable with the drawer shut.
- The five queue mutations publish there. Cleared by the next queue action, or by the
  drawer closing.
- Removes the `"queue"` owner from the banner model.

### Slice 3 — the mpv bar reports its own

- `mpvStatus`, rendered on the existing mpv setup bar next to its Retry control.
- `installMpv` publishes there. Cleared by the next attempt.
- Removes the `"mpv"` owner.

### Slice 4 — the detail page reports its own

- `detailStatus`, rendered on the detail surface.
- `play`/`playFrom` failures raised while `detailView` is open publish there; from the grid
  context menu they stay view-scoped.
- Cleared when the detail closes or is replaced.
- Removes the `"detail"` owner.

### Slice 5 — collapse the banner model

Once slices 1-4 have landed, the banner has exactly ONE writer class: the view.

- `errorParts` loses `owner` entirely.
- `clearOwned` / `clearViewErrors` collapse back into `setError(null)`.
- **KEEP `gen` and `retractThrough`.** They are not part of this defect class: they exist so
  a refresh retracts the LISTING diagnostic it superseded and nothing else (r11), and so a
  newer load's failure is not erased by an older one (r17). Those are view-vs-view rules and
  they stay.
- **KEEP `addError`'s weaker-claim merge** for the same reason (two listing failures can
  render the same sentence; r21).

## Non-goals

- Do not add a dismiss (×) control to any status line. The scan's precedent is
  success-auto-clears / failure-persists-until-the-next-action, and it has held.
- Do not touch the Rust backend. The curate-before-call + rollback behaviour is correct and
  owner-verified (`.agents/decisions.md` 2026-07-10); this plan is presentation only.
- Do not change WHEN a failure occurs, only where it is reported. No new retries.

## Verification

Full set per `.agents/repo-guidance.md`. Plus:

- **Every new guard must be RED-PROVEN.** Nine guards in the predecessor loop were VACUOUS
  and not one was caught by review, CI or a green run — only by injecting the regression
  and demanding the test fail for the RIGHT reason. See that plan's log before writing one.
- **An absence assertion needs a witness, not a stopwatch.** `mock.state.served` records
  responses as they go out; wait for the parked response to be delivered, then HOLD the
  assertion open (`holdsFor` in `tests/e2e/helpers.mjs`). A fixed sleep proves nothing.
- **HARNESS LIMITS — these surfaces cannot be guarded here, and the plan must say so rather
  than imply coverage:** the E2E harness cannot fail a queue mutation, an mpv install, or a
  Play. Slices 2, 3 and 4 are therefore verified by inspection and by an owner playtest, NOT
  by automation. Slice 1 IS fully guardable (the mock can 401 a watch-state edit) and must be.
- Owner playtest at the end: fail a mark-watched (disconnect the server mid-edit) and confirm
  the message appears on its own line and does not disturb the grid's own banner.

## Review protocol

The two-reviewer protocol from the predecessor loop is STANDING (`.agents/state.md`): two
independent reviewers on the same pinned diff, neither seeing the other's findings; the
author writes the fixes and runs every guard. **An author may never adjudicate their own
decline** — it goes to the reviewer that did not raise it. Reviewer-vs-reviewer
disagreement goes to the owner, but only when the two positions genuinely cannot both hold.

**Review the newest fix hardest.** In the predecessor loop the author's fixes carried
defects at the same rate as the original code, for eight rounds running.
