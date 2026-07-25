# Vendored mpv script: autocrop.lua

`autocrop.lua` is bundled **unmodified** from the mpv project and loaded at
runtime by the system `mpv` binary (a separate process) when the user enables
black-bar cropping in Vela's Settings.

`vela-autocrop.lua` in this directory is NOT an upstream file: it is
Vela-authored (MIT, like the rest of the repo) and only invokes the stock
script's public `script-binding` from a separate mpv script context. It exists
so the stock file can stay byte-identical to upstream — see the shim's header
and `.agents/plans/autocrop-resume.md`.

`vela-markers.lua` is likewise NOT an upstream file: Vela-authored, MIT, with
no upstream ancestor and no GPL code in it. It renders the intro / credits /
commercial skip control and performs the seek, reading its marker ranges from a
private per-launch payload file whose path arrives on the child process
environment — see the script header and
`.agents/plans/skip-credits-intros-v2.md`. `LICENSE.GPL` in this directory
covers stock `autocrop.lua` only and does not extend to either Vela-authored
script.

- **Source:** mpv `TOOLS/lua/autocrop.lua`
  (https://github.com/mpv-player/mpv)
- **Upstream commit:** `efb70d7f27780bbc7db2ad9a7f2fbf05e610c97e` (2025-08-29,
  `git-release-137`)
- **Modifications:** none. The file is byte-for-byte identical to upstream. If it
  is ever changed, note the modification here per the GPL.

## License

mpv's `autocrop.lua` carries no per-file license header and therefore falls under
mpv's project license, GPLv2-or-later. `LICENSE.GPL` in this directory is mpv's
GPL text, shipped alongside the script.

Vela's own code is MIT-licensed. This script is **aggregated**, not linked: it is
data handed to a separate `mpv` process at runtime via `--script=`, not compiled
or linked into Vela. Aggregation does not relicense Vela's own code. Vela's
package license metadata is declared as `MIT AND GPL-2.0-or-later` to reflect that
this GPL file ships in the package.
