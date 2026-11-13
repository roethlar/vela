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

// The index of the brace that closes the one at `open`. Reads Rust well enough
// to ignore the braces that are not code: those in comments, in string
// literals and in character literals.
//
// A string literal in Rust is a quote with, optionally, a prefix written
// directly before it: none, `b` or `c` for one with escapes, and `r`, `br` or
// `cr` with any number of `#` for a raw one. So words are read as words, and
// a word that runs into a quote is a prefix. One this reader does not know is
// refused, never guessed at: whatever it misreads here, it hides from the
// checks that use it. (It once knew only `r` and `br`, and looked no further
// than 62 `#`: review finding ps-2, round 3.)
function matchingBrace(text, open) {
  assert.equal(text[open], "{", "matching starts at an opening brace");
  const isWordStart = (c) => c !== undefined && /[A-Za-z_]/.test(c);
  const isWord = (c) => c !== undefined && /[A-Za-z0-9_]/.test(c);
  // From an opening quote to just past its closing one; a backslash takes the
  // next character with it.
  const afterString = (quote) => {
    let at = quote + 1;
    while (at < text.length && text[at] !== '"') at += text[at] === "\\" ? 2 : 1;
    return at + 1;
  };
  let depth = 0;
  let i = open;
  while (i < text.length) {
    const c = text[i];
    if (text.startsWith("//", i)) {
      const end = text.indexOf("\n", i);
      if (end === -1) break;
      i = end + 1;
    } else if (text.startsWith("/*", i)) {
      // Block comments nest in Rust.
      let level = 1;
      i += 2;
      while (i < text.length && level > 0) {
        if (text.startsWith("/*", i)) {
          level += 1;
          i += 2;
        } else if (text.startsWith("*/", i)) {
          level -= 1;
          i += 2;
        } else {
          i += 1;
        }
      }
    } else if (c === '"') {
      i = afterString(i);
    } else if (isWordStart(c)) {
      let wordEnd = i + 1;
      while (isWord(text[wordEnd])) wordEnd += 1;
      const word = text.slice(i, wordEnd);
      let hashes = 0;
      while (text[wordEnd + hashes] === "#") hashes += 1;
      const quote = wordEnd + hashes;
      if (text[quote] !== '"') {
        // An ordinary word. (`r#name`, a raw identifier, is the word `r`,
        // a `#`, and the word `name`; `b'x'` is the word `b` and a
        // character literal.)
        i = wordEnd;
      } else if (["r", "br", "cr"].includes(word)) {
        // A raw string ends at a quote followed by as many `#` as opened it;
        // nothing inside it is an escape.
        const end = text.indexOf(`"${"#".repeat(hashes)}`, quote + 1);
        assert.notEqual(end, -1, "a raw string must end");
        i = end + 1 + hashes;
      } else if (hashes === 0 && ["b", "c"].includes(word)) {
        i = afterString(quote);
      } else {
        assert.fail(
          `a literal with a prefix this reader does not know: ${word}${"#".repeat(hashes)}"`,
        );
      }
    } else if (c === "'") {
      if (text[i + 1] === "\\") {
        // An escaped character: '\n', '\'', '\\', '\u{7f}'.
        const end = text.indexOf("'", i + 3);
        assert.notEqual(end, -1, "a character literal must end");
        i = end + 1;
      } else if (text[i + 2] === "'") {
        i += 3; // 'x', including '{' and '"'
      } else if (text[i + 3] === "'" && text.codePointAt(i + 1) > 0xffff) {
        i += 4; // one character outside the basic plane
      } else {
        i += 1; // a lifetime or a loop label, not a literal
      }
    } else {
      if (c === "{") depth += 1;
      if (c === "}") {
        depth -= 1;
        if (depth === 0) return i;
      }
      i += 1;
    }
  }
  assert.fail("no closing brace");
}

// The part of a Rust file that is compiled into the app: all of it except its
// test module, without comment lines.
//
// Where the module sits and how the code around it is laid out do not matter:
// the module is cut out from its `mod tests {` to the brace that closes it,
// and everything else is returned, before it and after it. (An earlier version
// took what came before the module and required the module to be last, judged
// by indentation. Indented code after the module was compiled into the app and
// read by nothing: review finding ps-2, twice.)
function shippedCode(source, file) {
  const text = source.replace(/\r\n/g, "\n");
  const marker = "\n#[cfg(test)]\nmod tests {";
  const at = text.indexOf(marker);
  assert.notEqual(at, -1, `${file} must have a test module`);
  assert.equal(text.indexOf(marker, at + 1), -1, `${file} must have exactly one test module`);
  const close = matchingBrace(text, at + marker.length - 1);
  const code = `${text.slice(0, at)}\n${text.slice(close + 1)}`;
  // Two checks that the module ends where it was taken to end. A test left
  // outside it means the end was put too early; no function left at all means
  // the module took the code with it.
  assert.doesNotMatch(
    code,
    /^\s*#\[(tokio::)?test\b/m,
    `${file}: a test was found outside the test module`,
  );
  assert.match(code, /\bfn \w+\s*[(<]/, `${file}: no code compiled into the app was found`);
  return code
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

test("brace matching reads Rust strings, characters and comments", () => {
  // `shippedCode` below depends on finding where a test module ends. Each
  // case is one way a brace can appear in Rust without being code; `z }` is a
  // later brace the match must not run on to.
  const matched = (rust) => {
    const open = rust.indexOf("{");
    return rust.slice(open, matchingBrace(rust, open) + 1);
  };
  assert.equal(matched("m { a { b } c } z }"), "{ a { b } c }");
  assert.equal(matched('m { "}" } z }'), '{ "}" }', "a brace in a string");
  assert.equal(matched('m { "\\"}" } z }'), '{ "\\"}" }', "after an escaped quote in a string");
  assert.equal(matched('m { "\\\\" } z }'), '{ "\\\\" }', "a string that ends in a backslash");
  assert.equal(matched("m { '}' } z }"), "{ '}' }", "a brace character");
  assert.equal(matched("m { '\\'' '}' } z }"), "{ '\\'' '}' }", "after an escaped quote character");
  assert.equal(matched("m { '\\u{7d}' } z }"), "{ '\\u{7d}' }", "braces of a unicode escape");
  assert.equal(matched("m { '\"' } z }"), "{ '\"' }", "a quote character opens no string");
  assert.equal(matched("m { // }\n } z }"), "{ // }\n }", "a brace in a line comment");
  assert.equal(
    matched("m { /* } /* } */ } */ } z }"),
    "{ /* } /* } */ } */ }",
    "braces in nested block comments",
  );
  assert.equal(matched('m { r"}" } z }'), '{ r"}" }', "a raw string");
  assert.equal(matched('m { r#"a"}"# } z }'), '{ r#"a"}"# }', "a raw string with a quote in it");
  assert.equal(matched('m { r"\\" } z }'), '{ r"\\" }', "a raw string has no escapes");
  assert.equal(matched('m { br#"}"# b"}" } z }'), '{ br#"}"# b"}" }', "byte strings");
  assert.equal(matched('m { c"}" cr"}" } z }'), '{ c"}" cr"}" }', "C strings");
  assert.equal(
    matched('m { cr#""{""# } z }'),
    '{ cr#""{""# }',
    "a raw C string with quotes and a lone brace in it",
  );
  const manyHashes = "#".repeat(70);
  assert.equal(
    matched(`m { r${manyHashes}"}"#"${manyHashes} } z }`),
    `{ r${manyHashes}"}"#"${manyHashes} }`,
    "a raw string with more hash marks than anyone writes",
  );
  assert.equal(
    matched('m { let r#type = bar; let other = r; { } } z }'),
    "{ let r#type = bar; let other = r; { } }",
    "a raw identifier, and words that merely end in a prefix letter, are not literals",
  );
  assert.equal(matched("m { b'}' b'\\'' } z }"), "{ b'}' b'\\'' }", "byte characters");
  // What it does not know, it refuses.
  assert.throws(() => matched('m { k#"}"# } z }'), /a prefix this reader does not know: k#"/);
  assert.throws(() => matched('m { rb"}" } z }'), /a prefix this reader does not know: rb"/);
  assert.throws(() => matched('m { b#"}"# } z }'), /a prefix this reader does not know: b#"/);
  assert.throws(() => matched('m { r#"} z }'), /a raw string must end/);
  assert.equal(
    matched("m { fn f<'a>(x: &'a str) -> &'a str { x } } z }"),
    "{ fn f<'a>(x: &'a str) -> &'a str { x } }",
    "a lifetime is not a character",
  );
  assert.throws(() => matched("m { a "), /no closing brace/);
  assert.throws(() => matched("m { // }"), /no closing brace/);
});

test("the reader of shipped Rust code takes everything but the test module", () => {
  const tests = [
    "",
    "#[cfg(test)]",
    "mod tests {",
    "    use super::*;",
    "    #[test]",
    "    fn inside_the_tests() {",
    '        assert_eq!(format!("{{}}"), "}");',
    "        let _closing = '}'; // }",
    "    }",
    "}",
    "",
  ].join("\n");
  const before = "fn before() -> &'static str {\n    \"a\"\n}\n";
  const read = (source) => shippedCode(source, "sample.rs");

  // Code before the module, and code after it in the first column.
  const around = read(`${before}${tests}fn after() {}\n`);
  assert.match(around, /fn before\(\)/);
  assert.match(around, /fn after\(\)/);
  assert.doesNotMatch(around, /inside_the_tests/, "the module itself is left out");
  // Code after the module that does not start in the first column. This is
  // the layout that went unread (review finding ps-2).
  const indented = read(`${before}${tests}    fn indented_after(spec: &PlaySpec) {\n        spec.http_headers.len();\n    }\n`);
  assert.match(indented, /fn indented_after/);
  assert.match(indented, /spec\.http_headers\.len\(\);/);
  // A module above the code it tests.
  assert.match(read(`use std::io;\n${tests}fn below() {}\n`), /fn below\(\)/);
  // Comment lines are dropped; code lines are not.
  const commented = read(`${before}// fn in_a_comment() {}\n${tests}`);
  assert.doesNotMatch(commented, /in_a_comment/);

  // The layout a reader that misjudged a literal fell for (review finding
  // ps-2, round 3): a lone brace in a raw C string inside the tests, its twin
  // in code after them. The code after them must still be read.
  const paired = read(
    [
      before,
      "",
      "#[cfg(test)]",
      "mod tests {",
      '    const _OPEN: &std::ffi::CStr = cr#""{""#;',
      "    #[test]",
      "    fn inside_the_tests() {}",
      "}",
      "",
      "fn after_with_its_twin(spec: &PlaySpec) -> usize {",
      '    let _close = cr#""}""#;',
      "    spec.http_headers.len()",
      "}",
      "",
    ].join("\n"),
  );
  assert.match(paired, /fn after_with_its_twin/);
  assert.match(paired, /spec\.http_headers\.len\(\)/);
  assert.doesNotMatch(paired, /inside_the_tests/);

  // What must be refused rather than read as "nothing to check".
  assert.throws(() => read(`use std::io;\n${tests}`), /no code compiled into the app was found/);
  assert.throws(() => read(before), /must have a test module/);
  assert.throws(() => read(`${before}${tests}${tests}`), /exactly one test module/);
  assert.throws(
    () => read(`${before}${tests}#[test]\nfn outside() {}\n`),
    /a test was found outside the test module/,
  );
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
  const launchCode = shippedCode(playerLaunch, "player/launch.rs");
  const shipped = [
    shippedCode(playbackSupervision, "playback.rs"),
    launchCode,
    shippedCode(playerExternal, "player/external.rs"),
    shippedCode(playerHandle, "player/mod.rs"),
  ].join("\n");
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
  // 3. inside the code that writes the include, every line that names the
  //    headers, the text they are joined into or the file's content is one of
  //    these: nothing copies them anywhere else;
  assert.deepEqual(linesMentioning(launchCode, /(?<![\w-])(headers|joined|content)(?![\w-])/), [
    "fn write_header_include(headers: &[(String, String)]) -> Result<HeaderInclude, String> {",
    "write_header_include_at(&path, headers)",
    "headers: &[(String, String)],",
    "write_header_include_at_with(path, headers, |file, content| file.write_all(content))",
    "headers: &[(String, String)],",
    "for (name, value) in headers {",
    "let joined = headers",
    'let content = format!("http-header-fields=\\"{joined}\\"\\n");',
    "if let Err(error) = write_content(&mut f, content.as_bytes()) {",
  ]);
  // 4. none of this code changes Vela's own environment, which every program
  //    Vela starts inherits in full;
  assert.doesNotMatch(shipped, /\b(set_var|remove_var)\b/);
  // 5. and what the player is really started with, arguments and whole
  //    environment, is checked as well, in Rust.
  assert.match(
    playerExternal,
    /fn stream_auth_reaches_the_player_only_through_the_private_include\(\) \{/,
  );
  assert.match(playerExternal, /for \(key, value\) in effective_env\(&cmd, &own\) \{/);
  assert.doesNotMatch(plexSource, /X-Plex-Token=.*auth_token/);
});
