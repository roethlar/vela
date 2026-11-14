// Where the debug app the runner starts is: where Cargo builds it, as Cargo
// itself says (`cargo metadata`), not a path assumed here. The build honours
// CARGO_TARGET_DIR and a Cargo configuration's target directory, so a fixed
// `src-tauri/target/debug/vela` could be an older binary while the build
// wrote a new one elsewhere (review finding pa-6). run.mjs asks Cargo with the
// environment the build gets, from the directory the build runs Cargo in, and
// hands the answer to appBinaryFromMetadata.
import path from 'node:path';

// The directory, under the repository root, that Cargo is asked from. It is
// where `tauri build` runs `cargo build`, and Cargo reads its configuration
// files (a `.cargo/config.toml` naming a target-dir) upwards from the
// directory it runs in, and takes a relative CARGO_TARGET_DIR from there too.
// Asked from the repository root, Cargo would miss `src-tauri/.cargo/` and
// place a relative CARGO_TARGET_DIR one directory higher than the build does.
export const CARGO_DIR = 'src-tauri';

// The arguments run.mjs gives `cargo`, run in CARGO_DIR, to ask where it
// builds.
export const METADATA_ARGS = ['metadata', '--format-version', '1', '--no-deps', '--manifest-path', 'Cargo.toml'];

// The directory to run `cargo` METADATA_ARGS in, for a repository root.
export function cargoMetadataCwd(repoRoot) {
  return path.join(repoRoot, CARGO_DIR);
}

// From Cargo's answer (the text `cargo metadata` printed) and the
// environment the build gets, the path of the debug app binary:
// <target_directory>/debug/<the one binary target>. Throws, saying why, when
// the answer does not say, or when the environment asks Cargo to build for a
// named target (the binary is then under <target_directory>/<target>/debug/,
// where the debug build this runner makes is never looked for).
export function appBinaryFromMetadata(metadataText, env = {}) {
  let metadata;
  try {
    metadata = JSON.parse(metadataText);
  } catch (err) {
    throw new Error(`Cargo's metadata is not JSON (${err.message})`);
  }
  const target = metadata?.target_directory;
  if (typeof target !== 'string' || !path.isAbsolute(target)) {
    throw new Error(`Cargo's metadata names no absolute target directory (target_directory is ${JSON.stringify(target)})`);
  }
  if (env.CARGO_BUILD_TARGET) {
    throw new Error(
      `CARGO_BUILD_TARGET is ${JSON.stringify(env.CARGO_BUILD_TARGET)}: the build would write its binary under ${path.join(target, env.CARGO_BUILD_TARGET)}, not ${path.join(target, 'debug')}; unset it`,
    );
  }
  const bins = (Array.isArray(metadata.packages) ? metadata.packages : []).flatMap((p) =>
    (Array.isArray(p?.targets) ? p.targets : []).filter((t) => Array.isArray(t?.kind) && t.kind.includes('bin')),
  );
  if (bins.length !== 1 || typeof bins[0].name !== 'string' || bins[0].name === '' || bins[0].name.includes('/')) {
    throw new Error(`Cargo's metadata names ${bins.length} binary targets, not the one app binary`);
  }
  return path.join(target, 'debug', bins[0].name);
}
