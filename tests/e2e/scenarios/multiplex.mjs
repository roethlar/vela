// Two independently configured Plex sources through Vela's real Plex protocol
// path: distinct credentials/machines stay separated, one shared title collapses
// to one card with two backings, each explicit backing persists its override, and
// removing one Settings row leaves the other source live.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { pollUntil, seedConfig } from '../helpers.mjs';
import { createMockPlexTls, mockPlexSource, startMockPlex } from '../mockplex.mjs';

const TITLE = 'Shared Plex Movie';
const CANONICAL = 'imdb:tt7654321';
let tls;
let mockA;
let mockB;

function configPath(configRoot) {
  return path.join(configRoot, 'config', 'vela', 'config.json');
}

function readConfig(configRoot) {
  return JSON.parse(fs.readFileSync(configPath(configRoot), 'utf8'));
}

function readConnections(configRoot) {
  return JSON.parse(
    fs.readFileSync(path.join(configRoot, 'config', 'vela', 'connections.json'), 'utf8'),
  );
}

async function openContextMenu(driver) {
  await driver.exec(
    `const el = document.querySelector('button.poster[aria-label^="${TITLE}"]');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', {
       bubbles: true, cancelable: true,
       clientX: r.x + r.width / 2, clientY: r.y + r.height / 2,
     }));`,
  );
  await driver.waitFor(`return !!document.querySelector('.ctxmenu')`, 'multi-Plex context menu');
}

async function chooseBacking(driver, label) {
  await openContextMenu(driver);
  await driver.click(
    await driver.find(
      'xpath',
      `//button[@role='menuitem' and normalize-space(.)='Play Version']`,
    ),
  );
  await driver.waitFor(
    `return !!document.querySelector('[role="group"][aria-label="Play Version"]')`,
    'Play Version submenu',
  );
  const item = await driver.find(
    'xpath',
    `//*[@role='group' and @aria-label='Play Version']//button[@role='menuitem' and normalize-space(.)='${label}']`,
  );
  await driver.click(item);
}

export default {
  name: 'multiplex',

  async seed({ configRoot }) {
    tls = createMockPlexTls(configRoot);
    mockA = await startMockPlex({
      tls,
      name: 'Mock Plex A',
      machineIdentifier: 'machine-a',
      token: 'plex-token-a',
    });
    mockB = await startMockPlex({
      tls,
      name: 'Mock Plex B',
      machineIdentifier: 'machine-b',
      token: 'plex-token-b',
    });
    seedConfig(configRoot, [
      mockPlexSource(mockA, { id: 'plex-a' }),
      mockPlexSource(mockB, { id: 'plex-b' }),
    ]);
  },

  environment() {
    return { SSL_CERT_FILE: tls.ca };
  },

  async cleanup() {
    await Promise.all([mockA?.close(), mockB?.close()]);
  },

  async run({ driver, screenshot, configRoot }) {
    await driver.waitFor(
      `return document.readyState === 'complete' &&
        [...document.querySelectorAll('button.sideitem')].some(b => b.textContent.trim() === 'Movies')`,
      'multi-Plex Movies tab',
    );
    await driver.click(
      await driver.find(
        'xpath',
        `//button[contains(@class,'sideitem') and normalize-space(.)='Movies']`,
      ),
    );
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="${TITLE}"]')`,
      'collapsed multi-Plex movie',
    );

    const merged = await driver.exec(
      `const cards = [...document.querySelectorAll('button.poster[aria-label^="${TITLE}"]')];
       return { count: cards.length, twoSources: cards[0]?.innerText.includes('2 sources') };`,
    );
    assert.equal(merged.count, 1, 'the same movie on two Plex machines must collapse to one card');
    assert.equal(merged.twoSources, true, 'the collapsed Plex card must expose both backings');

    for (const [mock, expectedToken] of [[mockA, 'plex-token-a'], [mockB, 'plex-token-b']]) {
      assert.ok(
        mock.state.requests.some((request) => request.path === '/library/sections/1/all'),
        `${mock.name} must serve its own movie listing`,
      );
      assert.ok(
        mock.state.requests.every((request) => request.token === expectedToken),
        `${mock.name} must receive only its own source-row token`,
      );
    }
    await screenshot('01-collapsed');

    await chooseBacking(driver, 'Mock Plex A');
    await pollUntil(
      () => readConfig(configRoot).merged_overrides?.[CANONICAL] === 'plex-a',
      `${CANONICAL} to prefer plex-a`,
    );
    await pollUntil(
      () => mockA.state.requests.some((request) => request.path === '/library/metadata/1'),
      'the Plex A backing to resolve on Plex A',
    );

    await chooseBacking(driver, 'Mock Plex B');
    await pollUntil(
      () => readConfig(configRoot).merged_overrides?.[CANONICAL] === 'plex-b',
      `${CANONICAL} to prefer plex-b`,
    );
    await pollUntil(
      () => mockB.state.requests.some((request) => request.path === '/library/metadata/1'),
      'the Plex B backing to resolve on Plex B',
    );

    const beforeRemoveBSections = mockB.state.requests.filter(
      (request) => request.path === '/library/sections',
    ).length;
    await driver.click(await driver.find('css selector', 'button[aria-label="Settings"]'));
    await driver.waitFor(
      `return !!document.querySelector('[role="dialog"][aria-label="Settings"]')`,
      'Settings dialog for independent Plex removal',
    );
    const connected = await driver.exec(
      `const dialog = document.querySelector('[role="dialog"][aria-label="Settings"]');
       return {
         a: dialog.innerText.includes('Mock Plex A'),
         b: dialog.innerText.includes('Mock Plex B'),
         disconnect: dialog.innerText.includes('Disconnect'),
       };`,
    );
    assert.deepEqual(
      connected,
      { a: true, b: true, disconnect: false },
      'Settings must show two ordinary removable Plex rows',
    );
    const removeA = await driver.find(
      'xpath',
      `//div[contains(concat(' ',normalize-space(@class),' '),' row ') and ` +
        `.//span[contains(@class,'name') and normalize-space(.)='Mock Plex A']]` +
        `//button[normalize-space(.)='Remove']`,
    );
    await driver.click(removeA);

    await pollUntil(() => {
      const sources = readConnections(configRoot).sources ?? [];
      return sources.length === 1 && sources[0].id === 'plex-b' ? sources[0] : null;
    }, 'only Plex B to remain in connections');
    await driver.waitFor(
      `const dialog = document.querySelector('[role="dialog"][aria-label="Settings"]');
       return dialog && !dialog.innerText.includes('Mock Plex A') && dialog.innerText.includes('Mock Plex B');`,
      'Settings to retain only Plex B',
    );
    await pollUntil(
      () => mockB.state.requests.filter(
        (request) => request.path === '/library/sections',
      ).length > beforeRemoveBSections,
      'the surviving Plex B source to refresh after Plex A removal',
    );

    const survivor = readConnections(configRoot).sources[0];
    assert.equal(survivor.access_token, 'plex-token-b', 'removing Plex A preserves Plex B credentials');
    assert.equal(survivor.machine_identifier, 'machine-b', 'removing Plex A preserves Plex B pin');
    assert.deepEqual(mockA.state.contractViolations, [], 'Plex A mock contract');
    assert.deepEqual(mockB.state.contractViolations, [], 'Plex B mock contract');
    await screenshot('02-independent-remove');
  },
};
