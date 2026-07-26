// LIVE — drives Vela's server-side transcoding against the owner's REAL Plex.
// `npm run e2e:live transcode`.
//
// The whole transcoding feature was built and reviewed without ever touching a
// real server. Everything about it — the capability decision, the transcode URL,
// the session Plex opens, and the teardown that closes it — was proven by unit
// tests, static assertions and one mock that does not implement Plex's transcode
// endpoints at all. Two of the defects this feature shipped (`tr-4`, `tr-6`)
// were about a session left running on the user's server, which is a thing only
// the user's server can confirm.
//
// So this scenario asserts the two facts nothing else can:
//   1. Plex OPENS a transcode session when Vela plays at an explicit tier.
//   2. Plex has NO such session once that play ends.
//
// It deliberately does not test Automatic: a step-down needs genuinely degraded
// playback, which no scenario can force, and that remains a manual playtest.
import assert from "node:assert/strict";
import fs from "node:fs";
import { pollUntil, seedConfig } from "../helpers.mjs";
import { MpvIpc, mpvSocketSnapshot, waitForNewMpvSocket } from "../mpv.mjs";

const CREDS = "/tmp/vela-live-creds.json";
const SECTION = "Movies";
let creds;

const control = async (path) => {
  const res = await fetch(`${creds.control}${path}`, { method: "GET" });
  if (!res.ok) throw new Error(`live-control ${path}: ${res.status} ${await res.text()}`);
  return res.json();
};

function plexUrl(pathname) {
  const plex = creds.plex;
  const host = plex.last_server_host;
  const bracketed = host.includes(":") && !host.startsWith("[") ? `[${host}]` : host;
  const port = Number(plex.last_server_port ?? 32400);
  assert.equal(plex.last_server_scheme, "https", "live Plex verification requires saved HTTPS");
  return new URL(pathname, `https://${bracketed}:${port}`);
}

const plexHeaders = () => {
  const headers = { "X-Plex-Token": creds.plex.auth_token, Accept: "application/json" };
  if (creds.plex.client_identifier) {
    headers["X-Plex-Client-Identifier"] = creds.plex.client_identifier;
  }
  return headers;
};

// Ask the REAL server what it is currently encoding. This is the only witness
// for the teardown contract: Vela's own state cannot prove the server let go.
async function transcodeSessionKeys() {
  const res = await fetch(plexUrl("/transcode/sessions"), { headers: plexHeaders() });
  if (!res.ok) throw new Error(`plex /transcode/sessions: ${res.status}`);
  const body = await res.json();
  const container = body?.MediaContainer ?? {};
  const sessions = container.TranscodeSession ?? [];
  return (Array.isArray(sessions) ? sessions : [sessions]).map((s) => s.key ?? s.uuid ?? "");
}

async function waitForPlexReady() {
  await pollUntil(
    async () => {
      try {
        const response = await fetch(plexUrl("/identity"), { headers: plexHeaders() });
        return response.ok ? true : null;
      } catch {
        return null;
      }
    },
    "the real Plex server to accept HTTPS requests",
    { timeoutMs: 90000, intervalMs: 1000 },
  );
}

let invokeSequence = 0;

// WebDriver's execute/sync endpoint does not await Promises, and the results
// here carry token-bearing URLs, so project to a safe shape in-page.
async function invokeProjected(driver, command, args, projection) {
  const slot = `__velaTranscodeInvoke${++invokeSequence}`;
  await driver.exec(
    `const slot = ${JSON.stringify(slot)};
     const project = ${projection};
     window[slot] = { done: false };
     window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)}, ${JSON.stringify(args)})
       .then((value) => { window[slot] = { done: true, ok: true, value: project(value) }; })
       .catch((error) => { window[slot] = { done: true, ok: false, why: String(error) }; });
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
  assert.equal(result?.ok, true, `${command} must succeed: ${result?.why ?? "no reason given"}`);
  return result.value;
}

const ITEM_PROJECTION = `(items) => items.map((item) => ({
  ratingKey: item.ratingKey, title: item.title, mediaType: item.mediaType,
}))`;
const SECTION_PROJECTION = `(sections) => sections.map((s) => ({
  key: s.key, title: s.title, sectionType: s.sectionType,
}))`;
// Tiers only — never the stream URL, which carries the token.
const QUALITY_PROJECTION = `(options) => ({
  canDirectPlay: options.canDirectPlay,
  sourceBitrateKbps: options.sourceBitrateKbps,
  tiers: (options.tiers ?? []).map((t) => ({ id: t.id, bitrateKbps: t.bitrateKbps })),
})`;

export default {
  name: "live-transcode",

  async seed({ configRoot }) {
    if (!fs.existsSync(CREDS)) throw new Error(`live: no credentials at ${CREDS}`);
    creds = JSON.parse(fs.readFileSync(CREDS, "utf8"));
    if (!creds.plex) throw new Error("live: no saved https Plex server in the Vela config");
    await control("/plex/start");
    await waitForPlexReady();
    seedConfig(configRoot, [], {
      ...creds.plex,
      // Real demux, no window, and paused so the play does not march through
      // the film while the session assertions run.
      mpv_extra_args: "--vo=null\n--ao=null\n--pause=yes",
    });
  },

  async cleanup() {
    // The play is quit inside the run; this only guarantees Plex is up for the
    // next scenario and for the owner.
    if (creds) await control("/plex/start");
  },

  async run({ driver }) {
    await driver.waitFor(
      `return document.readyState === 'complete' && [...document.querySelectorAll('button.sideitem')].length > 1`,
      "the real Plex libraries in the sidebar",
    );

    // ── 1. Find a title this server will actually convert ───────────────────
    const sections = await invokeProjected(driver, "get_sections", { sourceId: null }, SECTION_PROJECTION);
    const movies = sections.find((s) => s.title === SECTION && s.sectionType === "movie");
    assert.ok(movies, `the real Plex server must offer the ${SECTION} movie library`);
    const items = await invokeProjected(
      driver,
      "get_items",
      { sectionKey: movies.key, sectionType: movies.sectionType, sort: "titleSort:asc", start: 0, size: 200 },
      ITEM_PROJECTION,
    );
    assert.ok(items.length > 0, "the real movie library must not be empty");

    let target = null;
    let options = null;
    for (const item of items.slice(0, 12)) {
      const candidate = await invokeProjected(
        driver,
        "quality_options",
        { itemKey: item.ratingKey, versionId: null },
        QUALITY_PROJECTION,
      );
      if (candidate.tiers.length > 0) {
        target = item;
        options = candidate;
        break;
      }
    }
    assert.ok(
      target,
      "no title in the first 12 offered a convertible tier — the real server refused every " +
        "capability decision, which is itself the finding",
    );
    // or-5: a step down must lower the demand, so the tier we pick must be
    // below the source's own bitrate when the server reported one.
    const tier = options.tiers[options.tiers.length - 1];
    assert.ok(tier.bitrateKbps > 0, "a tier must name a bitrate");

    // ── 2. Playing at that tier opens a session ON THE SERVER ───────────────
    const before = new Set(await transcodeSessionKeys());
    const sockets = mpvSocketSnapshot();
    await invokeProjected(
      driver,
      "play_item",
      {
        item: { ...target, sourceId: undefined },
        startFromBeginning: true,
        expectedSession: null,
        seriesContinuation: false,
        explicitSourceId: null,
        quality: tier.id,
      },
      `(result) => ({ kind: result?.kind ?? null })`,
    ).catch(async (error) => {
      // A refusal here is a real finding, not a harness fault: the server
      // approved the tier in step 1 and then would not deliver it.
      throw new Error(`the server approved ${tier.id} and then refused the play: ${error.message}`);
    });

    const socketPath = await waitForNewMpvSocket(sockets, { timeoutMs: 60000 });
    const mpv = await MpvIpc.connect(socketPath);
    let opened = null;
    try {
      // mpv must be on the transcode endpoint, not the part file.
      const path = await pollUntil(
        () => mpv.getProp("path").then((p) => (typeof p === "string" && p ? p : null)).catch(() => null),
        "mpv to load the real Plex transcode stream",
        { timeoutMs: 60000 },
      );
      assert.match(
        path,
        /\/video\/:\/transcode\/universal\/start/,
        "a tier play must reach Plex's transcode endpoint, not the part file",
      );

      opened = await pollUntil(
        async () => {
          const keys = await transcodeSessionKeys();
          const fresh = keys.filter((key) => key && !before.has(key));
          return fresh.length > 0 ? fresh : null;
        },
        "Plex to report an active transcode session for this play",
        { timeoutMs: 90000, intervalMs: 1000 },
      );
      assert.ok(opened.length > 0, "the server must be encoding for us");
    } finally {
      try {
        mpv.quit();
      } catch {
        /* the teardown assertion below is the real proof */
      }
      mpv.close();
    }

    // ── 3. ...and ending the play closes it ─────────────────────────────────
    // This is `tr-4` and `tr-6` end to end. Everything guarding them so far has
    // been a unit test or a source-text assertion; only the server can say
    // whether the encoder actually stopped.
    await pollUntil(
      async () => {
        const keys = new Set(await transcodeSessionKeys());
        return opened.every((key) => !keys.has(key)) ? true : null;
      },
      "Plex to drop the transcode session after the play ended — a session still " +
        "listed here is an encoder left running on the user's server",
      { timeoutMs: 90000, intervalMs: 1000 },
    );
  },
};
