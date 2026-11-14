// Two independently configured Plex sources through Vela's real Plex protocol
// path: distinct credentials/machines stay separated, one shared title collapses
// to one card with two backings, each explicit backing persists its override, and
// removing one Settings row leaves the other source live.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {
  DISPLAYLESS_MPV_ARGS,
  makeClips,
  playAndQuit,
  pollUntil,
  seedConfig,
  waitForPageUncovered,
} from '../helpers.mjs';
import { createMockPlexTls, mockPlexSource, startMockPlex } from '../mockplex.mjs';
import { shown } from '../proof.mjs';
import { plexStopProblems } from '../judge.mjs';

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
  rows: ['2', '6', '7'],

  async seed({ configRoot }) {
    tls = createMockPlexTls(configRoot);
    const mediaDir = makeClips(configRoot, ['plex-a.mp4']);
    mockA = await startMockPlex({
      tls,
      name: 'Mock Plex A',
      machineIdentifier: 'machine-a',
      token: 'plex-token-a',
      mediaFile: path.join(mediaDir, 'plex-a.mp4'),
    });
    mockB = await startMockPlex({
      tls,
      name: 'Mock Plex B',
      machineIdentifier: 'machine-b',
      token: 'plex-token-b',
    });
    seedConfig(
      configRoot,
      [mockPlexSource(mockA, { id: 'plex-a' }), mockPlexSource(mockB, { id: 'plex-b' })],
      {
        // The mocks serve HTTPS under a private CA. Vela's own client trusts
        // it through SSL_CERT_FILE (`environment` below); mpv's TLS (FFmpeg
        // with GnuTLS) never reads that variable. So mpv is told about the
        // same CA itself, and told to check the certificate, and BOTH players
        // fetch the stream with verification on. Without these lines the
        // embedded player's mpv (0.42, which verifies by default) refuses the
        // stream and the play ends at once, while the system's mpv 0.41 (which
        // does not verify by default) plays it without checking anything.
        // Whether Vela itself should set either is the owner's question
        // (.agents/plans/embedded-player.slice6-notes.md, the TLS findings).
        mpv_extra_args: `${DISPLAYLESS_MPV_ARGS}\n--tls-verify=yes\n--tls-ca-file=${tls.ca}`,
      },
    );
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
    await driver.waitFor(
      `const img = document.querySelector('button.poster[aria-label^="${TITLE}"] img');
       return !!img && img.complete && img.naturalWidth > 0;`,
      'credential-free Plex artwork',
    );
    const artworkSrc = await driver.exec(
      `return document.querySelector('button.poster[aria-label^="${TITLE}"] img')?.getAttribute('src')`,
    );
    assert.match(
      artworkSrc,
      /^vela-artwork:\/\/localhost\//,
      'the webview receives only Vela artwork protocol identity',
    );
    assert.ok(!/[?&]X-Plex-Token=/i.test(artworkSrc), 'the artwork URL must not contain a token');

    for (const mock of [mockA, mockB]) {
      assert.ok(
        mock.state.requests.some((request) => request.path === '/library/sections/1/all'),
        `${mock.name} must serve its own movie listing`,
      );
      assert.ok(
        mock.state.requests.every(
          (request) => request.tokenPresent && request.tokenMatches,
        ),
        `${mock.name} must receive only its own source-row token`,
      );
      assert.ok(
        mock.state.requests.every(
          (request) => !Object.keys(request.query).some((key) => /token/i.test(key)),
        ),
        `${mock.name} must never receive a token in a request query`,
      );
    }
    assert.ok(
      [...mockA.state.requests, ...mockB.state.requests].some(
        (request) => request.path === '/photo/:/transcode' && request.tokenMatches,
      ),
      'Plex artwork must reach the mock with header authentication',
    );
    await screenshot('01-collapsed');

    // What the player was handed carries no credential either (row 7 of the
    // embedded player's parity list): the stream's address and the title
    // mpv shows, read from mpv itself. The stream's own request is checked on
    // the wire by the mock (contractViolations, below).
    let playedPath = null;
    let lastSeenMs = null;
    await playAndQuit(driver, () => chooseBacking(driver, 'Mock Plex A'), {
      during: async (mpv) => {
        playedPath = await mpv.getProp('path');
        const title = await mpv.getProp('media-title');
        for (const [what, value] of [['path', playedPath], ['media-title', title]]) {
          assert.ok(!/token/i.test(value), `mpv's ${what} must carry no token: ${value}`);
          assert.ok(!value.includes('plex-token-'), `mpv's ${what} must carry no token: ${value}`);
        }
        assert.equal(title, TITLE, 'mpv shows the title, nothing more');
        // Row 2's running half: Plex hears the play is playing (a timeline
        // report every five seconds or so) while it runs, before any stop.
        await pollUntil(
          () => mockA.state.requests.some(
            (request) => request.path === '/:/timeline' && request.query.state === 'playing',
          ),
          'a "playing" timeline report to reach Plex A while the play runs',
          { timeoutMs: 10000 },
        );
        // Nor the process the player runs in: mpv's own in the external
        // player, Vela's in the embedded one. Its command line and its
        // environment are readable by any process of the user. This check
        // can fail only in the external player (guard finding atk-7): Vela's
        // own command line and environment are fixed when it starts, before
        // any token exists, so in the embedded player it cannot catch a leak.
        // There row 7 rests on the path and title above and the wire below.
        const pid = await mpv.getProp('pid');
        for (const what of ['cmdline', 'environ']) {
          const text = fs.readFileSync(`/proc/${pid}/${what}`, 'utf8');
          assert.ok(!text.includes('plex-token-'), `the player's ${what} must carry no token`);
          assert.ok(!/x-plex-token/i.test(text), `the player's ${what} must carry no token header`);
        }
      },
      // Where the play is just before it is stopped: the stopped report's
      // position is checked against it below.
      beforeStop: async (mpv) => {
        lastSeenMs = Math.round((await mpv.getProp('time-pos')) * 1000);
      },
    });
    // On the wire: the player fetched the stream itself (GET; Vela's own
    // check of it before the play is a HEAD), with the token in the header
    // and not in the address. The mock answers anything else with a 401 and
    // a contract violation (asserted at the end).
    const parts = mockA.state.requests.filter((request) => request.path.startsWith('/library/parts/machine-a/'));
    assert.ok(
      mockA.state.served.some(
        (request) =>
          request.method === 'GET' &&
          request.path.startsWith('/library/parts/machine-a/') &&
          request.tokenMatches &&
          [200, 206].includes(request.status),
      ),
      `the player fetched the stream with the token header: ${JSON.stringify(parts)}`,
    );
    assert.ok(
      parts.every((request) => request.tokenMatches && !Object.keys(request.query).some((key) => /token|redacted/i.test(key))),
      `every stream request carried the token in its header only: ${JSON.stringify(parts)}`,
    );
    assert.match(
      playedPath,
      new RegExp(`^https://127\\.0\\.0\\.1:${mockA.port}/library/parts/machine-a/`),
      'the Plex A backing plays its own stream, over HTTPS',
    );
    await pollUntil(
      () => readConfig(configRoot).merged_overrides?.[CANONICAL] === 'plex-a',
      `${CANONICAL} to prefer plex-a`,
    );
    await pollUntil(
      () => mockA.state.requests.some((request) => request.path === '/library/metadata/1'),
      'the Plex A backing to resolve on Plex A',
    );
    await pollUntil(
      () => mockA.state.requests.some((request) => request.path === '/:/timeline'),
      'Plex timeline authentication to reach Plex A',
    );
    await pollUntil(
      () => mockA.state.requests.some((request) => request.path === '/:/progress'),
      'Plex final progress authentication to reach Plex A',
    );
    assert.ok(
      mockA.state.requests
        .filter((request) => request.path === '/:/timeline' || request.path === '/:/progress')
        .every((request) => request.tokenPresent && request.tokenMatches),
      'Plex playback reporting must use the source token header',
    );
    // Row 2's final half (review finding pa-7): the stop itself reaches Plex,
    // as a timeline report with state=stopped for this item, the last of the
    // play's timeline reports, at the play's final position. playAndQuit
    // seeks to 6 s and stops a moment later, inside the 10 s clip, so that
    // position is between 6 and 10 s, and no "playing" report may come later
    // or say more. And it is where the play stopped, not the item's end: no
    // earlier than mpv's position read just before the stop (lastSeenMs, less
    // a little for timing) and at most 2 s later, and before the 10 s clip
    // ends, so a report at the item's full duration (which tells Plex it was
    // watched to the end) fails. The rules are plexStopProblems's (judge.mjs),
    // held by unit tests; this waits for the report and asks it.
    await pollUntil(
      () => mockA.state.requests.find(
        (request) => request.path === '/:/timeline' && request.query.state === 'stopped',
      ),
      'the final "stopped" timeline report to reach Plex A',
    );
    const timelines = mockA.state.requests.filter((request) => request.path === '/:/timeline');
    const detail = JSON.stringify(timelines.map((request) => request.query));
    console.log(`Plex A timeline reports: ${detail}`);
    assert.deepEqual(
      plexStopProblems(timelines.map((request) => request.query), {
        lastSeenMs,
        durationMs: 10000, // the item's duration in the mock
        ratingKey: '1',
        key: '/library/metadata/1',
        seekMs: 6000, // playAndQuit's seek
      }),
      [],
      `Plex A's timeline reports tell it the play was playing, and where it stopped: ${detail}`,
    );

    // Plex B has no media behind its part: Vela's check of the stream before
    // it starts a player is answered 404, so this choice persists its
    // override and is then refused on the page, before any player starts, in
    // either player. Nothing covers the page afterwards.
    await chooseBacking(driver, 'Mock Plex B');
    await driver.waitFor(
      `return document.body.innerText.includes(${JSON.stringify(`Couldn't play “${TITLE}” — File not found on the server`)})`,
      'the Plex B play to be refused on the page',
    );
    await waitForPageUncovered(driver, 'after the Plex B play was refused');
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
    const requestLog = JSON.stringify([mockA.state.requests, mockB.state.requests]);
    assert.ok(!requestLog.includes('plex-token-a'), 'Plex A token must not enter mock request logs');
    assert.ok(!requestLog.includes('plex-token-b'), 'Plex B token must not enter mock request logs');
    assert.deepEqual(mockA.state.contractViolations, [], 'Plex A mock contract');
    assert.deepEqual(mockB.state.contractViolations, [], 'Plex B mock contract');
    await screenshot('02-independent-remove');
    // The end of its proof of its rows in this player (proof.mjs).
    shown('2', '6', '7');
  },
};
