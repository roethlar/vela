# Vendored mpv script: autocrop.lua

`autocrop.lua` is bundled **unmodified** from the mpv project and loaded at
runtime by the system `mpv` binary (a separate process) when the user enables
black-bar cropping in Vela's Settings.

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
