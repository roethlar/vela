import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

const config = fs.readFileSync("src-tauri/src/config.rs", "utf8");
const connections = fs.readFileSync("src-tauri/src/connections.rs", "utf8");
const commands = fs.readFileSync("src-tauri/src/commands.rs", "utf8");
const durable = fs.readFileSync("src-tauri/src/durable.rs", "utf8");
const playback = fs.readFileSync("src-tauri/src/playback.rs", "utf8");
const backend = `${commands}\n${playback}`;
const page = fs.readFileSync("src/routes/+page.svelte", "utf8");

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
  assert.match(
    page,
    /fault\.value\.status === "recoverable_invalid" && fault\.value\.canRecover/,
  );
  assert.match(
    page,
    /<button[\s\S]{0,220}onclick=\{\(\) => recoverInvalidFile\(fault\.file\)\}/,
  );
  assert.match(page, /Rename and create new settings/);
  assert.match(page, /Rename damaged connections and reconnect/);
  assert.match(page, /<button disabled=\{durableBusy\} onclick=\{exitVela\}>Exit Vela<\/button>/);
  assert.match(page, /aria-busy=\{durableBusy\}/);
  assert.match(page, /durableHeading\?\.focus\(\)/);
  assert.match(
    fs.readFileSync("src-tauri/src/lib.rs", "utf8"),
    /commands::recover_invalid_file/,
  );
  assert.match(commands, /if !expected_gate\.can_recover\(file\)/);
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
    /match \(current, backup\) \{[\s\S]*\(Some\(current\), None\) if expected\.matches\(&current\)[\s\S]*\(None, Some\(backup\)\)[\s\S]*validate_selected_file\(marker\.file, &path\)\.is_ok\(\)/,
  );
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
