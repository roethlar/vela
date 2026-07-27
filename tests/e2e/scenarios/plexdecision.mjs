// A failed Plex capability request is not a valid conversion refusal. Exercise
// the real command/UI boundary and require the inline diagnostic to retain only
// the safe status category while ordinary playback remains available.
import assert from 'node:assert/strict';
import { seedConfig } from '../helpers.mjs';
import { createMockPlexTls, mockPlexSource, startMockPlex } from '../mockplex.mjs';

const TITLE = 'Decision Diagnostic Movie';
const BODY_SENTINEL = 'SYNTHETIC_DECISION_BODY';
let tls;
let mock;

async function openContextMenu(driver) {
  await driver.exec(
    `const el = document.querySelector('button.poster[aria-label^="${TITLE}"]');
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', {
       bubbles: true, cancelable: true,
       clientX: r.x + r.width / 2, clientY: r.y + r.height / 2,
     }));`,
  );
  await driver.waitFor(`return !!document.querySelector('.ctxmenu')`, 'Plex context menu');
}

export default {
  name: 'plexdecision',

  async seed({ configRoot }) {
    tls = createMockPlexTls(configRoot);
    mock = await startMockPlex({
      tls,
      name: 'Decision Plex',
      machineIdentifier: 'decision-machine',
      token: 'synthetic-decision-e2e-token',
      movieTitle: TITLE,
      decisionResponse: {
        status: 503,
        body: `<MediaContainer diagnostic="${BODY_SENTINEL}" />`,
      },
    });
    seedConfig(configRoot, [mockPlexSource(mock, { id: 'plex-decision' })]);
  },

  environment() {
    return { SSL_CERT_FILE: tls.ca };
  },

  async cleanup() {
    await mock?.close();
  },

  async run({ driver, screenshot }) {
    await driver.waitFor(
      `return [...document.querySelectorAll('button.sideitem')]
        .some((button) => button.textContent.trim() === 'Movies')`,
      'Plex Movies tab',
    );
    await driver.click(
      await driver.find(
        'xpath',
        `//button[contains(@class,'sideitem') and normalize-space(.)='Movies']`,
      ),
    );
    await driver.waitFor(
      `return !!document.querySelector('button.poster[aria-label^="${TITLE}"]')`,
      'Plex decision movie',
    );
    await openContextMenu(driver);
    await driver.click(
      await driver.find(
        'xpath',
        `//button[@role='menuitem' and normalize-space(.)='Play at Quality']`,
      ),
    );
    await driver.waitFor(
      `return !!document.querySelector('[role="group"][aria-label="Choose a quality"] [role="alert"]')`,
      'sanitized Plex decision alert',
    );

    const alert = await driver.exec(
      `return document.querySelector(
        '[role="group"][aria-label="Choose a quality"] [role="alert"]'
      )?.textContent.trim() ?? ''`,
    );
    assert.equal(
      alert,
      'Plex could not check conversion because the server returned HTTP 503.',
      'a failed request must not masquerade as a valid conversion refusal',
    );

    const decision = mock.state.requests.find(
      (request) => request.path === '/video/:/transcode/universal/decision',
    );
    assert.ok(decision, 'the submenu must reach the real Plex decision endpoint');
    assert.equal(decision.tokenMatches, true, 'the decision request must use header auth');
    assert.equal(
      decision.query['X-Plex-Client-Profile-Name'],
      'Web',
      'the diagnostic path must retain the HLS profile contract',
    );
    assert.ok(
      !Object.keys(decision.query).some((key) => /token/i.test(key)),
      'the decision query must remain token-free',
    );

    for (const secret of [
      mock.token,
      '127.0.0.1',
      String(mock.port),
      mock.machineIdentifier,
      '/library/metadata/1',
      decision.query.session,
      BODY_SENTINEL,
      'error sending request for url',
    ]) {
      assert.ok(
        !alert.includes(secret),
        'the inline diagnostic must retain no request or response context',
      );
    }
    const actions = await driver.exec(
      `return [...document.querySelectorAll('.ctxmenu > button[role="menuitem"]')]
        .map((button) => button.textContent.trim())`,
    );
    assert.ok(actions.includes('Play'), 'ordinary Original playback must remain available');
    assert.deepEqual(mock.state.contractViolations, [], 'mock Plex decision contract');
    await screenshot('01-sanitized-decision-error');
  },
};
