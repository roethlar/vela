// Packaging the embedded player's libmpv set into the Linux packages
// (.agents/plans/embedded-player.md, "libmpv is shipped with Vela" and
// Slice 3).
//
// Two promises are held here. The first is to everyone who has not chosen
// the embedded player: a default build packages no part of the set and is the
// package it has always been. The second is to whoever builds with
// `scripts/build.sh --with-libmpv`: the set lands where the loader reads it,
// and the build does not succeed unless every package it produced has passed
// packaging/libmpv/check-package.sh.
//
// These tests have no built set and no built package, so they hold the check
// with stand-ins: packages made here whose "library" is a text file, a
// check-libmpv.py that says what a test tells it to say, and the real
// check-libmpv.py run against a stand-in for the library. That covers the
// places, the names, who calls what and when, and that each thing found
// wrong (a file missing, one byte changed, a library from elsewhere, another
// version) fails the package. What they cannot show is that the libraries a
// build really produced load, and that the packaging tools leave them as
// they were. That is proved by building real packages with
// `scripts/build.sh --with-libmpv`, which runs the check on each of them
// (docs/development.md, "Packaging the set").
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (file) => fs.readFileSync(path.join(repoRoot, file), 'utf8').replace(/\r\n/g, '\n');

const tauriConfText = read('src-tauri/tauri.conf.json');
const tauriConf = JSON.parse(tauriConfText);
const overlayPath = 'src-tauri/tauri.libmpv.conf.json';
const overlay = JSON.parse(read(overlayPath));
const packageJson = JSON.parse(read('package.json'));
const buildScript = read('scripts/build.sh');
const pkgbuild = read('packaging/arch/PKGBUILD');
const checkPackagePath = path.join(repoRoot, 'packaging', 'libmpv', 'check-package.sh');
const checkPackage = read('packaging/libmpv/check-package.sh');
const checkLibmpv = read('packaging/libmpv/check-libmpv.py');
const buildLibmpv = read('scripts/build-libmpv.sh');
const versions = read('packaging/libmpv/versions.txt');
const loader = read('src-tauri/src/player/embedded/libmpv.rs');

// Shell and PKGBUILD text without its comment lines.
const code = (source) =>
  source
    .split('\n')
    .filter((line) => !/^\s*#/.test(line))
    .join('\n');

function has(source, text, message) {
  assert.ok(source.includes(text), `${message}\nexpected to find:\n${text}`);
}

// The value of a `NAME="value"` line of a shell script.
function shellValue(source, name) {
  const found = source.match(new RegExp(`^${name}="([^"\\n]*)"$`, 'm'));
  assert.ok(found, `${name} must be set on a line of its own`);
  return found[1];
}

// The one condition under which the PKGBUILD does anything with the set.
const pkgbuildCondition = 'if [ -n "${VELA_PACKAGE_LIBMPV:-}" ]; then';

// The part of package() that runs only under that condition, and the rest.
function pkgbuildOptIn() {
  const start = pkgbuild.indexOf(`\n  ${pkgbuildCondition}\n`);
  assert.notEqual(start, -1, 'package() must install the set under the one condition');
  const end = pkgbuild.indexOf('\n  fi\n', start);
  assert.notEqual(end, -1, 'that condition must close');
  return { block: pkgbuild.slice(start, end + 1), rest: pkgbuild.slice(0, start) + pkgbuild.slice(end) };
}

const posixBashUnavailable = process.platform === 'win32' ? 'requires a POSIX /bin/bash' : false;

test('a default build packages no part of the libmpv set', () => {
  // The configuration every build uses does not know the set exists.
  assert.doesNotMatch(tauriConfText, /libmpv/i);
  // Tauri lays the platform's own config over it without being asked, and
  // that file has three spellings: tauri.linux.conf.json, the same in .json5,
  // and Tauri.linux.toml. The set must not come in through any of them, so
  // every file of src-tauri/ whose name begins with "tauri", in either case,
  // is read here. The overlay is the one exception, and it must not have a
  // name Tauri picks up by itself.
  // The overlay is told apart by its file name and not by a joined path: a
  // joined path has the separator of the machine the test runs on, and on
  // Windows it would never equal the name written here with a slash.
  const overlayName = overlayPath.split('/').pop();
  const inspected = [];
  for (const entry of fs.readdirSync(path.join(repoRoot, 'src-tauri'), { withFileTypes: true })) {
    if (entry.isDirectory() || !/^tauri/i.test(entry.name) || entry.name === overlayName) continue;
    inspected.push(entry.name);
    assert.doesNotMatch(read(`src-tauri/${entry.name}`), /libmpv/i, `${entry.name} must not package the set`);
  }
  assert.ok(inspected.includes('tauri.conf.json'), 'the configuration every build uses was not among the files read');
  assert.doesNotMatch(overlayName, /^tauri\.(linux|windows|macos|android|ios)\.conf\.json5?$/i);
  assert.doesNotMatch(overlayName, /^tauri(\.(linux|windows|macos|android|ios))?\.toml$/i);

  // The commands a default build runs name neither the overlay nor the
  // variable the PKGBUILD reads.
  for (const script of ['tauri', 'build', 'build:arch', 'build:linux']) {
    assert.doesNotMatch(packageJson.scripts[script], /libmpv|--config|VELA_PACKAGE/i, `npm run ${script}`);
  }

  // In the PKGBUILD the set is mentioned in two places, both under the one
  // condition: package() installs it, and makepkg is told not to strip it.
  // Outside them the package is described as it always was.
  const { block, rest } = pkgbuildOptIn();
  has(block, '/usr/lib/Vela/libmpv/', 'package() installs the set under the condition');
  const strip = `\n${pkgbuildCondition}\n  options+=('!strip')\nfi\n`;
  has(rest, strip, 'makepkg must leave the set unstripped, and only when the set is packaged');
  const untouched = code(rest.replace(strip, '\n'));
  assert.doesNotMatch(untouched, /libmpv|!strip/i);
  has(untouched, "\noptions=('!debug')\n", "the default package's options are unchanged");
});

test('the overlay adds the set, and only the set, where the loader reads it', () => {
  assert.deepEqual(overlay, { bundle: { resources: { '../.libmpv/out/libmpv/': 'libmpv/' } } });

  // The source is the directory scripts/build-libmpv.sh fills. Tauri reads
  // the path relative to src-tauri/.
  const [source] = Object.keys(overlay.bundle.resources);
  assert.equal(path.resolve(repoRoot, 'src-tauri', source), path.join(repoRoot, '.libmpv', 'out', 'libmpv'));
  has(buildLibmpv, '\nLIBMPV_ROOT="${LIBMPV_ROOT:-$repo/.libmpv}"\n', 'where build-libmpv.sh works');
  has(buildLibmpv, '\nOUT="$LIBMPV_ROOT/out"\n', 'where build-libmpv.sh puts its output');
  has(buildLibmpv, '\n  local dest="$OUT/libmpv" ', 'where build-libmpv.sh puts the set');

  // The target is the directory the loader joins to Vela's resource directory.
  const [target] = Object.values(overlay.bundle.resources);
  has(loader, '_ => resource_dir.join("libmpv"),', 'the directory the loader reads');
  assert.equal(target, 'libmpv/');

  // Tauri merges an overlay into the base key by key. That ADDS the set only
  // while the base lists its resources as a map: over a list, the overlay's
  // map would replace the list, and a package with the set would lose the
  // bundled mpv scripts.
  const base = tauriConf.bundle.resources;
  assert.equal(Object.prototype.toString.call(base), '[object Object]');
  assert.ok(!Object.keys(base).includes(source) && !Object.values(base).includes(target));
});

test('the package check looks where the loader looks, for the library the loader loads', () => {
  // Tauri's resource directory of an installed Linux package is
  // /usr/lib/<productName>; the loader adds "libmpv" and the file name.
  assert.equal(shellValue(checkPackage, 'SET_PATH'), `usr/lib/${tauriConf.productName}/libmpv`);
  // A package of Vela is known to the check by its program. Tauri's bundles
  // install the crate's binary under its own name in usr/bin, and the
  // PKGBUILD installs it there by hand.
  const crate = read('src-tauri/Cargo.toml').match(/^\[package\]\nname = "([^"\n]+)"$/m)?.[1];
  assert.ok(crate, 'src-tauri/Cargo.toml must name the crate first in [package]');
  assert.equal(shellValue(checkPackage, 'APP_PATH'), `usr/bin/${tauriConf.mainBinaryName ?? crate}`);
  has(pkgbuild, ` "$pkgdir/${shellValue(checkPackage, 'APP_PATH')}"\n`, 'where the Arch package installs the program');
  has(loader, '\npub(crate) const LIBRARY_FILE: &str = "libmpv.so.2";\n', 'the file the loader loads');
  has(checkLibmpv, '\n    path = os.path.join(libdir, "libmpv.so.2")\n', 'the file the check loads');
  // By absolute path and with the loader's flags, so that a library of the
  // same name somewhere else on the machine cannot answer instead.
  has(checkLibmpv, '\n    libdir = os.path.realpath(args.pop(0))\n', 'an absolute directory');
  has(
    checkLibmpv,
    '\n        lib = ctypes.CDLL(path, mode=os.RTLD_NOW | os.RTLD_LOCAL | os.RTLD_DEEPBIND)\n',
    'the load itself',
  );

  // The check is handed the unpacked package's own directory, after making
  // sure that directory really is inside the package.
  has(checkPackage, '\n  local package="$1" root="$2" set="$2/$SET_PATH"\n', 'the directory that is checked');
  has(
    checkPackage,
    '\n  if [ "$real_set" != "$real_root/$SET_PATH" ]; then\n    fail "place: ',
    'a link out of the package is refused',
  );
  has(
    checkPackage,
    '\n    out="$(env -u LD_LIBRARY_PATH -u LD_PRELOAD python3 -I -B "$here/check-libmpv.py" "$set" \\\n' +
      '             --expect-mpv "$mpv_pin" --expect-client-api "$CLIENT_API" 2>&1)" || status=$?\n',
    'the load, from the unpacked package and with nothing of the builder\'s shell in the way',
  );
  // What that call really hands over, and what the check makes of the
  // answer, is run further down ("the package check calls check-libmpv.py on
  // the unpacked set...").
});

test('the package check expects the pinned mpv and the client API version of that release', () => {
  const mpvPin = versions.match(/^mpv\s+(\S+)\s+ship\s/m)?.[1];
  assert.ok(mpvPin, 'versions.txt must pin mpv');
  has(
    checkPackage,
    `\n  mpv_pin="$(awk '$1 == "mpv" { print $2 }' "$here/versions.txt")"\n`,
    'the mpv version comes from the one place it is written',
  );

  // The client API version is not in versions.txt. It is written in the check
  // beside the release it was looked up for, and an mpv bump that leaves it
  // behind stops the check (and fails here first).
  assert.equal(shellValue(checkPackage, 'CLIENT_API_OF_MPV'), mpvPin);
  has(checkPackage, '\n  [ "$mpv_pin" = "$CLIENT_API_OF_MPV" ] || die "', 'a stale number stops the check');

  // A version the loader itself would refuse cannot be the expected one.
  const [major, minor] = shellValue(checkPackage, 'CLIENT_API').split('.').map(Number);
  const loaderMajor = Number(loader.match(/^const API_MAJOR: c_ulong = (\d+);$/m)?.[1]);
  const loaderMinor = Number(loader.match(/^const API_MINOR_AT_LEAST: c_ulong = (\d+);$/m)?.[1]);
  assert.equal(major, loaderMajor);
  assert.ok(minor >= loaderMinor, `client API ${major}.${minor} is below what the loader accepts`);

  // check-libmpv.py takes the expectation, compares it with what the library
  // says, and a difference fails the run.
  has(checkLibmpv, '\n        elif a == "--expect-client-api":\n            expect_api = args.pop(0)\n', 'the option');
  has(checkLibmpv, '\n    v = lib.mpv_client_api_version()\n    api = f"{v >> 16}.{v & 0xffff}"\n', 'what the library says');
  has(
    checkLibmpv,
    '\n    good = expect_api is None or api == expect_api\n' +
      '    print(f"{\'ok  \' if good else \'FAIL\'} load: {path} (client API {api})"\n' +
      '          + ("" if good else f" (expected {expect_api})"))\n' +
      '    ok = ok and good\n',
    'the comparison',
  );
});

test('the Arch package installs the set where the loader reads it, with its licence texts', () => {
  const { block } = pkgbuildOptIn();
  // Every file of the directory, to the directory the check looks in.
  has(
    block,
    "\n    while IFS= read -r -d '' file; do\n" +
      `      install -Dm644 "$libmpv/$file" "$pkgdir/${shellValue(checkPackage, 'SET_PATH')}/$file"\n` +
      "    done < <(cd \"$libmpv\" && find . -type f -printf '%P\\0')\n",
    'the whole directory is installed where the loader reads it',
  );
  has(block, '\n    local libmpv="$VELA_PACKAGE_LIBMPV" ', 'from the directory the builder named');
  // A variable that names no set stops makepkg; it does not go on to build a
  // package without one.
  has(
    block,
    '\n    if [ ! -f "$libmpv/MANIFEST" ] || [ ! -f "$libmpv/libmpv.so.2" ]; then\n' +
      '      echo "VELA_PACKAGE_LIBMPV names $libmpv, which holds no libmpv set" >&2\n' +
      '      return 1\n' +
      '    fi\n',
    'no set, no package',
  );
  // The licence texts the MANIFEST names, also where Arch keeps licences,
  // which is where the check looks for them in a pacman package.
  const pkgname = pkgbuild.match(/^pkgname=(\S+)$/m)?.[1];
  assert.equal(shellValue(checkPackage, 'ARCH_LICENCES'), `usr/share/licenses/${pkgname}/libmpv`);
  has(
    block,
    '\n    while read -r kind component path; do\n' +
      '      [ "$kind" = licence ] || continue\n' +
      '      install -Dm644 "$libmpv/$path" "$pkgdir/usr/share/licenses/$pkgname/libmpv/$component/${path##*/}"\n' +
      '    done < "$libmpv/MANIFEST"\n',
    'the licence texts go where Arch keeps licences',
  );
  has(
    checkPackage,
    '\n          cmp -s "$set/$path" "$root/$ARCH_LICENCES/$component/${path##*/}" \\\n            || fail "licences: ',
    'and the check looks for each of them there',
  );
});

test('the release and CI workflows do not package the set yet', () => {
  // Putting the set into released packages is the owner's step, taken
  // together with the Settings control (docs/development.md, "Packaging the
  // set"). Until then neither workflow may opt in, by any name the opt-in has.
  const release = read('.github/workflows/release.yml');
  const ci = read('.github/workflows/ci.yml');
  for (const [workflow, text] of [['release.yml', release], ['ci.yml', ci]]) {
    assert.doesNotMatch(text, /tauri\.libmpv\.conf|with-libmpv|VELA_PACKAGE_LIBMPV|check-package/, workflow);
  }

  // The opt-in's names are not the only way in. A step can write a platform
  // config that Tauri then merges by itself (the Windows job of release.yml
  // already writes one), hand `tauri build` a config as inline JSON, or copy
  // the set into the resources every build bundles. So this goes further
  // than the names: release.yml builds every released package and has no
  // other business with the set, and it does not mention it at all. (A step
  // that brought the set in without ever writing "libmpv" would still not be
  // seen here; a change to a workflow has to be read all the same.)
  assert.doesNotMatch(release, /libmpv/i, 'release.yml');

  // ci.yml does have business with it: its `libmpv` job builds the set and
  // proves it. Every line of ci.yml that mentions the set is in that job, and
  // that job builds no package of Vela.
  let job = null;
  let inJobs = false;
  const outside = [];
  const libmpvJob = [];
  for (const line of ci.split('\n')) {
    if (/^\S/.test(line)) inJobs = /^jobs:\s*$/.test(line);
    const header = inJobs && line.match(/^ {2}([A-Za-z0-9_-]+):\s*(#.*)?$/);
    if (header) job = header[1];
    if (inJobs && job === 'libmpv') libmpvJob.push(line);
    else if (/libmpv/i.test(line)) outside.push(line.trim());
  }
  assert.ok(libmpvJob.length > 1, 'ci.yml must keep its libmpv job');
  assert.deepEqual(outside, [], 'ci.yml mentions the set outside its libmpv job');
  assert.doesNotMatch(
    libmpvJob.join('\n'),
    /tauri|makepkg|scripts\/build\.sh|build:arch|build:linux/i,
    "ci.yml's libmpv job must not build a package",
  );
});

// --- scripts/build.sh, run for real against fakes ---------------------------

function executable(file, body) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, body);
  fs.chmodSync(file, 0o755);
}

// A copy of build.sh in a directory that looks enough like the repository,
// with every program it calls replaced by a fake that records its call.
// `host` is what the copied script reads as /etc/os-release, which decides
// between the Tauri bundles and the Arch package.
// `archPackage` is where the stand-in for the Arch build leaves its package,
// and where the stand-in for makepkg says this build's package is (null: it
// cannot say); a path that is not absolute is inside the fixture.
function buildFixture(
  t,
  {
    host = 'ubuntu',
    uname = 'Linux',
    preflightStatus = 0,
    checkStatus = 0,
    produce = true,
    archPackage = 'packaging/arch/vela-1.0.0-1-x86_64.pkg.tar.zst',
    makepkgNames = archPackage,
  } = {},
) {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-packaging-')));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const fakeBin = path.join(root, 'fake-bin');
  const calls = path.join(root, 'calls.log');
  fs.writeFileSync(calls, '');
  fs.mkdirSync(path.join(root, 'node_modules'), { recursive: true });
  fs.writeFileSync(path.join(root, 'node_modules', '.package-lock.json'), '{}\n');
  fs.mkdirSync(path.join(root, 'packaging', 'arch'), { recursive: true });
  fs.mkdirSync(path.join(root, 'src-tauri'), { recursive: true });
  fs.copyFileSync(path.join(repoRoot, overlayPath), path.join(root, overlayPath));

  const osRelease = path.join(root, 'os-release');
  fs.writeFileSync(osRelease, `ID=${host}\n`);
  const script = buildScript.replaceAll('/etc/os-release', osRelease);
  assert.notEqual(script, buildScript, 'the fixture must decide which distribution the script sees');
  executable(path.join(root, 'scripts', 'build.sh'), script);

  executable(path.join(fakeBin, 'uname'), `#!/bin/sh\nprintf '${uname}\\n'\n`);
  // The toolchain check is not under test. Anything else Node is asked to do
  // is done by the real one.
  executable(
    path.join(fakeBin, 'node'),
    `#!/bin/sh\ncase "$1" in scripts/check-js-toolchain.mjs) exit 0 ;; esac\nexec "${process.execPath}" "$@"\n`,
  );
  for (const tool of ['cargo', 'rustup', 'pacman', 'apt-get', 'rpm', 'dnf']) {
    executable(path.join(fakeBin, tool), '#!/bin/sh\nexit 0\n');
  }
  executable(path.join(fakeBin, 'dpkg-query'), "#!/bin/sh\nprintf 'installed'\n");
  // The build. It records its command line, the variable the PKGBUILD reads
  // and the overlay it was handed (a temporary one is gone once build.sh has
  // exited), then leaves the packages a real build would.
  executable(
    path.join(fakeBin, 'npm'),
    `#!/bin/sh
set -eu
printf 'npm %s\\n' "$*" >> "${calls}"
printf 'VELA_PACKAGE_LIBMPV=%s\\n' "\${VELA_PACKAGE_LIBMPV-<unset>}" >> "${calls}"
previous=''
for argument in "$@"; do
  if [ "$previous" = --config ]; then cp "$argument" "${root}/overlay-used.json"; fi
  previous="$argument"
done
${produce ? '' : 'exit 0'}
case "$*" in
  'run build:arch')
    mkdir -p "${path.dirname(path.resolve(root, archPackage))}"
    printf 'package\\n' > "${path.resolve(root, archPackage)}" ;;
  *)
    mkdir -p src-tauri/target/release/bundle/deb src-tauri/target/release/bundle/rpm
    printf 'package\\n' > src-tauri/target/release/bundle/deb/Vela_1.0.0_amd64.deb
    printf 'package\\n' > src-tauri/target/release/bundle/rpm/Vela-1.0.0-1.x86_64.rpm ;;
esac
`,
  );
  // makepkg, asked which package this build makes. It records where it was
  // asked, and answers as the real one does: one full path to a line.
  executable(
    path.join(fakeBin, 'makepkg'),
    `#!/bin/sh
printf 'makepkg %s (in %s)\\n' "$*" "\${PWD#${root}/}" >> "${calls}"
[ "$*" = --packagelist ] || exit 64
${makepkgNames === null ? 'exit 1' : `printf '%s\\n' "${path.resolve(root, makepkgNames)}"`}
`,
  );
  // The package check. Asked beforehand whether it can run, it records the
  // question. Asked to check, it records what it was given and what dist/
  // held at that moment.
  executable(
    path.join(root, 'packaging', 'libmpv', 'check-package.sh'),
    `#!/bin/sh
if [ "$1" = --preflight ]; then
  shift
  printf 'preflight %s\\n' "$*" >> "${calls}"
  exit ${preflightStatus}
fi
printf 'check %s\\n' "$*" >> "${calls}"
printf 'in dist/ at that moment: %s\\n' "$(ls dist | wc -l | tr -d ' ')" >> "${calls}"
exit ${checkStatus}
`,
  );

  const addSet = (directory) => {
    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(path.join(directory, 'MANIFEST'), 'file 0 0 libmpv.so.2\n');
    fs.writeFileSync(path.join(directory, 'libmpv.so.2'), '');
    return directory;
  };
  const run = (args, { cwd = root, env = {} } = {}) =>
    spawnSync('/bin/bash', [path.join(root, 'scripts', 'build.sh'), ...args], {
      cwd,
      encoding: 'utf8',
      input: '',
      env: { ...process.env, ...env, PATH: `${fakeBin}:${process.env.PATH}` },
    });
  const log = () => fs.readFileSync(calls, 'utf8').split('\n').filter(Boolean);
  const dist = () => fs.readdirSync(path.join(root, 'dist')).sort();
  // What an earlier, successful build left in dist/. Without it, "dist/ is
  // empty after a failed build" would also be true of a script that never
  // touched dist/.
  const leaveEarlierBuild = () => {
    fs.mkdirSync(path.join(root, 'dist'), { recursive: true });
    fs.writeFileSync(path.join(root, 'dist', 'Vela_0.9.0_amd64.deb'), 'the package of an earlier build\n');
  };
  return { root, addSet, run, log, dist, leaveEarlierBuild, defaultSet: path.join(root, '.libmpv', 'out', 'libmpv') };
}

const debAndRpm = [
  'src-tauri/target/release/bundle/deb/Vela_1.0.0_amd64.deb',
  'src-tauri/target/release/bundle/rpm/Vela-1.0.0-1.x86_64.rpm',
];

test('a build without --with-libmpv passes no overlay and checks no package', { skip: posixBashUnavailable }, (t) => {
  // A set lying ready in the checkout, and the PKGBUILD's variable left in
  // the builder's shell, must change nothing.
  const fixture = buildFixture(t);
  fixture.addSet(fixture.defaultSet);
  const built = fixture.run(['--bundles', 'deb,rpm'], { env: { VELA_PACKAGE_LIBMPV: fixture.defaultSet } });
  assert.equal(built.status, 0, `${built.stdout}\n${built.stderr}`);
  assert.deepEqual(fixture.log(), ['npm run tauri -- build --bundles deb,rpm', 'VELA_PACKAGE_LIBMPV=<unset>']);
  assert.deepEqual(fixture.dist(), ['Vela-1.0.0-1.x86_64.rpm', 'Vela_1.0.0_amd64.deb']);
  assert.doesNotMatch(built.stdout, /libmpv/);
});

test('a default Arch build clears the variable that would package the set', { skip: posixBashUnavailable }, (t) => {
  const fixture = buildFixture(t, { host: 'arch' });
  fixture.addSet(fixture.defaultSet);
  const built = fixture.run([], { env: { VELA_PACKAGE_LIBMPV: fixture.defaultSet } });
  assert.equal(built.status, 0, `${built.stdout}\n${built.stderr}`);
  // The build, and makepkg asked which package it made: no question to the check.
  assert.deepEqual(fixture.log(), ['npm run build:arch', 'VELA_PACKAGE_LIBMPV=<unset>', 'makepkg --packagelist (in packaging/arch)']);
  assert.deepEqual(fixture.dist(), ['vela-1.0.0-1-x86_64.pkg.tar.zst']);
});

test('a default Arch build hands out the package it made, and not an earlier one with the set', { skip: posixBashUnavailable }, async (t) => {
  // packaging/arch keeps the packages of earlier builds, an opt-in one among
  // them. A build that was not asked for the set must not hand that one out.
  const older = 'vela-0.9.0-1-x86_64.pkg.tar.zst';
  const withOlder = (options) => {
    const fixture = buildFixture(t, { host: 'arch', ...options });
    fs.writeFileSync(path.join(fixture.root, 'packaging', 'arch', older), 'an earlier build, with the set\n');
    return fixture;
  };

  await t.test('beside this build\'s package', () => {
    const fixture = withOlder();
    const built = fixture.run([]);
    assert.equal(built.status, 0, `${built.stdout}\n${built.stderr}`);
    assert.deepEqual(fixture.dist(), ['vela-1.0.0-1-x86_64.pkg.tar.zst']);
  });

  await t.test('makepkg was told to put its package elsewhere (PKGDEST): it is found there', () => {
    const fixture = withOlder({ archPackage: 'built packages/vela-1.0.0-1-x86_64.pkg.tar.zst' });
    const built = fixture.run([]);
    assert.equal(built.status, 0, `${built.stdout}\n${built.stderr}`);
    assert.deepEqual(fixture.dist(), ['vela-1.0.0-1-x86_64.pkg.tar.zst']);
    assert.ok(built.stdout.includes(`==> Artifacts (also in ${fixture.root}/built packages):`), built.stdout);
  });

  await t.test('the package makepkg names is not there: the older one does not stand in for it', () => {
    const fixture = withOlder({ produce: false });
    fixture.leaveEarlierBuild();
    const built = fixture.run([]);
    assert.equal(built.status, 1, `${built.stdout}\n${built.stderr}`);
    assert.ok(
      built.stderr.includes(
        `==> makepkg names ${fixture.root}/packaging/arch/vela-1.0.0-1-x86_64.pkg.tar.zst as the package of this build, and it is not there`,
      ),
      built.stderr,
    );
    assert.deepEqual(fixture.dist(), []);
  });

  await t.test('makepkg cannot say which package it made: nothing is handed out', () => {
    const fixture = withOlder({ makepkgNames: null });
    const built = fixture.run([]);
    assert.equal(built.status, 0, `${built.stdout}\n${built.stderr}`);
    assert.match(built.stdout, /nothing matched/);
    assert.deepEqual(fixture.dist(), []);
  });
});

test('--with-libmpv passes the overlay and checks every package before any reaches dist/', { skip: posixBashUnavailable }, (t) => {
  const fixture = buildFixture(t);
  const set = fixture.addSet(fixture.defaultSet);
  fixture.leaveEarlierBuild();
  const built = fixture.run(['--bundles', 'deb,rpm', '--with-libmpv']);
  assert.equal(built.status, 0, `${built.stdout}\n${built.stderr}`);
  assert.deepEqual(fixture.log(), [
    // Before anything is built: can the packages be checked at all?
    `preflight --set ${set}`,
    `npm run tauri -- build --bundles deb,rpm --config ${overlayPath}`,
    'VELA_PACKAGE_LIBMPV=<unset>',
    `check --set ${set} ${debAndRpm.join(' ')}`,
    'in dist/ at that moment: 0',
  ]);
  assert.deepEqual(fixture.dist(), ['Vela-1.0.0-1.x86_64.rpm', 'Vela_1.0.0_amd64.deb']);
});

test('--with-libmpv asks beforehand whether an AppImage can be checked only when one is to be built', { skip: posixBashUnavailable }, (t) => {
  for (const [bundles, asked] of [['appimage', ' --appimage'], ['deb,appimage', ' --appimage'], ['deb', '']]) {
    const fixture = buildFixture(t);
    const set = fixture.addSet(fixture.defaultSet);
    // (The stand-in build makes no AppImage, so how each build ends is not
    // the question here.)
    fixture.run(['--bundles', bundles, '--with-libmpv']);
    assert.equal(fixture.log()[0], `preflight --set ${set}${asked}`, bundles);
  }
  // The default bundle of a Debian-family host is an AppImage.
  const fixture = buildFixture(t);
  const set = fixture.addSet(fixture.defaultSet);
  fixture.run(['--with-libmpv']);
  assert.equal(fixture.log()[0], `preflight --set ${set} --appimage`);
});

test('a package that fails the check fails the build and leaves dist/ empty', { skip: posixBashUnavailable }, (t) => {
  const fixture = buildFixture(t, { checkStatus: 1 });
  fixture.addSet(fixture.defaultSet);
  // The packages of an earlier build must not be left standing as if they
  // were this build's.
  fixture.leaveEarlierBuild();
  const built = fixture.run(['--bundles', 'deb,rpm', '--with-libmpv']);
  assert.equal(built.status, 1, `${built.stdout}\n${built.stderr}`);
  assert.match(built.stderr, /A package does not carry the libmpv set as it must/);
  assert.doesNotMatch(built.stderr, /could not be checked/);
  assert.equal(fixture.log().filter((line) => line.startsWith('check ')).length, 1);
  assert.deepEqual(fixture.dist(), []);
});

test('a --with-libmpv build that could not check its packages says so before it builds anything', { skip: posixBashUnavailable }, (t) => {
  // No bsdtar, no Python, a client API number left behind by an mpv bump:
  // the check would stop with status 2 after the whole release build.
  const fixture = buildFixture(t, { preflightStatus: 2 });
  const set = fixture.addSet(fixture.defaultSet);
  fixture.leaveEarlierBuild();
  const built = fixture.run(['--bundles', 'deb,rpm', '--with-libmpv']);
  assert.equal(built.status, 1, `${built.stdout}\n${built.stderr}`);
  assert.match(built.stderr, /^--with-libmpv: the packages could not be checked once built \(see above\), so nothing is built$/m);
  assert.deepEqual(fixture.log(), [`preflight --set ${set}`]);
  // Nothing was built, so what an earlier build left is still what there is.
  assert.deepEqual(fixture.dist(), ['Vela_0.9.0_amd64.deb']);
});

test('a check that could not judge the packages is not reported as a package without the set', { skip: posixBashUnavailable }, (t) => {
  // Status 1 from the check is its verdict on a package. Any other failure
  // is the check's own, and the build must not name a cause it did not find.
  for (const status of [2, 143]) {
    const fixture = buildFixture(t, { checkStatus: status });
    fixture.addSet(fixture.defaultSet);
    fixture.leaveEarlierBuild();
    const built = fixture.run(['--bundles', 'deb,rpm', '--with-libmpv']);
    assert.equal(built.status, 1, `${built.stdout}\n${built.stderr}`);
    assert.ok(
      built.stderr.includes(`The packages could not be checked for the libmpv set (see above; the check ended with status ${status}).`),
      built.stderr,
    );
    assert.doesNotMatch(built.stderr, /does not carry the libmpv set/);
    assert.deepEqual(fixture.dist(), []);
  }
});

test('--with-libmpv with nothing built is a failed build, not an unchecked one', { skip: posixBashUnavailable }, (t) => {
  const fixture = buildFixture(t, { produce: false });
  fixture.addSet(fixture.defaultSet);
  fixture.leaveEarlierBuild();
  const built = fixture.run(['--bundles', 'deb,rpm', '--with-libmpv']);
  assert.equal(built.status, 1, `${built.stdout}\n${built.stderr}`);
  assert.match(built.stderr, /--with-libmpv: the build left no package to check/);
  assert.deepEqual(fixture.dist(), []);
  // The same build without the opt-in is today's: it says nothing matched
  // and exits 0.
  const plain = buildFixture(t, { produce: false });
  const unchecked = plain.run(['--bundles', 'deb,rpm']);
  assert.equal(unchecked.status, 0, `${unchecked.stdout}\n${unchecked.stderr}`);
  assert.match(unchecked.stdout, /nothing matched/);
});

test('--with-libmpv=DIR packages the set of that directory, named from where the builder stands', { skip: posixBashUnavailable }, (t) => {
  const fixture = buildFixture(t);
  // A set also lies in the default place: the named one must win.
  fixture.addSet(fixture.defaultSet);
  const elsewhere = path.join(fixture.root, 'elsewhere');
  const set = fixture.addSet(path.join(elsewhere, 'my set'));
  const built = fixture.run(['--bundles', 'deb,rpm', '--with-libmpv=my set'], { cwd: elsewhere });
  assert.equal(built.status, 0, `${built.stdout}\n${built.stderr}`);

  const [preflight, npm, , check] = fixture.log();
  assert.equal(preflight, `preflight --set ${set}`);
  const used = npm.match(/^npm run tauri -- build --bundles deb,rpm --config (.+)$/)?.[1];
  assert.ok(used, npm);
  assert.notEqual(used, overlayPath, 'the committed overlay names another directory');
  // What Tauri was handed is the committed overlay with the directory replaced.
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(fixture.root, 'overlay-used.json'), 'utf8')), {
    bundle: { resources: { [`${set}/`]: 'libmpv/' } },
  });
  assert.equal(check, `check --set ${set} ${debAndRpm.join(' ')}`);
  // The committed overlay was not touched, and the temporary one is gone.
  assert.equal(fs.readFileSync(path.join(fixture.root, overlayPath), 'utf8'), read(overlayPath));
  assert.equal(fs.existsSync(used), false);
  assert.equal(fs.existsSync(path.dirname(used)), false);
});

test('--with-libmpv stops before building when there is no set to package', { skip: posixBashUnavailable }, (t) => {
  const fixture = buildFixture(t);
  const built = fixture.run(['--bundles', 'deb,rpm', '--with-libmpv']);
  assert.equal(built.status, 1, `${built.stdout}\n${built.stderr}`);
  assert.match(built.stderr, /^--with-libmpv: there is no libmpv set in \.libmpv\/out\/libmpv\n/);
  assert.match(built.stderr, /scripts\/build-libmpv\.sh --container/);
  assert.deepEqual(fixture.log(), []);

  // A directory with a MANIFEST and no library is not a set either.
  fs.mkdirSync(fixture.defaultSet, { recursive: true });
  fs.writeFileSync(path.join(fixture.defaultSet, 'MANIFEST'), '');
  assert.equal(fixture.run(['--bundles', 'deb,rpm', '--with-libmpv']).status, 1);
  assert.deepEqual(fixture.log(), []);

  // Nor is one with a library and no MANIFEST. The package check could not
  // judge a package against it, and would say so only after the whole build.
  fs.rmSync(path.join(fixture.defaultSet, 'MANIFEST'));
  fs.writeFileSync(path.join(fixture.defaultSet, 'libmpv.so.2'), '');
  const withoutManifest = fixture.run(['--bundles', 'deb,rpm', '--with-libmpv']);
  assert.equal(withoutManifest.status, 1, `${withoutManifest.stdout}\n${withoutManifest.stderr}`);
  assert.match(withoutManifest.stderr, /^--with-libmpv: there is no libmpv set in \.libmpv\/out\/libmpv\n/);
  assert.deepEqual(fixture.log(), []);
});

test('--with-libmpv=DIR takes an absolute directory as it is', { skip: posixBashUnavailable }, (t) => {
  // The form docs/development.md shows: --with-libmpv=/path/to/a/set. It
  // must not be read as relative to where the builder stands.
  const fixture = buildFixture(t);
  fixture.addSet(fixture.defaultSet);
  const set = fixture.addSet(path.join(fixture.root, 'kept', 'a set'));
  const elsewhere = path.join(fixture.root, 'elsewhere');
  fs.mkdirSync(elsewhere);
  const built = fixture.run(['--bundles', 'deb,rpm', `--with-libmpv=${set}`], { cwd: elsewhere });
  assert.equal(built.status, 0, `${built.stdout}\n${built.stderr}`);
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(fixture.root, 'overlay-used.json'), 'utf8')), {
    bundle: { resources: { [`${set}/`]: 'libmpv/' } },
  });
  assert.equal(fixture.log()[0], `preflight --set ${set}`);
  assert.equal(fixture.log()[3], `check --set ${set} ${debAndRpm.join(' ')}`);
});

test('--with-libmpv= with no directory after it is refused, not taken for the default set', { skip: posixBashUnavailable }, (t) => {
  // What `--with-libmpv="$SET"` becomes when SET is empty. A set lies in the
  // default place, ready to be packaged by mistake.
  const fixture = buildFixture(t);
  fixture.addSet(fixture.defaultSet);
  const built = fixture.run(['--bundles', 'deb,rpm', '--with-libmpv=']);
  assert.equal(built.status, 2, `${built.stdout}\n${built.stderr}`);
  assert.match(built.stderr, /^--with-libmpv= needs a directory$/m);
  assert.deepEqual(fixture.log(), []);
});

test('--with-libmpv on Arch names the set to the PKGBUILD and checks the package', { skip: posixBashUnavailable }, (t) => {
  const fixture = buildFixture(t, { host: 'arch' });
  const set = fixture.addSet(fixture.defaultSet);
  const built = fixture.run(['--with-libmpv']);
  assert.equal(built.status, 0, `${built.stdout}\n${built.stderr}`);
  assert.deepEqual(fixture.log(), [
    `preflight --set ${set}`,
    'npm run build:arch',
    `VELA_PACKAGE_LIBMPV=${set}`,
    // Which package did this build make? makepkg knows, asked beside the PKGBUILD.
    'makepkg --packagelist (in packaging/arch)',
    `check --set ${set} ${fixture.root}/packaging/arch/vela-1.0.0-1-x86_64.pkg.tar.zst`,
    'in dist/ at that moment: 0',
  ]);
  assert.deepEqual(fixture.dist(), ['vela-1.0.0-1-x86_64.pkg.tar.zst']);
});

test('--with-libmpv on Arch checks and hands out the package this build made, and no other', { skip: posixBashUnavailable }, async (t) => {
  // packaging/arch is never cleared. An older package that carries the set
  // would pass the check as well as this build's.
  const older = 'vela-0.9.0-1-x86_64.pkg.tar.zst';
  const withOlder = (options) => {
    const fixture = buildFixture(t, { host: 'arch', ...options });
    fixture.addSet(fixture.defaultSet);
    fs.writeFileSync(path.join(fixture.root, 'packaging', 'arch', older), 'an earlier build, with the set\n');
    return fixture;
  };
  const checked = (fixture) => fixture.log().filter((line) => line.startsWith('check '));

  await t.test('an older package beside the new one is neither checked nor copied', () => {
    const fixture = withOlder();
    const built = fixture.run(['--with-libmpv']);
    assert.equal(built.status, 0, `${built.stdout}\n${built.stderr}`);
    assert.deepEqual(checked(fixture), [
      `check --set ${fixture.defaultSet} ${fixture.root}/packaging/arch/vela-1.0.0-1-x86_64.pkg.tar.zst`,
    ]);
    assert.deepEqual(fixture.dist(), ['vela-1.0.0-1-x86_64.pkg.tar.zst']);
  });

  await t.test('makepkg was told to put its package elsewhere (PKGDEST): it is found there', () => {
    const fixture = withOlder({ archPackage: 'built packages/vela-1.0.0-1-x86_64.pkg.tar.zst' });
    const built = fixture.run(['--with-libmpv']);
    assert.equal(built.status, 0, `${built.stdout}\n${built.stderr}`);
    assert.deepEqual(checked(fixture), [
      `check --set ${fixture.defaultSet} ${fixture.root}/built packages/vela-1.0.0-1-x86_64.pkg.tar.zst`,
    ]);
    assert.deepEqual(fixture.dist(), ['vela-1.0.0-1-x86_64.pkg.tar.zst']);
    assert.ok(built.stdout.includes(`==> Artifacts (also in ${fixture.root}/built packages):`), built.stdout);
  });

  await t.test('the package makepkg names is not there: the older one does not stand in for it', () => {
    const fixture = withOlder({ produce: false });
    fixture.leaveEarlierBuild();
    const built = fixture.run(['--with-libmpv']);
    assert.equal(built.status, 1, `${built.stdout}\n${built.stderr}`);
    assert.ok(
      built.stderr.includes(
        `==> makepkg names ${fixture.root}/packaging/arch/vela-1.0.0-1-x86_64.pkg.tar.zst as the package of this build, and it is not there`,
      ),
      built.stderr,
    );
    assert.deepEqual(checked(fixture), []);
    assert.deepEqual(fixture.dist(), []);
  });

  await t.test('makepkg cannot say which package it made: nothing is checked, and that is a failure', () => {
    const fixture = withOlder({ makepkgNames: null });
    const built = fixture.run(['--with-libmpv']);
    assert.equal(built.status, 1, `${built.stdout}\n${built.stderr}`);
    assert.match(built.stderr, /--with-libmpv: the build left no package to check/);
    assert.deepEqual(checked(fixture), []);
    assert.deepEqual(fixture.dist(), []);
  });
});

test('--with-libmpv is refused where there is no set to package', { skip: posixBashUnavailable }, (t) => {
  const fixture = buildFixture(t, { uname: 'Darwin' });
  fixture.addSet(fixture.defaultSet);
  const built = fixture.run(['--with-libmpv']);
  assert.equal(built.status, 2, `${built.stdout}\n${built.stderr}`);
  assert.match(built.stderr, /--with-libmpv: the libmpv set exists for Linux packages only/);
  assert.deepEqual(fixture.log(), []);
});

// --- packaging/libmpv/check-package.sh, on packages made here ----------------
//
// These need no built set. They prove the check's second mode, and what its
// first mode refuses before it ever loads a library. They need what the check
// needs: Linux, and bsdtar to pack and unpack.

const onLinux = process.platform === 'linux';
const bsdtarMissing = onLinux && spawnSync('bsdtar', ['--version'], { encoding: 'utf8' }).status !== 0;
const checkUnavailable = !onLinux
  ? 'the libmpv set is packaged on Linux only'
  : bsdtarMissing
    ? 'needs bsdtar (libarchive), as check-package.sh does'
    : false;

// Without bsdtar every test below that runs the check is skipped, with that
// reason printed, and a skipped test holds nothing. GitHub's `frontend` job
// runs `npm run check` on a runner that is not known to have bsdtar and
// installs nothing, so until its workflow installs libarchive-tools, CI
// does not run these tests; a developer's machine and the E2E venue do
// (docs/development.md, "Packaging the set", says what the workflow needs).

// The kinds of package the check unpacks, each made here from a file tree
// with nothing but bsdtar: `pack(root, tree)` writes the package into `root`
// and returns its path.
//
// The deb is a real one: an ar archive of debian-binary, control.tar.gz and
// data.tar.gz, as Tauri's bundler writes it. The rpm is NOT a package rpm
// itself would install. It is the frame of one (the 96-byte lead and two
// headers, both empty) around the gzip-compressed cpio archive an rpm carries
// its files in, which is all bsdtar reads of an rpm. No tool that writes a
// real rpm can be counted on here; real ones are checked when a build makes
// them.
const bsdtar = (cwd, ...args) => {
  const done = spawnSync('bsdtar', args, { cwd, encoding: 'utf8' });
  assert.equal(done.status, 0, `bsdtar ${args.join(' ')}\n${done.stderr}`);
};
const packageKinds = {
  'pkg.tar.zst': (root, tree) => {
    bsdtar(root, '--zstd', '-cf', 'vela-1.0.0-1-x86_64.pkg.tar.zst', '-C', tree, 'usr');
    return path.join(root, 'vela-1.0.0-1-x86_64.pkg.tar.zst');
  },
  'pkg.tar.xz': (root, tree) => {
    bsdtar(root, '-cJf', 'vela-1.0.0-1-x86_64.pkg.tar.xz', '-C', tree, 'usr');
    return path.join(root, 'vela-1.0.0-1-x86_64.pkg.tar.xz');
  },
  // dataMember: false leaves the member that holds the files out; 'cut short'
  // packs half of it, inside a deb that is itself whole.
  deb: (root, tree, { dataMember = true } = {}) => {
    fs.writeFileSync(path.join(root, 'debian-binary'), '2.0\n');
    fs.writeFileSync(path.join(root, 'control'), 'Package: vela\nVersion: 1.0.0\n');
    bsdtar(root, '-czf', 'control.tar.gz', 'control');
    if (dataMember) bsdtar(root, '-czf', 'data.tar.gz', '-C', tree, 'usr');
    if (dataMember === 'cut short') {
      const data = fs.readFileSync(path.join(root, 'data.tar.gz'));
      fs.writeFileSync(path.join(root, 'data.tar.gz'), data.subarray(0, data.length >> 1));
    }
    const members = ['debian-binary', 'control.tar.gz', ...(dataMember ? ['data.tar.gz'] : [])];
    bsdtar(root, '--format=ar', '-cf', 'Vela_1.0.0_amd64.deb', ...members);
    return path.join(root, 'Vela_1.0.0_amd64.deb');
  },
  rpm: (root, tree) => {
    bsdtar(root, '--format=newc', '-czf', 'payload.cpio.gz', '-C', tree, 'usr');
    const lead = Buffer.alloc(96);
    lead.set([0xed, 0xab, 0xee, 0xdb, 3, 0, 0, 0, 0, 1]); // magic, format 3.0, a binary package
    lead.write('vela-1.0.0-1', 10, 'latin1');
    lead.writeUInt16BE(1, 76); // built for Linux
    lead.writeUInt16BE(5, 78); // a signature header follows
    const emptyHeader = Buffer.from([0x8e, 0xad, 0xe8, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    const file = path.join(root, 'Vela-1.0.0-1.x86_64.rpm');
    fs.writeFileSync(
      file,
      Buffer.concat([lead, emptyHeader, emptyHeader, fs.readFileSync(path.join(root, 'payload.cpio.gz'))]),
    );
    return file;
  },
};

// A package of the given kind holding the given files ({ path: contents })
// and links ({ path: target }), all below usr/. It is a package of Vela: it
// has the program, unless that path is given as null. Every directory is
// packed with mode 755 and every file with 644, whatever this process's umask
// is, except the paths named in `modes` ({ path: mode }).
function packageOf(t, kind, files, links = {}, modes = {}, options = {}) {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-package-')));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const tree = path.join(root, 'tree');
  fs.mkdirSync(path.join(tree, 'usr'), { recursive: true });
  for (const [name, contents] of Object.entries({ 'usr/bin/vela': 'the app\n', ...files })) {
    if (contents === null) continue;
    fs.mkdirSync(path.dirname(path.join(tree, name)), { recursive: true });
    fs.writeFileSync(path.join(tree, name), contents);
  }
  for (const entry of fs.readdirSync(tree, { recursive: true, withFileTypes: true })) {
    fs.chmodSync(path.join(entry.parentPath, entry.name), entry.isDirectory() ? 0o755 : 0o644);
  }
  for (const [name, mode] of Object.entries(modes)) fs.chmodSync(path.join(tree, name), mode);
  for (const [name, target] of Object.entries(links)) {
    fs.mkdirSync(path.dirname(path.join(tree, name)), { recursive: true });
    fs.symlinkSync(target, path.join(tree, name));
  }
  return { root, file: packageKinds[kind](root, tree, options) };
}
const pacmanPackage = (t, files, links, modes) => packageOf(t, 'pkg.tar.zst', files, links, modes);

// One run of the check. Every run in this suite gets a temporary directory of
// its own (or the one named in `env`) and must leave it empty, however the
// run ended: an unpacked package with the set is some 60 MB.
function checkPackageRun(args, script = checkPackagePath, env = {}) {
  const tmp = env.TMPDIR ?? fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-check-tmp-')));
  try {
    const run = spawnSync('/bin/bash', [script, ...args], {
      encoding: 'utf8',
      env: { ...process.env, ...env, TMPDIR: tmp },
    });
    assert.deepEqual(fs.readdirSync(tmp), [], `the check left this in its temporary directory\n${run.stdout}\n${run.stderr}`);
    return run;
  } finally {
    if (env.TMPDIR === undefined) fs.rmSync(tmp, { recursive: true, force: true });
  }
}

// A set that is right in everything this suite can make right: the
// components versions.txt pins, a licence text for each of them, every file
// as its MANIFEST records it, and its place. Its library is no library, so
// the real check goes as far as loading it and fails THERE, and each
// comparison it makes before that can be seen to pass or to fail.
const pinned = versions
  .split('\n')
  .map((line) => line.trim().split(/\s+/))
  .filter((fields) => !fields[0].startsWith('#') && fields[2] === 'ship')
  .map(([name, version]) => [name, version]);
const library = 'not a library\n';
const privateLibrary = (name) => `${name} is no library either\n`;
const licenceOf = (component) => `the licence of ${component}\n`;
const licencePath = (component) => `licenses/${component}/LICENSE.LGPL`;
const sha256 = (contents) => crypto.createHash('sha256').update(contents).digest('hex');
const fileLine = (name, contents) => `file ${sha256(contents)} ${Buffer.byteLength(contents)} ${name}`;
// The MANIFEST of such a set: its components, a licence text named for each
// (unless `licences` is false), and every file with its true checksum and
// size. `libraries` names private libraries beside libmpv.so.2.
const manifestOf = (components, { licences = true, libraries = [] } = {}) =>
  [
    ...components.map(([name, version]) => `component ${name} ${version}`),
    ...(licences ? components.map(([name]) => `licence ${name} ${licencePath(name)}`) : []),
    fileLine('libmpv.so.2', library),
    ...libraries.map((name) => fileLine(name, privateLibrary(name))),
    ...components.map(([name]) => fileLine(licencePath(name), licenceOf(name))),
    '',
  ].join('\n');
// The files of the set a MANIFEST made by manifestOf describes, by their
// path in the set, the MANIFEST among them. `differing` is what is not as
// the MANIFEST says: { path: other contents, or null to leave the file out }.
const setFilesOf = (manifest, differing = {}) => {
  const inSet = { MANIFEST: manifest };
  for (const line of manifest.split('\n')) {
    const name = line.match(/^file \S+ \S+ (.+)$/)?.[1];
    if (!name) continue;
    const component = name.match(/^licenses\/([^/]+)\//)?.[1];
    inSet[name] = component ? licenceOf(component) : name === 'libmpv.so.2' ? library : privateLibrary(name);
  }
  return { ...inSet, ...differing };
};
// A package with the set that `manifest` describes, in its place.
//   files         what differs from the MANIFEST inside the set (see setFilesOf)
//   archLicences  what differs in the directory Arch keeps licences in:
//                 { component: other text, or null to leave it out };
//                 null for no such directory at all
const packageWith = (t, manifest, { files = {}, archLicences = {}, modes = {}, kind = 'pkg.tar.zst' } = {}) => {
  const inSet = setFilesOf(manifest, files);
  const inArchDirectory = {};
  for (const line of manifest.split('\n')) {
    const component = line.match(/^file \S+ \S+ licenses\/([^/]+)\/LICENSE\.LGPL$/)?.[1];
    if (component && archLicences !== null) inArchDirectory[`${component}/LICENSE.LGPL`] = licenceOf(component);
  }
  Object.assign(inArchDirectory, Object.fromEntries(
    Object.entries(archLicences ?? {}).map(([component, text]) => [`${component}/LICENSE.LGPL`, text]),
  ));
  return packageOf(
    t,
    kind,
    {
      ...Object.fromEntries(Object.entries(inSet).map(([name, contents]) => [`usr/lib/Vela/libmpv/${name}`, contents])),
      ...Object.fromEntries(
        Object.entries(inArchDirectory).map(([name, contents]) => [`usr/share/licenses/vela/libmpv/${name}`, contents]),
      ),
    },
    {},
    modes,
  );
};
const referenceWith = (t, manifest) => {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-given-')));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  fs.writeFileSync(path.join(directory, 'MANIFEST'), manifest);
  return directory;
};

// An AppImage is an ELF program with a squashfs appended to it. The check
// finds the squashfs where the ELF section header table ends; this is the
// smallest ELF header that says where that is: at byte 128, behind a table of
// one 64-byte entry at byte 64. `image` follows it.
const appImageOf = (image) => {
  const head = Buffer.alloc(128);
  head.set([0x7f, 0x45, 0x4c, 0x46, 2, 1, 1]); // ELF, 64-bit, little-endian, version 1
  head.writeUInt16LE(2, 16); // an executable
  head.writeUInt16LE(62, 18); // for x86-64
  head.writeBigUInt64LE(64n, 40); // the section header table starts at byte 64,
  head.writeUInt16LE(64, 52); // after a header of 64 bytes,
  head.writeUInt16LE(64, 58); // and has entries of 64 bytes,
  head.writeUInt16LE(1, 60); // one of them
  return Buffer.concat([head, image]);
};
// A squashfs, as far as its first four bytes go.
const squashfsLike = Buffer.from('hsqs and nothing a real squashfs has\n');

// A stand-in for unsquashfs, in front of PATH: it records its arguments in
// `calls` (one call to a line, as JSON), checks that the image it was told to
// read begins where it was told it does, and writes `usr/bin/vela` and the
// shell lines `extra` into the directory it was given (in $dest). Returns the
// PATH to use.
function unsquashfsStandIn(t, { calls, extra = '' }) {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-unsquashfs-')));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  executable(
    path.join(directory, 'unsquashfs'),
    `#!/bin/bash
set -eu
"${process.execPath}" -e 'console.log(JSON.stringify(process.argv.slice(1)))' -- "$@" >> "${calls}"
dest='' offset=''
while [ $# -gt 1 ]; do
  case "$1" in -d) dest="$2"; shift ;; -o) offset="$2"; shift ;; esac
  shift
done
[ "$(tail -c +$((offset + 1)) "$1" | head -c 4)" = hsqs ] || exit 1
mkdir -p "$dest/usr/bin" "$dest/usr/lib/Vela/mpv-scripts"
echo app > "$dest/usr/bin/vela"
echo script > "$dest/usr/lib/Vela/mpv-scripts/autocrop.lua"
${extra}
`,
  );
  return `${directory}:${process.env.PATH}`;
}
const recordedCalls = (calls) =>
  fs.existsSync(calls) ? fs.readFileSync(calls, 'utf8').split('\n').filter(Boolean).map((line) => JSON.parse(line)) : [];

test('the package check, asked for a package without the set, finds any part of one', { skip: checkUnavailable }, async (t) => {
  // The set a default build was built without: one library, known by its
  // checksum, as a MANIFEST lists it.
  const library = 'not really a library\n';
  const sha = spawnSync('sha256sum', { input: library, encoding: 'utf8' }).stdout.split(' ')[0];
  const reference = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-reference-')));
  t.after(() => fs.rmSync(reference, { recursive: true, force: true }));
  fs.writeFileSync(path.join(reference, 'MANIFEST'), `file ${sha} ${library.length} libvela-avcodec.so.63\n`);

  await t.test('a package with none passes', (t) => {
    const clean = pacmanPackage(t, { 'usr/lib/Vela/mpv-scripts/autocrop.lua': 'script\n' });
    const run = checkPackageRun(['--without-libmpv', '--set', reference, clean.file]);
    assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^ok   absent: nothing named like the set, and none of the package's 2 files /m);
    assert.match(run.stdout, /^RESULT PASS /m);
  });

  const sneaked = {
    'usr/lib/Vela/libmpv/MANIFEST': 'usr/lib/Vela/libmpv',
    'usr/lib/libmpv.so.2': 'usr/lib/libmpv.so.2',
    'usr/share/extra/libvela-dav1d.so.7': 'usr/share/extra/libvela-dav1d.so.7',
  };
  for (const [file, reported] of Object.entries(sneaked)) {
    await t.test(`${file} is found by its name`, (t) => {
      const dirty = pacmanPackage(t, { [file]: 'anything\n' });
      const run = checkPackageRun(['--without-libmpv', dirty.file]);
      assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
      assert.ok(run.stdout.includes(`\nFAIL absent: the package has ${reported}\n`), run.stdout);
      assert.match(run.stdout, /^RESULT FAIL /m);
    });
  }

  await t.test('a file of the set under another name is found by its contents', (t) => {
    const renamed = pacmanPackage(t, { 'usr/share/vela/data.bin': library });
    const byName = checkPackageRun(['--without-libmpv', renamed.file]);
    assert.equal(byName.status, 0, 'by name alone there is nothing to find');
    assert.match(byName.stdout, /no set was given, so contents were not compared/);
    const byContents = checkPackageRun(['--without-libmpv', '--set', reference, renamed.file]);
    assert.equal(byContents.status, 1, `${byContents.stdout}\n${byContents.stderr}`);
    assert.match(
      byContents.stdout,
      /^FAIL absent: usr\/share\/vela\/data\.bin has the contents of the set's libvela-avcodec\.so\.63$/m,
    );
  });

  await t.test('a set whose MANIFEST lists no file is nothing to compare with', (t) => {
    // Compared with nothing, every package would be free of the set.
    const empty = referenceWith(t, 'component mpv 0.42.0\n');
    const run = checkPackageRun(['--without-libmpv', '--set', empty, pacmanPackage(t, {}).file]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.ok(
      run.stdout.includes(`\nFAIL absent: the MANIFEST in ${empty} lists no file to compare with\n`),
      run.stdout,
    );
    assert.doesNotMatch(run.stdout, /^ok   absent:/m);
  });

  await t.test('a directory given as the set that has no MANIFEST stops the check', (t) => {
    const notASet = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-given-')));
    t.after(() => fs.rmSync(notASet, { recursive: true, force: true }));
    for (const mode of [['--without-libmpv'], []]) {
      const run = checkPackageRun([...mode, '--set', notASet, pacmanPackage(t, {}).file]);
      assert.equal(run.status, 2, `${run.stdout}\n${run.stderr}`);
      assert.equal(run.stderr, `check-package: no libmpv set in ${notASet} (it has no MANIFEST)\n`);
      assert.equal(run.stdout, '', 'no package is judged against a set that is none');
    }
  });
});

test('the package check fails the run when any one of several packages fails', { skip: checkUnavailable }, async (t) => {
  // scripts/build.sh hands every package it built to ONE run of the check,
  // the deb before the rpm, and copies them all to dist/ when that run exits
  // 0. So the exit status has to be the worst any package got, wherever in
  // the list that package stands, and each package has to get its own line.
  const passing = (t) => pacmanPackage(t, {}).file;
  const failing = (t) => pacmanPackage(t, { 'usr/lib/libmpv.so.2': 'x\n' }).file;
  const results = (run) => run.stdout.split('\n').filter((line) => line.startsWith('RESULT '));
  const orders = {
    'a failing package before a passing one': [failing, passing],
    'a passing package before a failing one': [passing, failing],
    'a failing package between two passing ones': [passing, failing, passing],
  };
  for (const [what, order] of Object.entries(orders)) {
    await t.test(what, (t) => {
      const packages = order.map((make) => [make === passing ? 'PASS' : 'FAIL', make(t)]);
      const run = checkPackageRun(['--without-libmpv', ...packages.map(([, file]) => file)]);
      assert.deepEqual(results(run), packages.map(([verdict, file]) => `RESULT ${verdict} ${file}`), run.stdout);
      assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    });
  }
  await t.test('every package passing', (t) => {
    const packages = [passing(t), passing(t)];
    const run = checkPackageRun(['--without-libmpv', ...packages]);
    assert.deepEqual(results(run), packages.map((file) => `RESULT PASS ${file}`), run.stdout);
    assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
  });
});

test('the package check refuses a set that is not in the package where the loader reads it', { skip: checkUnavailable }, async (t) => {
  await t.test('no set at all', (t) => {
    const without = pacmanPackage(t, {});
    const run = checkPackageRun([without.file]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^FAIL place: the package has no usr\/lib\/Vela\/libmpv, /m);
    assert.match(run.stdout, /^RESULT FAIL /m);
  });

  await t.test('a set in another directory', (t) => {
    const misplaced = pacmanPackage(t, { 'usr/lib/vela/libmpv/libmpv.so.2': 'x\n' });
    const run = checkPackageRun([misplaced.file]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^FAIL place: the package has no usr\/lib\/Vela\/libmpv, /m);
    assert.match(run.stdout, /^FAIL place: it does have usr\/lib\/vela\/libmpv\/libmpv\.so\.2$/m);
  });

  await t.test('a link to a set somewhere else', (t) => {
    // The directory answers at the right path, but its files are not there:
    // on a user's machine the link would lead wherever it leads.
    const linked = pacmanPackage(
      t,
      { 'usr/share/elsewhere/libmpv.so.2': 'x\n', 'usr/share/elsewhere/MANIFEST': '' },
      { 'usr/lib/Vela/libmpv': '../../share/elsewhere' },
    );
    const run = checkPackageRun([linked.file]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^FAIL place: usr\/lib\/Vela\/libmpv is reached through a symbolic link /m);
    assert.doesNotMatch(run.stdout, /load:/, 'nothing is loaded from a set that is not in its place');
  });

  await t.test('a file that is no package, and a file that is not there', (t) => {
    const other = pacmanPackage(t, {});
    const renamed = path.join(other.root, 'vela.zip');
    fs.copyFileSync(other.file, renamed);
    const run = checkPackageRun([renamed, path.join(other.root, 'missing.deb')]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^FAIL unpack: not a \.deb, \.rpm, \.AppImage or pacman package$/m);
    assert.match(run.stdout, /^FAIL unpack: no such file$/m);
  });
});

test('the package check unpacks each kind of package, and fails one it cannot unpack', { skip: checkUnavailable }, async (t) => {
  // A deb and an rpm are what the documented --with-libmpv build produces.
  // An AppImage is read by unsquashfs and has tests of its own below.
  const script = { 'usr/lib/Vela/mpv-scripts/autocrop.lua': 'script\n' };
  for (const kind of Object.keys(packageKinds)) {
    await t.test(`a ${kind} without the set: its files are looked at, all of them`, (t) => {
      const given = referenceWith(t, 'file 0 0 libmpv.so.2\n');
      const run = checkPackageRun(['--without-libmpv', '--set', given, packageOf(t, kind, script).file]);
      assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
      assert.match(run.stdout, /^ok   absent: nothing named like the set, and none of the package's 2 files /m);
    });

    await t.test(`a ${kind} with a library of the set: found`, (t) => {
      const dirty = packageOf(t, kind, { ...script, 'usr/lib/Vela/libmpv/libvela-ass.so.9': 'x\n' });
      const run = checkPackageRun(['--without-libmpv', dirty.file]);
      assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
      assert.match(run.stdout, /^FAIL absent: the package has usr\/lib\/Vela\/libmpv\/libvela-ass\.so\.9$/m);
    });

    await t.test(`a ${kind} with the set: found in its place, and loaded from there`, (t) => {
      // No licence text in the directory Arch keeps licences in.
      const run = checkPackageRun([packageWith(t, manifestOf(pinned), { archLicences: null, kind }).file]);
      assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
      assert.match(run.stdout, /^ok   place: the set is at usr\/lib\/Vela\/libmpv$/m);
      assert.match(run.stdout, /^ok   pins: /m);
      assert.match(run.stdout, /^ok   manifest: every listed file present and unmodified, /m);
      assert.match(run.stdout, /^FAIL load: \/\S+\/root\/usr\/lib\/Vela\/libmpv\/libmpv\.so\.2: /m);
      // Only a pacman package is asked for them there, under either ending.
      assert.equal(/^FAIL licences: /m.test(run.stdout), kind.startsWith('pkg.tar.'), run.stdout);
    });

    await t.test(`a ${kind} cut short fails, whichever question was asked`, (t) => {
      // Big enough, and random, so that half of it ends in the middle of a file.
      const whole = packageOf(t, kind, { ...script, 'usr/lib/Vela/noise.bin': crypto.randomBytes(1 << 16) });
      const bytes = fs.readFileSync(whole.file);
      fs.writeFileSync(whole.file, bytes.subarray(0, bytes.length >> 1));
      for (const mode of [['--without-libmpv'], []]) {
        const run = checkPackageRun([...mode, whole.file]);
        assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
        assert.match(run.stdout, /^FAIL unpack: the package could not be unpacked$/m);
        assert.doesNotMatch(run.stdout, /^ok /m, 'nothing is said to be in order about a package that was not read');
        assert.match(run.stdout, /^RESULT FAIL /m);
      }
    });
  }

  await t.test('a deb without the member that holds its files', (t) => {
    const hollow = packageOf(t, 'deb', script, {}, {}, { dataMember: false });
    const run = checkPackageRun(['--without-libmpv', hollow.file]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^ {5}no data\.tar member$/m);
    assert.match(run.stdout, /^FAIL unpack: the package could not be unpacked$/m);
  });

  await t.test('a whole deb whose member that holds its files is cut short', (t) => {
    // The deb itself unpacks; its files do not, or not all of them. What did
    // come out before the damage must not be judged as if it were the package.
    const files = { ...script, 'usr/lib/Vela/noise.bin': crypto.randomBytes(1 << 16) };
    const damaged = packageOf(t, 'deb', files, {}, {}, { dataMember: 'cut short' });
    for (const mode of [['--without-libmpv'], []]) {
      const run = checkPackageRun([...mode, damaged.file]);
      assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
      assert.match(run.stdout, /^FAIL unpack: the package could not be unpacked$/m);
      assert.doesNotMatch(run.stdout, /^ok /m);
    }
  });
});

test('the package check judges nothing that is not a package of Vela', { skip: checkUnavailable }, async (t) => {
  // "It holds no part of the set" is true of an empty file, of a package of
  // something else, and of a file that was never unpacked. None of them may
  // pass for a default build of Vela, and what is asked of a package with
  // the set is not asked of them either.
  const notVela = /^FAIL unpack: this is not a package of Vela: it has no usr\/bin\/vela$/m;
  const failsBothWays = (file, pattern) => {
    for (const mode of [['--without-libmpv'], []]) {
      const run = checkPackageRun([...mode, file]);
      assert.equal(run.status, 1, `${mode} ${run.stdout}\n${run.stderr}`);
      assert.match(run.stdout, pattern);
      assert.doesNotMatch(run.stdout, /^ok /m, 'nothing is said to be in order about it');
      assert.match(run.stdout, /^RESULT FAIL /m);
    }
  };
  const scratch = (t) => {
    const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-other-')));
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
    return directory;
  };

  for (const name of ['Vela-1.0.0-1.x86_64.rpm', 'vela-1.0.0-1-x86_64.pkg.tar.zst', 'vela-1.0.0-1-x86_64.pkg.tar.xz']) {
    await t.test(`an empty file called ${name}`, (t) => {
      // bsdtar unpacks an empty file without complaint, to nothing.
      const file = path.join(scratch(t), name);
      fs.writeFileSync(file, '');
      failsBothWays(file, notVela);
    });
  }

  await t.test('an empty file called Vela_1.0.0_amd64.deb', (t) => {
    const file = path.join(scratch(t), 'Vela_1.0.0_amd64.deb');
    fs.writeFileSync(file, '');
    failsBothWays(file, /^FAIL unpack: the package could not be unpacked$/m);
  });

  await t.test('a deb that holds a set, under the name of an rpm', (t) => {
    // Unpacked as an rpm it comes out as its three members, and the set
    // inside one of them is not seen. That must not read as "no set".
    const deb = packageWith(t, manifestOf(pinned), { kind: 'deb' });
    const renamed = path.join(deb.root, 'Vela-1.0.0-1.x86_64.rpm');
    fs.copyFileSync(deb.file, renamed);
    failsBothWays(renamed, notVela);
  });

  await t.test('a package of something else', (t) => {
    const other = packageOf(t, 'pkg.tar.zst', { 'usr/bin/vela': null, 'usr/bin/other': 'another program\n' });
    failsBothWays(other.file, notVela);
  });

  await t.test('a package in which the program is a link to somewhere else', (t) => {
    // A link that leads out of the package finds the builder's own files.
    const linked = packageOf(t, 'pkg.tar.zst', { 'usr/bin/vela': null }, { 'usr/bin/vela': '/bin/sh' });
    failsBothWays(linked.file, notVela);
  });

  await t.test('a file that is no package, and a file that is not there, asked for no set', (t) => {
    const other = pacmanPackage(t, {});
    const renamed = path.join(other.root, 'vela.zip');
    fs.copyFileSync(other.file, renamed);
    const run = checkPackageRun(['--without-libmpv', renamed, path.join(other.root, 'missing.deb')]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^FAIL unpack: not a \.deb, \.rpm, \.AppImage or pacman package$/m);
    assert.match(run.stdout, /^FAIL unpack: no such file$/m);
    assert.deepEqual(run.stdout.match(/^RESULT \w+/gm), ['RESULT FAIL', 'RESULT FAIL']);
  });
});

test('the package check compares the set in the package with the pins, the set it was given and the licence directory', { skip: checkUnavailable }, async (t) => {
  assert.ok(pinned.length > 1 && pinned.some(([name]) => name === 'ffmpeg'), 'versions.txt must pin the set');
  const right = manifestOf(pinned);

  await t.test('all three agree, and the library is then loaded from the unpacked package', (t) => {
    const given = referenceWith(t, right);
    const run = checkPackageRun(['--set', given, packageWith(t, right).file]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^ok   place: the set is at usr\/lib\/Vela\/libmpv$/m);
    assert.match(run.stdout, /^ok   access: every user may read every file of the set$/m);
    assert.ok(run.stdout.includes(`\nok   source: its MANIFEST is the one in ${given}\n`), run.stdout);
    assert.match(run.stdout, /^ok   pins: its components are the versions versions\.txt pins$/m);
    // check-libmpv.py itself, on the unpacked set: every file is as listed.
    assert.match(
      run.stdout,
      /^ok   manifest: every listed file present and unmodified, nothing unlisted, a licence text for every component$/m,
    );
    assert.doesNotMatch(run.stdout, /^FAIL (place|access|source|pins|manifest|licences)/m);
    // What stops this package is the load, and the file it tried is the
    // package's own, in the temporary directory it was unpacked into.
    assert.match(run.stdout, /^FAIL load: \/\S+\/root\/usr\/lib\/Vela\/libmpv\/libmpv\.so\.2: /m);
    assert.equal(run.stdout.match(/^FAIL /gm).length, 1, 'nothing else is wrong with it');
    assert.match(run.stdout, /^RESULT FAIL /m);
  });

  // The files of the package against the MANIFEST inside it: each of these
  // sets is right in everything but one file.
  const damaged = {
    'one byte of the library changed': [{ 'libmpv.so.2': library.replace('not', 'noT') }, 'modified: libmpv.so.2'],
    'one byte of a licence text changed': [
      { [licencePath('ffmpeg')]: licenceOf('ffmpeg').replace('the', 'thE') },
      `modified: ${licencePath('ffmpeg')}`,
    ],
    'a file the MANIFEST lists left out': [{ [licencePath('dav1d')]: null }, `missing: ${licencePath('dav1d')}`],
    'a file the MANIFEST does not list': [{ 'libvela-extra.so.1': 'x\n' }, 'not in MANIFEST: libvela-extra.so.1'],
  };
  for (const [what, [files, problem]] of Object.entries(damaged)) {
    await t.test(what, (t) => {
      const run = checkPackageRun([packageWith(t, right, { files }).file]);
      assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
      assert.deepEqual(run.stdout.match(/^FAIL manifest: .*$/gm), [`FAIL manifest: ${problem}`], run.stdout);
      assert.doesNotMatch(run.stdout, /^ok   manifest:/m);
      // And its library was not loaded: there is no line about a load but this.
      assert.deepEqual(run.stdout.match(/^\S+ +load: .*$/gm), [
        'skip load: libmpv.so.2 was not loaded, because the set failed the check above; nothing of a set that failed is run',
      ]);
    });
  }

  await t.test('libmpv.so.2 a link to a library outside the package: not loaded', (t) => {
    // The same bytes, elsewhere: what the package installs is a link that
    // leads out of it.
    const elsewhere = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-elsewhere-')));
    t.after(() => fs.rmSync(elsewhere, { recursive: true, force: true }));
    fs.writeFileSync(path.join(elsewhere, 'libmpv.so.2'), library);
    const linked = packageOf(
      t,
      'pkg.tar.zst',
      Object.fromEntries(
        Object.entries(setFilesOf(right, { 'libmpv.so.2': null })).map(([name, contents]) => [`usr/lib/Vela/libmpv/${name}`, contents]),
      ),
      { 'usr/lib/Vela/libmpv/libmpv.so.2': path.join(elsewhere, 'libmpv.so.2') },
    );
    const run = checkPackageRun([linked.file]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^FAIL manifest: reached through a symbolic link: libmpv\.so\.2$/m);
    assert.deepEqual(run.stdout.match(/^\S+ +load: .*$/gm), [
      'skip load: libmpv.so.2 was not loaded, because the set failed the check above; nothing of a set that failed is run',
    ]);
  });

  await t.test('a component at another version', (t) => {
    const stale = manifestOf(pinned.map(([name, version]) => [name, name === 'ffmpeg' ? '1.0.0' : version]));
    const run = checkPackageRun([packageWith(t, stale).file]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    const ffmpeg = pinned.find(([name]) => name === 'ffmpeg')[1];
    assert.ok(
      run.stdout.includes(`\nFAIL pins: versions.txt pins ffmpeg ${ffmpeg}, and the set in the package is not built from it\n`),
      run.stdout,
    );
    assert.ok(
      run.stdout.includes('\nFAIL pins: the set in the package is built from ffmpeg 1.0.0, which versions.txt does not pin\n'),
      run.stdout,
    );
    assert.equal(run.stdout.match(/^FAIL pins:/gm).length, 2, 'the other components are as pinned');
  });

  await t.test('a pinned component that is not in the set', (t) => {
    const short = manifestOf(pinned.filter(([name]) => name !== 'dav1d'));
    const run = checkPackageRun([packageWith(t, short).file]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^FAIL pins: versions\.txt pins dav1d \S+, and the set in the package is not built from it$/m);
  });

  await t.test('another set than the one the build was given', (t) => {
    const given = referenceWith(t, `${right}glibc 2.35\n`);
    const run = checkPackageRun(['--set', given, packageWith(t, right).file]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.ok(
      run.stdout.includes(`\nFAIL source: its MANIFEST is not the one in ${given}; the package holds another set\n`),
      run.stdout,
    );
    assert.match(run.stdout, /^ok   pins: /m);
  });

  await t.test('no set given: the comparison that was not made is named, not passed over', (t) => {
    // A set with a library changed and its MANIFEST written to match is
    // whole, pinned and loads. Only the built set can tell it is another.
    const run = checkPackageRun([packageWith(t, right).file]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(
      run.stdout,
      /^skip source: no set was given \(--set DIR\), so its MANIFEST was not compared with the built set's$/m,
    );
    assert.doesNotMatch(run.stdout, /^(ok  |FAIL) source:/m);
    // The line stands where the comparison would: after the place, before the pins.
    assert.match(run.stdout, /^ok   access: .*\nskip source: .*\nok   pins: /m);
  });

  await t.test('a --set that names nothing stops the check; it is not taken for "no set"', (t) => {
    // What --set="$DIR" becomes when DIR is empty.
    const good = packageWith(t, right).file;
    const ways = {
      '--set= needs a directory': ['--set=', good],
      '--set needs a directory': ['--set', '', good],
    };
    for (const mode of [[], ['--without-libmpv']]) {
      for (const [message, args] of Object.entries(ways)) {
        const run = checkPackageRun([...mode, ...args]);
        assert.equal(run.status, 2, `${args} ${run.stdout}\n${run.stderr}`);
        assert.equal(run.stderr, `check-package: ${message}\n`);
        assert.equal(run.stdout, '', 'no package is judged');
      }
      // The same with --set as the last word on the line.
      const last = checkPackageRun([...mode, good, '--set']);
      assert.equal(last.status, 2, `${last.stdout}\n${last.stderr}`);
      assert.equal(last.stderr, 'check-package: --set needs a directory\n');
    }
  });

  // The package's files will belong to root, and Vela runs as whoever starts
  // it. Tauri's rpm bundler carries a resource's permissions over from the
  // build tree, so a set copied under a strict umask arrives closed.
  const closed = {
    'a library only its owner may read': ['usr/lib/Vela/libmpv/libmpv.so.2', 0o600, '---'],
    'a licence text only its owner and group may read': ['usr/lib/Vela/libmpv/licenses/mpv/LICENSE.LGPL', 0o640, '---'],
    'a directory of the set that others may not enter': ['usr/lib/Vela/libmpv/licenses', 0o750, '---'],
    'a directory of the set that others may enter but not list': ['usr/lib/Vela/libmpv/licenses/mpv', 0o711, '--x'],
    'the set directory itself closed': ['usr/lib/Vela/libmpv', 0o700, '---'],
    "Vela's resource directory closed": ['usr/lib/Vela', 0o700, '---'],
    // Unpacking gives the builder the owner's permissions in full. What is
    // reported is what the package gives everyone else, which that leaves alone.
    'a library its owner may read and not write': ['usr/lib/Vela/libmpv/libmpv.so.2', 0o400, '---'],
  };
  for (const [what, [file, mode, others]] of Object.entries(closed)) {
    await t.test(what, (t) => {
      const run = checkPackageRun([packageWith(t, right, { modes: { [file]: mode } }).file]);
      assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
      assert.ok(
        run.stdout.includes(`\nFAIL access: ${file} cannot be read by every user (other users have '${others}')\n`),
        run.stdout,
      );
      assert.equal(run.stdout.match(/^FAIL access:/gm).length, 1, 'only what is closed is named');
      assert.doesNotMatch(run.stdout, /^ok   access:/m);
    });
  }

  // The directories above the set have to be entered to reach it. A package
  // installs them too, with the modes it gives them.
  const above = {
    'usr/lib closed': [{ 'usr/lib': 0o700 }, 'usr/lib', '---'],
    'usr closed': [{ usr: 0o700 }, 'usr', '---'],
    'usr/lib readable but not to be entered': [{ 'usr/lib': 0o744 }, 'usr/lib', 'r--'],
    'usr and usr/lib both closed: the first is named': [{ usr: 0o700, 'usr/lib': 0o700 }, 'usr', '---'],
  };
  for (const [what, [modes, first, others]] of Object.entries(above)) {
    await t.test(what, (t) => {
      const run = checkPackageRun([packageWith(t, right, { modes }).file]);
      assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
      assert.deepEqual(run.stdout.match(/^FAIL access: .*$/gm), [
        `FAIL access: ${first} cannot be entered by every user (other users have '${others}'), and the set is below it`,
      ]);
      assert.doesNotMatch(run.stdout, /^ok   access:/m);
    });
  }

  await t.test('a directory above the set that every user may enter but not list is enough', (t) => {
    const run = checkPackageRun([packageWith(t, right, { modes: { usr: 0o711, 'usr/lib': 0o711 } }).file]);
    assert.match(run.stdout, /^ok   access: every user may read every file of the set$/m);
  });

  await t.test("the builder's own umask is not taken for the package's permissions", (t) => {
    // Unpacking applies the umask. Under a strict one every file would come
    // out closed, and a good package would be blamed for the builder's shell.
    const good = packageWith(t, right).file;
    const run = spawnSync('/bin/bash', ['-c', 'umask 077 && exec /bin/bash "$0" "$1"', checkPackagePath, good], {
      encoding: 'utf8',
    });
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^ok   access: every user may read every file of the set$/m);
    assert.match(run.stdout, /^FAIL load: /m);
  });

  for (const [what, inArchDirectory] of [['missing from', null], ['changed in', 'another text\n']]) {
    await t.test(`a licence text ${what} the directory Arch keeps licences in`, (t) => {
      const run = checkPackageRun([packageWith(t, right, { archLicences: { mpv: inArchDirectory } }).file]);
      assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
      assert.deepEqual(run.stdout.match(/^FAIL licences: .*$/gm), [
        "FAIL licences: usr/share/licenses/vela/libmpv/mpv/LICENSE.LGPL is missing or is not the set's licenses/mpv/LICENSE.LGPL",
      ]);
    });
  }

  await t.test('a set whose MANIFEST names no licence text at all', (t) => {
    // With nothing named there is nothing to find missing, and that must not
    // read as "every licence text is in its place".
    const run = checkPackageRun([packageWith(t, manifestOf(pinned, { licences: false })).file]);
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^FAIL licences: the set's MANIFEST names no licence text$/m);
  });
});

// A copy of the package check beside a check-libmpv.py that does what a test
// tells it to: it records how it was called, runs the Python lines in `first`,
// prints `lines` and exits with `status`. With it the check can be seen to
// pass a package, which the real check-libmpv.py never does for a library
// that is none, and to make its own judgement of each answer it can be given.
function checkBesideStandIn(t, { lines = [], status = 0, first = [] }) {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-standin-')));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  fs.copyFileSync(checkPackagePath, path.join(directory, 'check-package.sh'));
  fs.writeFileSync(path.join(directory, 'versions.txt'), versions);
  const record = path.join(directory, 'called.json');
  fs.writeFileSync(
    path.join(directory, 'check-libmpv.py'),
    [
      'import json, os, sys, time',
      `with open(${JSON.stringify(record)}, "w") as f:`,
      '    json.dump({',
      '        "arguments": sys.argv[1:],',
      '        "library": open(os.path.join(sys.argv[1], "libmpv.so.2")).read(),',
      // What lies in the check's temporary directory beside this package:
      // the set is at <that directory>/<n>/root/usr/lib/Vela/libmpv.
      '        "unpacked": sorted(os.listdir(os.path.join(sys.argv[1], *[".."] * 6))),',
      '        "LD_LIBRARY_PATH": os.environ.get("LD_LIBRARY_PATH"),',
      '        "LD_PRELOAD": os.environ.get("LD_PRELOAD"),',
      '        "isolated": bool(sys.flags.isolated),',
      '    }, f)',
      ...first,
      `print(${JSON.stringify(lines.join('\n'))})`,
      `sys.exit(${status})`,
      '',
    ].join('\n'),
  );
  return {
    script: path.join(directory, 'check-package.sh'),
    called: () => JSON.parse(fs.readFileSync(record, 'utf8')),
    wasCalled: () => fs.existsSync(record),
  };
}

test('the package check calls check-libmpv.py on the unpacked set, and passes a package only when it finds nothing wrong', { skip: checkUnavailable }, async (t) => {
  const right = manifestOf(pinned);
  const mpvPin = versions.match(/^mpv\s+(\S+)\s+ship\s/m)?.[1];
  const itsOk = [
    'ok   manifest: every listed file present and unmodified, nothing unlisted, a licence text for every component',
    'ok   load: /somewhere/libmpv.so.2 (client API 2.6)',
  ];

  for (const kind of ['pkg.tar.zst', 'deb']) {
    await t.test(`a ${kind} whose set it passes: the package passes, and this is what it was asked`, (t) => {
      const standIn = checkBesideStandIn(t, { lines: [...itsOk, 'RESULT PASS'], status: 0 });
      const given = referenceWith(t, right);
      const tmp = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-tmp-')));
      t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
      const good = packageWith(t, right, { kind }).file;
      // A library path and a preload, as a builder's shell may have them.
      const run = checkPackageRun(['--set', given, good], standIn.script, {
        TMPDIR: tmp,
        LD_LIBRARY_PATH: '/somewhere/else',
        LD_PRELOAD: '',
      });
      assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
      const expected = [
        `== ${good}`,
        'ok   place: the set is at usr/lib/Vela/libmpv',
        'ok   access: every user may read every file of the set',
        `ok   source: its MANIFEST is the one in ${given}`,
        'ok   pins: its components are the versions versions.txt pins',
        ...itsOk,
        // Only a pacman package keeps the licence texts in a second place.
        ...(kind === 'deb'
          ? []
          : [`ok   licences: all ${pinned.length} licence texts are also under usr/share/licenses/vela/libmpv`]),
        // Its own closing line is not printed: this one is the package's.
        `RESULT PASS ${good}`,
        '',
      ];
      assert.deepEqual(run.stdout.split('\n'), expected);

      const called = standIn.called();
      const [setDirectory, ...expectations] = called.arguments;
      // The set of the unpacked package, in the temporary directory, by its
      // full path; the package's own library was in it.
      assert.match(setDirectory, /\/1\/root\/usr\/lib\/Vela\/libmpv$/);
      assert.ok(setDirectory.startsWith(`${tmp}/`), setDirectory);
      assert.equal(called.library, library);
      // What it has to find: the pinned mpv and that release's client API.
      assert.deepEqual(expectations, ['--expect-mpv', mpvPin, '--expect-client-api', shellValue(checkPackage, 'CLIENT_API')]);
      // Nothing of the builder's shell that could hand it another library.
      assert.equal(called.LD_LIBRARY_PATH, null);
      assert.equal(called.LD_PRELOAD, null);
      assert.equal(called.isolated, true);
    });
  }

  const refused = {
    'it prints a FAIL line, though it exits 0 and says the set passed': [
      { lines: ['FAIL manifest: modified: libvela-swscale.so.10', itsOk[1], 'RESULT PASS'], status: 0 },
      'FAIL manifest: modified: libvela-swscale.so.10',
    ],
    'it stops with a status that is not 0 and no line to say why': [
      { lines: [itsOk[0]], status: 3 },
      'FAIL load: check-libmpv.py stopped with status 3',
    ],
    'it ends without saying that the set passed': [
      { lines: itsOk, status: 0 },
      'FAIL load: check-libmpv.py ended without saying that the set passed',
    ],
    'it says the set failed and exits 0': [
      { lines: [...itsOk, 'RESULT FAIL'], status: 0 },
      'FAIL load: check-libmpv.py ended without saying that the set passed',
    ],
  };
  // Loading the library runs its start-up code. A package that has failed a
  // check before the load is not known to hold the set's code, so
  // check-libmpv.py is not even started for it.
  const failedBefore = {
    'its set is not the one the build was given': (t) => ({
      args: ['--set', referenceWith(t, `${right}glibc 2.35\n`)],
      file: packageWith(t, right).file,
      line: /^FAIL source: /m,
    }),
    'a component is not the pinned version': (t) => ({
      args: [],
      file: packageWith(t, manifestOf(pinned.map(([name, version]) => [name, name === 'ffmpeg' ? '1.0.0' : version]))).file,
      line: /^FAIL pins: /m,
    }),
    'a library of the set is closed to other users': (t) => ({
      args: [],
      file: packageWith(t, right, { modes: { 'usr/lib/Vela/libmpv/libmpv.so.2': 0o600 } }).file,
      line: /^FAIL access: /m,
    }),
  };
  for (const [what, make] of Object.entries(failedBefore)) {
    await t.test(`${what}: nothing of it is loaded`, (t) => {
      const standIn = checkBesideStandIn(t, { lines: [...itsOk, 'RESULT PASS'], status: 0 });
      const { args, file, line } = make(t);
      const run = checkPackageRun([...args, file], standIn.script);
      assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
      assert.match(run.stdout, line);
      assert.equal(standIn.wasCalled(), false, 'check-libmpv.py was started on a package that had failed');
      assert.match(
        run.stdout,
        /^skip load: libmpv\.so\.2 was not loaded, because the package failed a check above; nothing of a set that failed is run$/m,
      );
      assert.match(run.stdout, /^RESULT FAIL /m);
    });
  }

  for (const [what, [answer, line]] of Object.entries(refused)) {
    await t.test(what, (t) => {
      const standIn = checkBesideStandIn(t, answer);
      const good = packageWith(t, right).file;
      const run = checkPackageRun([good], standIn.script);
      assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
      assert.deepEqual(run.stdout.match(/^FAIL .*$/gm), [line], run.stdout);
      assert.deepEqual(run.stdout.match(/^RESULT .*$/gm), [`RESULT FAIL ${good}`]);
      assert.doesNotMatch(run.stdout, /^ok   licences:/m, 'nothing after it is said to be in order');
    });
  }
});

// The check's temporary directory. That every ordinary run leaves it empty is
// held by checkPackageRun, for every run in this file. These are the ends
// that are not ordinary.
test('the package check removes what it unpacked, also when it is cancelled', { skip: checkUnavailable }, async (t) => {
  const right = manifestOf(pinned);
  const emptyDirectory = (t) => {
    const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-tmp-')));
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
    return directory;
  };
  // Start the check in a process group of its own and wait until what it
  // started says it is running (it creates `marker`). `ended` resolves when
  // the check has ended, with how it ended.
  const started = async ({ script = checkPackagePath, args, tmp, marker, env = {} }) => {
    const child = spawn('/bin/bash', [script, ...args], {
      detached: true,
      stdio: 'ignore',
      env: { ...process.env, ...env, TMPDIR: tmp },
    });
    const ended = new Promise((resolve) => child.once('exit', (code, killedBy) => resolve({ code, killedBy })));
    const giveUp = Date.now() + 20_000;
    while (!fs.existsSync(marker)) {
      if (child.exitCode !== null || child.signalCode !== null || Date.now() > giveUp) {
        try {
          process.kill(-child.pid, 'SIGKILL');
        } catch {
          // It has ended by itself.
        }
        throw new Error('the check never reached the step it was to be caught in');
      }
      await new Promise((resolve) => setTimeout(resolve, 2));
    }
    return { child, ended };
  };
  // The same, then ONE signal, to the check alone or to its whole group.
  const interrupted = async ({ signal, wholeGroup, ...check }) => {
    const { child, ended } = await started(check);
    process.kill(wholeGroup ? -child.pid : child.pid, signal);
    return ended;
  };
  const statusOf = { SIGHUP: 129, SIGINT: 130, SIGTERM: 143 };

  await t.test('one unpacked package at a time: the first is gone when the second is checked', (t) => {
    const standIn = checkBesideStandIn(t, { lines: ['RESULT PASS'] });
    const run = checkPackageRun([packageWith(t, right).file, packageWith(t, right, { kind: 'deb' }).file], standIn.script);
    assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
    assert.deepEqual(standIn.called().unpacked, ['2', '2.log']);
  });

  await t.test('two checks at the same time each have a directory of their own', async (t) => {
    // The first check is held at the load step while a second one runs from
    // start to finish in the same TMPDIR and removes what IT unpacked. The
    // first must then still have its set.
    const scratch = emptyDirectory(t);
    const marker = path.join(scratch, 'loading');
    const release = path.join(scratch, 'release');
    const standIn = checkBesideStandIn(t, {
      lines: ['RESULT PASS'],
      first: [
        `open(${JSON.stringify(marker)}, "w").close()`,
        `while not os.path.exists(${JSON.stringify(release)}): time.sleep(0.005)`,
        'open(os.path.join(sys.argv[1], "libmpv.so.2")).close()',
      ],
    });
    const tmp = emptyDirectory(t);
    const first = await started({ script: standIn.script, args: [packageWith(t, right).file], tmp, marker });
    const second = spawnSync('/bin/bash', [checkPackagePath, '--without-libmpv', pacmanPackage(t, {}).file], {
      encoding: 'utf8',
      env: { ...process.env, TMPDIR: tmp },
    });
    fs.writeFileSync(release, '');
    const end = await first.ended;
    assert.equal(second.status, 0, `${second.stdout}\n${second.stderr}`);
    assert.deepEqual(end, { code: 0, killedBy: null }, 'the first check lost its files to the second');
    assert.deepEqual(fs.readdirSync(tmp), []);
  });

  await t.test('a hangup of the whole process group, as a closed terminal sends it', async (t) => {
    // The check is waiting for check-libmpv.py inside a command substitution.
    // There a subshell is hung up too and hangs up the group a second time,
    // which is what kept bash from running its EXIT trap in two runs of
    // three. It is a race, so it is run several times over: without the trap
    // on the signal, all of them coming out clean is a chance in thousands.
    const marker = path.join(emptyDirectory(t), 'loading');
    const standIn = checkBesideStandIn(t, {
      first: [`open(${JSON.stringify(marker)}, "w").close()`, 'time.sleep(60)'],
    });
    const good = packageWith(t, right).file;
    const ends = [];
    for (let round = 0; round < 8; round += 1) {
      fs.rmSync(marker, { force: true });
      const tmp = emptyDirectory(t);
      ends.push(await interrupted({ script: standIn.script, args: [good], tmp, marker, signal: 'SIGHUP', wholeGroup: true }));
      assert.deepEqual(fs.readdirSync(tmp), [], `round ${round}: the unpacked package was left behind`);
    }
    // And it left by its own door each time, with the status of a hangup.
    assert.deepEqual(ends, Array(8).fill({ code: statusOf.SIGHUP, killedBy: null }));
  });

  for (const signal of ['SIGTERM', 'SIGINT']) {
    await t.test(`${signal} to the whole process group`, async (t) => {
      const marker = path.join(emptyDirectory(t), 'loading');
      const standIn = checkBesideStandIn(t, {
        first: [`open(${JSON.stringify(marker)}, "w").close()`, 'time.sleep(60)'],
      });
      const tmp = emptyDirectory(t);
      const end = await interrupted({
        script: standIn.script,
        args: [packageWith(t, right).file],
        tmp,
        marker,
        signal,
        wholeGroup: true,
      });
      assert.deepEqual(end, { code: statusOf[signal], killedBy: null });
      assert.deepEqual(fs.readdirSync(tmp), []);
    });
  }

  for (const signal of ['SIGTERM', 'SIGHUP', 'SIGINT']) {
    await t.test(`${signal} to the check alone while a package is still unpacking: it is let finish first`, async (t) => {
      // An unsquashfs stand-in that goes on writing for a while after it has
      // said it is running. Removing the directory under it would leave
      // behind whatever it wrote next.
      const scratch = emptyDirectory(t);
      const marker = path.join(scratch, 'unpacking');
      const finished = path.join(scratch, 'finished');
      const image = path.join(scratch, 'Vela_1.0.0_amd64.AppImage');
      fs.writeFileSync(image, appImageOf(squashfsLike));
      const PATH = unsquashfsStandIn(t, {
        calls: path.join(scratch, 'calls'),
        extra: `: > "${marker}"
i=0
while [ "$i" -lt 10 ]; do
  echo more > "$dest/usr/file$i"
  i=$((i + 1))
  sleep 0.02
done
: > "${finished}"`,
      });
      const tmp = emptyDirectory(t);
      const end = await interrupted({ args: ['--without-libmpv', image], tmp, marker, signal, wholeGroup: false, env: { PATH } });
      assert.equal(fs.existsSync(finished), true, 'the check ended while the package was still being unpacked');
      assert.deepEqual(end, { code: statusOf[signal], killedBy: null });
      assert.deepEqual(fs.readdirSync(tmp), []);
    });
  }

  // A program of the same name in front of PATH that does `instead` when the
  // check calls it on its temporary directory itself (which only the removal
  // at the end does), and is the real program for every other call.
  const inFront = (t, program, instead) => {
    const real = spawnSync('/bin/sh', ['-c', `command -v ${program}`], { encoding: 'utf8' }).stdout.trim();
    assert.ok(real.startsWith('/'), `${program} must be a program on PATH`);
    const directory = emptyDirectory(t);
    executable(
      path.join(directory, program),
      `#!/bin/sh\nfor last; do :; done\nif [ "$(dirname "$last")" = "$TMPDIR" ]; then ${instead}; fi\nexec ${real} "$@"\n`,
    );
    return `${directory}:${process.env.PATH}`;
  };

  await t.test('a signal that arrives while the directory is being removed does not stop the removal', (t) => {
    // The first step of the removal sends the check a signal, as a second
    // Ctrl-C would. The check has to go on to the second step.
    const PATH = inFront(t, 'chmod', 'kill -TERM "$PPID"');
    const run = checkPackageRun(['--without-libmpv', pacmanPackage(t, {}).file], checkPackagePath, { PATH });
    assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^RESULT PASS /m);
  });

  await t.test('a directory that could not be removed is named', (t) => {
    const PATH = inFront(t, 'rm', 'exit 1');
    const tmp = emptyDirectory(t);
    const run = spawnSync('/bin/bash', [checkPackagePath, '--without-libmpv', pacmanPackage(t, {}).file], {
      encoding: 'utf8',
      env: { ...process.env, PATH, TMPDIR: tmp },
    });
    const [left] = fs.readdirSync(tmp);
    assert.ok(left, 'the stand-in for rm did not take the place of the removal');
    assert.equal(run.stderr, `check-package: could not remove ${path.join(tmp, left)}\n`);
    // What the check found out about the package stands.
    assert.equal(run.status, 0, run.stdout);
  });
});

// check-libmpv.py itself, with a stand-in for the library: what it prints and,
// above all, how its run ENDS for each thing it can find wrong. The package
// check reads its lines and its status, and a FAIL line that did not also
// fail the run was the one link no test held.
const checkLibmpvUnavailable =
  process.platform === 'linux' ? false : 'check-libmpv.py loads the library the way Linux does';

test('check-libmpv.py fails its run for each thing it finds wrong with a set', { skip: checkLibmpvUnavailable }, async (t) => {
  // Runs main() of the real script with ctypes.CDLL and /proc/self/maps
  // replaced: the "library" answers what `say` tells it to, and the process
  // "has mapped" the files `say.mapped` lists.
  const driver = [
    'import ctypes, importlib.util, io, json, os, sys',
    'spec = importlib.util.spec_from_file_location("check_libmpv", sys.argv[1])',
    'module = importlib.util.module_from_spec(spec)',
    'spec.loader.exec_module(module)',
    'say = json.loads(sys.argv[3])',
    'class Function:',
    '    def __init__(self, answer):',
    '        self.answer = answer',
    '    def __call__(self, *arguments):',
    '        return self.answer(*arguments)',
    'kept = []',
    'def c_string(text):',
    '    kept.append(ctypes.create_string_buffer(text.encode()))',
    '    return ctypes.addressof(kept[-1])',
    'class Library:',
    '    def __init__(self):',
    '        self.mpv_client_api_version = Function(lambda: (say["api"][0] << 16) | say["api"][1])',
    '        self.mpv_create = Function(lambda: 1)',
    '        self.mpv_initialize = Function(lambda handle: 0)',
    '        self.mpv_set_option_string = Function(lambda *arguments: 0)',
    '        self.mpv_get_property_string = Function(lambda handle, name: c_string(say[name.decode()]))',
    '        for name in ("mpv_terminate_destroy", "mpv_free", "mpv_command", "mpv_wait_event", "mpv_error_string"):',
    '            setattr(self, name, Function(lambda *arguments: None))',
    'loads = []',
    'def CDLL(path, mode=0):',
    '    loads.append({"path": path, "now": bool(mode & os.RTLD_NOW), "deepbind": bool(mode & os.RTLD_DEEPBIND),',
    '                  "global": bool(mode & os.RTLD_GLOBAL)})',
    '    if not os.path.isfile(path):',
    '        raise OSError(f"{path}: cannot open shared object file: No such file or directory")',
    '    return Library()',
    'ctypes.CDLL = CDLL',
    'real_open = open',
    'def open_or_maps(file, *arguments, **options):',
    '    if file == "/proc/self/maps":',
    '        return io.StringIO("".join(f"7f0000000000-7f0000001000 r-xp 00000000 00:00 0    {path}\\n"',
    '                                   for path in say["mapped"]))',
    '    return real_open(file, *arguments, **options)',
    'module.open = open_or_maps',
    'sys.argv = ["check-libmpv.py", sys.argv[2]] + say["arguments"]',
    'status = module.main()',
    'print("LOADS " + json.dumps(loads))',
    'sys.exit(status)',
  ].join('\n');

  const mpvPin = versions.match(/^mpv\s+(\S+)\s+ship\s/m)?.[1];
  const clientApi = shellValue(checkPackage, 'CLIENT_API');
  const privateName = 'libvela-avcodec.so.63';
  const right = manifestOf(pinned, { libraries: [privateName] });
  // A set on disk, as `right` describes it but for `differing` (and whatever
  // `prepare` does to it), and what the check makes of it when the library
  // answers `say`.
  const verdict = (t, differing = {}, say = {}, prepare = () => {}) => {
    const set = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-set-')));
    t.after(() => fs.rmSync(set, { recursive: true, force: true }));
    for (const [name, contents] of Object.entries(setFilesOf(right, differing))) {
      if (contents === null) continue;
      fs.mkdirSync(path.dirname(path.join(set, name)), { recursive: true });
      fs.writeFileSync(path.join(set, name), contents);
    }
    prepare(set);
    const scenario = {
      api: clientApi.split('.').map(Number),
      'mpv-version': `mpv v${mpvPin}`,
      'ffmpeg-version': '9.0',
      mapped: ['libmpv.so.2', privateName],
      // What check-package.sh asks for.
      arguments: ['--expect-mpv', mpvPin, '--expect-client-api', clientApi],
      ...say,
    };
    scenario.mapped = scenario.mapped.map((name) => (name.startsWith('/') ? name : path.join(set, name)));
    const script = path.join(repoRoot, 'packaging', 'libmpv', 'check-libmpv.py');
    const run = spawnSync('python3', ['-B', '-c', driver, script, set, JSON.stringify(scenario)], { encoding: 'utf8' });
    const lines = run.stdout.split('\n').filter(Boolean);
    const loads = JSON.parse(lines.find((line) => line.startsWith('LOADS '))?.slice(6) ?? 'null');
    return { set, status: run.status, lines: lines.filter((line) => !line.startsWith('LOADS ')), loads, stderr: run.stderr };
  };
  const okLines = (set) => [
    'ok   manifest: every listed file present and unmodified, nothing unlisted, a licence text for every component',
    `ok   load: ${set}/libmpv.so.2 (client API ${clientApi})`,
    `ok   version: 'mpv v${mpvPin}', FFmpeg '9.0'`,
    'ok   isolation: 2 libraries mapped from the set',
  ];
  // Exactly one line differs from a passing run, and the run fails.
  const failsBy = (result, index, line) => {
    const expected = okLines(result.set);
    expected[index] = line;
    assert.deepEqual(result.lines, [...expected, 'RESULT FAIL'], result.stderr);
    assert.equal(result.status, 1);
  };

  await t.test('a set with nothing wrong passes; the library was loaded by its full path, bound to its own set', (t) => {
    const result = verdict(t);
    assert.deepEqual(result.lines, [...okLines(result.set), 'RESULT PASS'], result.stderr);
    assert.equal(result.status, 0);
    assert.deepEqual(result.loads, [
      { path: `${result.set}/libmpv.so.2`, now: true, deepbind: true, global: false },
    ]);
  });

  const damaged = {
    'one byte of a library changed': [{ [privateName]: privateLibrary(privateName).replace('no', 'nO') }, `modified: ${privateName}`],
    'one byte of a licence text changed': [
      { [licencePath('ffmpeg')]: licenceOf('ffmpeg').replace('the', 'thE') },
      `modified: ${licencePath('ffmpeg')}`,
    ],
    'a licence text left out': [{ [licencePath('dav1d')]: null }, `missing: ${licencePath('dav1d')}`],
    'a file the MANIFEST does not list': [{ 'notes.txt': 'x\n' }, 'not in MANIFEST: notes.txt'],
  };
  // A set that fails its manifest check is not loaded: loading a library runs
  // its start-up code, and that code is not known to be the set's.
  const notLoaded = 'skip load: libmpv.so.2 was not loaded, because the set failed the check above; nothing of a set that failed is run';
  const failsUnloaded = (result, problems) => {
    assert.deepEqual(result.lines, [...problems.map((p) => `FAIL manifest: ${p}`), notLoaded, 'RESULT FAIL'], result.stderr);
    assert.equal(result.status, 1);
    assert.deepEqual(result.loads, [], 'a library was loaded');
  };
  for (const [what, [differing, problem]] of Object.entries(damaged)) {
    await t.test(`${what}, and nothing else wrong: not loaded`, (t) => {
      failsUnloaded(verdict(t, differing), [problem]);
    });
  }

  await t.test('libmpv.so.2 is a link to a library outside the set: not loaded', (t) => {
    // The same bytes, so only where they are is wrong.
    const elsewhere = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-elsewhere-')));
    t.after(() => fs.rmSync(elsewhere, { recursive: true, force: true }));
    fs.writeFileSync(path.join(elsewhere, 'libmpv.so.2'), library);
    const outside = path.join(elsewhere, 'libmpv.so.2');
    for (const relative of [false, true]) {
      const linked = verdict(t, { 'libmpv.so.2': null }, {}, (set) =>
        fs.symlinkSync(relative ? path.relative(set, outside) : outside, path.join(set, 'libmpv.so.2')),
      );
      failsUnloaded(linked, ['reached through a symbolic link: libmpv.so.2', 'symbolic link in the set: libmpv.so.2']);
    }
  });

  await t.test('the library is of another client API version', (t) => {
    const result = verdict(t, {}, { api: [2, 5] });
    failsBy(result, 1, `FAIL load: ${result.set}/libmpv.so.2 (client API 2.5) (expected ${clientApi})`);
  });

  await t.test('the library is another mpv release', (t) => {
    const result = verdict(t, {}, { 'mpv-version': 'mpv v0.41.0' });
    failsBy(result, 2, `FAIL version: 'mpv v0.41.0', FFmpeg '9.0' (expected 'mpv v${mpvPin}')`);
  });

  await t.test('a library of the set came from somewhere else', (t) => {
    const result = verdict(t, {}, { mapped: ['libmpv.so.2', `/usr/lib/${privateName}`] });
    failsBy(
      result,
      3,
      `FAIL isolation: 2 libraries mapped from the set; from elsewhere: ['/usr/lib/${privateName}']; never loaded: ['${privateName}']`,
    );
  });

  await t.test('another libmpv.so.2 is in the process as well', (t) => {
    const result = verdict(t, {}, { mapped: ['libmpv.so.2', privateName, '/opt/elsewhere/libmpv.so.2'] });
    failsBy(result, 3, "FAIL isolation: 3 libraries mapped from the set; from elsewhere: ['/opt/elsewhere/libmpv.so.2']");
  });

  await t.test('a library of the set was never loaded', (t) => {
    const result = verdict(t, {}, { mapped: ['libmpv.so.2'] });
    failsBy(result, 3, `FAIL isolation: 1 libraries mapped from the set; never loaded: ['${privateName}']`);
  });

  await t.test('there is no library to load', (t) => {
    failsUnloaded(verdict(t, { 'libmpv.so.2': null }), ['missing: libmpv.so.2']);
  });
});

test('the package check reads an AppImage with unsquashfs and never runs it', { skip: checkUnavailable }, async (t) => {
  // An AppImage could unpack itself, but whatever it is would then run, with
  // the rights of whoever checks it. These AppImages are stand-ins, and so is
  // unsquashfs (see unsquashfsStandIn); a real squashfs is read in the test
  // after this one, where squashfs-tools is installed.
  const scratch = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-appimage-')));
  t.after(() => fs.rmSync(scratch, { recursive: true, force: true }));
  const calls = path.join(scratch, 'unsquashfs-calls');
  const ran = path.join(scratch, 'ran');
  const image = (name, bytes = appImageOf(squashfsLike)) => {
    const file = path.join(scratch, name);
    fs.writeFileSync(file, bytes);
    // As a download leaves it: not executable. The check must not change it.
    fs.chmodSync(file, 0o644);
    return file;
  };
  const both = [['--without-libmpv'], []];

  await t.test('a file named like an AppImage that is a script: never run, refused before anything else', () => {
    // It does what an AppImage asked to unpack itself would do, and leaves a
    // mark that it ran. Executable or not, it must not run.
    const script = `#!/bin/sh
echo ran >> "${ran}"
mkdir -p squashfs-root/usr/bin
echo app > squashfs-root/usr/bin/vela
`;
    for (const mode of [0o644, 0o755]) {
      const file = image(`Vela_1.0.${mode}_amd64.AppImage`, script);
      fs.chmodSync(file, mode);
      for (const args of both) {
        const run = checkPackageRun([...args, file], checkPackagePath, {
          PATH: unsquashfsStandIn(t, { calls }),
          APPIMAGE_EXTRACT_AND_RUN: '1',
        });
        assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
        assert.match(run.stdout, /^ {5}this is not an AppImage this check can read: no squashfs follows an ELF program in it$/m);
        assert.match(run.stdout, /^FAIL unpack: the package could not be unpacked$/m);
        assert.doesNotMatch(run.stdout, /^ok /m);
        assert.match(run.stdout, /^RESULT FAIL /m);
      }
    }
    assert.equal(fs.existsSync(ran), false, 'the file named like an AppImage was run');
    assert.deepEqual(recordedCalls(calls), [], 'unsquashfs was given a file that is no AppImage');
  });

  await t.test('an ELF program with no squashfs after it is refused, and so is a cut-off header', () => {
    const noImage = image('Vela_1.0.3_amd64.AppImage', appImageOf(Buffer.from('not a squashfs\n')));
    const cutOff = image('Vela_1.0.4_amd64.AppImage', appImageOf(squashfsLike).subarray(0, 40));
    for (const file of [noImage, cutOff]) {
      const run = checkPackageRun(['--without-libmpv', file], checkPackagePath, { PATH: unsquashfsStandIn(t, { calls }) });
      assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
      assert.match(run.stdout, /^ {5}this is not an AppImage this check can read: /m);
    }
    assert.deepEqual(recordedCalls(calls), []);
  });

  await t.test('one without the set passes; unsquashfs reads it from where its program ends', () => {
    const file = image('Vela_1.0.0_amd64.AppImage');
    const run = checkPackageRun(['--without-libmpv', file], checkPackagePath, { PATH: unsquashfsStandIn(t, { calls }) });
    assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^ok   absent: nothing named like the set /m);
    const [call, ...more] = recordedCalls(calls);
    assert.deepEqual(more, []);
    assert.deepEqual(call.slice(0, 4), ['-no-progress', '-no-xattrs', '-o', '128']);
    assert.equal(call[4], '-d');
    assert.match(call[5], /\/1\/root$/);
    assert.equal(call[6], file, 'the builder\'s file is read where it lies; no copy is made to run');
    assert.equal(call.length, 7);
    assert.equal(fs.statSync(file).mode & 0o777, 0o644);
    fs.rmSync(calls);
  });

  await t.test('one with a library of the set is found out', () => {
    const PATH = unsquashfsStandIn(t, {
      calls,
      extra: 'mkdir -p "$dest/usr/lib/Vela/libmpv"\necho x > "$dest/usr/lib/Vela/libmpv/libvela-ass.so.9"',
    });
    const run = checkPackageRun(['--without-libmpv', image('Vela_1.0.1_amd64.AppImage')], checkPackagePath, { PATH });
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^FAIL absent: the package has usr\/lib\/Vela\/libmpv$/m);
    assert.match(run.stdout, /^FAIL absent: the package has usr\/lib\/Vela\/libmpv\/libvela-ass\.so\.9$/m);
  });

  await t.test('a library of the set in a directory nobody may read is found out too', () => {
    // A package can carry a directory without any permission bit. Unpacked by
    // an ordinary account it cannot be listed, and a check that only looked
    // where it was allowed to would pass this package.
    const PATH = unsquashfsStandIn(t, {
      calls,
      extra: [
        'mkdir -p "$dest/usr/share/closed"',
        'echo x > "$dest/usr/share/closed/libvela-ass.so.9"',
        'chmod 000 "$dest/usr/share/closed/libvela-ass.so.9" "$dest/usr/share/closed"',
      ].join('\n'),
    });
    const run = checkPackageRun(['--without-libmpv', image('Vela_1.0.2_amd64.AppImage')], checkPackagePath, { PATH });
    assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /^FAIL absent: the package has usr\/share\/closed\/libvela-ass\.so\.9$/m);
  });

  await t.test('without unsquashfs an AppImage is not judged, and the run says it could not check', (t) => {
    const PATH = pathWithout(t, 'unsquashfs');
    const good = pacmanPackage(t, {}).file;
    for (const args of both) {
      const run = checkPackageRun([...args, image('Vela_1.0.0_amd64.AppImage'), good], checkPackagePath, { PATH });
      assert.equal(run.status, 2, `${run.stdout}\n${run.stderr}`);
      assert.match(
        run.stdout,
        /^FAIL unpack: unsquashfs is not installed \(squashfs-tools\), and an AppImage is read with nothing else, so this package could not be checked$/m,
      );
      // The other package is judged all the same.
      assert.deepEqual(run.stdout.match(/^RESULT \w+/gm), ['RESULT FAIL', args.length ? 'RESULT PASS' : 'RESULT FAIL']);
    }
  });
});

// A real squashfs behind an ELF header: needs mksquashfs to make one and
// unsquashfs to read it (squashfs-tools).
const squashfsToolsMissing = ['mksquashfs', 'unsquashfs'].find(
  (tool) => spawnSync('/bin/sh', ['-c', `command -v ${tool}`]).status !== 0,
);
test('the package check reads a real squashfs behind an ELF program', {
  skip: checkUnavailable || (squashfsToolsMissing && `needs squashfs-tools (${squashfsToolsMissing} is not installed)`),
}, async (t) => {
  const scratch = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-squashfs-')));
  t.after(() => fs.rmSync(scratch, { recursive: true, force: true }));
  const appImage = (name, files, modes = {}) => {
    const tree = path.join(scratch, `${name}.tree`);
    for (const [file, contents] of Object.entries({ 'usr/bin/vela': 'the app\n', ...files })) {
      fs.mkdirSync(path.dirname(path.join(tree, file)), { recursive: true });
      fs.writeFileSync(path.join(tree, file), contents);
    }
    for (const entry of fs.readdirSync(tree, { recursive: true, withFileTypes: true })) {
      fs.chmodSync(path.join(entry.parentPath, entry.name), entry.isDirectory() ? 0o755 : 0o644);
    }
    for (const [file, mode] of Object.entries(modes)) fs.chmodSync(path.join(tree, file), mode);
    const squashfs = path.join(scratch, `${name}.squashfs`);
    const made = spawnSync('mksquashfs', [tree, squashfs, '-noappend', '-quiet', '-all-root'], { encoding: 'utf8' });
    assert.equal(made.status, 0, made.stderr);
    const file = path.join(scratch, name);
    fs.writeFileSync(file, appImageOf(fs.readFileSync(squashfs)));
    return file;
  };
  const clean = checkPackageRun(['--without-libmpv', appImage('Vela_1.0.0_amd64.AppImage', {})]);
  assert.equal(clean.status, 0, `${clean.stdout}\n${clean.stderr}`);
  const dirty = checkPackageRun([
    '--without-libmpv',
    appImage('Vela_1.0.1_amd64.AppImage', { 'usr/lib/Vela/libmpv/libvela-ass.so.9': 'x\n' }),
  ]);
  assert.equal(dirty.status, 1, `${dirty.stdout}\n${dirty.stderr}`);
  assert.match(dirty.stdout, /^FAIL absent: the package has usr\/lib\/Vela\/libmpv\/libvela-ass\.so\.9$/m);
  // The modes in the image are the package's, and unsquashfs keeps them.
  const right = manifestOf(pinned);
  const inSet = Object.fromEntries(Object.entries(setFilesOf(right)).map(([name, contents]) => [`usr/lib/Vela/libmpv/${name}`, contents]));
  const closed = checkPackageRun([appImage('Vela_1.0.2_amd64.AppImage', inSet, { 'usr/lib/Vela/libmpv/libmpv.so.2': 0o600 })]);
  assert.equal(closed.status, 1, `${closed.stdout}\n${closed.stderr}`);
  assert.match(closed.stdout, /^ok   place: the set is at usr\/lib\/Vela\/libmpv$/m);
  assert.match(closed.stdout, /^FAIL access: usr\/lib\/Vela\/libmpv\/libmpv\.so\.2 cannot be read by every user/m);
});

test('the package check never lets an unpacked package write outside its temporary directory', { skip: checkUnavailable }, async (t) => {
  // What bsdtar does by default, and what the check relies on: it refuses a
  // name with "..", takes the leading "/" off an absolute name (which then
  // lands inside the package's tree), and refuses to write through a
  // symbolic link or to make a hard link to a file outside. Each refusal
  // fails the package, in either mode.
  const scratch = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-escape-')));
  t.after(() => fs.rmSync(scratch, { recursive: true, force: true }));
  const outside = path.join(scratch, 'outside');
  fs.mkdirSync(outside);
  fs.writeFileSync(path.join(outside, 'victim'), 'as it was\n');
  const make = [
    'import io, sys, tarfile',
    'out, outside, which = sys.argv[1], sys.argv[2], sys.argv[3]',
    'def add(t, name, data=b"escaped\\n", **fields):',
    '    entry = tarfile.TarInfo(name)',
    '    for key, value in fields.items(): setattr(entry, key, value)',
    '    entry.size = len(data) if entry.type == tarfile.REGTYPE else 0',
    '    t.addfile(entry, io.BytesIO(data) if entry.type == tarfile.REGTYPE else None)',
    'with tarfile.open(out, "w", format=tarfile.GNU_FORMAT) as t:',
    '    add(t, "usr/bin/vela", b"the app\\n")',
    '    if which == "dotdot": add(t, "../" * 30 + outside.lstrip("/") + "/dotdot")',
    '    if which == "absolute": add(t, outside + "/absolute")',
    '    if which == "through a link":',
    '        add(t, "usr/out", type=tarfile.SYMTYPE, linkname=outside)',
    '        add(t, "usr/out/through")',
    '    if which == "a hard link out":',
    '        add(t, "usr/hard", type=tarfile.LNKTYPE, linkname=outside + "/victim")',
    '        add(t, "usr/hard", b"overwritten\\n")',
    '    if which == "a hard link through a link":',
    '        add(t, "usr/out", type=tarfile.SYMTYPE, linkname=outside)',
    '        add(t, "usr/hard", type=tarfile.LNKTYPE, linkname="usr/out/victim")',
    '        add(t, "usr/hard", b"overwritten\\n")',
  ].join('\n');
  for (const which of ['dotdot', 'absolute', 'through a link', 'a hard link out', 'a hard link through a link']) {
    await t.test(which, () => {
      // An uncompressed tar under a pacman package's name: bsdtar reads either.
      const file = path.join(scratch, `vela-1.0.0-1-x86_64.pkg.tar.zst`);
      const made = spawnSync('python3', ['-I', '-c', make, file, outside, which], { encoding: 'utf8' });
      assert.equal(made.status, 0, made.stderr);
      for (const mode of [['--without-libmpv'], []]) {
        const run = checkPackageRun([...mode, file]);
        if (which !== 'absolute') {
          assert.equal(run.status, 1, `${run.stdout}\n${run.stderr}`);
          assert.match(run.stdout, /^FAIL unpack: the package could not be unpacked$/m);
        }
        assert.deepEqual(fs.readdirSync(outside), ['victim'], `${which}: the package wrote outside`);
        assert.equal(fs.readFileSync(path.join(outside, 'victim'), 'utf8'), 'as it was\n');
      }
    });
  }
});

test('the package check stops when mpv was bumped and its client API version was not looked up again', { skip: checkUnavailable }, (t) => {
  const copy = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-bump-')));
  t.after(() => fs.rmSync(copy, { recursive: true, force: true }));
  const script = path.join(copy, 'check-package.sh');
  fs.copyFileSync(checkPackagePath, script);
  const bumped = versions.replace(/^(mpv\s+)\S+/m, (_, lead) => `${lead}9.99.0`);
  assert.notEqual(bumped, versions);
  fs.writeFileSync(path.join(copy, 'versions.txt'), bumped);
  const anything = pacmanPackage(t, {});
  const run = checkPackageRun([anything.file], script);
  assert.equal(run.status, 2, `${run.stdout}\n${run.stderr}`);
  assert.match(run.stderr, /versions\.txt pins mpv 9\.99\.0, but the client API version written in this script/);
  assert.equal(run.stdout, '', 'no package is judged with a number that may be stale');
  // A package that must not have the set needs no such number.
  assert.equal(checkPackageRun(['--without-libmpv', anything.file], script).status, 0);
  // Asked beforehand, the check says the same, so a build learns it at its start.
  const asked = checkPackageRun(['--preflight'], script);
  assert.equal(asked.status, 2, `${asked.stdout}\n${asked.stderr}`);
  assert.match(asked.stderr, /versions\.txt pins mpv 9\.99\.0, but the client API version written in this script/);
  assert.equal(checkPackageRun(['--preflight', '--without-libmpv'], script).status, 0);
});

// packaging/arch/PKGBUILD's package(), run without makepkg. The file is bash,
// and package() needs only what makepkg would have set for it: where the
// PKGBUILD lies, where to install to, and the files of the repository it
// installs, which are made here as stand-ins. Returns the options the
// PKGBUILD ends up with, what package() printed, and the tree it installed.
function runPkgbuild(t, env = {}) {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-pkgbuild-')));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const fromRepository = [...pkgbuild.matchAll(/"\$repo\/([^"]+)"/g)].map((found) => found[1]);
  assert.ok(fromRepository.includes('src-tauri/target/release/vela'), 'package() installs the program');
  for (const name of fromRepository) {
    fs.mkdirSync(path.dirname(path.join(root, name)), { recursive: true });
    fs.writeFileSync(path.join(root, name), `${name}\n`);
  }
  fs.mkdirSync(path.join(root, 'packaging', 'arch'), { recursive: true });
  fs.writeFileSync(path.join(root, 'packaging', 'arch', 'PKGBUILD'), pkgbuild);
  const pkgdir = path.join(root, 'pkg');
  const clean = { ...process.env };
  delete clean.VELA_PACKAGE_LIBMPV;
  const run = spawnSync(
    '/bin/bash',
    ['-c', 'set -e; startdir="$PWD"; pkgdir="$1"; source ./PKGBUILD; printf "%s\\n" "${options[*]}" > "$2"; package', 'makepkg', pkgdir, path.join(root, 'options')],
    { cwd: path.join(root, 'packaging', 'arch'), encoding: 'utf8', env: { ...clean, ...env } },
  );
  const installed = {};
  if (fs.existsSync(pkgdir)) {
    for (const entry of fs.readdirSync(pkgdir, { recursive: true, withFileTypes: true })) {
      if (!entry.isFile()) continue;
      const file = path.join(entry.parentPath, entry.name);
      installed[path.relative(pkgdir, file)] = fs.readFileSync(file, 'utf8');
    }
  }
  return { ...run, root, pkgdir, installed, options: fs.readFileSync(path.join(root, 'options'), 'utf8').trim() };
}

test("the PKGBUILD's package(), run: the set goes in when the variable names one, with a line to say so, and not otherwise", { skip: checkUnavailable }, async (t) => {
  const manifest = manifestOf(pinned, { libraries: ['libvela-avcodec.so.63'] });
  const setOnDisk = (t, files = setFilesOf(manifest)) => {
    const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-set-')));
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
    for (const [name, contents] of Object.entries(files)) {
      fs.mkdirSync(path.dirname(path.join(directory, name)), { recursive: true });
      fs.writeFileSync(path.join(directory, name), contents);
    }
    return directory;
  };
  // The tree package() installed, as the pacman package makepkg would make of it.
  const packed = (built) => {
    for (const entry of fs.readdirSync(built.pkgdir, { recursive: true, withFileTypes: true })) {
      fs.chmodSync(path.join(entry.parentPath, entry.name), entry.isDirectory() ? 0o755 : 0o644);
    }
    return packageKinds['pkg.tar.zst'](built.root, built.pkgdir);
  };

  await t.test('without the variable: the package it has always been, and not a word', (t) => {
    // A set lying about and the development build's variable change nothing.
    const set = setOnDisk(t);
    const built = runPkgbuild(t, { VELA_LIBMPV_DIR: set });
    assert.equal(built.status, 0, built.stderr);
    assert.deepEqual([built.stdout, built.stderr], ['', '']);
    assert.equal(built.options, '!debug');
    assert.deepEqual(Object.keys(built.installed).filter((name) => /libmpv|libvela/i.test(name)), []);
    const run = checkPackageRun(['--without-libmpv', '--set', set, packed(built)]);
    assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
    // The same with the variable set and empty.
    const empty = runPkgbuild(t, { VELA_PACKAGE_LIBMPV: '' });
    assert.deepEqual([empty.status, empty.stdout, empty.options], [0, '', '!debug']);
    assert.deepEqual(Object.keys(empty.installed), Object.keys(built.installed));
  });

  await t.test('with the variable: every file of the set, each licence text twice, unstripped, and it says so', (t) => {
    const set = setOnDisk(t);
    const built = runPkgbuild(t, { VELA_PACKAGE_LIBMPV: set });
    assert.equal(built.status, 0, built.stderr);
    // Whoever set the variable is told what it did, and that nothing here
    // checks the result.
    assert.equal(
      built.stdout,
      `==> VELA_PACKAGE_LIBMPV is set: packaging the libmpv set from ${set}\n` +
        '    Only scripts/build.sh --with-libmpv checks the package afterwards. Built any other way, check it yourself:\n' +
        `    packaging/libmpv/check-package.sh --set ${set} <the package>\n`,
    );
    assert.equal(built.options, '!debug !strip');
    const inPlace = Object.fromEntries(
      Object.entries(built.installed)
        .filter(([name]) => name.startsWith('usr/lib/Vela/libmpv/'))
        .map(([name, contents]) => [name.slice('usr/lib/Vela/libmpv/'.length), contents]),
    );
    assert.deepEqual(inPlace, setFilesOf(manifest));
    for (const [component] of pinned) {
      assert.equal(built.installed[`usr/share/licenses/vela/libmpv/${component}/LICENSE.LGPL`], licenceOf(component));
    }
    // And the package check takes it for what it is. (Its judgement of the
    // library is a stand-in's; everything else is the real check.)
    const standIn = checkBesideStandIn(t, { lines: ['RESULT PASS'] });
    const run = checkPackageRun(['--set', set, packed(built)], standIn.script);
    assert.equal(run.status, 0, `${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, new RegExp(`^ok   licences: all ${pinned.length} licence texts are also under `, 'm'));
  });

  await t.test('with the variable naming a directory that holds no set: no package', (t) => {
    for (const files of [{ MANIFEST: manifest }, { 'libmpv.so.2': library }]) {
      const notASet = setOnDisk(t, files);
      const built = runPkgbuild(t, { VELA_PACKAGE_LIBMPV: notASet });
      assert.equal(built.status, 1, built.stdout);
      assert.equal(built.stderr, `VELA_PACKAGE_LIBMPV names ${notASet}, which holds no libmpv set\n`);
      assert.deepEqual(Object.keys(built.installed).filter((name) => /libmpv|libvela/i.test(name)), []);
    }
  });
});

// A PATH on which every program of this process's PATH is found but those named.
function pathWithout(t, ...missing) {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-path-')));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  for (const from of process.env.PATH.split(':').filter(Boolean)) {
    let names = [];
    try {
      names = fs.readdirSync(from);
    } catch {
      // PATH may name a directory that is not there.
    }
    for (const name of names) {
      if (missing.includes(name)) continue;
      try {
        fs.symlinkSync(path.join(from, name), path.join(directory, name));
      } catch {
        // An earlier directory of PATH has a program of that name: it wins.
      }
    }
  }
  return directory;
}

test('the package check, asked beforehand, says whether it can run and judges nothing', { skip: checkUnavailable }, async (t) => {
  const set = referenceWith(t, manifestOf(pinned));

  await t.test('it can: status 0 and not a word', () => {
    for (const args of [['--preflight'], ['--preflight', '--set', set], ['--without-libmpv', '--preflight', `--set=${set}`]]) {
      const run = checkPackageRun(args);
      assert.deepEqual([run.status, run.stdout, run.stderr], [0, '', ''], args.join(' '));
    }
  });

  await t.test('named packages are refused, not passed over', (t) => {
    // A package named beside --preflight is not looked at. Status 0 would
    // read as "it passed".
    const run = checkPackageRun(['--preflight', '--set', set, pacmanPackage(t, {}).file]);
    assert.deepEqual([run.status, run.stdout], [2, '']);
    assert.equal(run.stderr, 'check-package: --preflight checks no package; name none\n');
  });

  await t.test('a set that is none', (t) => {
    const empty = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-given-')));
    t.after(() => fs.rmSync(empty, { recursive: true, force: true }));
    const run = checkPackageRun(['--preflight', '--set', empty]);
    assert.deepEqual([run.status, run.stdout], [2, '']);
    assert.equal(run.stderr, `check-package: no libmpv set in ${empty} (it has no MANIFEST)\n`);
  });

  await t.test('no bsdtar on this machine', (t) => {
    const PATH = pathWithout(t, 'bsdtar');
    for (const mode of [[], ['--without-libmpv']]) {
      const run = checkPackageRun(['--preflight', ...mode, '--set', set], checkPackagePath, { PATH });
      assert.deepEqual([run.status, run.stdout], [2, '']);
      assert.match(run.stderr, /^check-package: bsdtar is not installed \(libarchive; on Debian and Ubuntu: libarchive-tools\)\n$/);
    }
  });

  await t.test('no unsquashfs on this machine: it matters only when an AppImage is to be checked', (t) => {
    const PATH = pathWithout(t, 'unsquashfs');
    for (const mode of [[], ['--without-libmpv']]) {
      const run = checkPackageRun(['--preflight', ...mode, '--set', set, '--appimage'], checkPackagePath, { PATH });
      assert.deepEqual([run.status, run.stdout], [2, '']);
      assert.equal(run.stderr, 'check-package: unsquashfs is not installed (squashfs-tools); an AppImage is read with nothing else\n');
      const withoutAppImage = checkPackageRun(['--preflight', ...mode, '--set', set], checkPackagePath, { PATH });
      assert.deepEqual([withoutAppImage.status, withoutAppImage.stderr], [0, '']);
    }
    // With it, an AppImage can be checked.
    const scratch = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'vela-libmpv-calls-')));
    t.after(() => fs.rmSync(scratch, { recursive: true, force: true }));
    const found = checkPackageRun(['--preflight', '--set', set, '--appimage'], checkPackagePath, {
      PATH: unsquashfsStandIn(t, { calls: path.join(scratch, 'calls') }),
    });
    assert.deepEqual([found.status, found.stderr], [0, '']);
    // Named packages say for themselves whether one is an AppImage.
    const named = checkPackageRun(['--appimage', pacmanPackage(t, {}).file]);
    assert.deepEqual([named.status, named.stdout], [2, '']);
    assert.equal(named.stderr, 'check-package: --appimage goes with --preflight; name the packages instead\n');
  });

  await t.test('no Python on this machine', (t) => {
    const PATH = pathWithout(t, 'python3');
    const run = checkPackageRun(['--preflight', '--set', set], checkPackagePath, { PATH });
    assert.deepEqual([run.status, run.stdout], [2, '']);
    assert.equal(run.stderr, 'check-package: python3 is not installed; loading the library needs it\n');
    // Only loading the library needs it. A package that must hold no set is
    // judged without.
    assert.equal(checkPackageRun(['--preflight', '--without-libmpv', '--set', set], checkPackagePath, { PATH }).status, 0);
  });
});
