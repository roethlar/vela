import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

const config = fs.readFileSync("src-tauri/src/config.rs", "utf8");
const connections = fs.readFileSync("src-tauri/src/connections.rs", "utf8");
const commands = fs.readFileSync("src-tauri/src/commands.rs", "utf8");
const durable = fs.readFileSync("src-tauri/src/durable.rs", "utf8");
const lib = fs.readFileSync("src-tauri/src/lib.rs", "utf8");
// One play's supervision lives in playback.rs; how a launch is described and
// started lives in the player module (.agents/plans/embedded-player.md, Slice
// 1). The assertions below that were written against playback.rs now read all
// of it together, so a property may not hide by moving between those files.
const playerLaunch = fs.readFileSync("src-tauri/src/player/launch.rs", "utf8");
const playerExternal = fs.readFileSync("src-tauri/src/player/external.rs", "utf8");
const playerHandle = fs.readFileSync("src-tauri/src/player/mod.rs", "utf8");
const playbackSupervision = fs.readFileSync("src-tauri/src/playback.rs", "utf8");
const playback = [playbackSupervision, playerLaunch, playerExternal, playerHandle].join("\n");
const playlists = fs.readFileSync("src-tauri/src/playlists.rs", "utf8");
const plexApi = fs.readFileSync("src-tauri/src/plex_api.rs", "utf8");
const plexLibrary = fs.readFileSync("src-tauri/src/plex_library.rs", "utf8");
const plexSource = fs.readFileSync("src-tauri/src/source/plex.rs", "utf8");
const artwork = fs.readFileSync("src-tauri/src/artwork.rs", "utf8");
const tauriConfig = fs.readFileSync("src-tauri/tauri.conf.json", "utf8");
const backend = `${commands}\n${playback}`;
const page = fs.readFileSync("src/routes/+page.svelte", "utf8");
const mockPlex = fs.readFileSync("tests/e2e/mockplex.mjs", "utf8");

// The part of a Rust file that is compiled into the app: everything before its
// test module, without comment lines.
function shippedCode(source) {
  const parts = source.replace(/\r\n/g, "\n").split("\n#[cfg(test)]\nmod tests {\n");
  assert.equal(parts.length, 2, "each of these files ends in exactly one test module");
  return parts[0]
    .split("\n")
    .filter((line) => !/^\s*\/\//.test(line))
    .join("\n");
}

function linesMentioning(code, pattern) {
  return code
    .split("\n")
    .filter((line) => pattern.test(line))
    .map((line) => line.trim());
}

function attributesBefore(source, declaration) {
  const at = source.indexOf(declaration);
  assert.ok(at >= 0, `${declaration} must exist`);
  return source.slice(Math.max(0, at - 240), at);
}

test("settings and connections reject unknown active fields", () => {
  assert.match(
    attributesBefore(config, "pub struct AppConfig"),
    /serde\(default,\s*deny_unknown_fields\)/,
  );
  assert.match(
    attributesBefore(config, "pub struct SourceConfig"),
    /serde\(default,\s*deny_unknown_fields\)/,
  );
  assert.match(
    attributesBefore(connections, "struct ConnectionsConfig"),
    /serde\(default,\s*deny_unknown_fields\)/,
  );
  assert.match(config, /fn validate\(&self\) -> Result<\(\), String>/);
  assert.match(connections, /fn validate\(&self\) -> Result<\(\), String>/);
});

test("durable settings reads never default or disappear through Result helpers", () => {
  assert.doesNotMatch(
    backend,
    /(?:crate::)?config::load_config\(\)\s*\.(?:ok|unwrap_or_default)\s*\(/,
  );
  assert.doesNotMatch(
    backend,
    /(?:crate::)?config::load_config\(\)[\s\S]{0,100}\.unwrap_or\s*\(/,
  );
  assert.doesNotMatch(
    backend,
    /if\s+let\s+Ok\s*\([^)]*\)\s*=\s*(?:crate::)?config::load_config\(\)/,
  );
  assert.doesNotMatch(
    fs.readFileSync("src-tauri/src/lib.rs", "utf8"),
    /load_config\(\)\.unwrap_or_else/,
  );
});

test("boot checks the durable gate before any normal app request", () => {
  const bootStart = page.indexOf("async function boot()");
  const gate = page.indexOf('"get_durable_state_status"', bootStart);
  const normalBoot = page.indexOf("await bootNormal()", bootStart);
  const mpv = page.indexOf('"check_mpv"', bootStart);
  const sources = page.indexOf('"get_sources"', bootStart);
  assert.ok(bootStart >= 0 && gate > bootStart);
  assert.ok(normalBoot > gate);
  assert.ok(mpv > normalBoot);
  assert.ok(sources > normalBoot);
  assert.match(page, /Vela could not safely read your \{fault\.file === "settings"/);
  assert.match(page, /"Try again"/);
  assert.match(page, />Exit Vela</);
});

test("recoverable files expose explicit real recovery buttons only when allowed", () => {
  assert.match(page, /invoke<DurableRecoveryResult>\("recover_invalid_file", \{ file \}\)/);
  assert.match(page, /invoke<DurableRecoveryResult>\("rollback_invalid_file", \{/);
  assert.match(
    page,
    /fault\.value\.status === "recoverable_invalid" && fault\.value\.canRecover/,
  );
  assert.match(
    page,
    /<button[\s\S]{0,220}disabled=\{durableBusy\}[\s\S]{0,120}onclick=\{\(\) => recoverInvalidFile\(fault\.file\)\}/,
  );
  assert.match(page, /Rename and create new settings/);
  assert.match(page, /Rename damaged connections and reconnect/);
  assert.match(page, /\{#each fault\.value\.rollbackVersions as version \(version\.id\)\}/);
  assert.match(page, /`Restore \$\{formatRollbackDate\(version\.createdAtUnixMs\)\}`/);
  assert.match(
    page,
    /<button[\s\S]{0,100}disabled=\{durableBusy\}[\s\S]{0,120}onclick=\{\(\) => rollbackInvalidFile\(fault\.file, version\)\}/,
  );
  assert.match(page, /<button disabled=\{durableBusy\} onclick=\{exitVela\}>Exit Vela<\/button>/);
  assert.match(page, /aria-busy=\{durableBusy\}/);
  assert.match(page, /durableHeading\?\.focus\(\)/);
  assert.match(
    fs.readFileSync("src-tauri/src/lib.rs", "utf8"),
    /commands::recover_invalid_file/,
  );
  assert.match(
    fs.readFileSync("src-tauri/src/lib.rs", "utf8"),
    /commands::rollback_invalid_file/,
  );
  assert.match(
    commands,
    /let eligible = match version_id\.as_deref\(\)[\s\S]{0,180}None => expected_gate\.can_recover\(file\)/,
  );
  assert.match(commands, /expected_gate\.can_rollback\(file, version_id\)/);
});

test("recovery is snapshot-bound and preserves whole files with no-replace rename", () => {
  assert.match(durable, /pub\(crate\) enum DurableFile \{\s*Settings,\s*Connections,/);
  assert.match(durable, /if !expected\.matches\(&current\) \{\s*return Err\(RecoveryFileError::Stale\)/);
  assert.match(durable, /validate_selected_file\(file, path\)/);
  assert.match(durable, /crate::storage::rename_noreplace\(path, &backup_path\)/);
  assert.match(durable, /!expected\.matches\(&preserved\) \|\| preserved != current/);
  assert.match(durable, /install_selected_default\(file, path\)/);
  const at = commands.indexOf("pub async fn recover_invalid_file(");
  const signature = commands.slice(at, commands.indexOf(") ->", at));
  assert.doesNotMatch(signature, /\b(?:path|name):|String/);
});

test("rollback history is private, bounded, hash-bound, and backend-selected", () => {
  assert.match(
    config,
    /update_json_before_save[\s\S]{0,700}preserve_valid_history\([\s\S]{0,100}DurableFile::Settings/,
  );
  assert.match(
    connections,
    /update_json_before_save[\s\S]{0,700}preserve_valid_history\([\s\S]{0,100}DurableFile::Connections/,
  );
  assert.match(durable, /const HISTORY_LIMIT: usize = 3/);
  assert.match(
    durable,
    /"\{\}\.valid-\{\}-\{\}\.json"[\s\S]{0,180}sha256_hex\(bytes\)/,
  );
  assert.match(durable, /versions\.iter\(\)\.skip\(HISTORY_LIMIT\)/);
  assert.match(durable, /validate_selected_bytes\(file, bytes\)/);
  assert.match(durable, /crate::storage::write_private_new\(&path, bytes\)/);
  assert.match(durable, /sha256_hex\(&bytes\) != parsed\.sha256/);
  assert.match(
    durable,
    /exact_history_bytes\(file, path, version\)\.map_err\(\|_\| RecoveryFileError::Stale\)/,
  );
  assert.match(durable, /finish_selected_recovery_with_replacement/);
  assert.doesNotMatch(
    commands.slice(
      commands.indexOf("pub async fn rollback_invalid_file("),
      commands.indexOf(") ->", commands.indexOf("pub async fn rollback_invalid_file(")),
    ),
    /\bpath:/,
  );
});

test("recorded recovery fails closed across restart and resumes only exact states", () => {
  assert.match(
    attributesBefore(durable, "struct RecoveryMarker"),
    /serde\(rename_all = "camelCase", deny_unknown_fields\)/,
  );
  const hook = durable.indexOf("before_rename()");
  const rename = durable.indexOf(
    "crate::storage::rename_noreplace(path, &backup_path)",
    hook,
  );
  assert.ok(hook >= 0 && rename > hook, "the private marker must precede the rename");
  const load = durable.indexOf("pub(crate) fn load()");
  const markerCheck = durable.indexOf("load_recovery_marker(&marker_path)", load);
  const configLoad = durable.indexOf("config::load_unmigrated_at(&config_path)", load);
  assert.ok(
    load >= 0 && markerCheck > load && configLoad > markerCheck,
    "startup must block on the recovery marker before accepting settings",
  );
  assert.match(commands, /crate::durable::resume_incomplete_recovery\(gate\)/);
  assert.match(
    durable,
    /match \(current, backup\) \{[\s\S]*\(Some\(current\), None\) if expected\.matches\(&current\)[\s\S]*\(None, Some\(backup\)\)[\s\S]*installed_replacement_matches\(marker\.file, &path, &marker\.replacement\)/,
  );
  assert.match(durable, /replacement: RecoveryReplacement/);
  assert.match(durable, /RecoveryMarker::new_history/);
});

test("active source writes target connections instead of settings", () => {
  assert.match(commands, /connections::update\(move \|stored\| stored\.upsert\(cfg\)\)/);
  assert.match(commands, /connections::update\(move \|cfg\| remove_source_config/);
  assert.match(
    fs.readFileSync("src-tauri/src/source/plex.rs", "utf8"),
    /crate::connections::update/,
  );
  assert.doesNotMatch(commands, /config::update\([\s\S]{0,240}\.upsert\(/);
  assert.doesNotMatch(commands, /config::update\([\s\S]{0,240}\.sources/);
  assert.match(
    fs.readFileSync("src-tauri/src/source/plex.rs", "utf8"),
    /pub fn build_source\([\s\S]{0,160}-> Result</,
  );
  assert.match(
    fs.readFileSync("src-tauri/src/source/jellyfin.rs", "utf8"),
    /pub fn build_source\([\s\S]{0,160}-> Result</,
  );
});

test("Plex credentials never enter query strings or frontend artwork URLs", () => {
  const plexRequests = `${plexApi}\n${plexLibrary}`;
  assert.doesNotMatch(
    plexRequests,
    /(?:insert|append_pair)\(\s*["']X-Plex-Token["']/i,
  );
  assert.doesNotMatch(
    plexRequests,
    /\.query\(\s*&?\s*\[\s*\(\s*["']X-Plex-Token["']/i,
  );
  // Finding tr-10: the start URL used a local query array, so neither the
  // append-pair nor inline `.query(&[(...)]` shapes above could see it.
  assert.doesNotMatch(
    plexLibrary,
    /\(\s*["']X-Plex-Token["']\s*,\s*self\.auth_token(?:\.clone\(\))?\s*\)/i,
    "Plex query tuples must never carry the active token",
  );
  assert.doesNotMatch(plexLibrary, /poster_transcode_url|X-Plex-Token=\{/i);
  assert.match(plexApi, /\.header\("X-Plex-Token", auth_token\)/);
  assert.match(plexLibrary, /\.header\("X-Plex-Token", &self\.auth_token\)/);
  assert.match(plexSource, /crate::artwork::plex_artwork_url/);
  assert.match(artwork, /ARTWORK_MARKER_PREFIX: &str = "vela-artwork:"/);
  assert.match(artwork, /sanitize_item_artwork/);
  assert.match(config, /sanitize_legacy_artwork\(cfg\)/);
  assert.match(playlists, /sanitize_legacy_artwork_in/);
  assert.match(artwork, /request\.uri\(\)\.query\(\)\.is_some\(\)/);
  assert.match(artwork, /MAX_ARTWORK_BYTES/);
  assert.match(artwork, /accepted_image_mime/);
  assert.match(plexLibrary, /redirect\(reqwest::redirect::Policy::none\(\)\)/);
  assert.match(
    page,
    /convertFileSrc\(p\.slice\("vela-artwork:"\.length\), "vela-artwork"\)/,
  );
  assert.match(tauriConfig, /img-src[^"]*http:\/\/vela-artwork\.localhost/);
  assert.match(plexLibrary, /fn part_url\([^)]*auth_token: &str/);
  assert.match(plexLibrary, /key\.eq_ignore_ascii_case\("X-Plex-Token"\)/);
  assert.match(plexLibrary, /provider_part_keys_with_credentials_fail_closed/);
  assert.doesNotMatch(plexLibrary, /Plex API error:[^\\n]*Body:/);
  assert.doesNotMatch(mockPlex, /\btoken:\s*req\.headers/);
  assert.match(mockPlex, /tokenMatches: receivedToken === token/);
  assert.match(mockPlex, /String\(value\)\.includes\(token\)/);
});

test("Tauri emits a Windows-compatible artwork protocol origin", async () => {
  globalThis.window = {};
  const { clearMocks, mockConvertFileSrc } = await import("@tauri-apps/api/mocks");
  const { convertFileSrc } = await import("@tauri-apps/api/core");
  try {
    mockConvertFileSrc("windows");
    assert.equal(
      convertFileSrc("opaque.payload.300.450", "vela-artwork"),
      "http://vela-artwork.localhost/opaque.payload.300.450",
    );
  } finally {
    clearMocks();
    delete globalThis.window;
  }
});

test("mpv authentication stays in a private include until its exact child is reaped", () => {
  assert.match(playback, /struct HeaderInclude[\s\S]{0,800}impl Drop for HeaderInclude/);
  assert.match(playback, /struct ManagedChild[\s\S]{0,180}_header_include: Option<HeaderInclude>/);
  assert.match(playback, /mpv-headers-\{\}-\{nonce\}\.conf/);
  assert.match(playback, /opts\.mode\(0o600\)/);
  assert.match(
    playback,
    /harden_existing_regular\(path\)[\s\S]{0,500}write_content\(&mut f, content\.as_bytes\(\)\)/,
  );
  // The reaper keeps a player until its end is confirmed. That is three links:
  // the reaper asks the handle, the handle asks the process, and only a
  // confirmed end releases the player (and the include it owns).
  assert.match(
    lib,
    /retain_mut\(\|child\|[\s\S]{0,120}retain_after_poll\(&child\.has_ended\(\)\)/,
  );
  assert.match(
    playerHandle,
    /PlayerHandle::External\(child\) => child\.try_wait\(\)\.map\(\|status\| status\.is_some\(\)\)/,
  );
  assert.match(
    playerHandle,
    /fn retain_after_poll\(ended: &std::io::Result<bool>\) -> bool \{\s+!matches!\(ended, Ok\(true\)\)\s+\}/,
  );
  assert.match(playback, /partial_header_include_write_removes_the_credential_file/);
  assert.match(playback, /the_reaper_drops_a_player_only_on_a_confirmed_end/);
  assert.match(playback, /an_external_player_reports_its_end_only_after_the_process_exits/);
  assert.match(
    commands,
    /remove_consumed_header_include\(\)[\s\S]{0,180}child\.kill\(\)/,
  );
  assert.match(
    lib,
    /reap_queue[\s\S]{0,500}remove_consumed_header_include\(\)[\s\S]{0,200}queue\.clear\(\)/,
  );
  // The include reaches mpv as one option of the described launch, which the
  // external player renders as `--include=<path>`.
  assert.match(playerLaunch, /LaunchArg::set\("include", include\.path\.to_string_lossy\(\)\)/);
  assert.match(playerExternal, /cmd\.arg\(arg\.to_cli\(\)\);/);
  // Stream authentication must not reach mpv as an option. An option used to
  // be command-line text, and this test looked for that text; an option is now
  // data (`LaunchArg`), which that text never matched. So the check is made on
  // what cannot be respelled, over the code that describes and starts a launch:
  const shipped = [playbackSupervision, playerLaunch, playerExternal, playerHandle]
    .map(shippedCode)
    .join("\n");
  // 1. a play's headers are declared once and read in two places, the test for
  //    "are there any" and the call that writes the include;
  assert.deepEqual(linesMentioning(shipped, /http_headers/), [
    "pub http_headers: Vec<(String, String)>,",
    "let header_include = if spec.http_headers.is_empty() {",
    "let include = write_header_include(&spec.http_headers)?;",
  ]);
  // 2. mpv's header option is named once, as the content of that include, under
  //    any spelling or option constructor;
  assert.deepEqual(linesMentioning(shipped, /http-header-fields/i), [
    'let content = format!("http-header-fields=\\"{joined}\\"\\n");',
  ]);
  // 3. and what the player is really started with is checked as well, in Rust.
  assert.match(
    playerExternal,
    /fn stream_auth_reaches_the_player_only_through_the_private_include\(\) \{/,
  );
  assert.doesNotMatch(plexSource, /X-Plex-Token=.*auth_token/);
});
