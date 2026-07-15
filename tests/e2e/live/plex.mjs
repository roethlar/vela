// LIVE — drives Vela against the owner's REAL Plex server. `npm run e2e:live`.
//
// The Plex path has been the most dangerous code in this repo and the least testable. Eight
// review rounds went into stopping a scan reaching the WRONG server, and NONE of it could
// ever be exercised: the mock is Jellyfin, which has GUID ids and never rebinds, while a
// Plex section key is a server-LOCAL number, so "2" on one server is a different library on
// another. Everything guarding that — `sameSection`, the section binding, provenance — was
// verified by inspection and the owner's playtest, and by nothing else.
//
// This does not close all of that (a rebind needs a SECOND Plex server, which does not
// exist here). What it does close is the part that was pure faith: that the real Plex path
// works end to end against a real server — real XML browse/detail/episode data, playback,
// watch edits, section keys, provenance, and scan — and that it fails the way it is
// supposed to when the server goes away.
//
// Plex CANNOT be proxied (HTTPS behind a plex.direct certificate), so "the server goes
// away" means the real service is stopped, through a control endpoint on the Mac. It is
// restored on every handled exit path. The one watch-state fixture is proven clean before
// use and restored directly by both the scenario and the runner's signal cleanup. See
// scripts/live-control.mjs for the independent service-restoration backstop.
import assert from "node:assert/strict";
import fs from "node:fs";
import { createConnection } from "node:net";
import { pollUntil, seedConfig } from "../helpers.mjs";
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from "../mpv.mjs";

const CREDS = "/tmp/vela-live-creds.json";
const SECTION = "Movies";
const TARGET = "12 Years a Slave";
let creds;
let restoreRatingKey = null;
let restorePromise = null;
let invokeSequence = 0;

const SECTION_PROJECTION = `(sections) => sections.map((section) => ({
  key: section.key,
  title: section.title,
  sectionType: section.sectionType,
}))`;
const ITEM_PROJECTION = `(items) => items.map((item) => ({
  ratingKey: item.ratingKey,
  title: item.title,
  mediaType: item.mediaType,
  played: item.played,
  viewOffsetMs: item.viewOffsetMs,
  index: item.index,
  parentIndex: item.parentIndex,
  parentRatingKey: item.parentRatingKey,
  grandparentRatingKey: item.grandparentRatingKey,
}))`;
const DETAIL_PROJECTION = `(detail) => ({
  ratingKey: detail.ratingKey,
  title: detail.title,
  mediaType: detail.mediaType,
  index: detail.index,
  parentIndex: detail.parentIndex,
  parentRatingKey: detail.parentRatingKey,
  grandparentRatingKey: detail.grandparentRatingKey,
  genres: detail.genres?.length ?? 0,
  directors: detail.directors?.length ?? 0,
  writers: detail.writers?.length ?? 0,
  countries: detail.countries?.length ?? 0,
  cast: detail.cast?.length ?? 0,
  media: detail.media?.length ?? 0,
  streams: (detail.media ?? []).reduce((n, version) => n + (version.streams?.length ?? 0), 0),
})`;

// WebDriver's execute/sync endpoint does not await Promises. Start the Tauri
// invoke in-page, project away token-bearing artwork/stream URLs, then poll a
// small safe result. Failures deliberately expose no backend error text.
async function invokeProjected(driver, command, args, projection) {
  const slot = `__velaLiveInvoke${++invokeSequence}`;
  await driver.exec(
    `const slot = ${JSON.stringify(slot)};
     const project = ${projection};
     window[slot] = { done: false };
     window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)}, ${JSON.stringify(args)})
       .then((value) => { window[slot] = { done: true, ok: true, value: project(value) }; })
       .catch(() => { window[slot] = { done: true, ok: false }; });
     return true;`,
  );
  await driver.waitFor(
    `return window[${JSON.stringify(slot)}]?.done === true`,
    `${command} against the real Plex server`,
    { timeoutMs: 60000 },
  );
  const result = await driver.exec(
    `const slot = ${JSON.stringify(slot)};
     const result = window[slot];
     delete window[slot];
     return result;`,
  );
  assert.equal(result?.ok, true, `${command} must succeed against the real Plex server`);
  return result.value;
}

const getSections = (driver) =>
  invokeProjected(driver, "get_sections", { sourceId: null }, SECTION_PROJECTION);
const getItems = (driver, section) =>
  invokeProjected(
    driver,
    "get_items",
    {
      sectionKey: section.key,
      sectionType: section.sectionType,
      sort: "titleSort:asc",
      start: 0,
      size: 200,
    },
    ITEM_PROJECTION,
  );
const getChildren = (driver, ratingKey) =>
  invokeProjected(
    driver,
    "get_children",
    { ratingKey, start: 0, size: 200 },
    ITEM_PROJECTION,
  );
const getDetail = (driver, ratingKey) =>
  invokeProjected(driver, "get_item_detail", { ratingKey }, DETAIL_PROJECTION);

const control = async (path) => {
  const res = await fetch(`${creds.control}${path}`, { method: "GET" });
  if (!res.ok) throw new Error(`live-control ${path}: ${res.status} ${await res.text()}`);
  return res.json();
};

const banner = (driver) =>
  driver.exec(`return document.querySelector('div.error')?.textContent ?? null`);
const editLine = (driver) =>
  driver.exec(
    `return [...document.querySelectorAll('div.scanerror')].map((e) => e.textContent).join(' | ') || null`,
  );
const notice = (driver) =>
  driver.exec(`return document.querySelector('div.notice')?.textContent ?? null`);
const targetState = (driver) =>
  driver.exec(
    `const cards = [...document.querySelectorAll('main.grid button.poster')];
     const matches = cards.filter((card) => card.title === ${JSON.stringify(TARGET)});
     return {
       gridCount: cards.length,
       matches: matches.length,
       label: matches[0]?.getAttribute('aria-label') ?? null,
       watched: !!matches[0]?.querySelector('.watchedbadge'),
     }`,
  );

async function targetMenuItem(driver, label) {
  await driver.exec(
    `const matches = [...document.querySelectorAll('main.grid button.poster')]
       .filter((card) => card.title === ${JSON.stringify(TARGET)});
     if (matches.length !== 1) throw new Error('expected one exact target card');
     const el = matches[0];
     const r = el.getBoundingClientRect();
     el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
  );
  const item = await driver
    .waitFor(`return !!document.querySelector('.ctxmenu')`, "context menu")
    .then(() =>
      driver.find("xpath", `//button[@role='menuitem' and normalize-space(.)='${label}']`),
    );
  return item;
}

async function closeContextMenu(driver) {
  const backdrop = await driver.find("css selector", ".menubackdrop");
  await driver.click(backdrop);
}

async function openSection(driver, title) {
  const count = await driver.exec(
    `const matches = [...document.querySelectorAll('button.sideitem')]
       .filter((button) => button.textContent.trim() === ${JSON.stringify(title)});
     if (matches.length === 1) matches[0].click();
     return matches.length;`,
  );
  assert.equal(count, 1, `the real Plex server must offer exactly one ${title} library`);
}

async function clickPoster(driver, title) {
  await driver.waitFor(
    `return [...document.querySelectorAll('main.grid button.poster')]
       .filter((card) => card.title === ${JSON.stringify(title)}).length === 1`,
    `one exact ${title} card`,
    { timeoutMs: 60000 },
  );
  await driver.exec(
    `const card = [...document.querySelectorAll('main.grid button.poster')]
       .find((item) => item.title === ${JSON.stringify(title)});
     card.click();
     return true;`,
  );
}

function rawPlexKey(namespacedKey) {
  const colon = namespacedKey.indexOf(":");
  const raw = colon >= 0 ? namespacedKey.slice(colon + 1) : "";
  assert.match(raw, /^\d+$/, "the live cleanup key must be a numeric Plex rating key");
  return raw;
}

async function discoverFixtures(driver) {
  const sections = await getSections(driver);
  const movieSection = sections.find(
    (section) => section.title === SECTION && section.sectionType === "movie",
  );
  assert.ok(movieSection, `the real Plex server must offer the ${SECTION} movie library`);

  const movies = await getItems(driver, movieSection);
  const movie = movies.find((item) => item.title === TARGET);
  assert.ok(movie, `${TARGET} must be present in the real Plex movie listing`);
  assert.equal(movie.mediaType, "movie", `${TARGET} must remain a movie`);
  assert.equal(movie.played, false, `${TARGET} must begin unwatched`);
  assert.equal(movie.viewOffsetMs ?? 0, 0, `${TARGET} must begin with zero progress`);
  restoreRatingKey = rawPlexKey(movie.ratingKey);

  const movieDetail = await getDetail(driver, movie.ratingKey);
  assert.equal(movieDetail.ratingKey, movie.ratingKey);
  assert.equal(movieDetail.title, TARGET);
  assert.equal(movieDetail.mediaType, "movie");
  assert.ok(movieDetail.genres > 0, "the real movie detail must carry genres");
  assert.ok(movieDetail.directors > 0, "the real movie detail must carry directors");
  assert.ok(movieDetail.writers > 0, "the real movie detail must carry writers");
  assert.ok(movieDetail.countries > 0, "the real movie detail must carry countries");
  assert.ok(movieDetail.cast > 0, "the real movie detail must carry cast");
  assert.ok(movieDetail.media > 0, "the real movie detail must carry a media version");
  assert.ok(movieDetail.streams > 0, "the real movie detail must carry media streams");

  const showSection = sections.find((section) => section.sectionType === "show");
  assert.ok(showSection, "the real Plex server must offer a show library");
  const shows = await getItems(driver, showSection);
  const uniqueShows = shows.filter(
    (show, index, all) =>
      show.mediaType === "show" &&
      all.findIndex((candidate) => candidate.title === show.title) === index &&
      all.filter((candidate) => candidate.title === show.title).length === 1,
  );

  let episodePath = null;
  for (const show of uniqueShows.slice(0, 12)) {
    const seasons = (await getChildren(driver, show.ratingKey)).filter(
      (item) => item.mediaType === "season",
    );
    for (const season of seasons.slice(0, 3)) {
      const episodes = (await getChildren(driver, season.ratingKey)).filter(
        (item) => item.mediaType === "episode",
      );
      for (const episode of episodes.slice(0, 3)) {
        if (episodes.filter((candidate) => candidate.title === episode.title).length !== 1) {
          continue;
        }
        const detail = await getDetail(driver, episode.ratingKey);
        if (
          detail.parentRatingKey === season.ratingKey &&
          detail.grandparentRatingKey === show.ratingKey &&
          detail.index != null &&
          detail.parentIndex != null &&
          detail.media > 0 &&
          detail.streams > 0
        ) {
          episodePath = { showSection, show, season, episode, detail };
          break;
        }
      }
      if (episodePath) break;
    }
    if (episodePath) break;
  }
  assert.ok(
    episodePath,
    "a real show must expose a season and episode with parent keys and media streams",
  );

  return { movieSection, movie, movieDetail, ...episodePath };
}

async function exerciseEpisodeUi(driver, fixtures) {
  await openSection(driver, fixtures.showSection.title);
  await clickPoster(driver, fixtures.show.title);
  await clickPoster(driver, fixtures.season.title);
  await driver.waitFor(
    `return [...document.querySelectorAll('.season .eplist button.eprow')]
       .some((row) => {
         const title = row.querySelector('.eptitle');
         const itemTitle = [...(title?.childNodes ?? [])]
           .filter((node) => node.nodeType === Node.TEXT_NODE)
           .map((node) => node.textContent)
           .join('').trim();
         return itemTitle === ${JSON.stringify(fixtures.episode.title)};
       })`,
    `the real episode ${fixtures.episode.title}`,
    { timeoutMs: 60000 },
  );
  const matches = await driver.exec(
    `const matches = [...document.querySelectorAll('.season .eplist button.eprow')]
       .filter((row) => {
         const title = row.querySelector('.eptitle');
         const itemTitle = [...(title?.childNodes ?? [])]
           .filter((node) => node.nodeType === Node.TEXT_NODE)
           .map((node) => node.textContent)
           .join('').trim();
         return itemTitle === ${JSON.stringify(fixtures.episode.title)};
       });
     if (matches.length === 1) matches[0].click();
     return matches.length;`,
  );
  assert.equal(matches, 1, `the season must contain one exact ${fixtures.episode.title} row`);
  await driver.waitFor(
    `return document.querySelector('.season .panel h1')?.textContent.trim() === ${JSON.stringify(fixtures.episode.title)} &&
       !!document.querySelector('.season .mediaspecs .vlabel')?.textContent.trim()`,
    "the selected real episode's enriched detail and media streams",
    { timeoutMs: 60000 },
  );
}

async function exerciseMovieDetailUi(driver) {
  await openSection(driver, SECTION);
  await clickPoster(driver, TARGET);
  await driver.waitFor(
    `return document.querySelector('.detail h1')?.textContent.trim() === ${JSON.stringify(TARGET)} &&
       document.querySelectorAll('.detail .genres .chip').length > 0 &&
       !!document.querySelector('.detail .version .vlabel')?.textContent.trim()`,
    "the real movie's enriched detail, genres, and media streams",
    { timeoutMs: 60000 },
  );
}

async function playMovieAndQuit(driver) {
  const before = mpvSocketSnapshot();
  const play = await driver.find("css selector", ".detail button.playwide");
  await driver.click(play);
  const socketPath = await waitForNewMpvSocket(before, { timeoutMs: 40000 });
  const mpv = await MpvIpc.connect(socketPath);
  try {
    await pollUntil(
      () =>
        mpv
          .getProp("path")
          .then((path) =>
            typeof path === "string" &&
            path.startsWith("https://") &&
            !/X-Plex-Token/i.test(path)
              ? true
              : null,
          )
          .catch(() => null),
      "mpv to load the real Plex stream",
      { timeoutMs: 40000 },
    );
    const paused = await mpv.getProp("pause");
    assert.equal(paused, true, "the live Plex probe must remain paused");
    const duration = await pollUntil(
      () => mpv.getProp("duration").then((value) => (value > 0 ? value : null)).catch(() => null),
      "the real Plex stream duration",
      { timeoutMs: 40000 },
    );
    assert.ok(duration > 0, "the real Plex stream must demux to a positive duration");
    const mediaTitle = await mpv.getProp("media-title");
    assert.equal(mediaTitle, TARGET, "mpv must receive the selected movie title");
    const position = await pollUntil(
      () =>
        mpv
          .getProp("time-pos")
          .then((time) => (typeof time === "number" ? { time } : null))
          .catch(() => null),
      "paused real Plex playback position",
      { timeoutMs: 40000 },
    );
    assert.ok(position.time <= 0.1, "the paused live probe must not advance Plex progress");
  } finally {
    try {
      mpv.quit();
    } catch {
      /* socket teardown below is the proof */
    }
    mpv.close();
  }
  await pollUntil(
    () =>
      new Promise((resolve) => {
        const probe = createConnection(socketPath);
        probe.once("connect", () => {
          probe.destroy();
          resolve(false);
        });
        probe.once("error", () => resolve(true));
      }),
    "the real Plex mpv process to exit",
    { timeoutMs: 10000 },
  );
}

async function roundTripWatchState(driver, movieSection) {
  await openSection(driver, SECTION);
  await pollUntil(
    async () => ((await targetState(driver)).matches === 1 ? true : null),
    `${TARGET} after real playback`,
    { timeoutMs: 60000 },
  );

  const markWatched = await targetMenuItem(driver, "Mark watched");
  await driver.click(markWatched);
  await pollUntil(
    async () => {
      const state = await targetState(driver);
      return state.matches === 1 && state.watched && !state.label.includes("% watched")
        ? true
        : null;
    },
    `${TARGET} to refetch as watched`,
    { timeoutMs: 60000 },
  );
  await pollUntil(
    async () => {
      const item = (await getItems(driver, movieSection)).find((candidate) => candidate.title === TARGET);
      return item?.played === true && (item.viewOffsetMs ?? 0) === 0 ? true : null;
    },
    `${TARGET} to read back as watched from Plex`,
    { timeoutMs: 60000, intervalMs: 1000 },
  );

  const markUnwatched = await targetMenuItem(driver, "Mark unwatched");
  await driver.click(markUnwatched);
  await pollUntil(
    async () => {
      const state = await targetState(driver);
      return state.matches === 1 && !state.watched && !state.label.includes("% watched")
        ? true
        : null;
    },
    `${TARGET} to refetch as cleanly unwatched`,
    { timeoutMs: 60000 },
  );

  const restored = (await getItems(driver, movieSection)).find((item) => item.title === TARGET);
  assert.equal(restored?.played, false, `${TARGET} must be unwatched on Plex after the round trip`);
  assert.equal(restored?.viewOffsetMs ?? 0, 0, `${TARGET} must have zero Plex progress after the round trip`);
}

function restoreUrl(ratingKey) {
  const plex = creds.plex;
  const host = plex.last_server_host;
  const bracketed = host.includes(":") && !host.startsWith("[") ? `[${host}]` : host;
  const port = Number(plex.last_server_port ?? 32400);
  assert.equal(plex.last_server_scheme, "https", "live Plex cleanup requires saved HTTPS");
  assert.ok(Number.isInteger(port) && port > 0 && port <= 65535, "saved Plex port is valid");
  const url = new URL(`https://${bracketed}:${port}/:/unscrobble`);
  url.searchParams.set("identifier", "com.plexapp.plugins.library");
  url.searchParams.set("key", ratingKey);
  return url;
}

async function restoreTargetWatchState(driver = null, movieSection = null) {
  if (restorePromise) return restorePromise;
  restorePromise = (async () => {
    await control("/plex/start");
    if (!restoreRatingKey) return;
    const ratingKey = restoreRatingKey;
    const headers = { "X-Plex-Token": creds.plex.auth_token };
    if (creds.plex.client_identifier) {
      headers["X-Plex-Client-Identifier"] = creds.plex.client_identifier;
    }
    await pollUntil(
      async () => {
        try {
          const response = await fetch(restoreUrl(ratingKey), { method: "GET", headers });
          return response.ok ? true : null;
        } catch {
          return null;
        }
      },
      `${TARGET}'s Plex watch state cleanup`,
      { timeoutMs: 90000, intervalMs: 1000 },
    );
    if (driver && movieSection) {
      await pollUntil(
        async () => {
          const item = (await getItems(driver, movieSection)).find(
            (candidate) => candidate.title === TARGET,
          );
          return item?.played === false && (item.viewOffsetMs ?? 0) === 0 ? true : null;
        },
        `${TARGET}'s restored Plex state to read back through Vela`,
        { timeoutMs: 60000, intervalMs: 1000 },
      );
    }
    restoreRatingKey = null;
  })();
  try {
    return await restorePromise;
  } finally {
    restorePromise = null;
  }
}

export default {
  name: "live-plex",

  async seed({ configRoot }) {
    if (!fs.existsSync(CREDS)) throw new Error(`live: no credentials at ${CREDS}`);
    creds = JSON.parse(fs.readFileSync(CREDS, "utf8"));
    if (!creds.plex) throw new Error("live: no saved https Plex server in the Vela config");
    // Plex is restored from TOP-LEVEL config, not from `sources` (lib.rs).
    seedConfig(configRoot, [], {
      ...creds.plex,
      // Resolve the real Media/Part and start mpv, but keep time-pos at zero so
      // the playback tracker has no resume position to write to Plex.
      mpv_extra_args: "--vo=null\n--ao=null\n--pause=yes",
    });
  },

  async cleanup() {
    // Idempotent retry backstop. The run's inner finally reports restoration
    // failures; the runner retries this on ordinary teardown and handled signals.
    await restoreTargetWatchState();
  },

  async run({ driver }) {
    restoreRatingKey = null;
    let primaryError = null;
    let restorationError = null;
    let movieSection = null;
    try {
    // ── 1. A real Plex server, real libraries, real section keys ────────────
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].length > 1`,
      "the real Plex libraries in the sidebar",
    );
    const section = await driver.exec(
      `const b = [...document.querySelectorAll('button.sideitem')]
         .find((x) => x.textContent.trim() === ${JSON.stringify(SECTION)});
       if (b) b.click();
       return b ? b.textContent.trim() : null`,
    );
    assert.equal(section, SECTION, `the real Plex server must offer the ${SECTION} library`);
    await pollUntil(
      async () => {
        const target = await targetState(driver);
        return target.matches === 1 ? target : null;
      },
      `${TARGET} from the real Plex ${SECTION} library`,
    );
    const initialTarget = await targetState(driver);
    assert.equal(initialTarget.matches, 1, `exactly one ${TARGET} card is visible`);
    assert.ok(initialTarget.label, `${TARGET} has a stable accessible identity`);
    assert.equal(await banner(driver), null, "a healthy Plex server produces no banner");
    await targetMenuItem(driver, "Mark watched");
    await closeContextMenu(driver);

    // ── 2. Real XML browse/detail/episode/play/watch paths ──────────────────
    // The backend calls return only a deliberately projected, token-free shape.
    // They prove the migrated serde path itself; the rich-only UI selectors prove
    // the detail surfaces did not silently fall back to sparse listing data.
    const fixtures = await discoverFixtures(driver);
    movieSection = fixtures.movieSection;
    assert.equal(fixtures.movie.ratingKey, fixtures.movieDetail.ratingKey);
    await exerciseEpisodeUi(driver, fixtures);
    await exerciseMovieDetailUi(driver);
    await playMovieAndQuit(driver);
    await roundTripWatchState(driver, fixtures.movieSection);
    assert.equal(await banner(driver), null, "healthy Plex verification leaves no banner");

    // Navigation, playback, and the two successful edits all refetched the grid;
    // take the offline leg's identity/cardinality baseline only after they settle.
    const offlineBaseline = await targetState(driver);
    assert.equal(offlineBaseline.matches, 1, `${TARGET} is present before the offline leg`);
    assert.equal(offlineBaseline.watched, false, `${TARGET} is cleanly unwatched before failure`);
    assert.ok(!offlineBaseline.label.includes("% watched"), `${TARGET} has no progress before failure`);

    // ── 3. Scan Library reaches the REAL server ─────────────────────────────
    // The whole provenance/binding apparatus exists to make sure a scan reaches the server
    // the section key came FROM. Until now nothing exercised it against a real Plex at all
    // — the key had never been a real Plex section number in a test. A refusal here
    // ("Scan Library" absent, or a failure notice) means the provenance plumbing has
    // rejected a key its own server just issued.
    await driver.exec(
      `const b = [...document.querySelectorAll('button.sideitem')].find((x) => x.textContent.trim() === ${JSON.stringify(section)});
       const r = b.getBoundingClientRect();
       b.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));`,
    );
    const scan = await driver
      .waitFor(`return !!document.querySelector('.ctxmenu')`, "the library's context menu")
      .then(() =>
        driver.find("xpath", `//button[@role='menuitem' and normalize-space(.)='Scan Library']`),
      );
    await driver.click(scan);
    await pollUntil(
      async () => {
        const n = await notice(driver);
        return n && n.startsWith("Scan started") ? true : null;
      },
      "the real Plex server accepted a scan of a key it issued itself — the provenance check must not refuse its own server",
    );
    assert.equal(
      await banner(driver),
      null,
      "a scan that succeeded must not leave a failure on the view's banner",
    );

    // ── 4. Plex goes away, and a watch-state edit fails ─────────────────────
    // The owner's playtest, against the server it actually happened on.
    await control("/plex/stop");
    const markWatched = await targetMenuItem(driver, "Mark watched");
    await driver.click(markWatched);
    const said = await pollUntil(
      async () => {
        const target = await targetState(driver);
        assert.equal(target.matches, 1, `${TARGET} must never disappear while the edit fails`);
        assert.equal(
          target.label,
          offlineBaseline.label,
          `${TARGET}'s identity must remain unchanged while the edit fails`,
        );
        return (await editLine(driver)) || null;
      },
      "the edit's failure, on the edit's own line",
      { timeoutMs: 40000, intervalMs: 200 }, // dead Plex is a CONNECT timeout
    );

    assert.ok(
      said.includes("Couldn't mark"),
      `the line must NAME the action — against Plex this is the exact defect the owner hit: the edit's line and the grid's banner were the SAME sentence, twice — got ${JSON.stringify(said)}`,
    );
    assert.ok(
      !/https?:\/\//.test(said) && !/token/i.test(said),
      `no url and nothing token-shaped may reach the screen — a Plex url carries ?X-Plex-Token= — got ${JSON.stringify(said)}`,
    );
    assert.equal(
      await banner(driver),
      null,
      "the failed edit makes no listing request and therefore no view failure",
    );

    // ── 5. ...and the library is STILL THERE ───────────────────────────────
    const afterFailure = await targetState(driver);
    assert.equal(afterFailure.matches, 1, `${TARGET} remains after the named failure`);
    assert.equal(afterFailure.label, offlineBaseline.label, `${TARGET}'s identity remains exact`);
    assert.equal(
      afterFailure.gridCount,
      offlineBaseline.gridCount,
      "the failed edit must not change the loaded grid's cardinality",
    );

    // ── 6. Plex comes back, and Refresh recovers ───────────────────────────
    await control("/plex/start");
    const beforeRefresh = await targetState(driver);
    assert.equal(beforeRefresh.matches, 1, `${TARGET} remains cached while Plex restarts`);
    assert.equal(beforeRefresh.label, offlineBaseline.label, `${TARGET} remains exact before Refresh`);
    let refreshAttempts = 0;
    await pollUntil(
      async () => {
        const refreshReady = await driver.exec(
          `const b = document.querySelector('button.refreshbtn'); return !!b && !b.disabled`,
        );
        if (refreshReady) {
          const refresh = await driver.find("css selector", "button.refreshbtn");
          await driver.click(refresh);
          refreshAttempts++;
          await new Promise((r) => setTimeout(r, 2000));
        }
        const target = await targetState(driver);
        const settled = await driver.exec(
          `const b = document.querySelector('button.refreshbtn'); return !!b && !b.disabled`,
        );
        return refreshAttempts > 0 &&
          settled &&
          target.matches === 1 &&
          target.label === offlineBaseline.label &&
          (await banner(driver)) === null
          ? true
          : null;
      },
      `${TARGET} remains unwatched after Plex returns and Refresh succeeds`,
      { timeoutMs: 90000 }, // Plex takes a while to start serving after systemd says active
    );
    // Refresh does not own the edit line; its independent 8s presentation timer does.
    // Plex startup can outlast that timer, so accept either an already-dismissed line or
    // the remaining portion of its promised lifetime.
    await pollUntil(
      async () => ((await editLine(driver)) === null ? true : null),
      "the named edit failure to auto-dismiss independently of server recovery and Refresh",
      { timeoutMs: 10000, intervalMs: 200 },
    );
    await targetMenuItem(driver, "Mark watched");
    await closeContextMenu(driver);
    } catch (error) {
      primaryError = error;
    } finally {
      // Playback and watch-state checks intentionally touch one item, selected
      // only after proving it started clean. Restore it directly even when a UI
      // assertion fails; unlike scenario.cleanup, this failure is not swallowed.
      try {
        await restoreTargetWatchState(primaryError ? null : driver, movieSection);
      } catch (error) {
        restorationError = error;
      }
    }
    if (primaryError && restorationError) {
      throw new AggregateError(
        [primaryError, restorationError],
        "live Plex verification and watch-state restoration both failed",
      );
    }
    if (restorationError) throw restorationError;
    if (primaryError) throw primaryError;
  },
};
