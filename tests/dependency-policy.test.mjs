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
  assert.match(resolve, /\.take_at\(&request_id, Instant::now\(\)\)/);
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
    assert.match(pin.role, /^(ship|build)$/, `${pin.name}: role`);
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
  for (const pin of pins) {
    assert.ok(fetched.includes(pin.name), `${pin.name} is pinned but never fetched`);
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
  assert.match(collect, /install -m644 "\$PATCHES"\/\*\.patch "\$dest\/licenses\/mpv\/"/);
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
