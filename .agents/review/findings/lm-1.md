# lm-1: the libmpv decode check passes when nothing was decoded

**Severity**: MEDIUM — the CI job's proof that the shipped decoders work can
pass while a decoder produces nothing.
**Status**: Open
**Branch**: —
**Commit**: (filled in after commit)

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

(completed with the fix)

## Files changed

(completed with the fix)

## Guard proof

(completed with the fix)

## Coder dispute (if any)

None.

## Known gaps

(completed with the fix)

## Reviewer comments

Generation pass: `.agents/review/codereview-2026-11-13-libmpv-set.md`.
Verification pass: pending.
