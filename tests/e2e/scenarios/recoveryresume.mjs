// A crash after the damaged settings file was renamed leaves a private marker
// and exact backup. Relaunch blocks instead of treating the absent config as a
// first run; Retry installs fresh settings only from that exact recorded state.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
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
    marker: path.join(dir, 'durable-recovery.json'),
    backup: path.join(
      dir,
      'config.invalid-1-00000000-0000-0000-0000-000000000000.json',
    ),
  };
}

export default {
  name: 'recoveryresume',

  seed({ configRoot }) {
    const files = paths(configRoot);
    fs.mkdirSync(files.dir, { recursive: true });
    const damaged = Buffer.from(
      '{"continue_playing":"future","private_value":"synthetic-marker-secret"}',
    );
    seeded = {
      backup: damaged,
      connections: Buffer.from('{"sources":[]}'),
      playlists: Buffer.from('{"schemaVersion":1,"playlists":[]}'),
      marker: Buffer.from(
        JSON.stringify({
          file: 'settings',
          layout: 'post_split',
          backupFileName: path.basename(files.backup),
          byteLength: damaged.length,
          sha256: crypto.createHash('sha256').update(damaged).digest('hex'),
        }),
      ),
    };
    fs.writeFileSync(files.backup, seeded.backup, { mode: 0o600 });
    fs.writeFileSync(files.connections, seeded.connections, { mode: 0o600 });
    fs.writeFileSync(files.playlists, seeded.playlists, { mode: 0o600 });
    fs.writeFileSync(files.marker, seeded.marker, { mode: 0o600 });
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const files = paths(configRoot);
    await driver.waitFor(
      `return document.querySelector('#durable-fault-heading')?.textContent.trim() ===
        'Vela could not safely read your settings.'`,
      'the interrupted-recovery blocking screen',
    );
    let body = await driver.exec(`return document.body.innerText`);
    assert.ok(body.includes('could not finish a protected settings or connection update'));
    assert.ok(body.includes('Try again'));
    assert.ok(body.includes('Exit Vela'));
    assert.ok(!body.includes('Rename and create new settings'));
    assert.ok(!body.includes('Welcome to Vela'));
    assert.ok(!body.includes('synthetic-marker-secret'));
    await screenshot('01-blocked');

    const exit = await driver.find('xpath', `//button[normalize-space(.)='Exit Vela']`);
    await driver.click(exit).catch(() => {});
    await new Promise((resolve) => setTimeout(resolve, 250));
    assert.ok(!fs.existsSync(files.config));
    assert.deepEqual(fs.readFileSync(files.backup), seeded.backup);
    assert.deepEqual(fs.readFileSync(files.marker), seeded.marker);
    assert.deepEqual(fs.readFileSync(files.connections), seeded.connections);
    assert.deepEqual(fs.readFileSync(files.playlists), seeded.playlists);

    await restart();
    await driver.waitFor(
      `return document.querySelector('#durable-fault-heading')?.textContent.trim() ===
        'Vela could not safely read your settings.'`,
      'the interrupted recovery after relaunch',
    );
    const retry = await driver.find('xpath', `//button[normalize-space(.)='Try again']`);
    await driver.click(retry);
    await driver.waitFor(
      `return [...document.querySelectorAll('h1,h2')]
        .some((heading) => heading.textContent.trim() === 'Welcome to Vela')`,
      'fresh settings after recorded recovery resumes',
    );

    assert.ok(!fs.existsSync(files.marker), 'successful resume must remove the marker');
    assert.deepEqual(fs.readFileSync(files.backup), seeded.backup);
    assert.equal(fs.statSync(files.backup).mode & 0o777, 0o600);
    assert.doesNotThrow(() => JSON.parse(fs.readFileSync(files.config, 'utf8')));
    assert.deepEqual(fs.readFileSync(files.connections), seeded.connections);
    assert.deepEqual(fs.readFileSync(files.playlists), seeded.playlists);
    body = await driver.exec(`return document.body.innerText`);
    assert.ok(!body.includes('synthetic-marker-secret'));
    await screenshot('02-resumed');

    await restart();
    await driver.waitFor(
      `return [...document.querySelectorAll('h1,h2')]
        .some((heading) => heading.textContent.trim() === 'Welcome to Vela')`,
      'normal startup after recorded recovery',
    );
    assert.equal(
      await driver.exec(
        `return document.querySelector('#durable-fault-heading')?.textContent ?? null`,
      ),
      null,
    );
  },
};
