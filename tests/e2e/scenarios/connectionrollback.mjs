// Damaged connections can restore one exact dated connection version without
// resetting settings/playlists or entering the reconnect flow.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { createMockPlexTls, mockPlexSource, startMockPlex } from '../mockplex.mjs';

let tls;
let mock;
let versions;
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

function historyName(version) {
  return `connections.valid-${version.createdAt}-${versionId(version)}.json`;
}

function versionId(version) {
  return crypto.createHash('sha256').update(version.bytes).digest('hex');
}

export default {
  name: 'connectionrollback',

  async seed({ configRoot }) {
    tls = createMockPlexTls(configRoot);
    mock = await startMockPlex({
      tls,
      name: 'Rollback Plex',
      machineIdentifier: 'rollback-machine',
      token: 'synthetic-connection-rollback-token',
    });
    const source = mockPlexSource(mock, { id: 'plex-rollback' });
    versions = [
      {
        createdAt: Date.UTC(2026, 6, 20, 12, 0),
        bytes: Buffer.from(JSON.stringify({ sources: [{ ...source, device_id: 'history-one' }] })),
      },
      {
        createdAt: Date.UTC(2026, 6, 21, 12, 0),
        bytes: Buffer.from(JSON.stringify({ sources: [{ ...source, device_id: 'history-two' }] })),
      },
      {
        createdAt: Date.UTC(2026, 6, 22, 12, 0),
        bytes: Buffer.from(JSON.stringify({ sources: [{ ...source, device_id: 'history-three' }] })),
      },
    ];
    const files = paths(configRoot);
    fs.mkdirSync(files.dir, { recursive: true });
    seeded = {
      config: Buffer.from('{"continue_playing":"on"}'),
      connections: Buffer.from(
        '{"sources":[{"id":"broken","kind":"future","access_token":"must-not-load"}]}',
      ),
      playlists: Buffer.from('{"schemaVersion":1,"playlists":[]}'),
    };
    fs.writeFileSync(files.config, seeded.config, { mode: 0o600 });
    fs.writeFileSync(files.connections, seeded.connections, { mode: 0o600 });
    fs.writeFileSync(files.playlists, seeded.playlists, { mode: 0o600 });
    for (const version of versions) {
      fs.writeFileSync(path.join(files.dir, historyName(version)), version.bytes, { mode: 0o600 });
    }
  },

  environment() {
    return { SSL_CERT_FILE: tls.ca };
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const files = paths(configRoot);
    await driver.waitFor(
      `return document.querySelector('#durable-fault-heading')?.textContent.trim() ===
        'Vela could not safely read your server connections.'`,
      'the connections rollback picker',
    );
    const offered = await driver.exec(`
      return [...document.querySelectorAll('button[data-rollback-version]')]
        .map((button) => button.dataset.rollbackVersion)
    `);
    assert.deepEqual(offered, [...versions].reverse().map(versionId));
    assert.ok(!(await driver.exec(`return document.body.innerText`)).includes(mock.token));
    await screenshot('01-picker');

    const selected = versions[1];
    const restore = await driver.find(
      'css selector',
      `button[data-rollback-version="${versionId(selected)}"]`,
    );
    await driver.click(restore);
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')]
        .some((button) => button.textContent.trim() === 'Movies')`,
      'the restored Plex connection',
    );

    const invalid = fs
      .readdirSync(files.dir)
      .filter((name) => name.startsWith('connections.invalid-') && name.endsWith('.json'));
    assert.equal(invalid.length, 1);
    assert.deepEqual(fs.readFileSync(path.join(files.dir, invalid[0])), seeded.connections);
    assert.deepEqual(fs.readFileSync(files.connections), selected.bytes);
    assert.deepEqual(fs.readFileSync(files.config), seeded.config);
    assert.deepEqual(fs.readFileSync(files.playlists), seeded.playlists);
    const body = await driver.exec(`return document.body.innerText`);
    assert.ok(!body.includes('Welcome to Vela'));
    assert.ok(!body.includes(mock.token));
    const notice = await driver.exec(`return document.querySelector('.notice')?.textContent ?? ''`);
    assert.ok(notice.includes('Restored the connections version from'));
    assert.ok(!notice.includes('Connect your servers again'));
    await screenshot('02-restored');

    await restart();
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')]
        .some((button) => button.textContent.trim() === 'Movies')`,
      'the restored connection after restart',
    );
    assert.equal(
      await driver.exec(
        `return document.querySelector('#durable-fault-heading')?.textContent ?? null`,
      ),
      null,
    );
  },
};
