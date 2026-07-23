// A damaged connections file has its own blocking copy and recovery action.
// Recovery preserves settings/playlists, installs an empty connection store,
// and enters the real no-sources reconnect flow.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

let seeded;

function paths(configRoot) {
  const dir = path.join(configRoot, 'config', 'vela');
  return {
    dir,
    config: path.join(dir, 'config.json'),
    connections: path.join(dir, 'connections.json'),
    playlists: path.join(dir, 'playlists.json'),
  };
}

function backups(dir) {
  return fs.readdirSync(dir).filter((name) =>
    name.startsWith('connections.invalid-') && name.endsWith('.json'));
}

async function waitForFault(driver, action) {
  await driver.waitFor(
    `return document.querySelector('#durable-fault-heading')?.textContent.trim() ===
      'Vela could not safely read your server connections.'`,
    'the damaged-connections blocking screen',
  );
  const body = await driver.exec(`return document.body.innerText`);
  assert.ok(body.includes('No connection or token was loaded.'));
  assert.ok(body.includes('Your settings, recents, and playlists will not be reset.'));
  assert.ok(!body.includes('Welcome to Vela'));
  assert.ok(!body.includes('synthetic-damaged-token'));
  return driver.find('xpath', `//button[normalize-space(.)='${action}']`);
}

export default {
  name: 'connectionrecovery',

  seed({ configRoot }) {
    const files = paths(configRoot);
    fs.mkdirSync(files.dir, { recursive: true });
    seeded = {
      config: Buffer.from('{"continue_playing":"on"}'),
      connections: Buffer.from(
        '{"sources":[{"id":"broken","kind":"future","access_token":"synthetic-damaged-token"}]}',
      ),
      playlists: Buffer.from('{"schemaVersion":1,"playlists":[]}'),
    };
    fs.writeFileSync(files.config, seeded.config, { mode: 0o600 });
    fs.writeFileSync(files.connections, seeded.connections, { mode: 0o600 });
    fs.writeFileSync(files.playlists, seeded.playlists, { mode: 0o600 });
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const files = paths(configRoot);
    await screenshot('01-blocked');
    const exit = await waitForFault(driver, 'Exit Vela');
    await driver.click(exit).catch(() => {});
    await new Promise((resolve) => setTimeout(resolve, 250));
    assert.deepEqual(fs.readFileSync(files.config), seeded.config);
    assert.deepEqual(fs.readFileSync(files.connections), seeded.connections);
    assert.deepEqual(fs.readFileSync(files.playlists), seeded.playlists);

    await restart();
    const recover = await waitForFault(driver, 'Rename damaged connections and reconnect');
    await driver.click(recover);
    await driver.waitFor(
      `return [...document.querySelectorAll('h1,h2')]
        .some((heading) => heading.textContent.trim() === 'Welcome to Vela')`,
      'the no-sources reconnect flow',
    );

    const held = backups(files.dir);
    assert.equal(held.length, 1, 'connection recovery must create exactly one invalid backup');
    assert.deepEqual(fs.readFileSync(path.join(files.dir, held[0])), seeded.connections);
    assert.equal(fs.statSync(path.join(files.dir, held[0])).mode & 0o777, 0o600);
    assert.deepEqual(JSON.parse(fs.readFileSync(files.connections, 'utf8')), { sources: [] });
    assert.deepEqual(fs.readFileSync(files.config), seeded.config);
    assert.deepEqual(fs.readFileSync(files.playlists), seeded.playlists);
    const notice = await driver.exec(`return document.querySelector('.notice')?.textContent ?? ''`);
    assert.ok(notice.includes(held[0]));
    assert.ok(notice.includes('Connect your servers again'));
    assert.ok(!(await driver.exec(`return document.body.innerText`)).includes('synthetic-damaged-token'));
    await screenshot('02-reconnect');

    await restart();
    await driver.waitFor(
      `return [...document.querySelectorAll('h1,h2')]
        .some((heading) => heading.textContent.trim() === 'Welcome to Vela')`,
      'fresh empty connections after restart',
    );
    assert.equal(
      await driver.exec(`return document.querySelector('#durable-fault-heading')?.textContent ?? null`),
      null,
    );
  },
};
