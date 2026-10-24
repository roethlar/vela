// Fresh-config smoke: the app boots on a throwaway config, renders the
// welcome empty state with a live Tauri IPC bridge and the current version
// in the footer, and "Add a source" opens the Settings panel.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

export default {
  name: 'smoke',
  async run({ driver, screenshot, repoRoot }) {
    await driver.waitFor(
      `return document.readyState === 'complete' && !!document.querySelector('h1,h2')`,
      'app render',
    );
    const state = await driver.exec(`return {
      heading: document.querySelector('h1,h2')?.textContent?.trim(),
      hasTauriIpc: !!window.__TAURI_INTERNALS__,
      footer: document.body.innerText.match(/Vela v[0-9][0-9.]*/)?.[0] ?? null,
    }`);
    assert.equal(state.heading, 'Welcome to Vela', 'fresh config should show the empty state');
    assert.ok(state.hasTauriIpc, 'Tauri IPC bridge missing from the webview');
    const pkg = JSON.parse(fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'));
    // Catches running a stale embedded frontend as much as a wrong version.
    assert.equal(state.footer, `Vela v${pkg.version}`);
    // The Geist UI face must actually load and be the body font. Do not use
    // document.fonts.check(): it returns true when no face matches at all.
    await driver.waitFor(
      `return [...document.fonts].some((f) => f.family.replace(/["']/g, "") === "Geist Variable" && f.status === "loaded")`,
      'Geist Variable font loaded',
    );
    const bodyFont = await driver.exec(`return getComputedStyle(document.body).fontFamily`);
    assert.match(bodyFont, /^["']?Geist Variable["']?\s*,/, 'body must use the Geist UI font');
    await screenshot('01-welcome');

    const addSource = await driver.find(
      'xpath',
      `//button[contains(normalize-space(.),'Add a source')]` +
        ` | //a[contains(normalize-space(.),'Add a source')]`,
    );
    await driver.click(addSource);
    await driver.waitFor(
      `return Array.from(document.querySelectorAll('h1,h2,h3'))` +
        `.some(h => h.textContent.trim() === 'Settings')`,
      'Settings panel',
    );
    const missing = await driver.exec(
      `return ['Connected','Servers','Player','Appearance']` +
        `.filter(s => !Array.from(document.querySelectorAll('button,a,h2,h3'))` +
        `.some(e => e.textContent.trim() === s))`,
    );
    assert.deepEqual(missing, [], `Settings sections missing: ${missing}`);
    await screenshot('02-settings');
  },
};
