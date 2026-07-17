// Slice 3 motion and full-surface empty states on the real Tauri/WebKit app.
// Assertions wait on DOM/server predicates; screenshots are inspection evidence,
// never timing witnesses for an animation frame.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { mockSource, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

const ENTER = '';
const expectReducedMotion = process.env.VELA_E2E_EXPECT_REDUCED_MOTION === '1';

let mainMock;
let emptyMock;

const resumeMovie = {
  id: 'motion-resume',
  name: 'Motion Resume',
  year: 2026,
  runTimeTicks: 100_000_000,
};

const watchedMovie = {
  id: 'motion-watched',
  name: 'Motion Watched',
  year: 2025,
  runTimeTicks: 100_000_000,
};

const fillerMovies = Array.from({ length: 14 }, (_, index) => ({
  id: `motion-filler-${index + 1}`,
  name: `Motion Filler ${String(index + 1).padStart(2, '0')}`,
  year: 2010 + index,
  runTimeTicks: 100_000_000,
}));

function milliseconds(value) {
  const duration = value.trim();
  return duration.endsWith('ms')
    ? Number.parseFloat(duration)
    : Number.parseFloat(duration) * 1000;
}

function millisecondList(value) {
  return value.split(',').map((entry) => milliseconds(entry));
}

function assertNear(actual, expected, label, tolerance = 0.05) {
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `${label}: expected ${expected}ms, got ${actual}ms`,
  );
}

function assertSuppressed(snapshot, label) {
  for (const property of [
    'animationDuration',
    'animationDelay',
    'transitionDuration',
    'transitionDelay',
  ]) {
    const values = millisecondList(snapshot[property]);
    assert.ok(
      values.length > 0 && values.every((value) => value <= 0.01),
      `${label} ${property} is not suppressed: ${snapshot[property]}`,
    );
  }
}

function assertSingleAnimation(snapshot, { name, duration, delay = 0, ease }, label) {
  assert.equal(snapshot.animationName, name, `${label} animation name`);
  assertNear(milliseconds(snapshot.animationDuration), duration, `${label} duration`);
  assertNear(milliseconds(snapshot.animationDelay), delay, `${label} delay`);
  assert.equal(snapshot.animationTimingFunction, ease, `${label} must use --ease`);
}

function assertTransition(snapshot, property, duration, ease, label) {
  const properties = snapshot.transitionProperty.split(',').map((entry) => entry.trim());
  const durations = millisecondList(snapshot.transitionDuration);
  const delays = millisecondList(snapshot.transitionDelay);
  const index = properties.indexOf(property);
  assert.notEqual(index, -1, `${label} must transition ${property}`);
  assertNear(durations[index % durations.length], duration, `${label} ${property} duration`);
  assertNear(delays[index % delays.length], 0, `${label} ${property} delay`);
  assert.ok(
    snapshot.transitionTimingFunction.includes(ease),
    `${label} must use --ease: ${snapshot.transitionTimingFunction}`,
  );
}

async function styleSnapshot(driver, selector, pseudo = null) {
  return driver.exec(`
    const node = document.querySelector(${JSON.stringify(selector)});
    if (!node) return null;
    const style = getComputedStyle(node, ${JSON.stringify(pseudo)});
    return {
      animationName: style.animationName,
      animationDuration: style.animationDuration,
      animationDelay: style.animationDelay,
      animationTimingFunction: style.animationTimingFunction,
      transitionProperty: style.transitionProperty,
      transitionDuration: style.transitionDuration,
      transitionDelay: style.transitionDelay,
      transitionTimingFunction: style.transitionTimingFunction,
      translate: style.translate,
      willChange: style.willChange,
    };
  `);
}

async function rootEase(driver) {
  return driver.exec(`
    const node = document.createElement('div');
    node.style.transitionTimingFunction = 'var(--ease)';
    document.body.append(node);
    const value = getComputedStyle(node).transitionTimingFunction;
    node.remove();
    return value;
  `);
}

async function clickSidebar(driver, label) {
  await driver.waitFor(
    `return [...document.querySelectorAll('button.sideitem')]
      .some((button) => button.textContent.trim() === ${JSON.stringify(label)})`,
    `sidebar item “${label}”`,
  );
  await driver.click(
    await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)=${JSON.stringify(label)}]`,
    ),
  );
}

async function clickPoster(driver, title) {
  await driver.waitFor(
    `return [...document.querySelectorAll('button.poster .t, button.poster .y')]
      .some((label) => label.textContent.trim() === ${JSON.stringify(title)})`,
    `poster “${title}”`,
  );
  await driver.click(
    await driver.find(
      'xpath',
      `//button[contains(@class,'poster')][.//*[contains(@class,'t') or contains(@class,'y')][normalize-space(.)=${JSON.stringify(title)}]]`,
    ),
  );
}

async function waitForEmptyState(driver, heading, hint, { announce = false } = {}) {
  await driver.waitFor(
    `return [...document.querySelectorAll('.empty-state')].some((state) =>
      state.querySelector('h2')?.textContent.trim() === ${JSON.stringify(heading)}
      && state.querySelector('p')?.textContent.trim() === ${JSON.stringify(hint)})`,
    `empty state “${heading}”`,
  );
  const states = await driver.exec(`return [...document.querySelectorAll('.empty-state')]
    .map((state) => ({
      heading: state.querySelector('h2')?.textContent.trim() ?? '',
      hint: state.querySelector('p')?.textContent.trim() ?? '',
      role: state.getAttribute('role'),
      decorativeIcon: !!state.querySelector('.empty-state-icon svg[aria-hidden="true"]'),
    }))`);
  const state = states.find((entry) => entry.heading === heading);
  assert.ok(state, `“${heading}” must use EmptyState`);
  assert.equal(state.hint, hint, `“${heading}” hint`);
  assert.equal(state.role, announce ? 'status' : null, `“${heading}” announcement role`);
  assert.equal(state.decorativeIcon, true, `“${heading}” decorative icon`);
}

async function openSettings(driver) {
  if (!(await driver.exec(`return !!document.querySelector('[role="dialog"][aria-label="Settings"]')`))) {
    await driver.click(await driver.find('css selector', 'button[aria-label="Settings"]'));
    await driver.waitFor(
      `return !!document.querySelector('[role="dialog"][aria-label="Settings"]')`,
      'Settings dialog',
    );
  }
}

async function closeSettings(driver) {
  if (!(await driver.exec(`return !!document.querySelector('[role="dialog"][aria-label="Settings"]')`))) return;
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
  await openSettings(driver);
  await driver.click(
    await driver.find(
      'xpath',
      `//button[@role='tab' and normalize-space(.)='Appearance']`,
    ),
  );
  await driver.click(
    await driver.find(
      'xpath',
      `//button[contains(@class,'themecard') and .//span[normalize-space(.)=${JSON.stringify(label)}]]`,
    ),
  );
  await driver.waitFor(
    `return document.documentElement.dataset.theme === ${JSON.stringify(id)}`,
    `${label} theme`,
  );
  assert.equal(
    await driver.exec(`return localStorage.getItem('vela-theme')`),
    id,
    `${label} must persist`,
  );
}

async function assertSettingsMotion(driver, ease) {
  const overlay = await styleSnapshot(driver, '.overlay[role="presentation"]');
  const panel = await styleSnapshot(driver, '[role="dialog"][aria-label="Settings"]');
  assert.ok(overlay && panel, 'Settings motion surfaces must render');
  if (expectReducedMotion) {
    assertSuppressed(overlay, 'Settings scrim');
    assertSuppressed(panel, 'Settings panel');
  } else {
    assertSingleAnimation(
      overlay,
      { name: 'vela-fade', duration: 160, ease },
      'Settings scrim',
    );
    assertSingleAnimation(
      panel,
      { name: 'vela-pop', duration: 180, ease },
      'Settings panel',
    );
  }
}

async function assertTranslateAndKillSwitch(driver) {
  const probe = await driver.exec(`
    const node = document.createElement('div');
    node.style.cssText = [
      'position:fixed',
      'left:-9999px',
      'translate:2px 0',
      'animation:vela-rise 1s var(--ease) 500ms backwards',
      'transition:opacity 1s var(--ease) 500ms',
    ].join(';');
    document.body.append(node);
    const style = getComputedStyle(node);
    const result = {
      supported: CSS.supports('translate', '0 1px'),
      translate: style.translate,
      animationDuration: style.animationDuration,
      animationDelay: style.animationDelay,
      transitionDuration: style.transitionDuration,
      transitionDelay: style.transitionDelay,
    };
    node.remove();
    return result;
  `);
  assert.equal(probe.supported, true, 'WebKit must support the individual translate property');
  if (expectReducedMotion) {
    assert.equal(probe.translate, 'none', 'reduced motion must suppress individual translate');
    assertSuppressed(
      {
        ...probe,
        animationName: 'vela-rise',
        animationTimingFunction: '',
        transitionProperty: 'opacity',
        transitionTimingFunction: '',
      },
      'injected kill-switch probe',
    );
  } else {
    assert.notEqual(probe.translate, 'none', 'individual translate must compute when motion is enabled');
  }
}

async function assertHeroMotion(driver, ease) {
  const cards = await driver.exec(`return [...document.querySelectorAll('.flowcard')].map((node) => {
    const style = getComputedStyle(node);
    return {
      transitionProperty: style.transitionProperty,
      transitionDuration: style.transitionDuration,
      transitionDelay: style.transitionDelay,
      transitionTimingFunction: style.transitionTimingFunction,
      animationDuration: style.animationDuration,
      animationDelay: style.animationDelay,
      willChange: style.willChange,
    };
  })`);
  assert.ok(cards.length > 0 && cards.length <= 9, `cover-flow must render 1–9 cards, got ${cards.length}`);
  for (const [index, card] of cards.entries()) {
    assert.ok(card.willChange.split(',').map((value) => value.trim()).includes('transform'));
    if (expectReducedMotion) {
      assertSuppressed(card, `flowcard ${index}`);
    } else {
      assertTransition(card, 'transform', 320, ease, `flowcard ${index}`);
      assertTransition(card, 'filter', 320, ease, `flowcard ${index}`);
    }
  }

  const ground = await driver.exec(`
    const flow = document.querySelector('.flow');
    if (!flow) return null;
    const style = getComputedStyle(flow, '::after');
    return {
      content: style.content,
      backgroundImage: style.backgroundImage,
      pointerEvents: style.pointerEvents,
      zIndex: style.zIndex,
      width: Number.parseFloat(style.width),
      height: Number.parseFloat(style.height),
    };
  `);
  assert.ok(ground, 'cover-flow ground pseudo-element must render');
  assert.notEqual(ground.content, 'none');
  assert.ok(ground.backgroundImage.includes('radial-gradient'), ground.backgroundImage);
  assert.equal(ground.pointerEvents, 'none');
  assert.equal(ground.zIndex, '0');
  assert.ok(ground.width > 0 && ground.height > 0, 'cover-flow ground has geometry');
}

async function assertGridMotion(driver, ease) {
  const cards = await driver.exec(`return [...document.querySelectorAll('main.grid > .poster')].map((node) => {
    const style = getComputedStyle(node);
    const art = getComputedStyle(node.querySelector('.art'));
    return {
      animationName: style.animationName,
      animationDuration: style.animationDuration,
      animationDelay: style.animationDelay,
      animationTimingFunction: style.animationTimingFunction,
      transitionProperty: style.transitionProperty,
      transitionDuration: style.transitionDuration,
      transitionDelay: style.transitionDelay,
      transitionTimingFunction: style.transitionTimingFunction,
      artTransitionProperty: art.transitionProperty,
      artTransitionDuration: art.transitionDuration,
      artTransitionDelay: art.transitionDelay,
    };
  })`);
  assert.equal(cards.length, 16, 'the populated grid must expose the full stagger cap');
  if (expectReducedMotion) {
    for (const [index, card] of cards.entries()) {
      assertSuppressed(card, `grid poster ${index}`);
      assert.ok(
        millisecondList(card.artTransitionDuration).every((value) => value <= 0.01)
          && millisecondList(card.artTransitionDelay).every((value) => value <= 0.01),
        `grid art ${index} transition is not suppressed`,
      );
    }
  } else {
    const actualDelays = cards.map((card) => Math.round(milliseconds(card.animationDelay)));
    const expectedDelays = cards.map((_, index) => Math.min(index, 14) * 22);
    assert.deepEqual(actualDelays, expectedDelays, 'grid stagger must cap at 14 × 22ms');
    for (const [index, card] of cards.entries()) {
      assertSingleAnimation(
        card,
        { name: 'vela-rise', duration: 400, delay: expectedDelays[index], ease },
        `grid poster ${index}`,
      );
    }
  }

  const badge = await styleSnapshot(
    driver,
    'button.poster[aria-label^="Motion Watched"] .watchedbadge',
  );
  assert.ok(badge, 'the played grid item must render its watched badge');
  if (expectReducedMotion) {
    assertSuppressed(badge, 'watched badge');
  } else {
    assertSingleAnimation(
      badge,
      { name: 'vela-pop', duration: 130, ease },
      'watched badge',
    );
  }
}

async function assertCrumbAndSurfaceMotion(driver, selector, ease, label) {
  const crumbs = await styleSnapshot(driver, '.crumbs');
  const surface = await styleSnapshot(driver, selector);
  assert.ok(crumbs && surface, `${label} and crumbs must render`);
  if (expectReducedMotion) {
    assertSuppressed(crumbs, `${label} crumbs`);
    assertSuppressed(surface, label);
  } else {
    assertSingleAnimation(
      crumbs,
      { name: 'vela-slide-down', duration: 160, ease },
      `${label} crumbs`,
    );
    assertSingleAnimation(
      surface,
      { name: 'vela-rise', duration: 200, ease },
      label,
    );
  }
}

export default {
  name: 'uimotion',

  async seed({ configRoot }) {
    if (expectReducedMotion) {
      const gtkConfig = path.join(configRoot, 'config', 'gtk-3.0');
      fs.mkdirSync(gtkConfig, { recursive: true });
      fs.writeFileSync(
        path.join(gtkConfig, 'settings.ini'),
        '[Settings]\ngtk-enable-animations=false\n',
      );
    }

    mainMock = await startMockJellyfin({
      views: [
        {
          id: 'motion-movies',
          name: 'Motion Movies',
          collectionType: 'movies',
          movies: [resumeMovie, watchedMovie, ...fillerMovies],
        },
        {
          id: 'motion-empty',
          name: 'Empty Library',
          collectionType: 'movies',
          movies: [],
        },
        {
          id: 'motion-shows',
          name: 'Motion Shows',
          collectionType: 'tvshows',
          movies: [
            {
              id: 'motion-show',
              name: 'Empty Show',
              type: 'Series',
              year: 2026,
            },
          ],
        },
      ],
      children: {
        'motion-show': [
          {
            id: 'motion-season',
            name: 'Empty Season',
            type: 'Season',
            seriesId: 'motion-show',
            seriesName: 'Empty Show',
            index: 1,
          },
        ],
        'motion-season': [],
      },
      playlists: [
        { id: 'motion-server-empty', name: 'Empty Server List', itemIds: [] },
      ],
      serveResume: true,
    });
    mainMock.state.userData['motion-watched'].played = true;

    emptyMock = await startMockJellyfin({ views: [] });
    seedConfig(configRoot, [
      mockSource(mainMock, { id: 'motion-main', name: 'Motion Source' }),
      mockSource(emptyMock, { id: 'motion-void', name: 'Motion Void' }),
    ]);
  },

  async cleanup() {
    await Promise.all([mainMock?.close(), emptyMock?.close()]);
  },

  async run({ driver, screenshot }) {
    try {
      await driver.waitFor(
        `return document.readyState === 'complete'
          && [...document.querySelectorAll('button.sideitem')]
            .some((button) => button.textContent.trim() === 'Motion Source')
          && [...document.querySelectorAll('button.sideitem')]
            .some((button) => button.textContent.trim() === 'Motion Void')`,
        'both motion sources',
      );
      if (expectReducedMotion) {
        assert.equal(
          await driver.exec(`return matchMedia('(prefers-reduced-motion: reduce)').matches`),
          true,
          'the reduced-motion run must expose the GTK preference to WebKit',
        );
      }

      const ease = await rootEase(driver);
      assert.ok(ease.startsWith('cubic-bezier('), `unexpected --ease value: ${ease}`);
      await assertTranslateAndKillSwitch(driver);

      await waitForEmptyState(
        driver,
        'No titles on Home yet',
        'Choose a library from the sidebar to start browsing.',
      );

      await clickSidebar(driver, 'Motion Void');
      await waitForEmptyState(
        driver,
        'No libraries found',
        'Check the connected server, then use Refresh libraries.',
      );

      await clickSidebar(driver, 'All');
      await waitForEmptyState(
        driver,
        'No titles on Home yet',
        'Choose a library from the sidebar to start browsing.',
      );

      await openSettings(driver);
      await assertSettingsMotion(driver, ease);
      await chooseTheme(driver, 'Vela Dark', 'dark');
      await closeSettings(driver);

      const resumeRequestsBefore = mainMock.state.requests.filter(
        (request) => request.path === `/Users/${mainMock.userId}/Items/Resume`,
      ).length;
      mainMock.state.userData['motion-resume'].positionTicks = 25_000_000;
      await driver.click(
        await driver.find('css selector', 'button[aria-label="Refresh libraries"]'),
      );
      await driver.waitFor(
        `return document.querySelector('.flowcard.center')?.getAttribute('title') === 'Motion Resume'
          && !document.querySelector('button[aria-label="Refresh libraries"]')?.disabled`,
        'ordinary Refresh to reveal Continue Watching',
      );
      assert.ok(
        mainMock.state.requests.filter(
          (request) => request.path === `/Users/${mainMock.userId}/Items/Resume`,
        ).length > resumeRequestsBefore,
        'Refresh libraries must issue a new Resume request',
      );
      await assertHeroMotion(driver, ease);
      await screenshot('01-dark-home');

      await clickSidebar(driver, 'Motion Source');
      await clickSidebar(driver, 'Motion Movies');
      await driver.waitFor(
        `return document.querySelectorAll('main.grid > button.poster').length === 16
          && !!document.querySelector('button.poster[aria-label^="Motion Watched"] .watchedbadge')`,
        'the populated movie grid and watched badge',
      );
      await assertGridMotion(driver, ease);

      await driver.click(
        await driver.find('css selector', 'button.poster[aria-label^="Motion Resume"]'),
      );
      await driver.waitFor(
        `return !!document.querySelector('.detail') && !!document.querySelector('.crumbs')`,
        'movie detail surface',
      );
      await assertCrumbAndSurfaceMotion(driver, '.detail', ease, 'item detail');
      const detailPrimary = await styleSnapshot(driver, '.detail button.primary');
      assert.ok(detailPrimary, 'detail primary action must render');
      if (expectReducedMotion) {
        assertSuppressed(detailPrimary, 'detail primary action');
      } else {
        assertTransition(detailPrimary, 'translate', 80, ease, 'detail primary action');
      }
      await screenshot('02-dark-detail');

      await driver.click(await driver.find('css selector', '.crumbs button.back'));
      await clickSidebar(driver, 'Empty Library');
      await waitForEmptyState(
        driver,
        'No titles in this view',
        'Go back, refresh libraries, or choose another library.',
      );
      const browseCrumbs = await styleSnapshot(driver, '.crumbs');
      if (expectReducedMotion) {
        assertSuppressed(browseCrumbs, 'browse crumbs');
      } else {
        assertSingleAnimation(
          browseCrumbs,
          { name: 'vela-slide-down', duration: 160, ease },
          'browse crumbs',
        );
      }

      await chooseTheme(driver, 'One Light', 'one-light');
      await closeSettings(driver);
      await screenshot('03-one-light-empty-library');

      const search = await driver.find(
        'css selector',
        'input[aria-label="Search your libraries"]',
      );
      await driver.type(search, `Motion Missing${ENTER}`);
      await waitForEmptyState(
        driver,
        'No matches for “Motion Missing”',
        'Check the spelling or try a broader search.',
        { announce: true },
      );

      await clickSidebar(driver, 'Playlists');
      await waitForEmptyState(
        driver,
        'No playlists yet',
        'Name one above, then add titles from their context menus.',
      );
      const playlistName = await driver.find(
        'css selector',
        'section.playlists #playlist-create',
      );
      await driver.type(playlistName, 'Motion List');
      await driver.waitFor(
        `return !document.querySelector('section.playlists form.create button.primary')?.disabled`,
        'enabled playlist Create action',
      );
      const createButton = await styleSnapshot(
        driver,
        'section.playlists form.create button.primary',
      );
      if (expectReducedMotion) assertSuppressed(createButton, 'playlist Create action');
      await driver.click(
        await driver.find(
          'css selector',
          'section.playlists form.create button.primary',
        ),
      );
      await waitForEmptyState(
        driver,
        'This playlist is empty',
        "Use a title's context menu to add it here.",
      );

      await driver.waitFor(
        `return !!document.querySelector('button[aria-label="Open Empty Server List from Motion Source"]')`,
        'empty server playlist navigation',
      );
      await driver.click(
        await driver.find(
          'css selector',
          'button[aria-label="Open Empty Server List from Motion Source"]',
        ),
      );
      await waitForEmptyState(
        driver,
        'This server playlist is empty',
        'Add videos on Motion Source, then reopen it here.',
      );
      await screenshot('04-one-light-empty-server-playlist');

      await clickSidebar(driver, 'Motion Shows');
      await clickPoster(driver, 'Empty Show');
      await clickPoster(driver, 'Empty Season');
      await waitForEmptyState(
        driver,
        'No episodes in this season',
        'Go back and choose another season.',
      );
      assert.equal(
        await driver.exec(`return !!document.querySelector('.season [aria-busy="true"]')`),
        false,
        'zero episodes must be a settled empty result, not a loader',
      );
      await assertCrumbAndSurfaceMotion(driver, '.season', ease, 'season detail');
      await screenshot('05-one-light-empty-season');

      assert.deepEqual(mainMock.state.contractViolations, []);
      assert.deepEqual(emptyMock.state.contractViolations, []);
    } finally {
      try {
        if (
          (await driver.exec(`return document.readyState === 'complete'`))
          && (await driver.exec(`return document.documentElement.dataset.theme`)) !== 'dark'
        ) {
          await chooseTheme(driver, 'Vela Dark', 'dark');
        }
        await closeSettings(driver);
      } catch {
        // The throwaway profile is discarded; restoration is best-effort after
        // an earlier driver/app failure, but deterministic on the success path.
      }
    }
  },
};
