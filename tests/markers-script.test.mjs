// vela-markers.lua run by a real Lua against a fake mpv: what the script
// publishes and does, given the property values and observer calls mpv would
// make. It runs under each of the two Luas mpv runs the script with: LuaJIT
// (the system mpv's, usually) and Lua 5.2 (the libmpv Vela ships;
// packaging/libmpv/versions.txt). A Lua that is not installed is skipped,
// with that reason; CI's `frontend` job installs both, and the last test
// holds it to that.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const script = path.join(repoRoot, 'src-tauri/resources/mpv-scripts/vela-markers.lua');
const LUAS = ['luajit', 'lua5.2'];
const installed = (lua) => spawnSync(lua, ['-v'], { stdio: 'ignore' }).status === 0;

// The fake mpv. Its payload is one intro, 2 s to 12 s, under the policy the
// test names, drawn. The play is paused at 3 s, inside the intro, and the
// position is seen before the OSD has a size; then the size arrives, and no
// further position does (a paused play sends none). Prints what the script
// published before and after the size, and how many seeks it asked for.
const HARNESS = String.raw`
local script, payload, policy = ...
local props = { ["osd-dimensions"] = { w = 0, h = 0 } }
local set, observers, seeks = {}, {}, 0
package.preload["mp.utils"] = function()
  return { parse_json = function() return { markers = { { kind = "intro", start_ms = 2000, end_ms = 12000 } } } end }
end
package.preload["mp.options"] = function()
  return { read_options = function(o) o["intro-policy"] = policy; o["payload"] = payload; o["draw"] = true end }
end
local function nothing() end
mp = {
  get_property_native = function(name) return props[name] end,
  get_property_number = function(name) return props[name] end,
  set_property_native = function(name, value) set[name] = value end,
  observe_property = function(name, _, fn) observers[name] = fn end,
  commandv = function(command) if command == "seek" then seeks = seeks + 1 end end,
  register_event = nothing, add_key_binding = nothing, add_forced_key_binding = nothing,
  remove_key_binding = nothing, input_define_section = nothing, input_enable_section = nothing,
  input_disable_section = nothing, set_mouse_area = nothing, osd_message = nothing,
  get_script_name = function() return "vela_markers" end,
  create_osd_overlay = function() return { update = nothing, remove = nothing } end,
}
dofile(script)
props["time-pos"] = 3
observers["time-pos"]("time-pos", 3)
io.write("before=", tostring(set["user-data/vela-markers/active"]), "\n")
props["osd-dimensions"] = { w = 1280, h = 720 }
observers["osd-dimensions"]("osd-dimensions", props["osd-dimensions"])
io.write("after=", tostring(set["user-data/vela-markers/active"]), "\n")
io.write("seeks=", tostring(seeks), "\n")
`;

function runScript(lua, policy) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-markers-test-'));
  try {
    const payload = path.join(dir, 'payload.json');
    fs.writeFileSync(payload, '{}');
    const harness = path.join(dir, 'harness.lua');
    fs.writeFileSync(harness, HARNESS);
    const run = spawnSync(lua, [harness, script, payload, policy], { encoding: 'utf8' });
    assert.equal(run.status, 0, run.stderr);
    return Object.fromEntries(run.stdout.trim().split('\n').map((line) => line.split('=')));
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

for (const lua of LUAS) {
  const skip = !installed(lua) && `${lua} is not installed`;

  test(`${lua}: a play paused inside a marker shows its button once the OSD has a size`, { skip }, () => {
    const seen = runScript(lua, 'button');
    assert.equal(seen.before, '', 'nothing is shown while the OSD has no size');
    assert.equal(seen.after, 'intro', 'the size arriving shows the button of the marker the play is paused in');
  });

  test(`${lua}: the size arriving does not skip a marker that was already skipped`, { skip }, () => {
    const seen = runScript(lua, 'autoskip');
    assert.equal(seen.seeks, '1', 'one skip: the position inside the intro, not the size arriving after it');
  });
}

test('CI installs both Luas before it runs npm run check, so the tests above run there', () => {
  const ci = fs.readFileSync(path.join(repoRoot, '.github/workflows/ci.yml'), 'utf8');
  const start = ci.indexOf('\n  frontend:\n');
  assert.ok(start >= 0, 'ci.yml has a frontend job');
  const rest = ci.slice(start + 1);
  const end = rest.slice(1).search(/\n {2}[a-z][a-z0-9-]*:\n/);
  const job = end < 0 ? rest : rest.slice(0, end + 1);
  const check = job.indexOf('- run: npm run check');
  assert.ok(check >= 0, 'the frontend job runs npm run check');
  const installs = [...job.slice(0, check).matchAll(/apt-get install -y ([^\n]*)/g)].flatMap((m) => m[1].trim().split(/\s+/));
  for (const lua of LUAS) {
    assert.ok(installs.includes(lua), `the frontend job installs ${lua} before npm run check (it installs: ${installs.join(' ')})`);
  }
});
