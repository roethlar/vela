# ps-2 contested: how far to harden a test that reads Rust source

**Status**: Awaiting the owner
**Finding**: `.agents/review/findings/ps-2.md` (LOW)
**Reviewed head**: `053cb36ea59d095a20e9ed5faf5145556c061b19`
**Raised against**: Slice 1 of `.agents/plans/embedded-player.md`

## What is and is not in dispute

Not in dispute: the shipped code. Stream authentication (Plex's
`X-Plex-Token` header) reaches mpv through an owner-only include file and
nothing else, as before Slice 1. No reviewer round found otherwise.

Not in dispute either: every comment the reviewer made. Each was
reproduced and each was right.

What is unsettled is how complete a regression guard has to be. The
finding was that one check stopped seeing a regression after a refactor.
The repair made two guards:

1. A Rust test that builds a launch with headers and looks through
   everything the player is started with, arguments and whole environment,
   for the header names, values and option name. Round 1 found a hole in
   it (the inherited environment); that was closed and has drawn no
   comment since.
2. A JavaScript test that reads the Rust source and requires that the
   headers are read in exactly two places and that mpv's header option is
   named in exactly one. To read "the code compiled into the app" it has
   to leave each file's test module out, so it has to find where that
   module ends.

Rounds 2, 3 and 4 were all about the second guard, and all had the same
form: Rust written so that the reader misjudges where the test module
ends, with code hidden behind it. Indented code after the module (round
2); a raw C string with a lone brace (round 3); a lifetime or a raw
identifier directly before a string inside `stringify!` (round 4). Each
repair closed what was named and the next round found the neighbouring
case. A reader of Rust written in a test file, short of a real Rust
lexer, will keep having a neighbouring case.

So the loop was stopped after four rounds instead of started a fifth time
(the agent recorded that it would before round 4's result was known).

## What exists now

Both guards, as of `053cb36`. The JavaScript one catches a credential
passed as an option in any ordinary form, a second reader of the headers,
code placed after a test module in any indentation, and the literal
prefixes Rust has. It does not catch a regression that is ALSO disguised
with the token sequences from round 4. The Rust one does not depend on
how anything is written: it would catch the header in the arguments or
the environment whatever the source looks like.

## What the owner is asked to decide

One of:

- **A. Accept it as it stands.** Close ps-2 with the round-4 comment
  listed as a known limit of the JavaScript guard. The Rust guard is the
  one that does not care how the source is written. This is the agent's
  recommendation: the finding is LOW, and the remaining gap needs someone
  to write deliberately strange code around a credential.
- **B. Make the confinement structural.** Give the headers a type of
  their own in a small file with no test module in it
  (`player/stream_auth.rs`), private inside, with one method that writes
  the include. Then no other code CAN read them, the compiler enforces it,
  and the JavaScript guard only has to pin that one file's text, with no
  reader of Rust at all. About a slice's worth of work touching
  `PlaySpec` and the two sources that fill it. The agent did not start
  it: it is a design change beyond the finding, in code that handles
  credentials, and was not something to decide alone.
- **C. Another round** with another repair of the reader (it would add
  lifetimes and raw identifiers as tokens). Not recommended, for the
  reason above.

Under every option the fix commits stay; nothing is to be reverted.

## Adjudicator

None offered. The reviewer is not wrong on any point, so there is nothing
for a third harness to adjudicate: the question is one of proportion, and
that is the owner's.
