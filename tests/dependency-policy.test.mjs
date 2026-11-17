import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

// The patterns below spell line breaks as \n; a Windows checkout with
// core.autocrlf=true has CRLF, so normalize at read time.
const readText = async (...args) => (await readFile(...args)).replace(/\r\n/g, "\n");

test("the released Wayland API uses the locally secured scanner", async () => {
  const cargoToml = await readText(
    path.join(repoRoot, "src-tauri", "Cargo.toml"),
    "utf8",
  );
  const cargoLock = await readText(
    path.join(repoRoot, "src-tauri", "Cargo.lock"),
    "utf8",
  );
  const vendorToml = await readText(
    path.join(
      repoRoot,
      "src-tauri",
      "vendor",
      "wayland-scanner",
      "Cargo.toml",
    ),
    "utf8",
  );
  const vendorParser = await readText(
    path.join(
      repoRoot,
      "src-tauri",
      "vendor",
      "wayland-scanner",
      "src",
      "parse.rs",
    ),
    "utf8",
  );
  const patchSection = cargoToml.match(
    /\[patch\.crates-io\]\n(?<body>[\s\S]*?)(?=\n\[|$)/,
  )?.groups?.body;
  assert.ok(patchSection, "Cargo.toml must retain the crates.io patch section");
  assert.match(
    patchSection,
    /^wayland-scanner = \{ path = "vendor\/wayland-scanner" \}$/m,
    "the vulnerable published scanner must be replaced locally",
  );
  assert.match(vendorToml, /^name = "wayland-scanner"$/m);
  assert.match(vendorToml, /^version = "0\.31\.10"$/m);
  assert.match(vendorToml, /^quick-xml = "0\.41"$/m);
  assert.match(vendorParser, /byte_ref\.xml10_content\(\)/);
  assert.doesNotMatch(vendorParser, /byte_ref\.xml_content\(\)/);

  for (const crate of ["wayland-backend", "wayland-client"]) {
    const packageBlock = cargoLock.match(
      new RegExp(
        `\\[\\[package\\]\\]\\nname = "${crate}"\\n[\\s\\S]*?(?=\\n\\[\\[package\\]\\]|$)`,
      ),
    )?.[0];
    assert.match(packageBlock ?? "", /^source = "registry\+/m);
  }

  const scannerBlock = cargoLock.match(
    /\[\[package\]\]\nname = "wayland-scanner"\n[\s\S]*?(?=\n\[\[package\]\]|$)/,
  )?.[0];
  assert.ok(scannerBlock, "the local scanner must be locked");
  assert.doesNotMatch(scannerBlock, /^source = /m);
  assert.match(scannerBlock, / "quick-xml",/);
});

test("non-x86 Windows cannot select an x86-64-v3 mpv build", async () => {
  const commands = await readText(
    path.join(repoRoot, "src-tauri", "src", "commands.rs"),
    "utf8",
  );
  const fallback = commands.match(
    /#\[cfg\(all\(target_os = "windows", not\(target_arch = "x86_64"\)\)\)\]\s*fn cpu_supports_v3\(\) -> bool \{(?<body>[\s\S]*?)\n\}/,
  )?.groups?.body;
  assert.ok(fallback, "the non-x86 Windows fallback must remain explicit");
  assert.match(fallback, /^\s*false\s*$/);
});

test("Ask source choices stay one-shot, session-safe, and credential-free", async () => {
  const commands = await readText(
    path.join(repoRoot, "src-tauri", "src", "commands.rs"),
    "utf8",
  );
  const lib = await readText(path.join(repoRoot, "src-tauri", "src", "lib.rs"), "utf8");

  const publicChoice = commands.match(
    /pub struct PlaybackSourceChoiceDto \{(?<body>[\s\S]*?)\n\}/,
  )?.groups?.body;
  assert.ok(publicChoice, "the public source-choice DTO must remain explicit");
  assert.match(publicChoice, /source_id/);
  assert.match(publicChoice, /source_name/);
  assert.match(publicChoice, /locality/);
  assert.match(publicChoice, /quality_label/);
  assert.doesNotMatch(publicChoice, /token|url|endpoint|header|session/i);

  const select = commands.match(
    /async fn select_playback_version\((?<body>[\s\S]*?)\n\}\n\nstruct PlayLaunchRequest/,
  )?.groups?.body;
  assert.ok(select, "the shared playback selector must remain available");
  assert.match(select, /policy != crate::selection::PlaybackSourcePolicy::Ask/);
  assert.match(select, /if server_owned \{\s*vec!\[crate::source::backing_ref_of\(item\)\]/);

  const launchPrefix = commands.match(
    /async fn play_by_key_locked\((?<body>[\s\S]*?)let prior_affinity =/,
  )?.groups?.body;
  assert.ok(launchPrefix, "the serialized playback launch boundary must remain explicit");
  assert.doesNotMatch(
    launchPrefix,
    /playback_run\.lock\(\)\.await\s*=\s*None|playlist_cursor\.lock\(\)\.await\s*=\s*None/,
    "opening a cancellable prompt must not erase the current playback run",
  );

  const resolve = commands.match(
    /pub async fn resolve_playback_source_choice\((?<body>[\s\S]*?)\n\}\n\n#\[tauri::command\]\npub async fn cancel_playback_source_choice/,
  )?.groups?.body;
  assert.ok(resolve, "the source-choice resolver must remain registered");
  // The command hands its request id to `resolve_choice`, which takes the
  // held choice by that id (review finding pc-2 moved it there).
  assert.match(resolve, /resolve_choice\(\s*state,\s*&request_id,/);
  const resolveChoice = commands.match(
    /async fn resolve_choice<P, F>\((?<body>[\s\S]*?)\n\}\n/,
  )?.groups?.body;
  assert.ok(resolveChoice, "the source-choice resolver's body must remain explicit");
  assert.match(resolveChoice, /\.take_at\(request_id, Instant::now\(\)\)/);
  assert.match(resolve, /persist_explicit_choice: false/);

  const emittedChoice = commands.match(
    /fn emit_source_choice_required\((?<body>[\s\S]*?)\n\}/,
  )?.groups?.body;
  assert.ok(emittedChoice, "automatic sequence choices must use an id-only event");
  assert.match(emittedChoice, /json!\(\{ "requestId": request_id \}\)/);
  assert.doesNotMatch(emittedChoice, /choices|title|source_name|quality_label/);
  assert.match(lib, /commands::get_playback_source_choice/);
  assert.match(lib, /commands::resolve_playback_source_choice/);
  assert.match(lib, /commands::cancel_playback_source_choice/);
  assert.match(lib, /commands::finish_playback_run/);
});

// ---------------------------------------------------------------------------
// The libmpv set Vela ships for the embedded player
// (.agents/plans/embedded-player.md, "libmpv is shipped with Vela").
// ---------------------------------------------------------------------------

const libmpvRecipe = path.join(repoRoot, "packaging", "libmpv");
const libmpvScriptPath = path.join(repoRoot, "scripts", "build-libmpv.sh");

async function libmpvPins() {
  const text = await readText(path.join(libmpvRecipe, "versions.txt"), "utf8");
  return text
    .split("\n")
    .filter((line) => line.trim() !== "" && !line.startsWith("#"))
    .map((line) => {
      const fields = line.trim().split(/\s+/);
      const [name, version, role, sha256, url, ...mirrors] = fields;
      return { name, version, role, sha256, url, mirrors, fieldCount: fields.length };
    });
}

// The system libraries the shipped set may load from the machine. Changing
// this list is a decision about what Vela ships and what it leaves to the
// distribution, so it is written twice on purpose: here and in the script.
const libmpvAllowedSystem = [
  "ld-linux-aarch64.so.1",
  "ld-linux-x86-64.so.2",
  "libEGL.so.1",
  "libasound.so.2",
  "libc.so.6",
  "libdl.so.2",
  "libdrm.so.2",
  "libfontconfig.so.1",
  "libgcc_s.so.1",
  "libgnutls.so.30",
  "libm.so.6",
  "libpthread.so.0",
  "libpulse.so.0",
  "librt.so.1",
  "libstdc++.so.6",
  "libva-drm.so.2",
  "libva.so.2",
  "libz.so.1",
];

test("every component of the shipped libmpv set is pinned by version, checksum and URL", async () => {
  const pins = await libmpvPins();
  assert.ok(pins.length >= 10, "the pinned-versions file must list the set");
  const names = new Set();
  for (const pin of pins) {
    assert.ok(pin.fieldCount >= 5, `${pin.name}: name, version, role, sha256, url [mirror ...]`);
    assert.ok(!names.has(pin.name), `${pin.name} is pinned twice`);
    names.add(pin.name);
    assert.match(pin.version, /^\d+(\.\d+)+$/, `${pin.name}: version`);
    assert.match(pin.role, /^(ship|build|build-macos)$/, `${pin.name}: role`);
    assert.match(pin.sha256, /^[0-9a-f]{64}$/, `${pin.name}: sha256`);
    const extension = pin.url.match(/\.tar\.(gz|xz)$/)?.[0];
    for (const source of [pin.url, ...pin.mirrors]) {
      assert.match(source, /^https:\/\/\S+\.tar\.(gz|xz)$/, `${pin.name}: an https tarball`);
      assert.ok(
        source.includes(pin.version),
        `${pin.name}: the URL must name the pinned version ${pin.version}`,
      );
      // One cache file per component, named from the first URL.
      assert.ok(source.endsWith(extension), `${pin.name}: every source is the same archive type`);
    }
    assert.equal(new Set([pin.url, ...pin.mirrors]).size, 1 + pin.mirrors.length);
  }
  for (const required of ["mpv", "ffmpeg"]) {
    assert.equal(pins.find((pin) => pin.name === required)?.role, "ship");
  }
});

test("the libmpv build script builds exactly the pinned components", async () => {
  const pins = await libmpvPins();
  const script = await readText(libmpvScriptPath, "utf8");
  const shipped = pins.filter((pin) => pin.role === "ship").map((pin) => pin.name).sort();

  const defaultSteps = script.match(/^steps="\$\{\*:-(?<steps>[^}]+)\}"$/m)?.groups?.steps;
  assert.ok(defaultSteps, "the default step list must stay one readable line");
  const steps = defaultSteps.trim().split(/\s+/);
  assert.equal(steps.at(-1), "collect", "collect must run last");
  assert.deepEqual(
    steps.slice(0, -1).sort(),
    shipped,
    "the default steps must be exactly the shipped components",
  );
  const builders = [...script.matchAll(/^build_([a-z0-9_]+)\(\) \{$/gm)].map((m) => m[1]).sort();
  assert.deepEqual(builders, shipped, "one build function per shipped component");

  // Nothing is downloaded that is not pinned, and nothing bypasses fetch().
  const fetched = [...script.matchAll(/"\$\((?:unpack|fetch) ([a-z0-9-]+)\)"/g)].map((m) => m[1]);
  const pinned = new Set(pins.map((pin) => pin.name));
  assert.ok(fetched.length >= shipped.length);
  for (const name of fetched) assert.ok(pinned.has(name), `${name} is fetched but not pinned`);
  const windowsScript = await readText(path.join(repoRoot, "scripts", "build-libmpv-windows.sh"), "utf8");
  const windowsFetched = [...windowsScript.matchAll(/"\$\((?:unpack|fetch) ([a-z0-9-]+)\)"/g)].map((m) => m[1]);
  const macScriptText = await readText(path.join(repoRoot, "scripts", "build-libmpv-macos.sh"), "utf8");
  const macFetched = [...macScriptText.matchAll(/"\$\((?:unpack|fetch) ([a-z0-9-]+)\)"/g)].map((m) => m[1]);
  for (const pin of pins) {
    assert.ok(fetched.includes(pin.name) || windowsFetched.includes(pin.name) || macFetched.includes(pin.name), `${pin.name} is pinned but never fetched by any platform`);
    if (pin.role === "build-macos") assert.ok(macFetched.includes(pin.name), `${pin.name} is a macOS build input the macOS build never fetches`);
  }
  assert.equal(script.match(/\bcurl\b/g)?.length, 1, "fetch() is the only download");
  assert.doesNotMatch(script, /\b(wget|git clone|pip3? install)\b/);
  assert.match(script, /sha256sum -c --quiet - >&2 \|\| die "checksum mismatch/);
  assert.match(script, /--wrap-mode=nodownload/);
  // bash does not carry `set -e` into $(...) on its own; without this a failed
  // fetch inside unpack() would not stop the build.
  assert.match(script, /^set -euo pipefail\n(#.*\n)*shopt -s inherit_errexit$/m);
});

test("the shipped libmpv set is an LGPL build with its licence texts", async () => {
  const pins = await libmpvPins();
  const script = await readText(libmpvScriptPath, "utf8");

  const mpv = script.match(/^build_mpv\(\) \{\n(?<body>[\s\S]*?)\n\}$/m)?.groups?.body;
  assert.ok(mpv, "build_mpv must remain a recognisable function");
  assert.match(mpv, /-Dgpl=false/);
  assert.match(mpv, /-Dlibmpv=true -Dcplayer=false/);
  assert.doesNotMatch(mpv, /-Dgpl=true/);

  const ffmpeg = script.match(/^build_ffmpeg\(\) \{\n(?<body>[\s\S]*?)\n\}$/m)?.groups?.body;
  assert.ok(ffmpeg, "build_ffmpeg must remain a recognisable function");
  assert.doesNotMatch(ffmpeg, /--enable-(gpl|nonfree|version3)/);
  assert.match(ffmpeg, /--enable-shared --disable-static/);
  assert.match(ffmpeg, /--disable-autodetect/);
  assert.match(
    ffmpeg,
    /grep -qx '#define FFMPEG_LICENSE "LGPL version 2\.1 or later"' config\.h\s*\\\n\s*\|\| die/,
    "the build must stop if FFmpeg itself reports another licence",
  );

  // Shared libraries only: nothing is linked statically into anything.
  assert.match(script, /-Ddefault_library=shared/);

  const collect = script.match(/^collect\(\) \{\n(?<body>[\s\S]*?)\n\}$/m)?.groups?.body;
  assert.ok(collect, "collect must remain a recognisable function");
  for (const pin of pins.filter((entry) => entry.role === "ship")) {
    assert.match(
      collect,
      new RegExp(`(lic ${pin.name} \\S|licenses/${pin.name}/LICENSE)`),
      `${pin.name}: its licence text must be collected`,
    );
  }
  assert.match(collect, /\|\| die "no licence text collected for shipped component/);
  // The manifest lists every file of the set, not only the libraries: the
  // integrity check can then notice a missing licence text.
  assert.match(collect, /find \. -type f ! -name MANIFEST -printf '%P\\n' \| LC_ALL=C sort/);
  assert.doesNotMatch(collect, /for lib in \*\.so\*; do\n\s+printf 'file /);
  // ...and names which of them are licence texts, so that a build patch (or
  // anything else kept beside a licence) cannot stand in for one.
  assert.match(collect, /roles="\$roles"\$'\\n'"licence \$name licenses\/\$name\/\$\(basename "\$f"\)"/);
  assert.match(collect, /roles="\$roles"\$'\\n'"patch mpv licenses\/mpv\/\$\(basename "\$patch_file"\)"/);
  assert.match(collect, /grep -q "\^licence \$name " \\\n\s+\|\| die "no licence text collected for shipped component/);
  assert.match(
    collect,
    /for patch_file in "\$PATCHES"\/\*\.patch; do\n\s+install -m644 "\$patch_file" "\$dest\/licenses\/mpv\/"/,
  );
});

test("the shipped libmpv set is isolated and leans only on the listed system libraries", async () => {
  const script = await readText(libmpvScriptPath, "utf8");
  const allowed = script
    .match(/^ALLOWED_SYSTEM="\n(?<body>[\s\S]*?)\n"$/m)
    ?.groups?.body.trim()
    .split(/\s+/)
    .sort();
  assert.deepEqual(allowed, [...libmpvAllowedSystem].sort());

  const collect = script.match(/^collect\(\) \{\n(?<body>[\s\S]*?)\n\}$/m)?.groups?.body ?? "";
  assert.match(collect, /\|\| die "the set needs system library/);
  assert.match(collect, /new="libvela-\$\{lib#lib\}"/, "private sonames");
  assert.match(collect, /patchelf --set-soname "\$new"/);
  assert.match(collect, /patchelf --replace-needed "\$old" "\$new"/);
  assert.match(collect, /patchelf --set-rpath '\$ORIGIN'/);
  assert.match(collect, /die "\$\(basename "\$lib"\) still needs/);
  assert.match(collect, /die "the set needs glibc \$max, above the baseline/);
});

test("the libmpv set is built on the distribution the Linux bundles are built on", async () => {
  const script = await readText(libmpvScriptPath, "utf8");
  const containerfile = await readText(path.join(libmpvRecipe, "Containerfile"), "utf8");
  const release = await readText(
    path.join(repoRoot, ".github", "workflows", "release.yml"),
    "utf8",
  );
  assert.match(release, /^\s+- os: ubuntu-22\.04$/m, "the release builds Linux bundles on 22.04");
  assert.match(containerfile, /^FROM docker\.io\/library\/ubuntu:22\.04$/m);
  // Ubuntu 22.04 ships glibc 2.35.
  assert.match(script, /^GLIBC_MAX="\$\{GLIBC_MAX:-2\.35\}"$/m);
  // The image pins the two tools it does not take from the distribution.
  assert.match(containerfile, /meson==\d+\.\d+\.\d+ patchelf==\d+(\.\d+)+/);
});

test("the mpv patch changes the build description only", async () => {
  const patch = await readText(
    path.join(libmpvRecipe, "patches", "mpv-vaapi-without-drm-output.patch"),
    "utf8",
  );
  const files = [...patch.matchAll(/^\+\+\+ b\/(\S+)$/gm)].map((m) => m[1]);
  assert.deepEqual(files, ["meson.build"]);
  assert.match(patch, /^\+features \+= \{'drm': false\}$/m);
  assert.match(patch, /^\+\s+get_option\('vaapi-drm'\)\.require\(libva\.found\(\) and drm\.found\(\)\)\)$/m);
});

test("CI builds the libmpv set from the pinned inputs and always checks it", async () => {
  const ci = await readText(path.join(repoRoot, ".github", "workflows", "ci.yml"), "utf8");
  const job = ci.match(/^  libmpv:\n(?<body>[\s\S]*?)(?=\n  [a-z][a-z0-9_-]*:\n|(?![\s\S]))/m)?.groups?.body;
  assert.ok(job, "CI must keep a libmpv job");
  assert.match(job, /^    runs-on: ubuntu-22\.04$/m);
  const key = job.match(/key: libmpv-linux-x86_64-\$\{\{ hashFiles\((?<files>[^)]*)\) \}\}/)?.groups
    ?.files;
  assert.ok(key, "the cache key must hash the inputs");
  for (const input of [
    "scripts/build-libmpv.sh",
    "packaging/libmpv/versions.txt",
    "packaging/libmpv/Containerfile",
    "packaging/libmpv/patches/**",
  ]) {
    assert.ok(key.includes(`'${input}'`), `the cache key must cover ${input}`);
  }
  assert.match(
    job,
    /- name: Build the set\n\s+if: steps\.cache\.outputs\.cache-hit != 'true'\n\s+run: scripts\/build-libmpv\.sh --container/,
  );
  // The check runs on every image, on the baseline first, cached or not.
  const check = job.match(/- name: Load and decode on clean distributions\n(?<body>[\s\S]*?)(?=\n      - |$)/)
    ?.groups?.body;
  assert.ok(check, "the load check must stay a step of its own");
  assert.doesNotMatch(check, /^\s+if:/m, "a cached set is checked too");
  const images = [...check.matchAll(/check-in-container\.sh (\S+)/g)].map((m) => m[1]);
  assert.equal(images[0], "docker.io/library/ubuntu:22.04", "the baseline itself comes first");
  assert.ok(images.length >= 3, "the baseline and at least two newer distributions");
  assert.match(job, /uses: actions\/upload-artifact@v7\n\s+with:\n\s+name: libmpv-linux-x86_64\n\s+if-no-files-found: error\n\s+path: \.libmpv\/out\/libmpv/);
});

const libmpvScriptUnavailable =
  process.platform === "linux" ? false : "the libmpv set is built on Linux only";

test("the libmpv build refuses a tarball that does not match its pin", { skip: libmpvScriptUnavailable }, async (t) => {
  const { spawnSync } = await import("node:child_process");
  const { existsSync } = await import("node:fs");
  const { chmod, mkdtemp, mkdir, rm, writeFile } = await import("node:fs/promises");
  const os = await import("node:os");
  const root = await mkdtemp(path.join(os.tmpdir(), "vela-libmpv-pin-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const lua = (await libmpvPins()).find((pin) => pin.name === "lua");

  // A WELL-FORMED archive with the pinned name and the wrong contents, laid
  // out so that the lua step would extract and compile it if the checksum
  // were not enforced. Junk bytes would not do: they fail at extraction
  // whether or not the checksum is checked.
  const tree = path.join(root, "tree", `lua-${lua.version}`, "src");
  await mkdir(tree, { recursive: true });
  await writeFile(path.join(tree, "sentinel.c"), "int vela_sentinel(void) { return 1; }\n");
  for (const header of ["lua.h", "luaconf.h", "lualib.h", "lauxlib.h", "lua.hpp"]) {
    await writeFile(path.join(tree, header), "/* stand-in */\n");
  }
  await mkdir(path.join(root, "src"), { recursive: true });
  const archive = path.join(root, "src", `lua-${lua.version}.tar.gz`);
  const packed = spawnSync("tar", ["-czf", archive, "-C", path.join(root, "tree"), `lua-${lua.version}`]);
  assert.equal(packed.status, 0, "the stand-in archive must be a real one");

  // A compiler that leaves a mark if it is ever reached.
  const bin = path.join(root, "bin");
  await mkdir(bin);
  const reached = path.join(root, "compiler-was-run");
  await writeFile(path.join(bin, "gcc"), `#!/bin/sh\necho reached > "${reached}"\nexit 1\n`);
  await chmod(path.join(bin, "gcc"), 0o755);

  const run = spawnSync("bash", [libmpvScriptPath, "lua"], {
    env: { ...process.env, LIBMPV_ROOT: root, PATH: `${bin}:${process.env.PATH}` },
    encoding: "utf8",
  });
  assert.equal(run.status, 1, "a mismatched tarball must stop the build");
  assert.match(run.stderr, /build-libmpv: checksum mismatch: .*lua-/);
  // It must stop AT the mismatch: nothing after it, nothing extracted,
  // nothing compiled.
  assert.match(
    run.stderr.trimEnd().split("\n").at(-1),
    /^build-libmpv: checksum mismatch: /,
    `nothing may run after the mismatch:\n${run.stderr}`,
  );
  assert.ok(!existsSync(path.join(root, "work", "src", "lua")), "the archive must not be extracted");
  assert.ok(!existsSync(reached), "the compiler must not be reached");
});
test("the libmpv build falls back to a mirror and keeps only a download that matches", { skip: libmpvScriptUnavailable }, async (t) => {
  const { spawnSync } = await import("node:child_process");
  const { createHash } = await import("node:crypto");
  const { copyFile, mkdtemp, mkdir, readFile: readBytes, rm, writeFile } = await import("node:fs/promises");
  const os = await import("node:os");
  // A scratch copy of the repo layout the script expects, with its own pins:
  // one component whose first source is missing, whose second serves the wrong
  // bytes, and whose third is right.
  const root = await mkdtemp(path.join(os.tmpdir(), "vela-libmpv-mirror-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(path.join(root, "scripts"), { recursive: true });
  await mkdir(path.join(root, "packaging", "libmpv"), { recursive: true });
  await copyFile(libmpvScriptPath, path.join(root, "scripts", "build-libmpv.sh"));
  const good = path.join(root, "good-lua-9.9.9.tar.gz");
  const bad = path.join(root, "bad-lua-9.9.9.tar.gz");
  await writeFile(good, "the pinned bytes\n");
  await writeFile(bad, "other bytes\n");
  const sha = createHash("sha256").update(await readBytes(good)).digest("hex");
  await writeFile(
    path.join(root, "packaging", "libmpv", "versions.txt"),
    `lua 9.9.9 ship ${sha} file://${root}/missing-lua-9.9.9.tar.gz file://${bad} file://${good}\n`,
  );
  const run = spawnSync("bash", [path.join(root, "scripts", "build-libmpv.sh"), "lua"], {
    env: { ...process.env, LIBMPV_ROOT: path.join(root, "out"), LIBMPV_DOWNLOAD_RETRIES: "0" },
    encoding: "utf8",
  });
  // The stand-in is not a tarball, so the step fails after the download; what
  // matters is which bytes were kept.
  assert.notEqual(run.status, 0);
  assert.match(run.stderr, /no good download from file:\/\/\S+missing-lua-9\.9\.9\.tar\.gz/);
  assert.match(run.stderr, /no good download from file:\/\/\S+bad-lua-9\.9\.9\.tar\.gz/);
  assert.doesNotMatch(run.stderr, /could not download 'lua'|checksum mismatch/);
  assert.equal(
    await readBytes(path.join(root, "out", "src", "lua-9.9.9.tar.gz"), "utf8"),
    "the pinned bytes\n",
    "only the matching download may be kept",
  );
});

test("the libmpv build stops when no source has the pinned bytes", { skip: libmpvScriptUnavailable }, async (t) => {
  const { spawnSync } = await import("node:child_process");
  const { copyFile, mkdtemp, mkdir, rm, writeFile } = await import("node:fs/promises");
  const { existsSync } = await import("node:fs");
  const os = await import("node:os");
  const root = await mkdtemp(path.join(os.tmpdir(), "vela-libmpv-nosource-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(path.join(root, "scripts"), { recursive: true });
  await mkdir(path.join(root, "packaging", "libmpv"), { recursive: true });
  await copyFile(libmpvScriptPath, path.join(root, "scripts", "build-libmpv.sh"));
  const bad = path.join(root, "bad-lua-9.9.9.tar.gz");
  await writeFile(bad, "other bytes\n");
  await writeFile(
    path.join(root, "packaging", "libmpv", "versions.txt"),
    `lua 9.9.9 ship ${"0".repeat(64)} file://${root}/missing-lua-9.9.9.tar.gz file://${bad}\n`,
  );
  const run = spawnSync("bash", [path.join(root, "scripts", "build-libmpv.sh"), "lua"], {
    env: { ...process.env, LIBMPV_ROOT: path.join(root, "out"), LIBMPV_DOWNLOAD_RETRIES: "0" },
    encoding: "utf8",
  });
  assert.equal(run.status, 1);
  assert.equal(
    run.stderr.trimEnd().split("\n").at(-1),
    "build-libmpv: could not download 'lua' from any of its sources",
    `the build must stop there:\n${run.stderr}`,
  );
  assert.ok(!existsSync(path.join(root, "out", "src", "lua-9.9.9.tar.gz")), "nothing is kept");
  assert.ok(!existsSync(path.join(root, "out", "src", "lua-9.9.9.tar.gz.part")), "no partial file");
});

test("the libmpv build names an unknown step instead of ignoring it", { skip: libmpvScriptUnavailable }, async (t) => {
  const { spawnSync } = await import("node:child_process");
  const { mkdtemp, rm } = await import("node:fs/promises");
  const os = await import("node:os");
  const root = await mkdtemp(path.join(os.tmpdir(), "vela-libmpv-step-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const run = spawnSync("bash", [libmpvScriptPath, "ffmpeg-gpl"], {
    env: { ...process.env, LIBMPV_ROOT: root },
    encoding: "utf8",
  });
  assert.equal(run.status, 1);
  assert.match(run.stderr, /build-libmpv: unknown step 'ffmpeg-gpl'/);
});

// The pure parts of check-libmpv.py, driven directly: they need no built
// library, only Python. `args` are passed to the named function as JSON.
async function libmpvCheckCall(name, ...args) {
  const { spawnSync } = await import("node:child_process");
  const code = [
    "import importlib.util, json, sys",
    "spec = importlib.util.spec_from_file_location('check_libmpv', sys.argv[1])",
    "module = importlib.util.module_from_spec(spec)",
    "spec.loader.exec_module(module)",
    "print(json.dumps(getattr(module, sys.argv[2])(*json.loads(sys.argv[3]))))",
  ].join("\n");
  const run = spawnSync(
    "python3",
    ["-B", "-c", code, path.join(libmpvRecipe, "check-libmpv.py"), name, JSON.stringify(args)],
    { encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}
const libmpvManifestProblems = (dir) => libmpvCheckCall("check_manifest", dir);

test("the libmpv integrity check covers every file of the set, licence texts included", { skip: libmpvScriptUnavailable }, async (t) => {
  const { createHash } = await import("node:crypto");
  const { appendFile, cp, mkdtemp, mkdir, readFile: readBytes, rm, writeFile } = await import("node:fs/promises");
  const os = await import("node:os");
  const root = await mkdtemp(path.join(os.tmpdir(), "vela-libmpv-manifest-"));
  t.after(() => rm(root, { recursive: true, force: true }));

  const files = {
    "libmpv.so.2": "library one\n",
    "libvela-avcodec.so.63": "library two\n",
    "licenses/mpv/LICENSE.LGPL": "lgpl\n",
    "licenses/mpv/mpv-vaapi-without-drm-output.patch": "patch\n",
    "licenses/ffmpeg/COPYING.LGPLv2.1": "lgpl\n",
  };
  const pristine = path.join(root, "pristine");
  for (const [name, body] of Object.entries(files)) {
    await mkdir(path.dirname(path.join(pristine, name)), { recursive: true });
    await writeFile(path.join(pristine, name), body);
  }
  const manifestFor = async (names) => {
    const lines = ["# test set", "arch x86_64", "component mpv 0.0.0", "component ffmpeg 0.0.0"];
    // The roles the build script writes: which listed files are a
    // component's licence text, and which is the build patch.
    for (const name of names) {
      const [, component, file] = name.match(/^licenses\/([^/]+)\/(.+)$/) ?? [];
      if (component) lines.push(`${file.endsWith(".patch") ? "patch" : "licence"} ${component} ${name}`);
    }
    for (const name of names) {
      const body = await readBytes(path.join(pristine, name));
      lines.push(`file ${createHash("sha256").update(body).digest("hex")} ${body.length} ${name}`);
    }
    return `${lines.join("\n")}\n`;
  };
  await writeFile(path.join(pristine, "MANIFEST"), await manifestFor(Object.keys(files)));

  // Each case starts from an intact copy and breaks exactly one thing.
  const variant = async (name, breakIt) => {
    const dir = path.join(root, name);
    await cp(pristine, dir, { recursive: true });
    await breakIt(dir);
    return libmpvManifestProblems(dir);
  };

  assert.deepEqual(await variant("intact", async () => {}), []);
  assert.deepEqual(
    await variant("licence-removed", (dir) => rm(path.join(dir, "licenses/ffmpeg/COPYING.LGPLv2.1"))),
    ["missing: licenses/ffmpeg/COPYING.LGPLv2.1"],
  );
  assert.deepEqual(
    await variant("licences-gone", (dir) => rm(path.join(dir, "licenses"), { recursive: true })),
    [
      "missing: licenses/ffmpeg/COPYING.LGPLv2.1",
      "missing: licenses/mpv/LICENSE.LGPL",
      "missing: licenses/mpv/mpv-vaapi-without-drm-output.patch",
    ],
  );
  assert.deepEqual(
    await variant("patch-removed", (dir) =>
      rm(path.join(dir, "licenses/mpv/mpv-vaapi-without-drm-output.patch")),
    ),
    ["missing: licenses/mpv/mpv-vaapi-without-drm-output.patch"],
  );
  assert.deepEqual(
    await variant("licence-edited", (dir) => appendFile(path.join(dir, "licenses/mpv/LICENSE.LGPL"), "x")),
    ["modified: licenses/mpv/LICENSE.LGPL"],
  );
  assert.deepEqual(
    await variant("library-removed", (dir) => rm(path.join(dir, "libvela-avcodec.so.63"))),
    ["missing: libvela-avcodec.so.63"],
  );
  assert.deepEqual(
    await variant("stray-licence", (dir) => writeFile(path.join(dir, "licenses/mpv/EXTRA"), "?")),
    ["not in MANIFEST: licenses/mpv/EXTRA"],
  );
  assert.deepEqual(
    await variant("stray-library", (dir) => writeFile(path.join(dir, "libextra.so.1"), "?")),
    ["not in MANIFEST: libextra.so.1"],
  );
  // A manifest that simply never listed a component's licence is as bad as a
  // set that lost it.
  assert.deepEqual(
    await variant("licence-never-listed", async (dir) => {
      await rm(path.join(dir, "licenses/ffmpeg"), { recursive: true });
      await writeFile(
        path.join(dir, "MANIFEST"),
        await manifestFor(Object.keys(files).filter((name) => !name.startsWith("licenses/ffmpeg/"))),
      );
    }),
    ["no licence text listed for component: ffmpeg"],
  );
  assert.deepEqual(
    await variant("entry-outside", (dir) =>
      appendFile(path.join(dir, "MANIFEST"), `file ${"0".repeat(64)} 1 ../outside\n`),
    ),
    ["MANIFEST entry leaves the set: ../outside"],
  );
  assert.deepEqual(await variant("no-manifest", (dir) => rm(path.join(dir, "MANIFEST"))).then((p) => p.length), 1);

  // Review round 1 on lm-2. A component's build patch is not its licence: a
  // set that keeps mpv's patch but lost its licence text, manifest entries
  // and all, is missing a licence.
  assert.deepEqual(
    await variant("only-the-patch-left", async (dir) => {
      await rm(path.join(dir, "licenses/mpv/LICENSE.LGPL"));
      await writeFile(
        path.join(dir, "MANIFEST"),
        await manifestFor(Object.keys(files).filter((name) => name !== "licenses/mpv/LICENSE.LGPL")),
      );
    }),
    ["no licence text listed for component: mpv"],
  );
  // A licence line must point at a file the manifest lists.
  assert.deepEqual(
    await variant("licence-names-no-file", (dir) =>
      appendFile(path.join(dir, "MANIFEST"), "licence ffmpeg licenses/ffmpeg/NOT-THERE\n"),
    ),
    ["licence of ffmpeg is not a listed file: licenses/ffmpeg/NOT-THERE"],
  );
  // Nothing in the set may be a symbolic link: files reached through one are
  // not in the set, whatever they contain.
  const { rename, symlink } = await import("node:fs/promises");
  assert.deepEqual(
    await variant("licences-symlinked-away", async (dir) => {
      await rename(path.join(dir, "licenses"), path.join(root, "licences-elsewhere"));
      await symlink(path.join(root, "licences-elsewhere"), path.join(dir, "licenses"));
    }),
    [
      "reached through a symbolic link: licenses/ffmpeg/COPYING.LGPLv2.1",
      "reached through a symbolic link: licenses/mpv/LICENSE.LGPL",
      "reached through a symbolic link: licenses/mpv/mpv-vaapi-without-drm-output.patch",
      "symbolic link in the set: licenses",
    ],
  );
  assert.deepEqual(
    await variant("library-symlinked-away", async (dir) => {
      await rename(path.join(dir, "libvela-avcodec.so.63"), path.join(root, "library-elsewhere"));
      await symlink(path.join(root, "library-elsewhere"), path.join(dir, "libvela-avcodec.so.63"));
    }),
    [
      "reached through a symbolic link: libvela-avcodec.so.63",
      "symbolic link in the set: libvela-avcodec.so.63",
    ],
  );
});

test("the libmpv decode check compares what was decoded with what was recorded", { skip: libmpvScriptUnavailable }, async () => {
  const expected = {
    video_frames: 12,
    frame_bytes: 21600,
    video_sha256: "a".repeat(64),
    audio_samples: 49152,
    audio_level: 2716.7,
  };
  const decoded = { ...expected, video_ended: "eof", audio_ended: "eof" };
  const judge = (changes) => libmpvCheckCall("judge_decode", expected, { ...decoded, ...changes });

  assert.deepEqual(await judge({}), []);
  // Audio decoders are not bit-exact: a level a few percent off still passes.
  assert.deepEqual(await judge({ audio_level: 2716.7 * 1.03, audio_samples: 49152 + 400 }), []);

  // The reviewer's two cases: one track produced nothing while the file
  // still played to its end on the other.
  assert.deepEqual(await judge({ video_frames: 0, frame_bytes: null, video_sha256: null }), [
    "video: 0 frames decoded, 12 recorded",
  ]);
  assert.deepEqual(await judge({ audio_samples: 0, audio_level: 0 }), [
    "audio: 0 samples decoded, 49152 recorded",
  ]);
  // Frames that came out, but wrong.
  assert.deepEqual(await judge({ video_sha256: "b".repeat(64) }), [
    "video: decoded frames differ from the recorded ones",
  ]);
  assert.deepEqual(await judge({ frame_bytes: 43200 }), [
    "video: frames of 43200 bytes, 21600 recorded",
  ]);
  assert.deepEqual(await judge({ video_frames: 11 }), ["video: 11 frames decoded, 12 recorded"]);
  // Sound of the right length that is silence, or far too quiet or loud.
  assert.deepEqual(await judge({ audio_level: 0 }), ["audio: level 0.0, 2716.7 recorded"]);
  assert.deepEqual(await judge({ audio_level: 2716.7 * 1.2 }), ["audio: level 3260.0, 2716.7 recorded"]);
  // A track whose decoding never finished.
  assert.deepEqual(await judge({ video_ended: "reason 4" }), [
    "video: decoding did not reach end of file (reason 4)",
  ]);
  assert.deepEqual(await judge({ audio_ended: "no end within the time allowed" }), [
    "audio: decoding did not reach end of file (no end within the time allowed)",
  ]);

  // A deliberately damaged clip must fail on the track that was damaged; one
  // that passes, or fails only on the other track, means the check went blind.
  assert.equal(await libmpvCheckCall("judge_broken", "video", ["video: 0 frames decoded, 12 recorded"]), null);
  assert.match(await libmpvCheckCall("judge_broken", "video", []), /was not noticed/);
  assert.match(
    await libmpvCheckCall("judge_broken", "audio", ["video: 0 frames decoded, 12 recorded"]),
    /the audio of a damaged clip was not noticed/,
  );
});

test("every libmpv fixture clip has a recorded result, and both tracks have a damaged clip", async () => {
  const { readdir } = await import("node:fs/promises");
  const fixtures = path.join(libmpvRecipe, "fixtures");
  const recorded = JSON.parse(await readText(path.join(fixtures, "expected.json"), "utf8"));
  const clips = (await readdir(fixtures)).filter((name) => /\.(mp4|mkv)$/.test(name)).sort();
  assert.ok(clips.length >= 3);
  assert.deepEqual(Object.keys(recorded.clips).sort(), clips, "one recorded result per clip");
  for (const [name, result] of Object.entries(recorded.clips)) {
    assert.ok(result.video_frames > 0, `${name}: video frames`);
    assert.ok(result.frame_bytes > 0, `${name}: frame size`);
    assert.match(result.video_sha256, /^[0-9a-f]{64}$/, `${name}: frame checksum`);
    assert.ok(result.audio_samples > 0 && result.audio_level > 100, `${name}: audible audio`);
  }
  const broken = (await readdir(path.join(fixtures, "broken"))).sort();
  assert.deepEqual(Object.keys(recorded.broken).sort(), broken, "one entry per damaged clip");
  for (const [name, entry] of Object.entries(recorded.broken)) {
    assert.ok(recorded.clips[entry.of], `${name}: made from a recorded clip`);
    assert.match(entry.must_fail, /^(video|audio)$/, `${name}: which track was damaged`);
  }
  assert.deepEqual(
    [...new Set(Object.values(recorded.broken).map((entry) => entry.must_fail))].sort(),
    ["audio", "video"],
    "the check is proved able to fail on each track",
  );

  // The container check hands the damaged clips to the check as damaged,
  // every time it runs.
  const runner = await readText(path.join(libmpvRecipe, "check-in-container.sh"), "utf8");
  assert.match(
    runner,
    /for clip in \/recipe\/fixtures\/broken\/\*; do broken=\\"\\\$broken --broken \\\$clip\\"; done/,
  );
  assert.match(runner, /\\\$broken \/recipe\/fixtures\/\*\.mp4 \/recipe\/fixtures\/\*\.mkv/);
});

// Windows cross-build and package checks.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (name) => fs.readFileSync(path.join(root, name), 'utf8').replace(/\r\n/g, '\n');
const scriptPath = path.join(root, 'scripts/build-libmpv-windows.sh');
const script = read('scripts/build-libmpv-windows.sh');
const checkerPath = path.join(root, 'packaging/libmpv/check-libmpv-windows.py');
const checker = read('packaging/libmpv/check-libmpv-windows.py');
const container = read('packaging/libmpv/Containerfile.windows');
const notes = read('packaging/libmpv/windows-license-notes.txt');
const ci = read('.github/workflows/ci.yml');
const pins = read('packaging/libmpv/versions.txt').split('\n')
  .filter((line) => line.trim() && !line.startsWith('#')).map((line) => line.split(/\s+/));
const body = (text, name) => text.match(new RegExp(`^${name}\\(\\) \\{\\n([\\s\\S]*?)\\n\\}`, 'm'))?.[1] ?? '';
const job = (text, name) => text.match(new RegExp(`^  ${name}:\\n([\\s\\S]*?)(?=\\n  [a-z][a-z0-9_-]*:\\n|(?![\\s\\S]))`, 'm'))?.[1] ?? '';
const noBash = process.platform === 'win32' ? 'cross build shell runs on Linux' : false;
const python = process.platform === 'win32' ? 'python' : 'python3';

// Check the actual recipe section, then delete each required setting and
// prove this same guard fails. A missing token cannot silently pass a mutation.
function requiredTokens(label, source, tokens) {
  for (const token of tokens) {
    test(`Windows ${label}: ${token} (green and red)`, () => {
      const guard = (candidate) => assert.ok(candidate.includes(token), `${label}: missing ${token}`);
      guard(source);
      assert.throws(() => guard(source.replaceAll(token, 'REMOVED_FOR_RED_PROOF')), /missing/);
    });
  }
}

requiredTokens('cross isolation', script, [
  'shopt -s inherit_errexit', 'unset PKG_CONFIG_PATH PKG_CONFIG_SYSROOT_DIR',
  'unset CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH LIBRARY_PATH CPPFLAGS',
  'export PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig"',
  'export CC_FOR_BUILD=gcc CXX_FOR_BUILD=g++ CFLAGS_FOR_BUILD= CXXFLAGS_FOR_BUILD= LDFLAGS_FOR_BUILD=',
  "system = 'windows'", "cpu_family = 'x86_64'", 'needs_exe_wrapper = true',
  "[properties]\nneeds_exe_wrapper = true\npkg_config_libdir = ['$PREFIX/lib/pkgconfig']",
  'set(CMAKE_SYSTEM_NAME Windows)', 'set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)',
  'set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)', 'set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)',
  'set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)', '--cross-file "$WORK/windows.ini"',
  '-DCMAKE_TOOLCHAIN_FILE="$WORK/windows.cmake"', '--wrap-mode=nodownload',
  '-DFETCHCONTENT_FULLY_DISCONNECTED=ON', '-Dauto_features=disabled', '-Ddefault_library=shared',
  '-static-libgcc -static-libstdc++',
  'install -m644 "$winpthread" "$PREFIX/lib/libwinpthread.a"',
  'install -m644 "$winpthread" "$PREFIX/lib/libpthread.a"',
  'stdcxx="$("$CXX" -print-file-name=libstdc++.a)"',
  'sha256sum -c --quiet - >&2 || die "checksum mismatch:',
]);
requiredTokens('container', container, [
  'FROM docker.io/library/debian:trixie-slim', 'gcc-mingw-w64-x86-64-posix',
  'g++-mingw-w64-x86-64-posix', 'mingw-w64-x86-64-dev', 'binutils-mingw-w64-x86-64',
  'pkg-config nasm ninja-build cmake meson',
  'curl ca-certificates patch xz-utils file',
]);
const configuration = {
  zlib: ['-DZLIB_BUILD_SHARED=OFF', '-DZLIB_BUILD_STATIC=ON', '-DZLIB_BUILD_TESTING=OFF', '"$PREFIX/lib/libzs.a" "$PREFIX/lib/libz.a"'],
  libiconv: ['--host="$TARGET"', '--enable-static --disable-shared --disable-nls --disable-rpath', 'make -C libcharset', 'make -C libcharset install-lib libdir="$PWD/lib" includedir="$PWD/lib"', 'make -C lib ', 'install -Dm644 include/iconv.h.inst "$PREFIX/include/iconv.h"'],
  freetype: ['-Dzlib=system', '-Dharfbuzz=disabled', '-Dmmap=disabled'],
  fribidi: ['-Ddocs=false -Dbin=false -Dtests=false'],
  harfbuzz: ['-Dfreetype=enabled', '-Dtests=disabled -Dutilities=disabled', '-Dsubset=disabled -Draster=disabled -Dvector=disabled -Dgpu=disabled'],
  libass: ['-Ddirectwrite=enabled -Dfontconfig=disabled -Dasm=enabled'],
  dav1d: ['-Denable_tools=false -Denable_tests=false -Denable_examples=false'],
  lua: ['-DLUA_COMPAT_ALL -DLUA_BUILD_AS_DLL', '-Wl,--out-implib,liblua5.2.dll.a', 'lua52.dll', 'lua5.2.pc'],
  uchardet: ['-DBUILD_BINARY=OFF -DBUILD_STATIC=OFF -DBUILD_SHARED_LIBS=ON'],
  shaderc: ['-DBUILD_SHARED_LIBS=OFF', '-DSHADERC_GLSLANG_DIR="$glslang"', '-DSHADERC_SPIRV_TOOLS_DIR="$tools"',
    '-DSHADERC_SPIRV_HEADERS_DIR="$headers"', '-DSPIRV-Headers_SOURCE_DIR="$headers"',
    '-DSHADERC_SKIP_INSTALL=OFF -DSHADERC_SKIP_TESTS=ON -DSHADERC_SKIP_EXAMPLES=ON',
    '-DSHADERC_SKIP_EXECUTABLES=ON', '-DSHADERC_ENABLE_WGSL_OUTPUT=OFF',
    '-DSHADERC_ENABLE_HLSL=OFF -DENABLE_HLSL=OFF -DENABLE_GLSLANG_BINARIES=OFF',
    '-DGLSLANG_TESTS=OFF -DENABLE_CTEST=OFF -DBUILD_EXTERNAL=OFF -DENABLE_OPT=ON',
    '-DSPIRV_SKIP_TESTS=ON -DSPIRV_SKIP_EXECUTABLES=ON', '-DSPIRV_TOOLS_BUILD_STATIC=ON',
    '--target shaderc_combined', '-lshaderc_combined', 'shaderc.pc'],
  spirv_cross: ['-DSPIRV_CROSS_STATIC=ON -DSPIRV_CROSS_SHARED=OFF',
    '-DSPIRV_CROSS_CLI=OFF -DSPIRV_CROSS_ENABLE_TESTS=OFF',
    '-DSPIRV_CROSS_ENABLE_GLSL=ON -DSPIRV_CROSS_ENABLE_HLSL=ON',
    '-DSPIRV_CROSS_ENABLE_MSL=OFF -DSPIRV_CROSS_ENABLE_CPP=OFF',
    '-DSPIRV_CROSS_ENABLE_REFLECT=OFF -DSPIRV_CROSS_ENABLE_UTIL=OFF', '-DSPIRV_CROSS_ENABLE_C_API=ON',
    'spirv-cross-c-shared.pc', '$(pkg-config --modversion spirv-cross-c)',
    '-lspirv-cross-c -lspirv-cross-hlsl -lspirv-cross-glsl -lspirv-cross-core'],
  ffmpeg: ['--target-os=mingw32 --arch=x86_64 --enable-cross-compile', '--cc="$CC" --cxx="$CXX"',
    'patch --batch --forward --fuzz=0 -d "$src" -p1 < "$recipe/windows/ffmpeg-schannel-revocation.patch"',
    '|| die "could not apply FFmpeg\'s Schannel revocation patch"',
    '--enable-shared --disable-static', '--disable-autodetect', '--disable-gpl --disable-nonfree --disable-version3',
    '--enable-zlib --enable-iconv --enable-schannel --enable-libdav1d', '--enable-d3d11va --enable-dxva2',
    '--enable-w32threads --disable-pthreads', '--disable-indevs --disable-outdevs --enable-indev=lavfi',
    'grep -qx \'#define FFMPEG_LICENSE "LGPL version 2.1 or later"\' config.h',
    'grep -qx "#define CONFIG_$feature 1" config.h || die',
    "grep -qx '#define CONFIG_LAVFI_INDEV 1' config_components.h || die"],
  libplacebo: ['-Dd3d11=enabled -Dshaderc=enabled -Dglslang=disabled', '-Dvulkan=disabled -Dopengl=disabled',
    '-Ddemos=false -Dtests=false -Dbench=false -Dfuzz=false'],
  mpv: ['-Dgpl=false -Dlibmpv=true -Dcplayer=false', '-Dbuild-date=false',
    '-Dlua=lua5.2 -Diconv=enabled -Dzlib=enabled -Duchardet=enabled -Dvector=enabled',
    '-Dd3d11=enabled -Dshaderc=enabled -Dspirv-cross=enabled',
    '-Dd3d-hwaccel=enabled -Dd3d9-hwaccel=enabled -Dwasapi=enabled',
    '-Dwin32-threads=enabled -Dlibavdevice=enabled'],
};
for (const [name, tokens] of Object.entries(configuration)) requiredTokens(name, body(script, `build_${name}`), tokens);
requiredTokens('collector', body(script, 'collect'), [
  'queue="libmpv-2.dll"', 'needed "$OUT/$lib"', 'allowed_system "$dep"',
  'missing or ambiguous DLL:', "file format pei-x86-64", '"$STRIP" --strip-unneeded',
  'check_closure', 'die "no licence text collected for component',
  "find . -type f ! -name MANIFEST -printf '%P\\n'", '"$(sha256sum "$f"', '"$(stat -c %s "$f")"',
  '"licence $name licenses/$name/$f"', '"$recipe/windows-license-notes.txt"',
  'lic spirv-cross LICENSE spirv.h spirv.hpp GLSL.std.450.h',
  'install -m644 "$recipe/windows/ffmpeg-schannel-revocation.patch" "$OUT/licenses/ffmpeg/"',
  '"patch ffmpeg licenses/ffmpeg/ffmpeg-schannel-revocation.patch"',
]);
requiredTokens('licence notes', notes, ['LGPL-3.0-or-later', 'Apache-2.0', 'Runtime Library Exception', 'modified libiconv']);

test('Windows Schannel patch changes only the two revocation availability flags when verifying (green and red)', () => {
  const patch = read('packaging/libmpv/windows/ffmpeg-schannel-revocation.patch');
  const guard = (candidate) => {
    assert.deepEqual([...candidate.matchAll(/^\+\+\+ b\/(.+)$/gm)].map((m) => m[1]),
      ['libavformat/tls_schannel.c'], 'only the Schannel source');
    assert.equal([...candidate.matchAll(/^@@ /gm)].length, 1, 'only one hunk');
    const hunk = candidate.slice(candidate.indexOf('\n@@ ')).split('\n').slice(2).filter(Boolean);
    assert.deepEqual(hunk.filter((line) => line.startsWith('-')).map((line) => line.trim()),
      ['-                                    SCH_CRED_REVOCATION_CHECK_CHAIN;'], 'only the old chain flag line');
    assert.deepEqual(hunk.filter((line) => line.startsWith('+')).map((line) => line.slice(1).trim()), [
      'SCH_CRED_REVOCATION_CHECK_CHAIN |',
      'SCH_CRED_IGNORE_NO_REVOCATION_CHECK |',
      'SCH_CRED_IGNORE_REVOCATION_OFFLINE;',
    ], 'only two availability flags added');
    const after = hunk.filter((line) => !line.startsWith('-')).map((line) => line.slice(1)).join('\n');
    assert.match(after, /if \(s->verify\)\s+schannel_cred\.dwFlags = SCH_CRED_AUTO_CRED_VALIDATION \|\s+SCH_CRED_REVOCATION_CHECK_CHAIN \|\s+SCH_CRED_IGNORE_NO_REVOCATION_CHECK \|\s+SCH_CRED_IGNORE_REVOCATION_OFFLINE;\s+else\s+schannel_cred\.dwFlags = SCH_CRED_MANUAL_CRED_VALIDATION \|/,
      'flags belong only to the verifying branch; automatic validation stays on');
  };
  guard(patch);
  for (const token of ['SCH_CRED_IGNORE_NO_REVOCATION_CHECK', 'SCH_CRED_IGNORE_REVOCATION_OFFLINE',
    'SCH_CRED_REVOCATION_CHECK_CHAIN', 'SCH_CRED_AUTO_CRED_VALIDATION', 'if (s->verify)']) {
    assert.throws(() => guard(patch.replace(token, 'REMOVED')), /flag|validation/);
  }
  assert.throws(() => guard(patch + '\n--- a/other.c\n+++ b/other.c\n'), /only the Schannel source/);
});

test('Windows applies the Schannel patch after verified unpack and before configuring FFmpeg (green and red)', () => {
  const guard = (candidate) => {
    const ffmpeg = body(candidate, 'build_ffmpeg');
    const unpack = ffmpeg.indexOf('src="$(unpack ffmpeg)"');
    const apply = ffmpeg.indexOf('patch --batch --forward --fuzz=0');
    const configure = ffmpeg.indexOf('./configure ');
    assert.ok(unpack >= 0 && unpack < apply && apply < configure, 'unpack, patch, configure order');
  };
  guard(script);
  for (const token of ['src="$(unpack ffmpeg)"', 'patch --batch --forward --fuzz=0', './configure ']) {
    const ffmpeg = body(script, 'build_ffmpeg');
    assert.throws(() => guard(script.replace(ffmpeg, ffmpeg.replace(token, 'REMOVED'))), /order/);
  }
});

test('Windows source graph uses every pin, exclusively, with licences (green and red)', () => {
  // The macOS set's own build inputs (role build-macos) are not Windows'.
  const windowsPins = pins.filter((p) => p[2] !== 'build-macos');
  const guard = (candidate, entries = windowsPins) => {
    const fetched = [...candidate.matchAll(/"\$\((?:unpack|fetch) ([a-z0-9-]+)\)"/g)].map((m) => m[1]);
    assert.deepEqual([...new Set(fetched)].sort(), entries.map((p) => p[0]).sort(), 'source graph equals pins');
    assert.equal((candidate.match(/\bcurl\b/g) ?? []).length, 1, 'one download boundary');
    assert.doesNotMatch(candidate, /\b(?:wget|git clone|pip3? install)\b/, 'no unpinned download');
    const steps = candidate.match(/^steps="\$\{\*:-(.*?)\}"$/m)?.[1].split(' ');
    assert.deepEqual(steps, [...Object.keys(configuration), 'collect'], 'all builders run in dependency order');
    for (const [name, , role, sha] of entries) {
      assert.match(sha, /^[0-9a-f]{64}$/, `${name}: checksum`);
      assert.match(role, /^(ship|build)$/, `${name}: role`);
      assert.ok(body(candidate, 'collect').includes(`lic ${name} `) || (name === 'lua' && candidate.includes('licence lua licenses/lua/LICENSE')), `${name}: licence`);
    }
  };
  guard(script);
  for (const [name] of windowsPins) {
    assert.throws(() => guard(script.replaceAll(`$(unpack ${name})`, '$(unpack UNPINNED)')), /source graph/);
    const changed = script.replaceAll(`lic ${name} `, 'lic MISSING ')
      .replaceAll(`licence ${name} licenses/${name}/LICENSE`, 'REMOVED');
    assert.throws(() => guard(changed), /licence/);
    assert.throws(() => guard(script, windowsPins.map((pin) => pin[0] === name ? [pin[0], pin[1], pin[2], 'bad'] : pin)), /checksum/);
    assert.throws(() => guard(script, windowsPins.map((pin) => pin[0] === name ? [pin[0], pin[1], 'bad', pin[3]] : pin)), /role/);
  }
  assert.throws(() => guard(script + '\ncurl https://unverified.invalid\n'), /download boundary/);
  assert.throws(() => guard(script + '\nwget https://unverified.invalid\n'), /unpinned download/);
  assert.throws(() => guard(script.replace('lua uchardet shaderc', 'lua shaderc')), /dependency order/);
});

test('Windows rejects GPL configuration and runtime DLL exemptions (green and red)', () => {
  const licence = (candidate) => {
    assert.doesNotMatch(body(candidate, 'build_mpv'), /-Dgpl=true/, 'mpv GPL');
    assert.doesNotMatch(body(candidate, 'build_ffmpeg'), /--enable-(gpl|nonfree|version3)/, 'FFmpeg LGPL');
  };
  licence(script);
  assert.throws(() => licence(script.replace('-Dgpl=false', '-Dgpl=true')), /mpv GPL/);
  for (const flag of ['gpl', 'nonfree', 'version3']) {
    assert.throws(() => licence(script.replace(`--disable-${flag}`, `--enable-${flag}`)), /FFmpeg LGPL/);
  }
  const runtime = (candidate) => {
    const allowed = candidate.match(/^ALLOWED_SYSTEM="([\s\S]*?)"$/m)?.[1];
    assert.ok(allowed, 'system allowlist');
    assert.doesNotMatch(allowed, /libgcc|libstdc\+\+|libwinpthread|libssp|shaderc|spirv|libmpv/i, 'runtime is not a system DLL');
  };
  runtime(script);
  for (const dll of ['libgcc_s_seh-1.dll', 'libstdc++-6.dll', 'libwinpthread-1.dll', 'libssp-0.dll']) {
    assert.throws(() => runtime(script.replace('  advapi32.dll', `  ${dll} advapi32.dll`)), /not a system DLL/);
  }
});

const buildJob = job(ci, 'libmpv-windows');
const checkJob = job(ci, 'libmpv-windows-check');
requiredTokens('CI cross build', buildJob, [
  'runs-on: ubuntu-22.04', 'path: .libmpv/out/libmpv-windows-x86_64',
  "key: libmpv-windows-x86_64-${{ hashFiles(", "'scripts/build-libmpv-windows.sh'",
  "'packaging/libmpv/versions.txt'", "'packaging/libmpv/Containerfile.windows'",
  "'packaging/libmpv/windows-license-notes.txt'", "'packaging/libmpv/windows-pe.py'", "if: steps.cache.outputs.cache-hit != 'true'",
  "'packaging/libmpv/windows/**'",
  'run: bash scripts/build-libmpv-windows.sh --container',
  'run: python3 -B packaging/libmpv/check-libmpv-windows.py .libmpv/out/libmpv-windows-x86_64 --manifest-only',
  'uses: actions/upload-artifact@v7', 'name: libmpv-windows-x86_64', 'if-no-files-found: error',
]);
requiredTokens('CI Windows check', checkJob, [
  'needs: libmpv-windows', 'runs-on: windows-latest', 'architecture: x64',
  'uses: actions/download-artifact@v8', 'name: libmpv-windows-x86_64',
  'path: .libmpv/out/libmpv-windows-x86_64',
  'run: python -B packaging/libmpv/check-libmpv-windows.py .libmpv/out/libmpv-windows-x86_64',
]);
test('Windows checks cached sets too (green and red)', () => {
  const guard = (candidate) => assert.doesNotMatch(candidate, /^\s+if:/m, 'no condition on check');
  guard(checkJob);
  guard(buildJob.split('- name: Check the manifest')[1]);
  assert.throws(() => guard(checkJob + '\n    if: false\n'), /no condition/);
});
requiredTokens('checker wiring', checker, [
  'problems = check_manifest(directory, args.pins)', 'skip load: manifest check failed',
  'with os.add_dll_directory(str(directory)):',
  'ctypes.CDLL(str(directory / "libmpv-2.dll"), winmode=0x00000100 | 0x00000800)',
  'kernel.SetDefaultDllDirectories(0x00000800 | 0x00000400)',
  'kernel.K32EnumProcessModules(kernel.GetCurrentProcess()', 'kernel.GetModuleFileNameW(handle',
  'snapshots.update(loaded_modules(kernel))', 'check_loaded(directory, files, snapshots, require_mpv, baseline, system_root)',
  'lib.mpv_client_api_version()', '("vo", "null"), ("ao", "null")',
  'b"av://lavfi:testsrc2=duration=2"', 'end.reason != 0 or end.error != 0 or not loaded',
  'time.monotonic() + 25', 'timeout=60, check=False', 'lib.mpv_terminate_destroy(handle)',
]);

function callPython(name, ...args) {
  const run = spawnSync(python, ['-B', '-c', [
    'import importlib.util, json, sys',
    "spec = importlib.util.spec_from_file_location('windows_check', sys.argv[1])",
    'module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)',
    'print(json.dumps(getattr(module, sys.argv[2])(*json.loads(sys.argv[3]))))',
  ].join('\n'), checkerPath, name, JSON.stringify(args)], { encoding: 'utf8' });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

test('Windows module audit accepts case-insensitive paths and rejects foreign/unlisted DLLs', () => {
  const set = 'C:\\set';
  const names = ['libmpv-2.dll', 'avcodec-63.dll'];
  const modules = ['c:\\SET\\LIBMPV-2.DLL', 'C:\\set\\avcodec-63.dll', 'C:\\Windows\\System32\\kernel32.dll'];
  const check = (paths, required = true) => callPython('check_loaded', set, names, paths, required);
  assert.deepEqual(check(modules), []);
  assert.match(check([...modules, 'D:\\elsewhere\\AVCODEC-63.DLL']).join(), /loaded from elsewhere/);
  assert.match(check([...modules, 'C:\\set\\unlisted.dll']).join(), /loaded unlisted/);
  assert.match(check([...modules, 'C:\\set\\nested\\avcodec-63.dll']).join(), /loaded unlisted|elsewhere/);
  assert.match(check([...modules, 'C:\\set-other\\unrelated.dll']).join(), /new module.*c:\\set-other\\unrelated.dll/);
  assert.match(check(modules.slice(1)).join(), /not mapped from the set/);
  assert.deepEqual(check([], false), []);
});

test('Windows PE dependency closure and runtime audit reject real missing dependencies', () => {
  const run = spawnSync(python, ['-I', '-B', 'tests/libmpv-windows-dependencies.py', '-v'], {
    cwd: root, encoding: 'utf8', timeout: 30000,
  });
  assert.equal(run.status, 0, `${run.error ?? ''}\n${run.stdout}\n${run.stderr}`);
});

function fixture(t) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-windows-check-'));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  const dir = path.join(tmp, 'set');
  const pinfile = path.join(tmp, 'versions.txt');
  fs.mkdirSync(dir);
  const ffmpegVersion = pins.find(([name]) => name === 'ffmpeg')[1];
  fs.writeFileSync(pinfile, `mpv 0.42.0 ship\nshaderc 2026.4 build\nffmpeg ${ffmpegVersion} ship\n`);
  const data = { 'libmpv-2.dll': 'test DLL', 'avcodec-63.dll': 'dependency',
    'licenses/ffmpeg/COPYING.LGPLv2.1': 'LGPL',
    'licenses/ffmpeg/ffmpeg-schannel-revocation.patch': read('packaging/libmpv/windows/ffmpeg-schannel-revocation.patch'),
    'licenses/mpv/LICENSE': 'LGPL', 'licenses/shaderc/LICENSE': 'Apache' };
  const lines = ['arch x86_64', 'platform windows', 'component mpv 0.42.0', 'component shaderc 2026.4',
    `component ffmpeg ${ffmpegVersion}`, 'licence ffmpeg licenses/ffmpeg/COPYING.LGPLv2.1',
    'patch ffmpeg licenses/ffmpeg/ffmpeg-schannel-revocation.patch',
    'licence mpv licenses/mpv/LICENSE', 'licence shaderc licenses/shaderc/LICENSE'];
  for (const [name, text] of Object.entries(data)) {
    fs.mkdirSync(path.dirname(path.join(dir, name)), { recursive: true });
    fs.writeFileSync(path.join(dir, name), text);
    lines.push(`file ${createHash('sha256').update(text).digest('hex')} ${Buffer.byteLength(text)} ${name}`);
  }
  const manifest = lines.join('\n') + '\n';
  const manifestPath = path.join(dir, 'MANIFEST');
  fs.writeFileSync(manifestPath, manifest);
  const problems = () => callPython('check_manifest', dir, pinfile);
  return { tmp, dir, pinfile, manifest, manifestPath, problems };
}

test('Windows manifest check fails for each broken rule before loading', async (t) => {
  const patchPath = 'licenses/ffmpeg/ffmpeg-schannel-revocation.patch';
  const patchRole = `patch ffmpeg ${patchPath}\n`;
  const changes = [
    ['missing Schannel patch', (f) => fs.unlinkSync(path.join(f.dir, patchPath)), /missing: licenses\/ffmpeg\/ffmpeg-schannel-revocation.patch/],
    ['changed Schannel patch', (f) => fs.appendFileSync(path.join(f.dir, patchPath), 'x'), /modified: licenses\/ffmpeg/],
    ['same-size changed Schannel patch', (f) => {
      const file = path.join(f.dir, patchPath);
      fs.writeFileSync(file, fs.readFileSync(file, 'utf8').replace('Vela:', 'vela:'));
    }, /modified: licenses\/ffmpeg/],
    ['Schannel patch not recorded', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace(patchRole, '')), /omits the FFmpeg Schannel revocation patch/],
    ['Schannel patch file not listed', (f) => {
      fs.unlinkSync(path.join(f.dir, patchPath));
      fs.writeFileSync(f.manifestPath, f.manifest.replace(/^file .* licenses\/ffmpeg\/ffmpeg-schannel-revocation.patch\n/m, ''));
    }, /invalid licence\/patch record/],
    ['Schannel patch entirely omitted', (f) => {
      fs.unlinkSync(path.join(f.dir, patchPath));
      fs.writeFileSync(f.manifestPath, f.manifest.replace(patchRole, '').replace(/^file .* licenses\/ffmpeg\/ffmpeg-schannel-revocation.patch\n/m, ''));
    }, /omits the FFmpeg Schannel revocation patch/],
    ['empty Schannel patch', (f) => {
      fs.writeFileSync(path.join(f.dir, patchPath), '');
      fs.writeFileSync(f.manifestPath, f.manifest.replace(/file [a-f0-9]{64} \d+ (licenses\/ffmpeg\/ffmpeg-schannel-revocation.patch)/,
        `file ${createHash('sha256').update('').digest('hex')} 0 $1`));
    }, /invalid licence\/patch record/],
    ['Schannel patch is not a licence', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('licence ffmpeg licenses/ffmpeg/COPYING.LGPLv2.1', `licence ffmpeg ${patchPath}`)), /no licence text listed for component: ffmpeg/],
    ['missing DLL', (f) => fs.unlinkSync(path.join(f.dir, 'avcodec-63.dll')), /missing/],
    ['changed DLL', (f) => fs.appendFileSync(path.join(f.dir, 'libmpv-2.dll'), 'x'), /modified/],
    ['same-size changed DLL', (f) => fs.writeFileSync(path.join(f.dir, 'libmpv-2.dll'), 'bad! DLL'), /modified/],
    ['changed licence', (f) => fs.appendFileSync(path.join(f.dir, 'licenses/shaderc/LICENSE'), 'x'), /modified/],
    ['extra DLL', (f) => fs.writeFileSync(path.join(f.dir, 'extra.dll'), 'x'), /not in MANIFEST/],
    ['missing licence file', (f) => fs.unlinkSync(path.join(f.dir, 'licenses/shaderc/LICENSE')), /missing/],
    ['licence not recorded', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('licence shaderc licenses/shaderc/LICENSE\n', '')), /no licence/],
    ['wrong version', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('component mpv 0.42.0', 'component mpv 0.41.0')), /versions differ/],
    ['wrong architecture', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('arch x86_64', 'arch aarch64')), /must name Windows x86_64/],
    ['wrong platform', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('platform windows', 'platform linux')), /must name Windows x86_64/],
    ['missing primary DLL record', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace(/^file .* libmpv-2.dll\n/m, '')), /does not list/],
    ['duplicate component', (f) => fs.appendFileSync(f.manifestPath, 'component mpv 0.42.0\n'), /duplicate component/],
    ['malformed hash', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace(/file [a-f0-9]{64}/, 'file bad')), /invalid file/],
    ['licence names no file', (f) => fs.appendFileSync(f.manifestPath, 'licence mpv licenses/mpv/absent\n'), /invalid licence/],
    ['empty licence', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace(/file ([a-f0-9]{64}) \d+ licenses\/mpv/, 'file $1 0 licenses/mpv')), /invalid licence/],
    ['unknown record', (f) => fs.appendFileSync(f.manifestPath, 'typo something\n'), /invalid MANIFEST/],
    ['duplicate DLL ignoring case', (f) => fs.appendFileSync(f.manifestPath, f.manifest.match(/^file .* libmpv-2.dll$/m)[0].replace('libmpv-2.dll', 'LIBMPV-2.DLL') + '\n'), /duplicate file/],
  ];
  for (const escape of ['../escape.dll', 'C:/escape.dll', 'C:escape.dll', 'bad\\escape.dll', '/escape.dll']) {
    changes.push([`escape ${escape}`, (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('avcodec-63.dll', escape)), /unsafe file name/]);
  }
  for (const [name, change, expected] of changes) {
    await t.test(name, (t) => {
      const f = fixture(t);
      assert.deepEqual(f.problems(), [], 'green');
      change(f);
      assert.match(f.problems().join('\n'), expected, 'red');
      const run = spawnSync(python, ['-B', checkerPath, f.dir, '--pins', f.pinfile], { encoding: 'utf8' });
      assert.equal(run.status, 1, run.stderr);
      assert.match(run.stdout, /skip load: manifest check failed/);
    });
  }
  await t.test('symbolic link', { skip: process.platform === 'win32' && 'link creation needs Windows privileges' }, (t) => {
    const f = fixture(t);
    assert.deepEqual(f.problems(), []);
    fs.renameSync(path.join(f.dir, 'avcodec-63.dll'), path.join(f.tmp, 'foreign.dll'));
    fs.symlinkSync(path.join(f.tmp, 'foreign.dll'), path.join(f.dir, 'avcodec-63.dll'));
    assert.match(f.problems().join(), /link or reparse/);
  });
  await t.test('manifest-only CLI succeeds without loading a DLL', (t) => {
    const f = fixture(t);
    const run = spawnSync(python, ['-B', checkerPath, f.dir, '--pins', f.pinfile, '--manifest-only'], { encoding: 'utf8' });
    assert.equal(run.status, 0, run.stderr);
    assert.match(run.stdout, /ok: MANIFEST/);
  });
});

test('Windows fetch rejects corrupt cached sources before any compiler runs', { skip: noBash }, (t) => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-windows-pin-'));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  fs.mkdirSync(path.join(tmp, 'src'));
  const lua = pins.find((p) => p[0] === 'lua');
  fs.writeFileSync(path.join(tmp, 'src', `lua-${lua[1]}.tar.gz`), 'bad source');
  const run = spawnSync('bash', [scriptPath, 'lua'], { env: { ...process.env, LIBMPV_ROOT: tmp }, encoding: 'utf8' });
  assert.equal(run.status, 1, run.stdout + run.stderr);
  assert.match(run.stderr, /checksum mismatch: .*lua-/);
  assert.equal(fs.existsSync(path.join(tmp, 'work-windows-x86_64/src/lua')), false);
});

test('Windows build refuses unknown steps before changing output', { skip: noBash }, () => {
  const run = spawnSync('bash', [scriptPath, 'ffmpeg-gpl'], { encoding: 'utf8' });
  assert.equal(run.status, 1);
  assert.match(run.stderr, /unknown step 'ffmpeg-gpl'/);
});

// macOS: policy checks run on every host, without an Apple SDK or downloads.
const macScript = read('scripts/build-libmpv-macos.sh');
const macHelperPath = path.join(root, 'packaging/libmpv/macos/macho.py');
const macHelper = read('packaging/libmpv/macos/macho.py');
const macCheckerPath = path.join(root, 'packaging/libmpv/check-libmpv-macos.py');
const macChecker = read('packaging/libmpv/check-libmpv-macos.py');
const macJob = job(ci, 'libmpv-macos');
const macCode = (source) => source.split('\n').filter((line) => !/^\s*#/.test(line)).join('\n');

function macRequired(label, source, tokens) {
  for (const token of tokens) {
    test(`macOS ${label}: ${token} (green and red)`, () => {
      const guard = (candidate) => assert.ok(macCode(candidate).includes(token), `${label}: missing ${token}`);
      guard(source);
      assert.throws(() => guard(source.replaceAll(token, 'REMOVED_FOR_RED_PROOF')), /missing/);
    });
  }
}

const macConfiguration = {
  freetype: ['-Dzlib=system -Dharfbuzz=disabled -Dmmap=enabled'],
  fribidi: ['-Ddocs=false -Dbin=false -Dtests=false'],
  harfbuzz: ['-Dfreetype=enabled -Dtests=disabled -Dutilities=disabled', '-Dsubset=disabled -Draster=disabled -Dvector=disabled -Dgpu=disabled'],
  libass: ['-Dcoretext=enabled -Dfontconfig=disabled -Ddirectwrite=disabled -Dasm=enabled'],
  dav1d: ['-Denable_asm=true -Denable_tools=false -Denable_tests=false -Denable_examples=false'],
  lua: ['-DLUA_COMPAT_ALL -DLUA_USE_MACOSX', '-dynamiclib', '-Wl,-install_name,@rpath/liblua5.2.0.dylib', 'lua5.2.pc'],
  uchardet: ['-DCMAKE_TOOLCHAIN_FILE="$WORK/macos.cmake"', '-DFETCHCONTENT_FULLY_DISCONNECTED=ON', '-DBUILD_BINARY=OFF -DBUILD_STATIC=OFF -DBUILD_SHARED_LIBS=ON'],
  ffmpeg: ['env -u CFLAGS -u CXXFLAGS -u OBJCFLAGS -u LDFLAGS', '--target-os=darwin --arch="$CPU_FAMILY" --cpu=generic --enable-cross-compile',
    '--cc="$CC" --cxx="$CXX" --objcc="$OBJC"', '--pkg-config="$WORK/pkg-config" --install-name-dir=@rpath',
    '--enable-shared --disable-static --enable-pic', '--disable-autodetect', '--disable-gpl --disable-nonfree --disable-version3',
    '--enable-zlib --enable-iconv --enable-securetransport --enable-libdav1d', '--enable-videotoolbox --enable-audiotoolbox --enable-pthreads',
    '--disable-indevs --disable-outdevs --enable-indev=lavfi', '--extra-libs=-liconv',
    'grep -qx \'#define FFMPEG_LICENSE "LGPL version 2.1 or later"\' config.h',
    'ZLIB ICONV SECURETRANSPORT LIBDAV1D VIDEOTOOLBOX AUDIOTOOLBOX', 'LAVFI_INDEV TESTSRC2_FILTER HTTPS_PROTOCOL TLS_PROTOCOL',
    'grep -qx "#define CONFIG_$feature 1" config.h || die', 'grep -qx "#define CONFIG_$feature 1" config_components.h || die'],
  libplacebo: ['$(unpack vulkan-headers)', '-Dopengl=disabled -Dvulkan=disabled -Dd3d11=disabled',
    '-Dshaderc=disabled -Dglslang=disabled -Ddemos=false -Dtests=false -Dbench=false -Dfuzz=false'],
  mpv: ['-Dgpl=false -Dlibmpv=true -Dcplayer=false -Dbuild-date=false',
    '-Dlua=lua5.2 -Diconv=enabled -Dzlib=enabled -Duchardet=enabled -Dvector=enabled',
    '-Dlibavdevice=enabled -Dcoreaudio=enabled -Dgl=enabled -Dplain-gl=enabled',
    '-Dcocoa=disabled -Dgl-cocoa=disabled -Dvideotoolbox-gl=enabled -Dvideotoolbox-pl=disabled',
    '-Dswift-build=disabled -Dmacos-cocoa-cb=disabled -Dmacos-media-player=disabled',
    '-Dmacos-touchbar=disabled -Davfoundation=enabled -Dvulkan=disabled',
    'patch -d "$src" -p1 < "$recipe/macos/mpv-sdk-only.patch"',
    "grep -qx '#define HAVE_SWIFT 0'", 'grep -qx "#define HAVE_$feature 1"',
    'for feature in GL COREAUDIO AVFOUNDATION LUA LIBAVDEVICE VIDEOTOOLBOX_GL; do'],
};
for (const [name, tokens] of Object.entries(macConfiguration)) macRequired(name, body(macScript, `build_${name}`), tokens);
macRequired('isolation and universal build', macScript, [
  'for ARCH in arm64 x86_64; do', 'CPU_FAMILY=aarch64', 'export MACOSX_DEPLOYMENT_TARGET=11.0',
  'export PATH="$VELA_LIBMPV_TOOLS/bin:/usr/bin:/bin:/usr/sbin:/sbin"',
  'unset PKG_CONFIG_PATH PKG_CONFIG_SYSROOT_DIR PKG_CONFIG_DIR',
  'unset CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH OBJC_INCLUDE_PATH LIBRARY_PATH CPPFLAGS',
  'unset CMAKE_PREFIX_PATH CMAKE_TOOLCHAIN_FILE CMAKE_INCLUDE_PATH CMAKE_LIBRARY_PATH CMAKE_FRAMEWORK_PATH',
  'export PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig"', "export PKG_CONFIG_LIBDIR='$PREFIX/lib/pkgconfig'",
  '--cross-file "$WORK/macos.ini"', '--wrap-mode=nodownload', '-Ddefault_library=shared -Dauto_features=disabled',
  "system = 'darwin'", "cpu_family = '$CPU_FAMILY'", 'needs_exe_wrapper = true',
  "pkg_config_libdir = ['$PREFIX/lib/pkgconfig']", "cmake_toolchain_file = '$WORK/macos.cmake'",
  'set(CMAKE_SYSTEM_NAME Darwin)', 'set(CMAKE_OSX_ARCHITECTURES $ARCH CACHE STRING "" FORCE)',
  'set(CMAKE_OSX_DEPLOYMENT_TARGET 11.0 CACHE STRING "" FORCE)', 'set(CMAKE_OSX_SYSROOT "$SDKROOT" CACHE PATH "" FORCE)',
  'set(CMAKE_FIND_ROOT_PATH "$PREFIX" "$SDKROOT")', 'set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)',
  'set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)', 'set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)',
  'set(CMAKE_IGNORE_PREFIX_PATH /opt/homebrew /usr/local)',
  'python3 -B "$recipe/macos/macho.py" collect "$PREFIX" "$WORK/stage" "$ARCH"',
  'python3 -B "$recipe/macos/macho.py" merge "$WORK_ROOT" "$OUT" "$recipe"',
  'python3 -B "$recipe/check-libmpv-macos.py" "$OUT" --manifest-only',
  'file="$(fetch "$name")" || exit 1',
]);
for (const language of ['c', 'cpp', 'objc', 'objcpp']) {
  macRequired('Meson architecture flags', macScript, [
    `${language}_args = ['-arch', '$ARCH', '-isysroot', '$SDKROOT', '-mmacosx-version-min=11.0'`,
    `${language}_link_args = ['-arch', '$ARCH', '-isysroot', '$SDKROOT', '-mmacosx-version-min=11.0'`,
  ]);
}

test('macOS sources and notices cover exactly the Linux components plus Vulkan headers and the macOS build inputs (green and red)', () => {
  const expected = pins.filter((p) => p[2] === 'ship' || p[2] === 'build-macos' || p[0] === 'vulkan-headers');
  const guard = (candidate, helper, entries = expected) => {
    const fetched = [...candidate.matchAll(/"\$\((?:unpack|fetch) ([a-z0-9-]+)\)"/g)].map((m) => m[1]);
    assert.deepEqual([...new Set(fetched)].sort(), entries.map((p) => p[0]).sort(), 'source graph equals selected pins');
    assert.equal((macCode(candidate).match(/\bcurl\b/g) ?? []).length, 1, 'one download boundary');
    assert.doesNotMatch(macCode(candidate), /\b(wget|git clone|pip3? install|brew install)\b/, 'no unpinned target source');
    const steps = candidate.match(/^steps="\$\{\*:-(.*?)\}"$/m)?.[1].split(' ');
    assert.deepEqual(steps, [...Object.keys(macConfiguration), 'collect'], 'all components built in dependency order');
    for (const [name, version, role, sha, url] of entries) {
      assert.match(sha, /^[0-9a-f]{64}$/, `${name}: checksum`);
      assert.match(role, /^(ship|build|build-macos)$/, `${name}: role`);
      assert.ok(url?.startsWith('https://') && url.includes(version), `${name}: versioned source URL`);
      assert.ok(helper.includes(`"${name}": [`) || (name === 'lua' && helper.includes('licence lua licenses/lua/LICENSE')), `${name}: licence`);
    }
  };
  guard(macScript, macHelper);
  for (const [name] of expected) {
    assert.throws(() => guard(macScript.replaceAll(`$(unpack ${name})`, '$(unpack UNPINNED)'), macHelper), /source graph/);
    assert.throws(() => guard(macScript, macHelper.replaceAll(`"${name}": [`, '"MISSING": [').replaceAll(`licence ${name} licenses/${name}/LICENSE`, 'REMOVED')), /licence/);
    for (const [index, bad, message] of [[2, 'bad', /role/], [3, 'bad', /checksum/], [4, 'https://unpinned.invalid', /versioned source URL/]]) {
      assert.throws(() => guard(macScript, macHelper, expected.map((p) => p[0] === name ? p.map((v, i) => i === index ? bad : v) : p)), message);
    }
  }
  assert.throws(() => guard(macScript + '\ncurl https://unpinned.invalid\n', macHelper), /download boundary/);
  assert.throws(() => guard(macScript + '\nwget https://unpinned.invalid\n', macHelper), /unpinned target source/);
  assert.throws(() => guard(macScript.replace('dav1d lua uchardet', 'dav1d uchardet'), macHelper), /dependency order/);
});

test('macOS LGPL switches and SDK-only build patch reject regressions (green and red)', () => {
  const licence = (candidate) => {
    assert.doesNotMatch(macCode(body(candidate, 'build_mpv')), /-Dgpl=true/, 'mpv GPL');
    assert.doesNotMatch(macCode(body(candidate, 'build_ffmpeg')), /--enable-(gpl|nonfree|version3)/, 'FFmpeg LGPL');
  };
  licence(macScript);
  assert.throws(() => licence(macScript.replace('-Dgpl=false', '-Dgpl=true')), /mpv GPL/);
  for (const flag of ['gpl', 'nonfree', 'version3']) {
    assert.throws(() => licence(macScript.replace(`--disable-${flag}`, `--enable-${flag}`)), /FFmpeg LGPL/);
  }
  const patch = read('packaging/libmpv/macos/mpv-sdk-only.patch');
  const guard = (candidate) => {
    assert.deepEqual([...candidate.matchAll(/^\+\+\+ b\/(.*)$/gm)].map((m) => m[1]), ['meson.build'], 'build description only');
    assert.match(candidate, /^\+    objc_link_flags = \['-isysroot', macos_sdk_path\]$/m, 'SDK only');
    assert.doesNotMatch(candidate.split('\n').filter((l) => l.startsWith('+')).join('\n'), /\/usr\/local|\/opt\/homebrew/, 'no host search path');
    // pl-9: the Apple audio outputs call cfstr_from_cstr from
    // osdep/utils-mac.c, which meson.build compiles only with cocoa; without
    // this hunk the library fails to load (undefined _cfstr_from_cstr).
    assert.match(
      candidate,
      /^ +'audio\/out\/ao_coreaudio_utils\.c'\)\n\+    if not features\['cocoa'\]\n\+        sources \+= files\('osdep\/utils-mac\.c'\)\n\+    endif\n endif$/m,
      'utils-mac.c for the audio outputs without cocoa',
    );
    // VideoToolbox decodes straight to the picture only with mpv's
    // IOSurface/CGL interop, which meson.build allows only with gl-cocoa
    // (Swift in this pin): built on plain-gl, with what it calls linked.
    assert.match(
      candidate,
      /^-    features\['gl-cocoa'\] or features\['ios-gl'\],\n\+    features\['gl-cocoa'\] or features\['ios-gl'\] or \(darwin and plain_gl\.allowed\(\)\),$/m,
      'videotoolbox-gl on plain-gl',
    );
    assert.match(
      candidate,
      /^ +sources \+= files\('video\/out\/hwdec\/hwdec_mac_gl\.c'\)\n\+    dependencies \+= dependency\('appleframeworks', modules: \['OpenGL', 'CoreVideo', 'IOSurface'\]\)\n endif$/m,
      'the interop linked to OpenGL, CoreVideo and IOSurface',
    );
  };
  guard(patch);
  assert.throws(() => guard(patch.replace(' or (darwin and plain_gl.allowed())', '')), /videotoolbox-gl on plain-gl/);
  assert.throws(() => guard(patch.replace("['OpenGL', 'CoreVideo', 'IOSurface']", "['OpenGL', 'CoreVideo']")), /the interop linked/);
  assert.throws(() => guard(patch.replace('+++ b/meson.build', '+++ b/player/main.c')), /build description/);
  assert.throws(() => guard(patch.replace("+    objc_link_flags = ['-isysroot', macos_sdk_path]", "+    objc_link_flags = ['-L/usr/local/lib']")), /SDK only/);
  assert.throws(() => guard(patch + '\n+    link_flags += ["-L/opt/homebrew/lib"]\n'), /host search path/);
  const utilsHunk = /^@@ -882,2 \+882,5 @@\n(?:[ +].*\n)*?(?=@@)/m;
  assert.match(patch, utilsHunk, 'the fixture holds the utils-mac.c hunk');
  assert.throws(() => guard(patch.replace(utilsHunk, '')), /utils-mac\.c for the audio outputs/);
  assert.throws(() => guard(patch.replace("+    if not features['cocoa']", "+    if features['cocoa']")), /utils-mac\.c for the audio outputs/);
  assert.throws(() => guard(patch.replace("+        sources += files('osdep/utils-mac.c')\n", '')), /utils-mac\.c for the audio outputs/);
});

macRequired('CI', macJob, [
  'runs-on: macos-latest', 'timeout-minutes: 150', 'run: bash packaging/libmpv/macos/prepare-tools.sh',
  'defaults:\n      run:\n        shell: bash',
  'path: .libmpv/out/libmpv-macos-universal', 'bash scripts/build-libmpv-macos.sh',
  '/usr/bin/python3 -I -B packaging/libmpv/check-libmpv-macos.py .libmpv/out/libmpv-macos-universal',
  'softwareupdate --install-rosetta --agree-to-license', 'name: libmpv-macos-universal',
  'uses: actions/upload-artifact@v7', 'if-no-files-found: error',
]);
test('macOS CI hashes every build input and always checks cached sets (green and red)', () => {
  const inputs = ['scripts/build-libmpv-macos.sh', 'packaging/libmpv/versions.txt', 'packaging/libmpv/macos/**',
    'packaging/libmpv/check-libmpv-macos.py', '.github/workflows/ci.yml', '.libmpv/toolchain-macos.txt'];
  const guard = (candidate) => {
    const key = candidate.match(/key: libmpv-macos-universal-\$\{\{ hashFiles\(([^)]*)\) \}\}/)?.[1];
    assert.ok(key, 'cache key');
    for (const input of inputs) assert.ok(key.includes(`'${input}'`), `key missing ${input}`);
    const check = candidate.match(/- name: Load and play both architectures\n([\s\S]*?)(?=\n      - |$)/)?.[1];
    assert.ok(check, 'separate check step');
    assert.doesNotMatch(check, /^\s+if:/m, 'cached sets must be checked');
    assert.ok(candidate.indexOf('Prepare host tools') < candidate.indexOf('Restore the universal set'), 'fingerprint before cache');
  };
  guard(macJob);
  for (const input of inputs) assert.throws(() => guard(macJob.replace(`'${input}'`, "'missing'")), /key missing/);
  assert.throws(() => guard(macJob.replace('libmpv-macos-universal-${{ hashFiles', 'constant-${{ hashFiles')), /cache key/);
  assert.throws(() => guard(macJob.replace('- name: Load and play both architectures\n', '- name: Load and play both architectures\n        if: false\n')), /cached sets/);
  assert.throws(() => guard(macJob.replace('- name: Load and play both architectures\n', '- name: no check\n')), /separate check step/);
  assert.throws(() => guard(macJob.replace('Prepare host tools', 'TEMP').replace('Restore the universal set', 'Prepare host tools').replace('TEMP', 'Restore the universal set')), /fingerprint before cache/);
});
macRequired('probe', read('probes/libmpv-macos/run.sh'), [
  'trap keep_evidence EXIT', 'bash packaging/libmpv/macos/prepare-tools.sh',
  'bash scripts/build-libmpv-macos.sh', '/usr/bin/python3 -I -B packaging/libmpv/check-libmpv-macos.py',
  'cp "$set_dir/MANIFEST" "$PROBE_OUT/MANIFEST"', '/usr/bin/otool -arch all -L "$dylib"',
  'tee "$PROBE_OUT/build.log"', 'tee "$PROBE_OUT/check.log"', 'exit "$status"',
]);
macRequired('checker wiring', macChecker, [
  'problems = macho.check_manifest(directory, args.pins)', 'skip load: manifest check failed',
  'macho.validate(directory)', 'for arch in ("arm64", "x86_64"):',
  '["/usr/bin/arch", f"-{arch}", "/usr/bin/python3", "-I", "-B"',
  'environment = clean_environment(os.environ)', 'timeout=90, check=False',
  'ctypes.CDLL(str(directory / "libmpv.2.dylib"), mode=os.RTLD_NOW | os.RTLD_LOCAL)',
  '["/usr/bin/vmmap", "-w", str(os.getpid())]', 'snapshots.update(loaded_images())',
  'check_loaded(directory, files, snapshots, executable, require_mpv,\n                                interpreter=sys.base_prefix)',
  'lib.mpv_client_api_version()', '("vo", "null"), ("ao", "null")',
  'b"av://lavfi:testsrc2=duration=2"', 'end.reason != 0 or end.error != 0 or not loaded',
  'time.monotonic() + 25', 'lib.mpv_terminate_destroy(handle)',
]);
macRequired('Mach-O collector', macHelper, [
  'queue, seen = ["libmpv.2.dylib"], set()', 'architectures(apple("lipo", "-archs", source), {arch})',
  'edits = ["-id", f"@rpath/{name}"]', 'edits.extend(["-change", dep, f"@rpath/{child}"])',
  'edits.extend(["-add_rpath", "@loader_path"])', 'apple("install_name_tool", *edits, target)',
  'apple("lipo", "-create", arm / name, intel / name, "-output", dest / name)',
  '"/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none"',
  'system = validate(dest)', 'no licence text collected for a component',
  'hashlib.sha256(content).hexdigest()', 'files, components = read_manifest(directory)',
]);

function macCall(module, name, ...args) {
  const run = spawnSync(python, ['-B', '-c', [
    'import importlib.util, json, sys',
    "s = importlib.util.spec_from_file_location('mac_check', sys.argv[1])",
    'm = importlib.util.module_from_spec(s); s.loader.exec_module(m)',
    'try:',
    '    result = getattr(m, sys.argv[2])(*json.loads(sys.argv[3]))',
    '    print(json.dumps(sorted(result) if isinstance(result, set) else result))',
    'except ValueError as e:',
    '    print(json.dumps({"error": str(e)}))',
  ].join('\n'), module, name, JSON.stringify(args)], { encoding: 'utf8' });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

test('macOS Mach-O audit rejects foreign links, wrong IDs/rpaths/floors and thin libraries (green and red)', () => {
  const name = 'libmpv.2.dylib';
  const names = [name, 'libavcodec.63.dylib'];
  const info = { id: ['@rpath/libmpv.2.dylib'], rpaths: ['@loader_path'], minimum: ['11.0'],
    needed: ['@rpath/libavcodec.63.dylib', '/usr/lib/libSystem.B.dylib', '/System/Library/Frameworks/CoreAudio.framework/Versions/A/CoreAudio'] };
  const judge = (change = {}) => macCall(macHelperPath, 'judge', name, { ...info, ...change }, names);
  assert.deepEqual(judge(), []);
  for (const dep of ['/opt/homebrew/lib/libz.dylib', '/usr/local/lib/libz.dylib', '/tmp/prefix/lib/libavcodec.63.dylib',
    '/usr/library/libz.dylib', '/System-fake/libz.dylib', '/System/../opt/libz.dylib',
    '@rpath/libmissing.dylib', '@loader_path/libavcodec.63.dylib', '@rpath/../libavcodec.63.dylib']) {
    assert.match(judge({ needed: [...info.needed, dep] }).join(), /outside the set\/system/);
  }
  assert.match(judge({ id: ['/tmp/libmpv.2.dylib'] }).join(), /install name/);
  assert.match(judge({ rpaths: [] }).join(), /rpaths/);
  assert.match(judge({ rpaths: ['@loader_path', '/opt/homebrew/lib'] }).join(), /rpaths/);
  assert.match(judge({ minimum: ['11.1'] }).join(), /above 11.0/);
  assert.match(judge({ minimum: [] }).join(), /deployment/);
  assert.equal(macCall(macHelperPath, 'architectures', 'x86_64 arm64\n'), null);
  for (const archs of ['arm64', 'x86_64', '', 'arm64 arm64', 'arm64 x86_64 arm64e']) {
    assert.match(macCall(macHelperPath, 'architectures', archs).error, /wrong architectures/);
  }
});

test('macOS otool and vmmap parsers read real-shaped records and fail closed (green and red)', () => {
  const commands = `file (architecture arm64):\nLoad command 0\n          cmd LC_ID_DYLIB\n         name @rpath/libmpv.2.dylib (offset 24)\nLoad command 1\n          cmd LC_LOAD_DYLIB\n         name /usr/lib/libSystem.B.dylib (offset 24)\nLoad command 2\n          cmd LC_RPATH\n         path @loader_path (offset 12)\nLoad command 3\n          cmd LC_BUILD_VERSION\n     platform 1\n        minos 11.0\n          sdk 15.0\n`;
  assert.deepEqual(macCall(macHelperPath, 'parse_load_commands', commands), {
    id: ['@rpath/libmpv.2.dylib'], needed: ['/usr/lib/libSystem.B.dylib'], rpaths: ['@loader_path'], minimum: ['11.0'],
  });
  for (const [old, replacement, reason] of [['LC_ID_DYLIB', 'LC_UUID', /install name/],
    ['platform 1', 'platform 2', /not a macOS/], ['minos 11.0', 'minos garbage', /deployment/],
    ['name /usr/lib', 'BROKEN /usr/lib', /has no name/], ['path @loader_path', 'BROKEN @loader_path', /has no path/]]) {
    assert.match(macCall(macHelperPath, 'parse_load_commands', commands.replace(old, replacement)).error, reason);
  }
  const text = '__TEXT 100000-200000 [16K] r-x/r-x SM=COW /set with spaces/libmpv.2.dylib\n__TEXT 300000-400000 [16K] r-x/r-x SM=COW /usr/lib/libSystem.B.dylib\n';
  assert.deepEqual(macCall(macCheckerPath, 'parse_vmmap', text), ['/set with spaces/libmpv.2.dylib', '/usr/lib/libSystem.B.dylib']);
  assert.match(macCall(macCheckerPath, 'parse_vmmap', 'vmmap: permission denied').error, /no image paths/);
});

test('macOS loaded-image audit rejects every foreign or unlisted library, including the harness (green and red)', () => {
  const set = path.resolve(os.tmpdir(), 'vela-mac-set');
  const names = ['libmpv.2.dylib', 'libavcodec.63.dylib'];
  const modules = names.map((n) => path.join(set, n)).concat(['/usr/lib/libSystem.B.dylib', '/System/Library/Frameworks/CoreAudio.framework/Versions/A/CoreAudio']);
  const judge = (paths, required = true) => macCall(macCheckerPath, 'check_loaded', set, names, paths, '/usr/bin/python3', required);
  assert.deepEqual(judge(modules), []);
  assert.deepEqual(judge([...modules, '/usr/bin/python3']), []);
  for (const file of ['/opt/homebrew/lib/libffi.dylib', '/usr/local/lib/libz.dylib', '/tmp/libavcodec.63.dylib',
    '/Library/Developer/CommandLineTools/Library/Frameworks/Python3.framework/Versions/3.9/Python3',
    `${set}-other/libmpv.2.dylib`, `${set}/nested/libavcodec.63.dylib`]) {
    assert.match(judge([...modules, file]).join(), /outside set\/system/);
  }
  assert.match(judge([...modules, path.join(set, 'unlisted.dylib')]).join(), /not in MANIFEST/);
  assert.match(judge(modules.slice(1)).join(), /not mapped/);
  assert.deepEqual(judge([], false), []);
  const clean = macCall(macCheckerPath, 'clean_environment', {
    DYLD_LIBRARY_PATH: '/bad', DYLD_INSERT_LIBRARIES: '/bad', DYLD_FUTURE_VARIABLE: '/bad',
    PYTHONPATH: '/bad', LD_PRELOAD: '/bad', PATH: '/usr/bin', KEEP: 'yes',
  });
  assert.deepEqual(clean, { PATH: '/usr/bin', KEEP: 'yes' });
});

function macFixture(t) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-macos-check-'));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  const dir = path.join(tmp, 'set');
  const pinfile = path.join(tmp, 'versions.txt');
  fs.mkdirSync(dir);
  fs.writeFileSync(pinfile, 'mpv 0.42.0 ship\nvulkan-headers 1.4.368 build\nshaderc 2026.4 build\n');
  const files = { 'libmpv.2.dylib': 'universal library', 'libavcodec.63.dylib': 'dependency',
    'licenses/mpv/LICENSE': 'LGPL', 'licenses/vulkan-headers/LICENSE': 'MIT', 'licenses/mpv/mpv-sdk-only.patch': 'build patch' };
  const lines = ['arch universal', 'platform macos', 'minimum-macos 11.0',
    'component mpv 0.42.0', 'component vulkan-headers 1.4.368',
    'licence mpv licenses/mpv/LICENSE', 'licence vulkan-headers licenses/vulkan-headers/LICENSE',
    'patch mpv licenses/mpv/mpv-sdk-only.patch'];
  for (const [name, data] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(dir, name)), { recursive: true });
    fs.writeFileSync(path.join(dir, name), data);
    lines.push(`file ${createHash('sha256').update(data).digest('hex')} ${Buffer.byteLength(data)} ${name}`);
  }
  const manifest = lines.join('\n') + '\n';
  const manifestPath = path.join(dir, 'MANIFEST');
  fs.writeFileSync(manifestPath, manifest);
  return { tmp, dir, pinfile, manifest, manifestPath,
    problems: () => macCall(macHelperPath, 'check_manifest', dir, pinfile) };
}

test('macOS MANIFEST rejects corruption before loading (each case green and red)', async (t) => {
  const changes = [
    ['missing dylib', (f) => fs.unlinkSync(path.join(f.dir, 'libavcodec.63.dylib')), /missing/],
    ['changed dylib', (f) => fs.writeFileSync(path.join(f.dir, 'libmpv.2.dylib'), 'UNIVERSAL library'), /modified/],
    ['extra dylib', (f) => fs.writeFileSync(path.join(f.dir, 'extra.dylib'), 'x'), /not in MANIFEST/],
    ['missing licence', (f) => fs.unlinkSync(path.join(f.dir, 'licenses/mpv/LICENSE')), /missing/],
    ['changed licence', (f) => fs.appendFileSync(path.join(f.dir, 'licenses/mpv/LICENSE'), 'x'), /modified/],
    ['no licence role', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('licence mpv licenses/mpv/LICENSE\n', '')), /no licence text/],
    ['patch not licence', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('licence mpv licenses/mpv/LICENSE', 'licence mpv licenses/mpv/mpv-sdk-only.patch')), /no licence text/],
    ['missing patch', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('patch mpv licenses/mpv/mpv-sdk-only.patch\n', '')), /omits the mpv build patch/],
    ['wrong version', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('component mpv 0.42.0', 'component mpv 0.41.0')), /versions differ/],
    ['wrong architecture', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('arch universal', 'arch arm64')), /must name macOS universal/],
    ['wrong floor', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('minimum-macos 11.0', 'minimum-macos 12.0')), /must name macOS universal/],
    ['hash malformed', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace(/file [a-f0-9]{64}/, 'file bad')), /invalid file/],
    ['duplicate record', (f) => fs.appendFileSync(f.manifestPath, f.manifest.match(/^file .* libmpv\.2\.dylib$/m)[0] + '\n'), /duplicate file/],
    ['system exemption', (f) => fs.appendFileSync(f.manifestPath, 'system /opt/homebrew/lib/libz.dylib\n'), /invalid MANIFEST/],
    ['escape', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace('libavcodec.63.dylib', '../escape.dylib')), /unsafe file/],
    ['no primary library', (f) => fs.writeFileSync(f.manifestPath, f.manifest.replace(/^file .* libmpv\.2\.dylib\n/m, '')), /does not list/],
  ];
  for (const [label, change, expected] of changes) {
    await t.test(label, (t) => {
      const f = macFixture(t);
      assert.deepEqual(f.problems(), [], 'green');
      change(f);
      assert.match(f.problems().join('\n'), expected, 'red');
      const run = spawnSync(python, ['-B', macCheckerPath, f.dir, '--pins', f.pinfile], { encoding: 'utf8' });
      assert.equal(run.status, 1, run.stderr);
      assert.match(run.stdout, /skip load: manifest check failed/);
    });
  }
  await t.test('manifest-only check succeeds without Apple tools', (t) => {
    const f = macFixture(t);
    const run = spawnSync(python, ['-B', macCheckerPath, f.dir, '--pins', f.pinfile, '--manifest-only'], { encoding: 'utf8' });
    assert.equal(run.status, 0, run.stderr);
    assert.match(run.stdout, /ok: MANIFEST/);
  });
  await t.test('symlinked library', { skip: process.platform === 'win32' && 'link creation requires privileges' }, (t) => {
    const f = macFixture(t);
    assert.deepEqual(f.problems(), []);
    fs.renameSync(path.join(f.dir, 'libmpv.2.dylib'), path.join(f.tmp, 'foreign.dylib'));
    fs.symlinkSync(path.join(f.tmp, 'foreign.dylib'), path.join(f.dir, 'libmpv.2.dylib'));
    assert.match(f.problems().join(), /symbolic link/);
  });
});

test('macOS fetch/unpack stops at a bad checksum even without inherit_errexit (green and red)', { skip: noBash }, (t) => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-macos-fetch-'));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  fs.mkdirSync(path.join(tmp, 'src')); fs.mkdirSync(path.join(tmp, 'tree/lua-1.0'), { recursive: true });
  fs.writeFileSync(path.join(tmp, 'tree/lua-1.0/source'), 'valid archive, wrong source');
  const packed = spawnSync('tar', ['-czf', path.join(tmp, 'src/lua-1.0.tar.gz'), '-C', path.join(tmp, 'tree'), 'lua-1.0']);
  assert.equal(packed.status, 0);
  fs.writeFileSync(path.join(tmp, 'pins'), `lua 1.0 ship ${'0'.repeat(64)} https://example.invalid/lua-1.0.tar.gz\n`);
  const functions = ['die', 'field', 'sources', 'fetch', 'unpack'].map((name) => {
    const match = macScript.match(new RegExp(`^${name}\\(\\) \\{[^\\n]*\\}$|^${name}\\(\\) \\{\\n[\\s\\S]*?\\n\\}`, 'm'));
    assert.ok(match, name); return match[0];
  }).join('\n');
  const invoke = (source) => spawnSync('bash', ['-c', `set -euo pipefail\n${source}\nsrc="$(unpack lua)" || exit 1\nprintf extracted`, 'test'], {
    env: { ...process.env, VERSIONS: path.join(tmp, 'pins'), SRC_DIR: path.join(tmp, 'src'), WORK: path.join(tmp, 'work') }, encoding: 'utf8',
  });
  const guard = (run) => {
    assert.equal(run.status, 1, 'checksum must stop the build');
    assert.match(run.stderr.trim().split('\n').at(-1), /checksum mismatch: .*lua-1.0/, 'stop at mismatch');
    assert.equal(run.stdout, '', 'nothing extracted');
  };
  guard(invoke(functions));
  assert.equal(fs.existsSync(path.join(tmp, 'work/src/lua')), false);
  const broken = functions.replace(/^  echo "\$sha  \$file" \| shasum .*\n/m, '');
  assert.notEqual(broken, functions);
  assert.throws(() => guard(invoke(broken)), /checksum must stop/);
});

test('macOS refuses an unknown step before requiring macOS or touching output', { skip: noBash }, () => {
  const run = spawnSync('bash', [path.join(root, 'scripts/build-libmpv-macos.sh'), 'ffmpeg-gpl'], { encoding: 'utf8' });
  assert.equal(run.status, 1);
  assert.match(run.stderr, /unknown step 'ffmpeg-gpl'/);
});

test('macOS controller actually runs both workers and fails for either architecture or a timeout', async (t) => {
  for (const failure of ['none', 'arm64', 'x86_64', 'timeout']) {
    await t.test(failure, (t) => {
      const f = macFixture(t);
      const harness = [
        'import importlib.util, json, os, sys, types',
        "s = importlib.util.spec_from_file_location('check', sys.argv[1])",
        'm = importlib.util.module_from_spec(s); s.loader.exec_module(m)',
        'failure, directory, pins = sys.argv[2:5]',
        'm.sys.platform = "darwin"',
        'calls = []',
        'm.macho.validate = lambda p: calls.append({"validate": str(p)})',
        'os.environ["DYLD_LIBRARY_PATH"] = "/bad"',
        'os.environ["DYLD_INSERT_LIBRARIES"] = "/bad"',
        'def run(command, env, timeout, check):',
        '    calls.append({"command": command, "env": {k: v for k, v in env.items() if k.startswith("DYLD_") or k == "PATH"}, "timeout": timeout, "check": check})',
        '    if failure == "timeout": raise m.subprocess.TimeoutExpired(command, timeout)',
        '    return types.SimpleNamespace(returncode=7 if command[-1] == failure else 0)',
        'm.subprocess.run = run',
        'sys.argv = [s.origin, directory, "--pins", pins]',
        'result = m.main()',
        'print(json.dumps({"status": result, "calls": calls}))',
      ].join('\n');
      const run = spawnSync(python, ['-B', '-c', harness, macCheckerPath, failure, f.dir, f.pinfile], { encoding: 'utf8' });
      assert.equal(run.status, 0, run.stderr);
      const result = JSON.parse(run.stdout.trim().split('\n').at(-1));
      assert.equal(result.status, failure === 'none' ? 0 : 1);
      assert.deepEqual(result.calls[0], { validate: fs.realpathSync(f.dir) });
      assert.equal(result.calls.length, 3, 'validate, native, Rosetta; neither worker is skipped');
      for (const [i, arch] of ['arm64', 'x86_64'].entries()) {
        const call = result.calls[i + 1];
        assert.deepEqual(call.command, ['/usr/bin/arch', `-${arch}`, '/usr/bin/python3', '-I', '-B', macCheckerPath,
          fs.realpathSync(f.dir), '--pins', fs.realpathSync(f.pinfile), '--worker', arch]);
        assert.deepEqual(call.env, { PATH: '/usr/bin:/bin:/usr/sbin:/sbin' });
        assert.equal(call.timeout, 90);
        assert.equal(call.check, false);
      }
    });
  }
});

test("build.rs wires the tested mpv resource copy to Tauri's mapping", () => {
  // This checks wiring and the mapping. The helper's Rust unit test checks
  // the filesystem effect for both Cargo layouts and a target triple.
  const conf = JSON.parse(fs.readFileSync(path.join(repoRoot, "src-tauri/tauri.conf.json"), "utf8"));
  assert.deepEqual(conf.bundle.resources, { "resources/mpv-scripts/": "mpv-scripts/" },
    "build.rs copies exactly this mapping; change the two together");
  const build = fs.readFileSync(path.join(repoRoot, "src-tauri/build.rs"), "utf8");
  const support = fs.readFileSync(path.join(repoRoot, "src-tauri/build_support.rs"), "utf8");
  assert.match(build, /#\[path = "build_support\.rs"\]\s*mod build_support;/);
  assert.match(build, /tauri_build::build\(\);[\s\S]*build_support::copy_mpv_scripts\(&out_dir, std::path::Path::new\("resources\/mpv-scripts"\)\)/);
  assert.match(build, /let out_dir = std::path::PathBuf::from\(std::env::var_os\("OUT_DIR"\)/);
  assert.match(support, /let target = profile_dir\.join\("mpv-scripts"\);/);
  assert.match(build, /println!\("cargo:rerun-if-changed=resources\/mpv-scripts"\);/);
  const lib = fs.readFileSync(path.join(repoRoot, "src-tauri/src/lib.rs"), "utf8");
  assert.match(lib, /#\[cfg\(test\)\]\s*#\[path = "\.\.\/build_support\.rs"\]\s*mod build_support;/);
});
