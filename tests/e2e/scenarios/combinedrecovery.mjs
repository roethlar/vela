// A damaged pre-split config is preserved whole. Vela does not mine its source
// block, creates no connections file, and requires a genuine reconnect.
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

export default {
  name: 'combinedrecovery',

  seed({ configRoot }) {
    const files = paths(configRoot);
    fs.mkdirSync(files.dir, { recursive: true });
    seeded = {
      config: Buffer.from(
        '{"sources":[{"id":"jf-old","kind":"jellyfin","name":"Old","base_url":"http://127.0.0.1:8096","access_token":"synthetic-combined-token","user_id":"u","device_id":"d"}],"future":true}',
      ),
      playlists: Buffer.from('{"schemaVersion":1,"playlists":[]}'),
    };
    fs.writeFileSync(files.config, seeded.config, { mode: 0o600 });
    fs.writeFileSync(files.playlists, seeded.playlists, { mode: 0o600 });
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const files = paths(configRoot);
    await driver.waitFor(
      `return document.querySelector('#durable-fault-heading')?.textContent.trim() ===
        'Vela could not safely read your settings.'`,
      'the damaged legacy-settings blocking screen',
    );
    const copy = await driver.exec(`return document.body.innerText`);
    assert.ok(copy.includes('This older settings file is damaged'));
    assert.ok(copy.includes('will not extract or guess any connection'));
    assert.ok(copy.includes('requires you to reconnect your servers'));
    assert.ok(!copy.includes('synthetic-combined-token'));
    await screenshot('01-blocked');

    const exit = await driver.find('xpath', `//button[normalize-space(.)='Exit Vela']`);
    await driver.click(exit).catch(() => {});
    await new Promise((resolve) => setTimeout(resolve, 250));
    assert.deepEqual(fs.readFileSync(files.config), seeded.config);
    assert.deepEqual(fs.readFileSync(files.playlists), seeded.playlists);
    assert.ok(!fs.existsSync(files.connections));

    await restart();
    await driver.waitFor(
      `return document.querySelector('#durable-fault-heading')?.textContent.trim() ===
        'Vela could not safely read your settings.'`,
      'the damaged legacy-settings screen after relaunch',
    );
    const recover = await driver.find(
      'xpath',
      `//button[normalize-space(.)='Rename and create new settings']`,
    );
    await driver.type(recover, ' ');
    await driver.waitFor(
      `return [...document.querySelectorAll('h1,h2')]
        .some((heading) => heading.textContent.trim() === 'Welcome to Vela')`,
      'the reconnect flow after legacy recovery',
    );

    const backups = fs.readdirSync(files.dir).filter((name) =>
      name.startsWith('config.invalid-') && name.endsWith('.json'));
    assert.equal(backups.length, 1);
    assert.deepEqual(fs.readFileSync(path.join(files.dir, backups[0])), seeded.config);
    assert.equal(fs.statSync(path.join(files.dir, backups[0])).mode & 0o777, 0o600);
    assert.doesNotThrow(() => JSON.parse(fs.readFileSync(files.config, 'utf8')));
    assert.ok(!fs.existsSync(files.connections), 'legacy recovery must not create connections');
    assert.deepEqual(fs.readFileSync(files.playlists), seeded.playlists);
    const notice = await driver.exec(`return document.querySelector('.notice')?.textContent ?? ''`);
    assert.ok(notice.includes(backups[0]));
    assert.ok(notice.includes('Connect your servers again'));
    assert.ok(!(await driver.exec(`return document.body.innerText`)).includes('synthetic-combined-token'));
    await screenshot('02-reconnect');

    await restart();
    await driver.waitFor(
      `return [...document.querySelectorAll('h1,h2')]
        .some((heading) => heading.textContent.trim() === 'Welcome to Vela')`,
      'fresh settings after restart',
    );
    assert.ok(!fs.existsSync(files.connections));
  },
};
