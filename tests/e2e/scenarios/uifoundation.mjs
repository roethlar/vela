// UI foundation contract on the real Tauri/WebKit app. Screenshots remain
// inspection evidence; deterministic assertions use DOM state and computed CSS.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { goHome, mockSource, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mock;
const expectReducedMotion = process.env.VELA_E2E_EXPECT_REDUCED_MOTION === '1';

const movie = {
  ratingKey: 'jf-mock:foundation',
  title: 'Foundation Movie',
  year: 2026,
  summary: 'A deterministic title for the UI foundation scenario.',
  durationMs: 10_000,
  mediaType: 'movie',
  viewOffsetMs: 5_000,
  played: false,
  sourceId: 'jf-mock',
  providerIds: [],
};

async function clickSide(driver, label) {
  await driver.click(
    await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)='${label}']`,
    ),
  );
}

async function openSettingsTab(driver, label) {
  if (!(await driver.exec(`return !!document.querySelector('[role="dialog"][aria-label="Settings"]')`))) {
    await driver.click(await driver.find('css selector', 'button[aria-label="Settings"]'));
    await driver.waitFor(
      `return !!document.querySelector('[role="dialog"][aria-label="Settings"]')`,
      'Settings dialog',
    );
  }
  await driver.click(
    await driver.find('xpath', `//button[@role='tab' and normalize-space(.)='${label}']`),
  );
}

async function closeSettings(driver) {
  await driver.click(
    await driver.find(
      'css selector',
      '[role="dialog"][aria-label="Settings"] button[aria-label="Close"]',
    ),
  );
  await driver.waitFor(
    `return !document.querySelector('[role="dialog"][aria-label="Settings"]')`,
    'Settings dialog to close',
  );
}

async function chooseTheme(driver, label, id) {
  await openSettingsTab(driver, 'Appearance');
  await driver.click(
    await driver.find(
      'xpath',
      `//button[contains(@class,'themecard') and .//span[normalize-space(.)='${label}']]`,
    ),
  );
  await driver.waitFor(
    `return document.documentElement.dataset.theme === ${JSON.stringify(id)}`,
    `${label} theme`,
  );
  const state = await driver.exec(`return {
    applied: document.documentElement.dataset.theme,
    stored: localStorage.getItem('vela-theme'),
    colorScheme: getComputedStyle(document.documentElement).colorScheme,
    selected: [...document.querySelectorAll('.themecard[aria-pressed="true"] .themename')]
      .map((node) => node.textContent.trim()),
  }`);
  assert.equal(state.applied, id, `${label} must apply to the document root`);
  assert.equal(state.stored, id, `${label} must persist to localStorage`);
  assert.deepEqual(state.selected, [label], `${label} must be the sole selected theme card`);
  assert.equal(state.colorScheme, id === 'one-light' ? 'light' : 'dark');
}

async function focusSnapshot(driver, selector) {
  await driver.waitFor(
    `
      const target = document.querySelector(${JSON.stringify(selector)});
      if (!target) return false;
      target.focus();
      const probe = document.createElement('span');
      probe.style.cssText = 'position:fixed;left:-9999px;border:1px solid var(--accent)';
      document.body.append(probe);
      const ready = document.activeElement === target
        && getComputedStyle(target).borderTopColor === getComputedStyle(probe).borderTopColor;
      probe.remove();
      return ready;
    `,
    `${selector} theme focus styles`,
  );
  return driver.exec(`
    const target = document.querySelector(${JSON.stringify(selector)});
    if (!target) return null;
    const probe = document.createElement('span');
    probe.style.cssText = [
      'position:fixed',
      'left:-9999px',
      'border:1px solid var(--accent)',
      'box-shadow:0 0 0 3px var(--accent-glow)',
    ].join(';');
    document.body.append(probe);
    const actual = getComputedStyle(target);
    const expected = getComputedStyle(probe);
    const result = {
      borderColor: actual.borderTopColor,
      expectedBorderColor: expected.borderTopColor,
      boxShadow: actual.boxShadow,
      expectedBoxShadow: expected.boxShadow,
    };
    probe.remove();
    return result;
  `);
}

function assertFocusUsesTheme(snapshot, label) {
  assert.ok(snapshot, `${label} must exist`);
  assert.equal(snapshot.borderColor, snapshot.expectedBorderColor, `${label} border uses --accent`);
  assert.equal(snapshot.boxShadow, snapshot.expectedBoxShadow, `${label} glow uses --accent-glow`);
}

async function styleSnapshot(driver, selector) {
  return driver.exec(`
    const node = document.querySelector(${JSON.stringify(selector)});
    if (!node) return null;
    const style = getComputedStyle(node);
    return {
      alignItems: style.alignItems,
      backgroundColor: style.backgroundColor,
      backgroundImage: style.backgroundImage,
      borderRadius: style.borderRadius,
      borderStyle: style.borderTopStyle,
      borderWidth: style.borderTopWidth,
      color: style.color,
      cursor: style.cursor,
      display: style.display,
      fontSize: style.fontSize,
      fontWeight: style.fontWeight,
      height: style.height,
      justifyContent: style.justifyContent,
      lineHeight: style.lineHeight,
      opacity: style.opacity,
      padding: style.padding,
      textAlign: style.textAlign,
      transitionDuration: style.transitionDuration,
      transitionProperty: style.transitionProperty,
      width: style.width,
    };
  `);
}

function subset(value, keys) {
  return Object.fromEntries(keys.map((key) => [key, value[key]]));
}

function assertSameStyles(label, snapshots, keys) {
  for (const [surface, snapshot] of Object.entries(snapshots)) {
    assert.ok(snapshot, `${label} must render on ${surface}`);
  }
  const entries = Object.entries(snapshots);
  const [referenceSurface, reference] = entries[0];
  for (const [surface, snapshot] of entries.slice(1)) {
    assert.deepEqual(
      subset(snapshot, keys),
      subset(reference, keys),
      `${label} differs between ${referenceSurface} and ${surface}`,
    );
  }
}

function assertMotionSuppressed(snapshot, label) {
  const durations = snapshot.transitionDuration.split(',').map((duration) => {
    const value = duration.trim();
    return value.endsWith('ms') ? Number.parseFloat(value) : Number.parseFloat(value) * 1000;
  });
  assert.ok(
    durations.length > 0 && durations.every((duration) => duration <= 0.01),
    `${label} transition is not suppressed: ${snapshot.transitionDuration}`,
  );
}

async function assertSettingsIcons(driver) {
  await openSettingsTab(driver, 'Player');
  await driver.waitFor(
    `return [...document.querySelectorAll('[role="dialog"] p')]
      .some((node) => /Found mpv|mpv wasn't found/.test(node.textContent))`,
    'mpv status in Settings',
  );
  const state = await driver.exec(`
    const dialog = document.querySelector('[role="dialog"][aria-label="Settings"]');
    const status = [...dialog.querySelectorAll('p')]
      .find((node) => /Found mpv|mpv wasn't found/.test(node.textContent));
    const warning = [...dialog.querySelectorAll('.warn')]
      .find((node) => node.textContent.includes('Advanced'));
    return {
      rawGlyphs: dialog.innerText.match(/[✓✗⚠]/gu) ?? [],
      statusHasSvg: !!status?.querySelector('svg[aria-hidden="true"]'),
      warningHasSvg: !!warning?.querySelector('svg[aria-hidden="true"]'),
    };
  `);
  assert.deepEqual(state.rawGlyphs, [], 'Settings must not render migrated status glyphs');
  assert.equal(state.statusHasSvg, true, 'the mpv availability status uses the shared SVG icon');
  assert.equal(state.warningHasSvg, true, 'the advanced warning uses the shared SVG icon');
}

export default {
  name: 'uifoundation',

  async seed({ configRoot }) {
    if (expectReducedMotion) {
      const gtkConfig = path.join(configRoot, 'config', 'gtk-3.0');
      fs.mkdirSync(gtkConfig, { recursive: true });
      fs.writeFileSync(path.join(gtkConfig, 'settings.ini'), '[Settings]\ngtk-enable-animations=false\n');
    }
    mock = await startMockJellyfin({
      movies: [
        {
          id: 'foundation',
          name: movie.title,
          year: movie.year,
          runTimeTicks: 100_000_000,
        },
      ],
      serveResume: true,
    });
    mock.state.userData.foundation.positionTicks = 50_000_000;
    seedConfig(configRoot, [mockSource(mock)], {
      recents: [{ item: movie, started_at_ms: 1, ended_at_ms: 2 }],
    });
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot, restart }) {
    try {
      await driver.waitFor(
        `return document.readyState === 'complete'
          && !!document.querySelector('input[aria-label="Search your libraries"]')
          && !!document.querySelector('[aria-label="Continue watching"] .flowcard.center')`,
        'the seeded Home foundation surfaces',
      );
      if (expectReducedMotion) {
        assert.equal(
          await driver.exec(`return matchMedia('(prefers-reduced-motion: reduce)').matches`),
          true,
          'the reduced-motion run must expose the OS preference to WebKit',
        );
      }

      await chooseTheme(driver, 'OLED Black', 'oled');
      await closeSettings(driver);
      const oled = await driver.exec(`
        const root = getComputedStyle(document.documentElement);
        const body = getComputedStyle(document.body);
        const center = getComputedStyle(document.querySelector('[aria-label="Continue watching"] .flowcard.center'));
        const art = getComputedStyle(document.querySelector('[aria-label="Continue watching"] .flowcard.center .art'));
        return {
          rootBackground: root.backgroundColor,
          bodyBackground: body.backgroundColor,
          bodyImage: body.backgroundImage,
          grainDisplay: getComputedStyle(document.querySelector('.grain')).display,
          surface: root.getPropertyValue('--surface').trim(),
          text: root.getPropertyValue('--text').trim(),
          accent: root.getPropertyValue('--accent').trim(),
          centerFilter: center.filter,
          centerOpacity: center.opacity,
          artOpacity: art.opacity,
        };
      `);
      assert.deepEqual(
        oled,
        {
          rootBackground: 'rgb(0, 0, 0)',
          bodyBackground: 'rgb(0, 0, 0)',
          bodyImage: 'none',
          grainDisplay: 'none',
          surface: '#070707',
          text: '#c7c7c7',
          accent: '#c58a0b',
          centerFilter: 'brightness(1)',
          centerOpacity: '1',
          artOpacity: '1',
        },
        'OLED Black must dim chrome without dimming the centered media card',
      );
      await screenshot('00-oled-home');

      await chooseTheme(driver, 'Vela Dark', 'dark');
      await closeSettings(driver);
      const darkSearchFocus = await focusSnapshot(
        driver,
        'input[aria-label="Search your libraries"]',
      );
      assertFocusUsesTheme(darkSearchFocus, 'dark search focus');
      await screenshot('01-dark-home');

      await chooseTheme(driver, 'One Light', 'one-light');
      await screenshot('02-one-light-settings');
      await closeSettings(driver);
      const lightSearchFocus = await focusSnapshot(
        driver,
        'input[aria-label="Search your libraries"]',
      );
      assertFocusUsesTheme(lightSearchFocus, 'light search focus');
      assert.notEqual(
        lightSearchFocus.boxShadow,
        darkSearchFocus.boxShadow,
        'Dark and One Light must compute different focus glows',
      );

      // A new real app session exercises app.html's pre-paint localStorage path,
      // not merely Settings' in-memory state.
      await restart();
      await driver.waitFor(
        `return document.readyState === 'complete'
          && !!document.querySelector('input[aria-label="Search your libraries"]')`,
        'the restarted authenticated app',
      );
      const persisted = await driver.exec(`return {
        applied: document.documentElement.dataset.theme,
        stored: localStorage.getItem('vela-theme'),
        colorScheme: getComputedStyle(document.documentElement).colorScheme,
      }`);
      assert.deepEqual(
        persisted,
        { applied: 'one-light', stored: 'one-light', colorScheme: 'light' },
        'One Light must survive a real app restart and apply before the UI is inspected',
      );
      await openSettingsTab(driver, 'Appearance');
      assert.deepEqual(
        await driver.exec(`return [...document.querySelectorAll('.themecard[aria-pressed="true"] .themename')]
          .map((node) => node.textContent.trim())`),
        ['One Light'],
        'the persisted theme card remains selected after restart',
      );

      await assertSettingsIcons(driver);
      const settingsPrimary = await styleSnapshot(
        driver,
        '[role="dialog"][aria-label="Settings"] button.primary',
      );
      await closeSettings(driver);

      await clickSide(driver, 'Playlists');
      await driver.waitFor(
        `return !!document.querySelector('section.playlists #playlist-create')`,
        'the playlist creation form',
      );
      const playlistFocus = await focusSnapshot(driver, 'section.playlists #playlist-create');
      assertFocusUsesTheme(playlistFocus, 'One Light playlist-name focus');
      await driver.type(await driver.find('css selector', 'section.playlists #playlist-create'), 'Foundation');
      await driver.waitFor(
        `return !document.querySelector('section.playlists button.primary')?.disabled`,
        'the enabled playlist primary button',
      );
      const playlistPrimary = await styleSnapshot(driver, 'section.playlists button.primary');
      await goHome(driver);
      await driver.waitFor(
        `return !!document.querySelector('[aria-label="Continue watching"] .flowcard.center .progress')`,
        'the Home progress primitive',
      );

      const heroProgress = await styleSnapshot(
        driver,
        '[aria-label="Continue watching"] .flowcard.center .progress',
      );
      const heroProgressBar = await styleSnapshot(
        driver,
        '[aria-label="Continue watching"] .flowcard.center .progress .bar',
      );
      const heroNoArt = await styleSnapshot(
        driver,
        '[aria-label="Continue watching"] .flowcard.center .noart',
      );
      const heroOverlay = await styleSnapshot(
        driver,
        '[aria-label="Continue watching"] .flowcard.center .playoverlay',
      );
      const heroPlayButton = await styleSnapshot(
        driver,
        '[aria-label="Continue watching"] .flowcard.center .playbtn',
      );

      await clickSide(driver, 'Mock Library');
      await driver.waitFor(
        `return !!document.querySelector('button.poster[aria-label^="Foundation Movie"] .progress')`,
        'the in-progress foundation grid card',
      );
      const gridProgress = await styleSnapshot(
        driver,
        'button.poster[aria-label^="Foundation Movie"] .progress',
      );
      const gridProgressBar = await styleSnapshot(
        driver,
        'button.poster[aria-label^="Foundation Movie"] .progress .bar',
      );
      const gridNoArt = await styleSnapshot(
        driver,
        'button.poster[aria-label^="Foundation Movie"] .noart',
      );

      await driver.click(
        await driver.find('css selector', 'button.poster[aria-label^="Foundation Movie"]'),
      );
      await driver.waitFor(
        `return !!document.querySelector('.detail .progress')
          && !!document.querySelector('.detail .noart')
          && !!document.querySelector('.detail .playoverlay')`,
        'the sparse Jellyfin item-detail foundation primitives',
      );

      const detailProgress = await styleSnapshot(driver, '.detail .progress');
      const detailProgressBar = await styleSnapshot(driver, '.detail .progress .bar');
      const detailNoArt = await styleSnapshot(driver, '.detail .noart');
      const detailOverlay = await styleSnapshot(driver, '.detail .playoverlay');
      const detailPlayButton = await styleSnapshot(driver, '.detail .playbtn');
      const detailPrimary = await styleSnapshot(driver, '.detail button.primary');

      assertSameStyles(
        '4px progress track',
        { hero: heroProgress, grid: gridProgress, detail: detailProgress },
        ['height', 'backgroundColor', 'backgroundImage'],
      );
      assert.equal(detailProgress.height, '4px', 'the shared progress track is exactly 4px');
      assertSameStyles(
        'gradient progress fill',
        { hero: heroProgressBar, grid: gridProgressBar, detail: detailProgressBar },
        ['height', 'backgroundColor', 'backgroundImage'],
      );
      assert.notEqual(detailProgressBar.backgroundImage, 'none', 'the shared progress fill is a gradient');
      assertSameStyles(
        'no-art placeholder',
        { hero: heroNoArt, grid: gridNoArt, detail: detailNoArt },
        [
          'display',
          'alignItems',
          'justifyContent',
          'backgroundImage',
          'color',
          'fontSize',
          'fontWeight',
          'lineHeight',
          'textAlign',
        ],
      );
      assertSameStyles(
        'primary button',
        {
          settings: settingsPrimary,
          playlists: playlistPrimary,
          detail: detailPrimary,
        },
        [
          'backgroundColor',
          'color',
          'borderStyle',
          'borderWidth',
          'borderRadius',
          'fontWeight',
          'cursor',
          'padding',
        ],
      );
      assertSameStyles(
        'play overlay',
        { hero: heroOverlay, detail: detailOverlay },
        ['backgroundColor', 'backgroundImage', 'transitionDuration', 'transitionProperty'],
      );
      assertSameStyles(
        'play button',
        { hero: heroPlayButton, detail: detailPlayButton },
        ['width', 'height', 'backgroundColor', 'color', 'borderRadius'],
      );
      if (expectReducedMotion) {
        for (const [label, snapshot] of Object.entries({
          settingsPrimary,
          playlistPrimary,
          heroOverlay,
          heroPlayButton,
          detailOverlay,
          detailPlayButton,
          detailPrimary,
        })) {
          assertMotionSuppressed(snapshot, label);
        }
      }
      await screenshot('03-one-light-detail');
    } finally {
      // Do not leak a light theme into another scenario if WebKit shares its
      // website-data store despite the harness' throwaway config directory.
      await driver
        .exec(`
          localStorage.setItem('vela-theme', 'dark');
          document.documentElement.setAttribute('data-theme', 'dark');
        `)
        .catch(() => {});
    }
  },
};
