// Bug 5 P1 (codex sspf-12): the Connected tab must show exactly one top-level
// row per SMB mount (+ its folder subrows) — NO leaked smb/ssh SOURCE row (whose
// Remove calls remove_source and errors: a dead-end) — and removing a share's
// last folder must CASCADE to a full unmount, not surface the backend
// last-folder error and leave a zombie zero-folder share.
//
// Hermetic: a native (Linux mountless, mountpoint:"") SMB mount is seeded
// directly in config, so the Connected tab renders from config (get_sources +
// list_smb_mounts) with NO SMB connection.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

export default {
  name: 'connectedtab',

  async seed({ configRoot }) {
    const configDir = path.join(configRoot, 'config', 'vela');
    fs.mkdirSync(configDir, { recursive: true });
    fs.writeFileSync(
      path.join(configDir, 'config.json'),
      JSON.stringify({
        smb_mounts: [
          {
            id: 'e2e-smb',
            name: 'E2E SMB',
            server: 'nas',
            share: 'media',
            username: '',
            password: '',
            domain: '',
            mountpoint: '', // native Linux mountless marker: no OS mount, no connection
            kind: '',
            local_folder_id: '',
            folders: [{ id: 'root', name: 'E2E SMB', path: '', kind: 'movie' }],
          },
        ],
        mpv_extra_args: '--vo=null\n--ao=null',
      }),
    );
  },

  async run({ driver, screenshot }) {
    // Open Settings (Connected is the default tab).
    await driver.waitFor(
      `return document.readyState === 'complete' && !!document.querySelector('button[aria-label="Settings"]')`,
      'the Settings gear',
    );
    await driver.click(await driver.find('css selector', 'button[aria-label="Settings"]'));
    await driver.waitFor(
      `const sec = [...document.querySelectorAll('section')].find(s => s.querySelector('h3')?.textContent.trim() === 'Connected');
       return !!sec && !!sec.querySelector('.row .badge');`,
      'the Connected section with rows',
    );

    const readRows = () =>
      driver.exec(`
        const sec = [...document.querySelectorAll('section')].find(s => s.querySelector('h3')?.textContent.trim() === 'Connected');
        return [...sec.querySelectorAll('.row')].map(r => ({
          badge: r.querySelector('.badge')?.textContent.trim(),
          sub: r.classList.contains('subrow'),
          btn: r.querySelector('button.rm')?.textContent.trim(),
        }));
      `);

    // Filter fix (9c3597a): exactly ONE top-level SMB row (the mount) — the
    // leaked SMB *source* row is gone. Pre-fix this is 2.
    const before = await readRows();
    const topSmb = before.filter((r) => r.badge === 'smb' && !r.sub);
    assert.equal(topSmb.length, 1, `exactly one top-level SMB row (the mount); got ${JSON.stringify(before)}`);
    const subs = before.filter((r) => r.sub);
    assert.equal(subs.length, 1, `one folder subrow under the mount; got ${JSON.stringify(before)}`);
    await screenshot('01-connected');

    // Cascade fix (removeSmbFolder): removing the share's only folder cascades to
    // a full unmount — no backend last-folder error, no SMB rows left. Pre-fix
    // the .err alert shows and the mount stays.
    await driver.click(await driver.find('css selector', '.row.subrow button.rm'));
    await driver.waitFor(
      `const sec = [...document.querySelectorAll('section')].find(s => s.querySelector('h3')?.textContent.trim() === 'Connected');
       return !!document.querySelector('.err') || !sec.querySelector('.row .badge');`,
      'the Connected rows to settle after removing the last folder',
    );
    const err = await driver.exec(
      `const e = document.querySelector('.err'); return e ? e.textContent.trim() : null;`,
    );
    assert.equal(err, null, `removing the last folder must not surface an error (must cascade to unmount); got: ${err}`);
    const after = await readRows();
    const smbLeft = after.filter((r) => r.badge === 'smb' || r.sub);
    assert.equal(smbLeft.length, 0, `the mount cascaded to a full unmount (no SMB rows left); got ${JSON.stringify(after)}`);
    await screenshot('02-cascaded');
  },
};
