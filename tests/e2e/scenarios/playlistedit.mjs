// Vela-native playlist editor: CRUD through the real UI, durable store
// assertions, restart persistence, and retained unavailable entries after a
// source is removed.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { mockSource, pollUntil, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mockA;
let mockB;

const titlesInPlaylist = (driver) =>
  driver.exec(
    `return [...document.querySelectorAll('section.playlists ol.entries > li .entrymeta strong')]
      .map((element) => element.textContent.trim())`,
  );

async function openPlaylists(driver) {
  await driver.waitFor(
    `return [...document.querySelectorAll('button.sideitem')]
      .some((button) => button.textContent.trim() === 'Playlists')`,
    'the Playlists sidebar entry',
  );
  const button = await driver.find(
    'xpath',
    `//button[contains(@class,'sideitem') and normalize-space(.)='Playlists']`,
  );
  await driver.click(button);
  await driver.waitFor(
    `return !!document.querySelector('section.playlists')`,
    'the Vela playlists view',
  );
}

async function openItemMenu(driver, title) {
  await driver.exec(
    `const element = [...document.querySelectorAll('button.poster')]
       .find((button) => button.getAttribute('aria-label')?.startsWith(${JSON.stringify(title)}));
     if (!element) return false;
     const rect = element.getBoundingClientRect();
     element.dispatchEvent(new MouseEvent('contextmenu', {
       bubbles: true,
       cancelable: true,
       clientX: rect.x + rect.width / 2,
       clientY: rect.y + rect.height / 2,
     }));
     return true;`,
  );
  await driver.waitFor(
    `return !!document.querySelector('.ctxmenu')`,
    `the context menu for ${title}`,
  );
}

async function addTitle(driver, title, playlistName) {
  await openItemMenu(driver, title);
  const openAdd = await driver.find(
    'xpath',
    `//button[@role='menuitem' and normalize-space(.)='Add to Playlist']`,
  );
  await driver.click(openAdd);
  await driver.waitFor(
    `return !!document.querySelector('.addsubmenu[aria-label="Choose a playlist"]')`,
    'the Add to Playlist submenu',
  );
  const target = await driver.find(
    'xpath',
    `//div[@aria-label='Choose a playlist']//button[starts-with(normalize-space(.),'${playlistName} ')]`,
  );
  await driver.click(target);
  await driver.waitFor(
    `return document.querySelector('.addstatus')?.textContent.includes('Added to “${playlistName}”.') ?? false`,
    `${title} to be added to ${playlistName}`,
  );
  await driver.exec(`document.querySelector('.menubackdrop')?.click()`);
}

export default {
  name: 'playlistedit',

  async seed({ configRoot }) {
    mockA = await startMockJellyfin({
      movies: [
        { id: 'alpha', name: 'Alpha', year: 2020 },
        { id: 'charlie', name: 'Charlie', year: 2022 },
      ],
    });
    mockB = await startMockJellyfin({
      movies: [{ id: 'beta', name: 'Beta', year: 2021 }],
    });
    seedConfig(configRoot, [
      mockSource(mockA, { id: 'jf-a', name: 'Mock JF A' }),
      mockSource(mockB, { id: 'jf-b', name: 'Mock JF B' }),
    ]);
  },

  async cleanup() {
    await Promise.all([mockA?.close(), mockB?.close()]);
  },

  async run({ driver, screenshot, configRoot, restart }) {
    const playlistFile = path.join(configRoot, 'config', 'vela', 'playlists.json');
    const connectionsFile = path.join(configRoot, 'config', 'vela', 'connections.json');
    const readStore = () => JSON.parse(fs.readFileSync(playlistFile, 'utf8'));

    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')]
        .some((button) => button.textContent.trim() === 'Playlists')`,
      'the Playlists sidebar entry',
    );
    await openPlaylists(driver);

    const createName = await driver.find('css selector', '#playlist-create');
    await driver.type(createName, 'Draft');
    const create = await driver.find('css selector', 'section.playlists form.create button[type="submit"]');
    await driver.click(create);
    const created = await pollUntil(() => {
      try {
        const store = readStore();
        return store.schemaVersion === 1 && store.playlists?.[0]?.name === 'Draft'
          ? store.playlists[0]
          : null;
      } catch {
        return null;
      }
    }, 'the created playlist in playlists.json');
    assert.equal(created.items.length, 0);
    await driver.waitFor(
      `return document.querySelector('section.playlists h1')?.textContent.trim() === 'Draft'`,
      'the new playlist detail',
    );

    await driver.exec(
      `const input = document.querySelector('#playlist-rename');
       input.value = 'Voyage';
       input.dispatchEvent(new Event('input', { bubbles: true }));`,
    );
    const saveName = await driver.find('css selector', 'section.playlists .rename button');
    await driver.click(saveName);
    await pollUntil(() => readStore().playlists[0]?.name === 'Voyage', 'the renamed playlist on disk');
    await driver.waitFor(
      `return document.querySelector('section.playlists h1')?.textContent.trim() === 'Voyage'
        && document.querySelector('section.playlists .status')?.textContent.includes('Playlist renamed.')`,
      'the renamed playlist detail and owned status',
    );

    const movies = await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)='Movies']`,
    );
    await driver.click(movies);
    await driver.waitFor(
      `return ['Alpha', 'Beta', 'Charlie'].every((title) =>
        [...document.querySelectorAll('button.poster')]
          .some((button) => button.getAttribute('aria-label')?.startsWith(title)))`,
      'all mixed-source movie cards',
    );
    for (const title of ['Alpha', 'Beta', 'Charlie']) {
      await addTitle(driver, title, 'Voyage');
    }
    await pollUntil(
      () => readStore().playlists[0]?.items?.length === 3,
      'all three playlist appends on disk',
    );

    await openPlaylists(driver);
    const openVoyage = await driver.find(
      'css selector',
      'section.playlists .playlistgrid button[aria-label^="Open Voyage,"]',
    );
    await driver.click(openVoyage);
    await driver.waitFor(
      `return document.querySelectorAll('section.playlists ol.entries > li').length === 3`,
      'the three persisted playlist entries',
    );
    assert.deepEqual(await titlesInPlaylist(driver), ['Alpha', 'Beta', 'Charlie']);
    assert.deepEqual(
      readStore().playlists[0].items.map((entry) => entry.item.ratingKey),
      ['jf-a:alpha', 'jf-b:beta', 'jf-a:charlie'],
    );

    const moveCharlie = await driver.find(
      'css selector',
      'button[aria-label="Move Charlie up"]',
    );
    await driver.click(moveCharlie);
    await driver.waitFor(
      `return [...document.querySelectorAll('section.playlists .entrymeta strong')]
        .map((element) => element.textContent.trim()).join('|') === 'Alpha|Charlie|Beta'`,
      'the reordered playlist in the editor',
    );
    await pollUntil(
      () => readStore().playlists[0].items.map((entry) => entry.item.title).join('|') === 'Alpha|Charlie|Beta',
      'the reordered playlist on disk',
    );

    const removeAlpha = await driver.find(
      'xpath',
      `//ol[@aria-label='Playlist items']/li[.//strong[normalize-space(.)='Alpha']]//button[normalize-space(.)='Remove']`,
    );
    await driver.click(removeAlpha);
    await driver.waitFor(
      `return [...document.querySelectorAll('section.playlists .entrymeta strong')]
        .map((element) => element.textContent.trim()).join('|') === 'Charlie|Beta'`,
      'Alpha to be removed from the editor',
    );
    await pollUntil(
      () => readStore().playlists[0].items.map((entry) => entry.item.title).join('|') === 'Charlie|Beta',
      'Alpha to be removed from the store',
    );
    await screenshot('01-edited-mixed-playlist');

    await restart(() => {
      const connections = JSON.parse(fs.readFileSync(connectionsFile, 'utf8'));
      connections.sources = connections.sources.filter((source) => source.id !== 'jf-b');
      fs.writeFileSync(connectionsFile, JSON.stringify(connections));
    });
    await openPlaylists(driver);
    const reopen = await driver.find(
      'css selector',
      'section.playlists .playlistgrid button[aria-label^="Open Voyage,"]',
    );
    await driver.click(reopen);
    await driver.waitFor(
      `return document.querySelectorAll('section.playlists ol.entries > li').length === 2`,
      'the playlist after restart and source removal',
    );
    assert.deepEqual(await titlesInPlaylist(driver), ['Charlie', 'Beta']);
    assert.equal(
      await driver.exec(
        `return !![...document.querySelectorAll('section.playlists ol.entries > li')]
          .find((row) => row.querySelector('.entrymeta strong')?.textContent.trim() === 'Beta')
          ?.classList.contains('unavailable')`,
      ),
      true,
      'the removed-source item stays visible and is marked unavailable',
    );
    assert.equal(
      await driver.exec(
        `return [...document.querySelectorAll('section.playlists ol.entries > li')]
          .find((row) => row.querySelector('.entrymeta strong')?.textContent.trim() === 'Beta')
          ?.querySelector('.dead')?.textContent.trim()`,
      ),
      'Unavailable',
    );
    assert.deepEqual(
      readStore().playlists[0].items.map((entry) => entry.item.ratingKey),
      ['jf-a:charlie', 'jf-b:beta'],
      'source removal never drops curated entries',
    );
    await screenshot('02-unavailable-retained-after-restart');

    const askDelete = await driver.find(
      'xpath',
      `//section[contains(@class,'playlists')]//button[normalize-space(.)='Delete playlist…']`,
    );
    await driver.click(askDelete);
    const confirmDelete = await driver.find(
      'xpath',
      `//section[contains(@class,'playlists')]//button[normalize-space(.)='Delete permanently']`,
    );
    await driver.click(confirmDelete);
    await driver.waitFor(
      `return document.querySelector('section.playlists')?.textContent.includes('No playlists yet') ?? false`,
      'the empty playlist list after deletion',
    );
    await pollUntil(() => readStore().playlists.length === 0, 'playlist deletion on disk');
  },
};
