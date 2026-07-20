import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

test("the released Wayland API uses the locally secured scanner", async () => {
  const cargoToml = await readFile(
    path.join(repoRoot, "src-tauri", "Cargo.toml"),
    "utf8",
  );
  const cargoLock = await readFile(
    path.join(repoRoot, "src-tauri", "Cargo.lock"),
    "utf8",
  );
  const vendorToml = await readFile(
    path.join(
      repoRoot,
      "src-tauri",
      "vendor",
      "wayland-scanner",
      "Cargo.toml",
    ),
    "utf8",
  );
  const vendorParser = await readFile(
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
  const commands = await readFile(
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
  const commands = await readFile(
    path.join(repoRoot, "src-tauri", "src", "commands.rs"),
    "utf8",
  );
  const lib = await readFile(path.join(repoRoot, "src-tauri", "src", "lib.rs"), "utf8");

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
