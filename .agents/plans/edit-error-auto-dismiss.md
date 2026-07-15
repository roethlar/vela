# Plan: failed edit errors auto-dismiss after eight seconds

Status: **IMPLEMENTED at `01e30cf`; Grok accepted r1; awaiting owner playtest
and owner-gated landing.** The owner confirmed the 0.1.49 failed-watch recovery
on the exact stopped-Plex path, then approved this follow-up behavior: a failed
edit error stays readable for about eight seconds and still clears immediately
when the next edit starts. The required Grok `reviewloop` accepted the code
slice with an independent guard proof and no findings.

Decision record: `.agents/decisions.md`, 2026-07-15. This plan supersedes only
the watch-state edit lifetime detail in the completed per-surface-status and
failed-watch-recovery plans. It does not reopen their surface separation or
recovery behavior.

## Owner-visible problem

Version 0.1.49 fixes the destructive failure path: with Plex stopped, Mark
watched leaves the Movies grid and **12 Years a Slave** present, and Plex still
reports the item unwatched after restart. The named red edit-error line is now
correct, but it persists indefinitely until another edit occurs. That makes a
handled failure look permanently active after the user has read it.

## Binding behavior

- A failed `setWatched` or `removeFromContinue` action publishes on the edit's
  existing action line, never on the view banner.
- The line follows navigation and remains visible for 8,000 ms measured from
  publication, then clears automatically.
- A newer edit or source-list change clears it immediately. An older timer may
  never clear a newer failure, even if its callback was already queued when
  cancellation occurred.
- Component teardown invalidates in-flight edit publication and cancels the
  timer. An edit completing after teardown must not arm a new timer.
- Timer expiry changes presentation only. It does not change `editAttempt`,
  watch state, recovery, navigation, loaded items, pagination, or scroll.
- Successful edits remain silent; the card state is their acknowledgement.
- Scan, queue, mpv, detail, and view-status lifetimes are unchanged. There is no
  manual dismiss control.

## Implementation slice

One independently committed code slice on
`fix/eet-1-edit-error-auto-dismiss`, with the ordinary version bump from 0.1.49
to 0.1.50 through `scripts/bump.sh`.

### Frontend timer

In `src/routes/+page.svelte`:

1. Add a named `EDIT_STATUS_TTL_MS = 8000`, one tracked timeout handle, and
   `clearEditStatus()` beside `editStatus` / `editAttempt`.
2. Add `publishEditFailure(attempt, text)`:
   - return unless `attempt === editAttempt`;
   - clear the prior status and timer;
   - publish the text;
   - arm the 8,000 ms timeout;
   - in the callback, clear only when its captured attempt still equals
     `editAttempt`.
3. Route both failed `setWatched` and failed `removeFromContinue` publications
   through the helper. Arm only after the failure is ready to display; the
   Home rollback repair must not consume the visible lifetime.
4. Replace the three direct clears (new `setWatched`, new
   `removeFromContinue`, source-list change) with `clearEditStatus()`, retaining
   their existing `editAttempt` increments.
5. On destroy, increment `editAttempt` before clearing. Clearing only the timer
   is insufficient because an in-flight invocation could otherwise complete
   after teardown and publish a new one.
6. Update the code and markup comments that encode indefinite persistence.

Do not add a backend timer, a production test hook, a success message, a view
banner write, or a manual close button.

### Hermetic guard

Strengthen `tests/e2e/scenarios/pagefail.mjs`:

1. Keep case 4's exact-grid/no-listing/no-view-failure assertions and its proof
   that a healthy Refresh does not synchronously erase the edit outcome.
2. Add a dedicated timer-ownership leg immediately afterward. In the webview,
   wrap `window.setTimeout` / `clearTimeout` only for the next two delays whose
   requested value is exactly 8,000 ms. Delegate every other timer unchanged.
   Accelerate those two callbacks to separate short deadlines, record their
   requested delays/fires, and deliberately ignore cancellation of the first
   handle to model a callback already queued. Restore the native functions and
   clear probe handles in `finally`.
3. Publish failure A, then start a delayed failed edit B before A's accelerated
   deadline. Require A to clear synchronously at B's click, B to publish, B to
   remain exact when A's forced stale callback fires, and B to disappear only
   when its own callback fires. Require the probe to record two exact 8,000 ms
   requests. Keep the grid exact and the view banner absent throughout.
4. Strengthen the later successful-next-edit case: assert the prior failure is
   present immediately before the click and absent immediately after it, then
   witness that the successful server request was delivered. Do not poll up to
   15 seconds for absence; the new auto-dismiss timer would make that guard
   vacuous.

Prove the changed guards red separately, restoring committed source after each:

1. Change `EDIT_STATUS_TTL_MS` from 8,000 to 9,000. The exact-duration probe
   must fail before accepting an unaccelerated timer.
2. Keep the exact 8,000 ms schedule but make its callback a no-op. The second
   failure must remain visible past its own deadline and fail the dismissal
   assertion.
3. Remove the timer callback's attempt check. The deliberately uncancelled old
   callback must erase failure B and fail the stale-owner assertion.
4. Remove the new-edit `clearEditStatus()` call from `setWatched`. The immediate
   clear assertion must fail while the delayed replacement edit is in flight.

### Live and durable alignment

- In `tests/e2e/live/plex.mjs` and `tests/e2e/live/jellyfin.mjs`, keep proof that
  the named failure appears on its own line. Replace the post-recovery
  indefinite-persistence assertion with eventual auto-dismissal; server restart
  and Refresh do not own the line, but the independent timer does.
- Amend `.agents/decisions.md`, the two superseded plans, `.agents/state.md`,
  and the review records. Do not rewrite historical review/archive evidence.

## Verification

- Syntax-check every changed `.mjs` file.
- `npm run check`
- `npm run build`
- From `src-tauri/`: `cargo check --locked`
- From `src-tauri/`: `cargo clippy --all-targets --locked -- -D warnings`
- From `src-tauri/`: `cargo test --locked`
- Linux real-app `npm run e2e -- pagefail` for every red proof and restored
  green, then full Linux `npm run e2e`.
- Opt-in `npm run e2e:live -- live-plex` and `npm run e2e:live --
  live-jellyfin`, preserving every existing service/proxy restoration rail.
- Grok `reviewloop` on the pinned code slice and every review-fix slice, with no
  round cap. Acceptance requires the reviewer's independent guard proof and
  `guard_confirmed: true`.
- Owner playtest on 0.1.50: repeat the stopped-Plex Mark watched failure. The
  grid/title stay exact, the named edit line is readable and disappears after
  about eight seconds, and the item remains unwatched/actionable after restart
  plus Refresh.

## Non-goals and known gaps

- No change to failed-edit Home repair or successful-edit refresh behavior.
- No change to server calls, retries, friendly error text, or error colors.
- No timers for view, scan, queue, mpv, or detail failures.
- The hermetic mock cannot currently force `removeFromContinue` to fail; its use
  of the shared publisher is inspection-covered. The real Jellyfin case covers
  a second watch-state failure but not that command specifically.
- Source-list teardown and component destroy remain inspection-covered; their
  existing outcome invalidation has no safe E2E control surface.
