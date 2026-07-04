# Review status

Workflow: see `.agents/playbooks/reviewloop.md`.
Per-finding detail: see `.agents/review/findings/<id>.md`.

Loop opened 2026-07-04. Scope: the 2026-07-04 feature batch,
`784677e..e717898` (five implemented plans + hero sizing; v0.1.4 → v0.1.6).
Reviewer harness: `codex` (codex-cli 0.142.5), dispatched headless one-shot.

## Legend
- `[ ]` Admitted, open (passed intake triage; not yet started)
- `[~]` In progress / pending review
- `[x]` Verified (awaiting owner-gated merge)
- `[!]` Contested — declined, disputed, or ruled invalid; awaiting owner adjudication
- `[-]` Declined at intake (kept for the record; no work)

Intake 2026-07-04: codex generation pass returned 4 candidates; the coder
contributed 2. Triage: 5 admitted (4 codex + 1 coder), 1 declined. Fix
branches are STACKED (shared files); merge order is rev-1 → rev-5.

## Findings

| ID | Severity | Impact (one line) | Status | Branch |
|----|----------|-------------------|--------|--------|
| rev-1 | MEDIUM | Dedup under-fills pages; infinite scroll ends early, titles unreachable | `[~]` | `fix/rev-1-dedup-page-underflow` |
| rev-2 | MEDIUM | Same-source versions collapse to one card; context menu crashes on duplicate keys | `[ ]` | `fix/rev-2-same-source-collapse` |
| rev-3 | MEDIUM | Mark watched on merged cards routes to watch-incapable source, always errors | `[ ]` | `fix/rev-3-watch-routing` |
| rev-4 | LOW | All-source failure renders as empty grid with no error | `[ ]` | `fix/rev-4-surface-total-failure` |
| rev-5 | LOW | Merged card hides real progress when first backing reported unwatched | `[ ]` | `fix/rev-5-adopt-most-progressed` |
| rev-6 | — | (declined at intake: scroll reset on listings-updated is designed refresh-on-change) | `[-]` | |
