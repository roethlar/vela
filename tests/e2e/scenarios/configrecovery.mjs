// A damaged post-split settings file blocks normal boot, Exit writes neither
// durable document, and the explicit recovery button preserves exact bytes
// while retaining a separately valid Plex connection.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createMockPlexTls, mockPlexSource, startMockPlex } from '../mockplex.mjs';

let tls;
let mock;
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

function invalidBackups(dir, stem) {
  return fs.readdirSync(dir).filter((name) =>
    name.startsWith(`${stem}.invalid-`) && name.endsWith('.json'));
}

async function waitForFault(driver, action) {
  await driver.waitFor(
    `return document.querySelector('#durable-fault-heading')?.textContent.trim() ===
      'Vela could not safely read your settings.'`,
    'the damaged-settings blocking screen',
  );
  const body = await driver.exec(`return document.body.innerText`);
  assert.ok(body.includes('may be damaged or may have been tampered with'));
  assert.ok(body.includes('Your server connections are stored separately and will not be changed.'));
  assert.ok(!body.includes('Welcome to Vela'));
  assert.ok(!body.includes(mock.token), 'the recovery screen must not expose the Plex token');
  return driver.find('xpath', `//button[normalize-space(.)='${action}']`);
}

export default {
  name: 'configrecovery',

  async seed({ configRoot }) {
    tls = createMockPlexTls(configRoot);
    mock = await startMockPlex({
      tls,
      name: 'Recovery Plex',
      machineIdentifier: 'recovery-machine',
      token: 'synthetic-config-recovery-token',
    });
    const files = paths(configRoot);
    fs.mkdirSync(files.dir, { recursive: true });
    seeded = {
      config: Buffer.from('{"continue_playing":"future","damaged":true}'),
      connections: Buffer.from(JSON.stringify({
        sources: [mockPlexSource(mock, { id: 'plex-recovery' })],
      })),
      playlists: Buffer.from('{"schemaVersion":1,"playlists":[]}'),
    };
    fs.writeFileSync(files.config, seeded.config, { mode: 0o600 });
    fs.writeFileSync(files.connections, seeded.connections, { mode: 0o600 });
    fs.writeFileSync(files.playlists, seeded.playlists, { mode: 0o600 });
  },

  environment() {
    return { SSL_CERT_FILE: tls.ca };
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const files = paths(configRoot);
    await screenshot('01-blocked');
    const exit = await waitForFault(driver, 'Exit Vela');
    await driver.click(exit).catch(() => {});
    await new Promise((resolve) => setTimeout(resolve, 250));
    assert.deepEqual(fs.readFileSync(files.config), seeded.config, 'Exit must not rewrite settings');
    assert.deepEqual(
      fs.readFileSync(files.connections),
      seeded.connections,
      'Exit must not rewrite connections',
    );
    assert.deepEqual(
      fs.readFileSync(files.playlists),
      seeded.playlists,
      'Exit must not rewrite playlists',
    );

    await restart();
    const recover = await waitForFault(driver, 'Rename and create new settings');
    await driver.click(recover);
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')]
        .some((button) => button.textContent.trim() === 'Movies')`,
      'the preserved Plex source after settings recovery',
    );

    const backups = invalidBackups(files.dir, 'config');
    assert.equal(backups.length, 1, 'settings recovery must create exactly one invalid backup');
    assert.deepEqual(
      fs.readFileSync(path.join(files.dir, backups[0])),
      seeded.config,
      'the renamed settings file must be byte-identical',
    );
    assert.equal(
      fs.statSync(path.join(files.dir, backups[0])).mode & 0o777,
      0o600,
      'the renamed settings file must be private',
    );
    assert.doesNotThrow(() => JSON.parse(fs.readFileSync(files.config, 'utf8')));
    assert.deepEqual(fs.readFileSync(files.connections), seeded.connections);
    assert.deepEqual(fs.readFileSync(files.playlists), seeded.playlists);
    const notice = await driver.exec(`return document.querySelector('.notice')?.textContent ?? ''`);
    assert.ok(notice.includes(backups[0]), 'the safe backup filename must be shown');
    assert.ok(notice.includes('server connections were kept'));
    assert.ok(!(await driver.exec(`return document.body.innerText`)).includes(mock.token));
    await screenshot('02-recovered');

    await restart();
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')]
        .some((button) => button.textContent.trim() === 'Movies')`,
      'the preserved source after restart',
    );
    assert.equal(
      await driver.exec(`return document.querySelector('#durable-fault-heading')?.textContent ?? null`),
      null,
      'fresh settings must load normally after restart',
    );
  },
};
