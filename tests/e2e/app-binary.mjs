// Which debug app the runner starts (review finding pa-6).
//
// A run that builds takes the executable from Cargo's own report of that
// build: the build is made with `--message-format=json-render-diagnostics`,
// which `tauri build` hands on to the one `cargo build` it runs, and Cargo
// then prints, on standard output, a compiler-artifact message for each
// binary it produced or found fresh, with the executable's path as Cargo
// resolved it under whatever configuration applies (CARGO_TARGET_DIR,
// CARGO_BUILD_TARGET_DIR, a configuration file's target-dir or build.target,
// a workspace). The path is used as Cargo printed it: an earlier version
// predicted it from `cargo metadata` and the environment, and every rule of
// Cargo's it did not rebuild was a way to start an older binary.
//
// A run with --skip-build builds nothing, so nothing reports an artifact. It
// starts <target_directory>/debug/<the app's binary target>, from
// `cargo metadata` asked as the build would ask Cargo. That is a prediction,
// and it does not follow a configuration file's build.target; such a run is
// recorded as not built, and the verdict refuses it.
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import readline from 'node:readline';

// The directory, under the repository root, that holds the app's package. It
// is where `tauri build` runs `cargo build` (Cargo reads its configuration
// files upwards from there and takes a relative target directory from
// there), so `cargo metadata` is asked from there too.
export const CARGO_DIR = 'src-tauri';

// What makes Cargo report its artifacts as JSON on standard output while it
// still renders its diagnostics, as text, on standard error.
export const MESSAGE_FORMAT_ARG = '--message-format=json-render-diagnostics';

// The runner's build: the debug app with its front end embedded, no bundle.
// Everything after the second `--` goes to Cargo.
export const BUILD_ARGS = ['run', 'tauri', '--', 'build', '--debug', '--no-bundle', '--', MESSAGE_FORMAT_ARG];

// The arguments `cargo` is given, in CARGO_DIR, for a --skip-build run.
export const METADATA_ARGS = ['metadata', '--format-version', '1', '--no-deps', '--manifest-path', 'Cargo.toml'];

// What the runner runs, for a repository root. Tests replace `build` with a
// plain `cargo build` of a made-up crate laid out as this one.
export function appCommands(repoRoot) {
  return {
    appManifest: path.join(repoRoot, CARGO_DIR, 'Cargo.toml'),
    build: { command: 'npm', args: BUILD_ARGS, cwd: repoRoot },
    metadata: { command: 'cargo', args: METADATA_ARGS, cwd: path.join(repoRoot, CARGO_DIR) },
  };
}

// The one place the runner learns the app binary. With skipBuild false it
// runs the build and returns the executable Cargo reported for the app;
// otherwise it builds nothing and returns the prediction. `env` is the one
// environment object every command here is given.
// Returns { appBinary, built, appBinaryFrom, identity }: `identity` is
// appIdentity(appBinary) taken as soon as the build has ended (or the
// prediction is made), which the driver checks before it starts each session.
export async function locateAppBinary({ skipBuild, env, commands, echo = (line) => process.stdout.write(`${line}\n`) }) {
  if (skipBuild) {
    const appBinary = predictAppBinary(commands, env);
    return { appBinary, built: false, appBinaryFrom: 'cargo metadata (nothing was built)', identity: appIdentity(appBinary) };
  }
  const appBinary = await buildAppBinary(commands, env, echo);
  return { appBinary, built: true, appBinaryFrom: "cargo's report of this run's build", identity: appIdentity(appBinary) };
}

// Which file a path names, and its state: device, inode, size and
// modification time (in nanoseconds), or null when there is no file there.
// A file put in its place (a new inode) or written over (a new size or
// modification time) gives another answer.
export function appIdentity(file) {
  let stat;
  try {
    stat = fs.statSync(file, { bigint: true });
  } catch (err) {
    if (err.code === 'ENOENT' || err.code === 'ENOTDIR') return null;
    throw err;
  }
  return `dev ${stat.dev} inode ${stat.ino} size ${stat.size} mtime ${stat.mtimeNs}ns`;
}

// Runs the build. Its standard error reaches the terminal as it comes; of its
// standard output, Cargo's JSON messages are kept and every other line (the
// front-end build's) is handed to `echo`. Throws when the build fails or
// Cargo's report does not name the app's executable.
async function buildAppBinary(commands, env, echo) {
  const { command, args, cwd } = commands.build;
  const child = spawn(command, args, { cwd, env, stdio: ['inherit', 'pipe', 'inherit'] });
  const messages = [];
  const lines = readline.createInterface({ input: child.stdout, crlfDelay: Infinity });
  lines.on('line', (line) => {
    const message = cargoMessage(line);
    if (message) messages.push(message);
    else echo(line);
  });
  const [status, signal] = await new Promise((resolve, reject) => {
    child.on('error', reject);
    child.on('close', (code, sig) => resolve([code, sig]));
  });
  if (status !== 0) {
    throw new Error(`\`${command} ${args.join(' ')}\` failed (${signal ? `signal ${signal}` : `exit ${status}`})`);
  }
  return appExecutableFromMessages(messages, commands.appManifest);
}

// A line of Cargo's JSON output, or null for any other line.
function cargoMessage(line) {
  if (!line.startsWith('{')) return null;
  try {
    const message = JSON.parse(line);
    return message && typeof message.reason === 'string' ? message : null;
  } catch {
    return null;
  }
}

// Whether a manifest path Cargo printed is the app's manifest: the same file,
// however either path is spelt.
function isAppManifest(manifestPath, appManifest) {
  if (typeof manifestPath !== 'string') return false;
  try {
    return fs.realpathSync(manifestPath) === fs.realpathSync(appManifest);
  } catch {
    return false;
  }
}

// From Cargo's messages of one build, the executable of the app's package's
// binary target, exactly as Cargo printed it. Another package's binary (a
// workspace member) is not the app and is passed over.
export function appExecutableFromMessages(messages, appManifest) {
  if (!messages.some((m) => m.reason === 'build-finished')) {
    throw new Error(`the build printed no report from Cargo (no build-finished message; was ${MESSAGE_FORMAT_ARG} passed on to Cargo?)`);
  }
  const executables = [
    ...new Set(
      messages
        .filter(
          (m) =>
            m.reason === 'compiler-artifact' &&
            Array.isArray(m.target?.kind) &&
            m.target.kind.includes('bin') &&
            typeof m.executable === 'string' &&
            isAppManifest(m.manifest_path, appManifest),
        )
        .map((m) => m.executable),
    ),
  ];
  if (executables.length === 0) {
    throw new Error(`Cargo reported no executable of the app's package (${appManifest})`);
  }
  if (executables.length > 1) {
    throw new Error(`more than one executable claims to be the app: ${executables.join(', ')}`);
  }
  if (!path.isAbsolute(executables[0])) {
    throw new Error(`Cargo reported the app's executable at a relative path: ${executables[0]}`);
  }
  return executables[0];
}

// For --skip-build: asks `cargo metadata`, as the build would ask Cargo, where
// it builds, and returns <target_directory>/debug/<the app's binary target>.
function predictAppBinary(commands, env) {
  const { command, args, cwd } = commands.metadata;
  // No standard input: Cargo reads none here, and a sandbox that refuses the
  // write end of an input pipe made the call report EPERM after Cargo had
  // answered.
  const res = spawnSync(command, args, { cwd, env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'] });
  if (res.error || res.status !== 0) {
    throw new Error(`\`${command} ${args.join(' ')}\` failed: ${res.error?.message || res.stderr?.trim() || `exit ${res.status}`}`);
  }
  return appBinaryFromMetadata(res.stdout, env, commands.appManifest);
}

// From the text `cargo metadata` printed: Cargo's target directory, as Cargo
// printed it, joined by a separator to debug/<the app package's one binary
// target>. No path is normalized: a `..` after a symbolic link means what it
// means to the file system, as it does to Cargo. Throws, saying why, when the
// answer does not say, or when CARGO_BUILD_TARGET is set (the binary is then
// under <target_directory>/<target>/debug/).
export function appBinaryFromMetadata(metadataText, env, appManifest) {
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
      `CARGO_BUILD_TARGET is ${JSON.stringify(env.CARGO_BUILD_TARGET)}: the binary would be under ${target}/${env.CARGO_BUILD_TARGET}/debug/, which is not looked for; unset it`,
    );
  }
  const app = (Array.isArray(metadata.packages) ? metadata.packages : []).filter((p) => isAppManifest(p?.manifest_path, appManifest));
  if (app.length !== 1) {
    throw new Error(`Cargo's metadata names ${app.length} packages whose manifest is ${appManifest}, not one`);
  }
  const bins = (Array.isArray(app[0].targets) ? app[0].targets : []).filter((t) => Array.isArray(t?.kind) && t.kind.includes('bin'));
  if (bins.length !== 1 || typeof bins[0].name !== 'string' || bins[0].name === '' || bins[0].name.includes('/')) {
    throw new Error(`Cargo's metadata names ${bins.length} binary targets of the app's package, not one`);
  }
  return `${target}/debug/${bins[0].name}`;
}
