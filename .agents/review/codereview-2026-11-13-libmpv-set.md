# Codereview 2026-11-13 — the shipped libmpv set (embedded player, Slice 0b)

Outcome: codereview codex over `5f1d9b6..b19d7d1`: **findings**, three
candidates, three admitted (`lm-1`, `lm-2`, `lm-3`, all MEDIUM), none
declined.

## Dispatch record

- Owner's dispatch word, verbatim (2026-11-12, the session goal):
  "codereview slices with codex". This is the generation half: a defect
  hunt over a pinned landed range.
- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, CLI transport, headless one-shot, stdin
  closed: `codex exec --sandbox workspace-write --json --output-schema
  <schema> -o <outfile> <prompt>`. Called plainly, with no model and no
  effort argument, per `.agents/machines.md`. The `--json` stream names no
  model; the host's `~/.codex/config.toml` reads `model = "gpt-6-astra"`,
  `model_reasoning_effort = "max"`, so that is what a plain call resolves
  to on `continuum`.
- Run from `~/dev/vela-probe-wt`, a detached `git worktree` of this clone
  checked out at the head SHA, and declared disposable in the prompt, so
  the reviewer's own experiments could not touch the working tree.
- Tier: standard. T1 evaluated against the range before dispatch: no
  changed path matches a sensitive-path glob; the repo has no
  `.agents/review/sensitive-paths` override.
- Thread `01a1d34d-e69c-7352-a133-4ed9dc612dcd`; the event stream ends in
  `turn.completed`; 25 tool calls. Dispatched 2026-11-13 05:07 UTC,
  verdict read 05:20 UTC.

## Reviewed range

- base: `5f1d9b62bc9767af4ffdc1fab023d4d78749356b`
- head: `b19d7d1c1f2c5b31e4fed224f30ca00c87524739`

One commit: `b19d7d1` (the build script, the recipe under
`packaging/libmpv/`, the CI job, the policy tests, the docs section).

## Acceptance (computed by the orchestrator)

The payload matches the schema; `verdict` is `findings` and `findings` is
non-empty; `reviewed_sha` and `base_sha` equal the dispatched pins;
`capability_ok` is literally `true`. Accepted as a valid findings verdict.

Payload, verbatim:

```json
{"verdict":"findings","capability_ok":true,"reviewed_sha":"b19d7d1c1f2c5b31e4fed224f30ca00c87524739","base_sha":"5f1d9b62bc9767af4ffdc1fab023d4d78749356b","findings":[{"title":"The decode check passes when a track produces no decoded frames","evidence":"packaging/libmpv/check-libmpv.py:182 accepts EOF plus a non-null video-codec property. Using existing local probe libraries, zeroing every video packet in h264-aac.mp4 while preserving audio produced RESULT PASS despite zero decoded video frames. Zeroing only the audio packets also passed.","predicted_failure":"Decoding failures can pass CI when the surviving track reaches EOF; a codec name does not prove successful decoding.","severity":"MEDIUM","better_approach":"Verify actual decoded output for both tracks using frame/sample counts or reference hashes. Add separate regressions that corrupt video and audio packets."},{"title":"Missing licence files are invisible to the integrity check","evidence":"scripts/build-libmpv.sh:327 records only *.so* files, and packaging/libmpv/check-libmpv.py:62 only rejects unlisted library files. Copying the existing probe set without its entire licenses directory still produced RESULT PASS for all three fixtures. docs/development.md:124 and :157 incorrectly describe verification of every file.","predicted_failure":"CI can accept and upload a damaged cached set missing all licence texts and the bundled mpv patch, defeating the documented whole-set integrity check.","severity":"MEDIUM","better_approach":"Include licence texts and patches in the manifest and require licence entries for every shipped component. Verify that removing a licence makes the check fail."},{"title":"The checksum regression test cannot distinguish rejection from failed extraction","evidence":"tests/dependency-policy.test.mjs:362 supplies plain text instead of a valid archive. Moving the path-printing statement before the checksum guard at scripts/build-libmpv.sh:72 left all 12 tests green. Under that mutation, a valid wrong-checksum archive reached a stub compiler because unpack() at scripts/build-libmpv.sh:79 does not propagate fetch's failure.","predicted_failure":"The security regression test stays green when unverified sources can reach compilation: its junk fixture fails during extraction regardless of whether checksum enforcement works.","severity":"MEDIUM","better_approach":"Use a valid archive with an incorrect checksum and a sentinel that detects extraction or compilation. Explicitly propagate fetch and extraction failures so the test verifies rejection at the intended boundary."}]}
```

## Intake

| Candidate | Verdict | Why |
| --- | --- | --- |
| Decode check passes with no decoded frames | ADMITTED as `lm-1` | Evidence is a reproduced run; the check's stated job is to prove decoding. |
| Licence files outside the integrity check | ADMITTED as `lm-2` | Reproduced; the licence texts are a shipping requirement (decision 2026-11-12, "Licensing is settled") and the docs claimed more than the check did. |
| Checksum test cannot tell rejection from failed extraction | ADMITTED as `lm-3` | The mutation was reproduced by the reviewer. Half of it (a failed `fetch` did not stop `unpack`) was found independently by the first CI run and fixed at `1e2b953` before this verdict was read; the test weakness remained. |

Per-finding records: `.agents/review/findings/lm-1.md`, `lm-2.md`,
`lm-3.md`.

## Notes

- A shell hook on this host redirects commands to the `ptk` MCP tool. The
  reviewer's first plain shell command was blocked by it and it worked
  through `ptk` from then on; an environment note, not an invalidation.
- The reviewer used the library set already built in
  `~/dev/vela-spike-libmpv/libset/out22/` for its own runs of the check
  (it cannot build one: no network, no containers in its sandbox).
- `main` moved to `38cb850` and `1e2b953` while the review ran. The
  reviewer was pinned to explicit SHAs; its verdict covers the range above
  and nothing later.
- Not covered by this review: the GitHub Actions run of the new job (the
  reviewer has no network). Its first run failed on a download and exposed
  the propagation defect noted under `lm-3`.
