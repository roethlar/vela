# lm-1: the libmpv decode check passes when nothing was decoded

**Severity**: MEDIUM — the CI job's proof that the shipped decoders work can
pass while a decoder produces nothing.
**Status**: In progress (fix landed; verification pending)
**Branch**: —
**Commit**: `ec97d6e`

## Evidence

`packaging/libmpv/check-libmpv.py` (at `b19d7d1`, line 182): a clip passes
when playback reaches end of file and the `video-codec` property was set.
The reviewer zeroed every video packet of `fixtures/h264-aac.mp4`, keeping
the audio: `RESULT PASS` with no video frame decoded. Zeroing only the
audio packets also passed. A codec name comes from the container, not from
the decoder.

## Predicted observable failure

A set whose AV1, HEVC, H.264 or audio decoder does not work on a
distribution (a mis-linked dav1d, for example) still passes the `libmpv` CI
job and is uploaded, because the other track carries playback to end of
file.

## What

The check claims to prove decoding and proves only that a file was opened
and that something reached its end.

## Approach

The check now decodes each clip through mpv's own encoding path and
compares what comes out with a recorded result, one track at a time so a
dead track cannot hide behind the other:

- Video: every decoded frame, unconverted, becomes one checksum line
  (FFmpeg's `framecrc` writer). The frame count, the frame size and the
  checksums must equal the recorded ones exactly. H.264, HEVC and AV1
  decoding is bit-exact, and the same shipped decoder runs everywhere.
- Audio: decoded to PCM; the sample count and the level (RMS) must be
  within 2% and 5% of the recorded ones. Audio decoders work in floating
  point, so exact equality is not promised across CPUs; silence or missing
  sound is off by far more.
- A track whose decoding does not reach end of file within 25 seconds
  fails. (A dead track can leave mpv waiting for it.)

Recorded results live in `fixtures/expected.json`; a clip with no recorded
result fails. Two damaged copies of the H.264 clip are committed
(`fixtures/broken/`, every packet of one track zeroed, which is the
reviewer's reproduction) and the container check passes them with
`--broken`: each must FAIL the comparison on its damaged track, on every
run, in every image. `judge_decode` and `judge_broken` are pure functions
so the policy tests can drive them without a built library.

## Files changed

- `packaging/libmpv/check-libmpv.py` — `measure`, `judge_decode`,
  `judge_broken`, `--broken`, `--record`; a time bound on every run
- `packaging/libmpv/fixtures/expected.json` — recorded results (new)
- `packaging/libmpv/fixtures/broken/` — the two damaged clips (new)
- `packaging/libmpv/make-fixtures.sh` — how every clip is made (new)
- `packaging/libmpv/check-in-container.sh` — passes the damaged clips
- `tests/dependency-policy.test.mjs` — two tests (the judging functions;
  the fixtures and the wiring)
- `docs/development.md` — what the check does

## Guard proof

- `tests/dependency-policy.test.mjs` › "the libmpv decode check compares
  what was decoded with what was recorded" and › "every libmpv fixture
  clip has a recorded result, and both tracks have a damaged clip".
  Run 2026-11-13 from the committed state, each restored with
  `git checkout`, each FAILING its test: frame count not compared; frame
  checksums not compared; audio amount not compared; audio level not
  compared; unfinished decoding accepted; damaged clips never judged; any
  failure accepted for a damaged clip whatever its track; a clip without
  a recorded result; damaged clips not passed to the check; only one
  track with a damaged clip.
- The tripwire on the real set: with `judge_decode` made to accept
  everything, the run ends `RESULT FAIL` with "the video of a damaged clip
  was not noticed" and the same for audio.
- On the real set, 2026-11-13: the three clips pass and both damaged
  clips fail as required on `continuum` and in clean Ubuntu 22.04,
  Debian 12 and Fedora 45 images (the decoded frames are identical in all
  four). The reviewer's reproduction, a damaged clip presented under the
  original's name: "video: 0 frames decoded, 12 recorded" and "audio: 0
  samples decoded, 49152 recorded", `RESULT FAIL`. An undamaged clip
  passed as `--broken`: "the video of a damaged clip was not noticed",
  `RESULT FAIL`.

## Coder dispute (if any)

None.

## Known gaps

- The decoding itself needs a built set, so it runs in the `libmpv` CI job
  and by hand, not in `npm run check`; the policy tests cover the judging
  and the wiring.
- Audio is compared by amount and level, not sample by sample.
- Hardware decoding is not exercised here (no GPU in the images).

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-libmpv-set.md`.
Verification pass: pending.
