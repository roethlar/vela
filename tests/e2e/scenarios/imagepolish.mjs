// Image-loading polish on the real Tauri/WebKit app. The mock parks named
// image responses so the unloaded state is a positive server witness, not a
// race against a fast local image decode.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { mockSource, pollUntil, seedConfig } from '../helpers.mjs';
import { startMockJellyfin } from '../mockjf.mjs';

let mock;
const expectReducedMotion = process.env.VELA_E2E_EXPECT_REDUCED_MOTION === '1';

const HELD_POSTER = '/Items/held-poster/Images/Primary';
const HELD_BACKDROP = '/Items/held-poster/Images/Backdrop/0';
const FAILED_POSTER = '/Items/failed-poster/Images/Primary';
const SUCCESS_EPISODE = '/Items/episode-success/Images/Primary';
const FAILED_EPISODE = '/Items/episode-failure/Images/Primary';

const frameSelector = (selector) =>
  `document.querySelector(${JSON.stringify(selector)})`;

const episodeFrame = (title) => `(() => {
  const row = [...document.querySelectorAll('.season .eprow')]
    .find((node) => node.querySelector('.eptitle')?.textContent.includes(${JSON.stringify(title)}));
  return row?.querySelector('.epthumb') ?? null;
})()`;

function imageStateScript(frameExpression) {
  return `
    const frame = ${frameExpression};
    if (!frame) return null;
    const image = frame.querySelector('img.image-reveal');
    if (!image) return { frame: true, image: false };
    const style = getComputedStyle(image);
    const rect = image.getBoundingClientRect();
    const frameRect = frame.getBoundingClientRect();
    const underlay = frame.querySelector(':scope > .noart');
    const underlayStyle = underlay ? getComputedStyle(underlay) : null;
    return {
      frame: true,
      image: true,
      src: image.currentSrc || image.src,
      complete: image.complete,
      naturalWidth: image.naturalWidth,
      naturalHeight: image.naturalHeight,
      loaded: image.classList.contains('image-loaded'),
      reveal: image.classList.contains('image-reveal'),
      cover: image.classList.contains('image-cover'),
      opacity: Number.parseFloat(style.opacity),
      position: style.position,
      transitionProperty: style.transitionProperty,
      transitionDuration: style.transitionDuration,
      rect: { width: rect.width, height: rect.height },
      frameRect: { width: frameRect.width, height: frameRect.height },
      underlay: underlay ? {
        text: underlay.textContent.trim(),
        display: underlayStyle.display,
        visibility: underlayStyle.visibility,
      } : null,
    };
  `;
}

async function imageState(driver, frameExpression) {
  return driver.exec(imageStateScript(frameExpression));
}

function milliseconds(duration) {
  const value = duration.trim();
  return value.endsWith('ms')
    ? Number.parseFloat(value)
    : Number.parseFloat(value) * 1000;
}

function assertImageTransition(snapshot, label) {
  const properties = snapshot.transitionProperty
    .split(',')
    .map((property) => property.trim());
  const durations = snapshot.transitionDuration
    .split(',')
    .map(milliseconds);
  assert.ok(properties.includes('opacity'), `${label} must transition opacity`);
  assert.ok(
    properties.every((property) => property === 'opacity'),
    `${label} must not transition layout/transform/filter: ${snapshot.transitionProperty}`,
  );
  assert.ok(durations.length > 0 && durations.every(Number.isFinite));
  if (expectReducedMotion) {
    assert.ok(
      durations.every((duration) => duration <= 0.01),
      `${label} transition must be suppressed: ${snapshot.transitionDuration}`,
    );
  } else {
    assert.ok(
      durations.every((duration) => duration >= 100 && duration <= 300),
      `${label} transition must stay within 100–300ms: ${snapshot.transitionDuration}`,
    );
  }
}

function assertCoverGeometry(snapshot, label) {
  assert.equal(snapshot.reveal, true, `${label} uses the reveal primitive`);
  assert.equal(snapshot.cover, true, `${label} uses the fixed-frame cover layer`);
  assert.equal(snapshot.position, 'absolute', `${label} must not participate in layout`);
  assert.ok(snapshot.rect.width > 0 && snapshot.rect.height > 0, `${label} has fixed geometry`);
  assert.ok(
    Math.abs(snapshot.rect.width - snapshot.frameRect.width) <= 3 &&
      Math.abs(snapshot.rect.height - snapshot.frameRect.height) <= 3,
    `${label} must cover its frame: ${JSON.stringify(snapshot)}`,
  );
}

function assertSameGeometry(before, after, label) {
  assert.ok(
    Math.abs(before.rect.width - after.rect.width) <= 0.5 &&
      Math.abs(before.rect.height - after.rect.height) <= 0.5 &&
      Math.abs(before.frameRect.width - after.frameRect.width) <= 0.5 &&
      Math.abs(before.frameRect.height - after.frameRect.height) <= 0.5,
    `${label} geometry changed across reveal`,
  );
}

async function waitForImageElement(driver, frameExpression, label) {
  await driver.waitFor(
    `
      const frame = ${frameExpression};
      return !!frame?.querySelector('img.image-reveal.image-cover');
    `,
    `${label} image element`,
  );
}

async function waitForLoaded(driver, frameExpression, label) {
  await driver.waitFor(
    `
      const frame = ${frameExpression};
      const image = frame?.querySelector('img.image-reveal.image-cover');
      return !!image
        && image.complete
        && image.naturalWidth > 0
        && image.classList.contains('image-loaded')
        && Number.parseFloat(getComputedStyle(image).opacity) >= 0.999;
    `,
    `${label} to decode and reveal`,
  );
  return imageState(driver, frameExpression);
}

async function waitForFailure(driver, frameExpression, title, label) {
  await driver.waitFor(
    `
      const frame = ${frameExpression};
      const image = frame?.querySelector('img.image-reveal.image-cover');
      const underlay = frame?.querySelector(':scope > .noart');
      return !!image
        && image.complete
        && image.naturalWidth === 0
        && !image.classList.contains('image-loaded')
        && Number.parseFloat(getComputedStyle(image).opacity) === 0
        && underlay?.textContent.trim().includes(${JSON.stringify(title)})
        && getComputedStyle(underlay).display !== 'none'
        && getComputedStyle(underlay).visibility !== 'hidden';
    `,
    `${label} failure underlay`,
  );
  const snapshot = await imageState(driver, frameExpression);
  assert.equal(snapshot.loaded, false, `${label} must remain unrevealed`);
  assert.equal(snapshot.naturalWidth, 0, `${label} must be a failed decode`);
  assert.equal(snapshot.opacity, 0, `${label} failed image stays transparent`);
  assert.ok(snapshot.underlay?.text.includes(title), `${label} keeps its title underlay`);
  assertImageTransition(snapshot, label);
  assertCoverGeometry(snapshot, label);
  return snapshot;
}

async function waitForImageArrival(pathname) {
  return pollUntil(
    () => mock.state.imageArrivals.find((request) => request.path === pathname),
    `${pathname} image request to arrive`,
  );
}

async function waitForImageResponse(pathname, status) {
  return pollUntil(
    () =>
      mock.state.imageServed.find(
        (response) => response.path === pathname && response.status === status,
      ),
    `${pathname} image response ${status}`,
  );
}

async function clickSidebar(driver, label) {
  await driver.click(
    await driver.find(
      'xpath',
      `//button[contains(@class,'sideitem') and normalize-space(.)=${JSON.stringify(label)}]`,
    ),
  );
}

async function clickPoster(driver, title) {
  await driver.waitFor(
    `return !!document.querySelector(${JSON.stringify(`button.poster[aria-label^="${title}"]`)})`,
    `${title} poster`,
  );
  await driver.click(
    await driver.find('css selector', `button.poster[aria-label^="${title}"]`),
  );
}

async function closeDetail(driver) {
  await driver.click(await driver.find('css selector', '.crumbs button.back'));
  await driver.waitFor(
    `return !document.querySelector('.detail, .season')`,
    'detail surface to close',
  );
}

async function chooseTheme(driver, label, id) {
  await driver.click(await driver.find('css selector', 'button[aria-label="Settings"]'));
  await driver.waitFor(
    `return !!document.querySelector('[role="dialog"][aria-label="Settings"]')`,
    'Settings dialog',
  );
  await driver.click(
    await driver.find('xpath', `//button[@role='tab' and normalize-space(.)='Appearance']`),
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

async function assertEveryRenderedImageTransition(driver, label) {
  const transitions = await driver.exec(`return [...document.querySelectorAll('img.image-reveal')]
    .map((image) => ({
      transitionProperty: getComputedStyle(image).transitionProperty,
      transitionDuration: getComputedStyle(image).transitionDuration,
    }))`);
  assert.ok(transitions.length > 0, `${label} must render a media image`);
  transitions.forEach((transition, index) =>
    assertImageTransition(transition, `${label} image ${index + 1}`),
  );
}

export default {
  name: 'imagepolish',

  async seed({ configRoot }) {
    if (expectReducedMotion) {
      const gtkConfig = path.join(configRoot, 'config', 'gtk-3.0');
      fs.mkdirSync(gtkConfig, { recursive: true });
      fs.writeFileSync(
        path.join(gtkConfig, 'settings.ini'),
        '[Settings]\ngtk-enable-animations=false\n',
      );
    }

    mock = await startMockJellyfin({
      views: [
        {
          id: 'image-movies',
          name: 'Image Movies',
          collectionType: 'movies',
          movies: [
            {
              id: 'held-poster',
              name: 'Held Poster',
              year: 2026,
              imageTag: 'held-primary-v1',
              backdropTag: 'held-backdrop-v1',
            },
            {
              id: 'failed-poster',
              name: 'Failed Poster',
              year: 2025,
              imageTag: 'failed-primary-v1',
            },
          ],
        },
        {
          id: 'image-shows',
          name: 'Image Shows',
          collectionType: 'tvshows',
          movies: [
            {
              id: 'image-show',
              name: 'Image Show',
              year: 2026,
              imageTag: 'show-primary-v1',
            },
          ],
        },
      ],
      children: {
        'image-show': [
          {
            id: 'image-season',
            name: 'Season One',
            type: 'Season',
            seriesId: 'image-show',
            seriesName: 'Image Show',
            index: 1,
            imageTag: 'season-primary-v1',
          },
        ],
        'image-season': [
          {
            id: 'episode-success',
            name: 'Successful Episode',
            type: 'Episode',
            seriesId: 'image-show',
            seasonId: 'image-season',
            seriesName: 'Image Show',
            seasonName: 'Season One',
            parentIndex: 1,
            index: 1,
            imageTag: 'episode-success-v1',
            seriesPrimaryImageTag: 'show-primary-v1',
          },
          {
            id: 'episode-failure',
            name: 'Failed Episode',
            type: 'Episode',
            seriesId: 'image-show',
            seasonId: 'image-season',
            seriesName: 'Image Show',
            seasonName: 'Season One',
            parentIndex: 1,
            index: 2,
            imageTag: 'episode-failure-v1',
            seriesPrimaryImageTag: 'show-primary-v1',
          },
        ],
      },
    });

    mock.state.holdImage(HELD_POSTER);
    mock.state.holdImage(HELD_BACKDROP);
    mock.state.holdImage(FAILED_POSTER);
    mock.state.setImage404(FAILED_POSTER);
    mock.state.setImage404(FAILED_EPISODE);
    seedConfig(configRoot, [mockSource(mock)]);
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    try {
      await driver.waitFor(
        `return document.readyState === 'complete'
          && [...document.querySelectorAll('button.sideitem')]
            .some((button) => button.textContent.trim() === 'Image Movies')`,
        'seeded image libraries',
      );
      if (expectReducedMotion) {
        assert.equal(
          await driver.exec(`return matchMedia('(prefers-reduced-motion: reduce)').matches`),
          true,
          'the reduced-motion run must expose the OS preference to WebKit',
        );
      }

      await chooseTheme(driver, 'Vela Dark', 'dark');
      await clickSidebar(driver, 'Image Movies');
      const heldPosterFrame = frameSelector(
        'button.poster[aria-label^="Held Poster"] .art',
      );
      await waitForImageArrival(HELD_POSTER);
      await waitForImageElement(driver, heldPosterFrame, 'held grid poster');
      assert.equal(
        mock.state.imageServed.some((response) => response.path === HELD_POSTER),
        false,
        'the held-state assertion must precede the poster response',
      );
      const heldPoster = await imageState(driver, heldPosterFrame);
      assert.equal(heldPoster.complete, false, 'the server-held poster stays incomplete');
      assert.equal(heldPoster.naturalWidth, 0, 'the server-held poster has not decoded');
      assert.equal(heldPoster.loaded, false, 'the server-held poster is not marked loaded');
      assert.equal(heldPoster.opacity, 0, 'the server-held poster stays transparent');
      assertCoverGeometry(heldPoster, 'held grid poster');
      assertImageTransition(heldPoster, 'held grid poster');
      await assertEveryRenderedImageTransition(driver, 'dark held grid');
      await screenshot('01-dark-held-poster');

      mock.state.releaseImage(HELD_POSTER);
      await waitForImageResponse(HELD_POSTER, 200);
      const loadedPoster = await waitForLoaded(
        driver,
        heldPosterFrame,
        'released grid poster',
      );
      assertSameGeometry(heldPoster, loadedPoster, 'grid poster');
      assertImageTransition(loadedPoster, 'loaded grid poster');
      await screenshot('02-dark-loaded-poster');

      await clickPoster(driver, 'Held Poster');
      const backdropFrame = frameSelector('.detail .backdrop');
      await waitForImageArrival(HELD_BACKDROP);
      await waitForImageElement(driver, backdropFrame, 'held detail backdrop');
      assert.equal(
        mock.state.imageServed.some((response) => response.path === HELD_BACKDROP),
        false,
        'the held-state assertion must precede the backdrop response',
      );
      const heldBackdrop = await imageState(driver, backdropFrame);
      assert.equal(heldBackdrop.complete, false, 'the held backdrop stays incomplete');
      assert.equal(heldBackdrop.naturalWidth, 0, 'the held backdrop has not decoded');
      assert.equal(heldBackdrop.loaded, false, 'the held backdrop is not marked loaded');
      assert.equal(heldBackdrop.opacity, 0, 'the held backdrop stays transparent');
      assertCoverGeometry(heldBackdrop, 'held detail backdrop');
      assertImageTransition(heldBackdrop, 'held detail backdrop');
      await assertEveryRenderedImageTransition(driver, 'dark held detail');
      await screenshot('03-dark-held-backdrop');

      mock.state.releaseImage(HELD_BACKDROP);
      await waitForImageResponse(HELD_BACKDROP, 200);
      const loadedBackdrop = await waitForLoaded(
        driver,
        backdropFrame,
        'released detail backdrop',
      );
      assertSameGeometry(heldBackdrop, loadedBackdrop, 'detail backdrop');
      assertImageTransition(loadedBackdrop, 'loaded detail backdrop');
      await screenshot('04-dark-loaded-backdrop');

      await chooseTheme(driver, 'One Light', 'one-light');
      await closeDetail(driver);
      await clickPoster(driver, 'Failed Poster');
      const failedPosterFrame = frameSelector('.detail .posterframe');
      await waitForImageArrival(FAILED_POSTER);
      await waitForImageElement(driver, failedPosterFrame, 'failed detail poster');
      mock.state.releaseImage(FAILED_POSTER);
      await waitForImageResponse(FAILED_POSTER, 404);
      await waitForFailure(
        driver,
        failedPosterFrame,
        'Failed Poster',
        'failed detail poster',
      );
      await assertEveryRenderedImageTransition(driver, 'One Light failed detail');
      await screenshot('05-one-light-failed-detail');

      await closeDetail(driver);
      await clickSidebar(driver, 'Image Shows');
      await clickPoster(driver, 'Image Show');
      await clickPoster(driver, 'Season One');
      await driver.waitFor(
        `return document.querySelectorAll('.season .eprow').length === 2`,
        'the seeded season episode rows',
      );

      await waitForImageResponse(SUCCESS_EPISODE, 200);
      await waitForImageResponse(FAILED_EPISODE, 404);
      const successfulRowFrame = episodeFrame('Successful Episode');
      const failedRowFrame = episodeFrame('Failed Episode');
      const successfulRow = await waitForLoaded(
        driver,
        successfulRowFrame,
        'successful episode row',
      );
      assertImageTransition(successfulRow, 'successful episode row');
      await waitForFailure(
        driver,
        failedRowFrame,
        'Failed Episode',
        'failed episode row',
      );

      const panelFrame = frameSelector('.season .stillwrap');
      const successfulPanel = await waitForLoaded(
        driver,
        panelFrame,
        'successful selected-episode panel',
      );
      assert.ok(
        successfulPanel.src.includes('/Items/episode-success/Images/Primary'),
        `the successful episode must own the initial panel, got ${successfulPanel.src}`,
      );
      assertImageTransition(successfulPanel, 'successful selected-episode panel');

      await driver.click(
        await driver.find(
          'xpath',
          `//button[contains(@class,'eprow') and .//span[contains(@class,'eptitle') and contains(normalize-space(.),'Failed Episode')]]`,
        ),
      );
      await driver.waitFor(
        `return [...document.querySelectorAll('.season .eprow')]
          .some((row) => row.classList.contains('selected')
            && row.querySelector('.eptitle')?.textContent.includes('Failed Episode'))`,
        'the failed episode selection',
      );
      await waitForFailure(
        driver,
        panelFrame,
        'Failed Episode',
        'failed selected-episode panel',
      );
      await assertEveryRenderedImageTransition(driver, 'One Light season detail');
      await screenshot('06-one-light-season-images');

      assert.deepEqual(mock.state.contractViolations, [], 'image scenario query contracts');
    } finally {
      // Do not leak the light theme if the WebKit website-data store outlives
      // this scenario's throwaway config directory.
      await driver
        .exec(`
          localStorage.setItem('vela-theme', 'dark');
          document.documentElement.setAttribute('data-theme', 'dark');
        `)
        .catch(() => {});
    }
  },
};
