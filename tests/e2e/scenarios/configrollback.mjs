// Damaged settings expose the three newest private validated versions as dated
// buttons. Selecting the middle version preserves the damage and restores only
// that exact settings document.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const versions = [
  {
    createdAt: Date.UTC(2026, 6, 20, 12, 0),
    bytes: Buffer.from('{"continue_playing":"off"}'),
  },
  {
    createdAt: Date.UTC(2026, 6, 21, 12, 0),
    bytes: Buffer.from('{"continue_playing":"on"}'),
  },
  {
    createdAt: Date.UTC(2026, 6, 22, 12, 0),
    bytes: Buffer.from('{"continue_playing":"only-tv"}'),
  },
];
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

function versionId(version) {
  return crypto.createHash('sha256').update(version.bytes).digest('hex');
}

function historyName(version) {
  return `config.valid-${version.createdAt}-${versionId(version)}.json`;
}

export default {
  name: 'configrollback',

  seed({ configRoot }) {
    const files = paths(configRoot);
    fs.mkdirSync(files.dir, { recursive: true });
    seeded = {
      config: Buffer.from('{"continue_playing":"future","damaged":true}'),
      connections: Buffer.from('{"sources":[]}'),
      playlists: Buffer.from('{"schemaVersion":1,"playlists":[]}'),
    };
    fs.writeFileSync(files.config, seeded.config, { mode: 0o600 });
    fs.writeFileSync(files.connections, seeded.connections, { mode: 0o600 });
    fs.writeFileSync(files.playlists, seeded.playlists, { mode: 0o600 });
    for (const version of versions) {
      fs.writeFileSync(path.join(files.dir, historyName(version)), version.bytes, { mode: 0o600 });
    }
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const files = paths(configRoot);
    await driver.waitFor(
      `return document.querySelector('#durable-fault-heading')?.textContent.trim() ===
        'Vela could not safely read your settings.'`,
      'the settings rollback picker',
    );
    const offered = await driver.exec(`
      return [...document.querySelectorAll('button[data-rollback-version]')].map((button) => ({
        id: button.dataset.rollbackVersion,
        text: button.textContent.trim(),
      }))
    `);
    assert.deepEqual(
      offered.map(({ id }) => id),
      [...versions].reverse().map(versionId),
      'the three dated versions must be newest first',
    );
    assert.ok(offered.every(({ text }) => text.startsWith('Restore ') && text.includes('2026')));
    const body = await driver.exec(`return document.body.innerText`);
    assert.ok(body.includes('Rename and create new settings'));
    assert.ok(body.includes('Exit Vela'));
    await screenshot('01-picker');

    const selected = versions[1];
    const restore = await driver.find(
      'css selector',
      `button[data-rollback-version="${versionId(selected)}"]`,
    );
    await driver.click(restore);
    await driver.waitFor(
      `return [...document.querySelectorAll('h1,h2')]
        .some((heading) => heading.textContent.trim() === 'Welcome to Vela')`,
      'normal app after settings rollback',
    );

    const invalid = fs
      .readdirSync(files.dir)
      .filter((name) => name.startsWith('config.invalid-') && name.endsWith('.json'));
    assert.equal(invalid.length, 1);
    assert.deepEqual(fs.readFileSync(path.join(files.dir, invalid[0])), seeded.config);
    assert.deepEqual(fs.readFileSync(files.config), selected.bytes);
    assert.deepEqual(fs.readFileSync(files.connections), seeded.connections);
    assert.deepEqual(fs.readFileSync(files.playlists), seeded.playlists);
    const notice = await driver.exec(`return document.querySelector('.notice')?.textContent ?? ''`);
    assert.ok(notice.includes('Restored the settings version from'));
    assert.ok(notice.includes(invalid[0]));
    await screenshot('02-restored');

    await restart();
    await driver.waitFor(
      `return [...document.querySelectorAll('h1,h2')]
        .some((heading) => heading.textContent.trim() === 'Welcome to Vela')`,
      'restored settings after restart',
    );
    assert.equal(
      await driver.exec(
        `return document.querySelector('#durable-fault-heading')?.textContent ?? null`,
      ),
      null,
    );
  },
};
