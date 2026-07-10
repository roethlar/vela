// Minimal stateful mock Jellyfin server for hermetic server-flow scenarios.
// Serves exactly the endpoint surface Vela's Jellyfin client uses (see
// src-tauri/src/source/jellyfin.rs) with PascalCase JSON, records every
// request for assertions, and lets scenarios read/flip watch state directly
// (it runs in the runner process — no control endpoint needed).
//
// `movies` describes the single "Mock Library" movie section. A movie with a
// `mediaFile` gets a Range-capable /Videos/{id}/stream. Multiple instances
// (distinct ports) act as multiple servers for merged-view / dead-end
// scenarios.
import fs from 'node:fs';
import http from 'node:http';

export function startMockJellyfin({
  userId = 'u1',
  movies = [{ id: 'm1', name: 'Mock Movie', year: 2020 }],
  latest = [], // items for /Users/{userId}/Items/Latest → the "Recently Added" Home hub
} = {}) {
  const state = {
    // Per-movie UserData; Stopped check-ins update positionTicks like a real
    // server, and PlayedItems POST/DELETE flip played.
    userData: Object.fromEntries(movies.map((m) => [m.id, { played: false, positionTicks: 0 }])),
    requests: [], // { method, path, query } in arrival order
    checkins: [], // parsed /Sessions/Playing* bodies: { endpoint, body }
    contractViolations: [], // Items requests whose query broke the client contract
  };

  const byId = Object.fromEntries(movies.map((m) => [m.id, m]));
  const toJson = (m) => ({
    Id: m.id,
    Name: m.name,
    Type: 'Movie',
    ProductionYear: m.year ?? 2020,
    RunTimeTicks: m.runTimeTicks ?? 6_000_000_000, // default 10 min in 100ns ticks
    Overview: 'A film that exists only for the harness.',
    UserData: {
      Played: state.userData[m.id].played,
      PlaybackPositionTicks: state.userData[m.id].positionTicks,
    },
  });

  // Range support matching the app's retired loopback-proxy semantics:
  // bytes=a-b / bytes=a- / bytes=-suffix, end clamped to EOF, 416 for
  // unsatisfiable starts (eh-13 — mpv probes MP4 with suffix/EOF ranges,
  // and an edge must not crash the runner).
  const serveRange = (req, res, mediaFile) => {
    const size = fs.statSync(mediaFile).size;
    const m = /^bytes=(\d*)-(\d*)$/.exec(req.headers.range ?? '');
    let start = 0;
    let end = size - 1;
    if (m && (m[1] !== '' || m[2] !== '')) {
      if (m[1] === '') {
        start = Math.max(0, size - Number(m[2])); // suffix: last N bytes
      } else {
        start = Number(m[1]);
        if (m[2] !== '') end = Math.min(Number(m[2]), size - 1);
      }
      if (start >= size || start > end) {
        // start>end covers reversed ranges like bytes=50-40 — unsatisfiable.
        res.writeHead(416, { 'Content-Range': `bytes */${size}` });
        return res.end();
      }
      res.writeHead(206, {
        'Content-Type': 'video/mp4',
        'Accept-Ranges': 'bytes',
        'Content-Length': end - start + 1,
        'Content-Range': `bytes ${start}-${end}/${size}`,
      });
    } else {
      res.writeHead(200, {
        'Content-Type': 'video/mp4',
        'Accept-Ranges': 'bytes',
        'Content-Length': size,
      });
    }
    fs.createReadStream(mediaFile, { start, end }).pipe(res);
  };

  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://mock');
    const path = url.pathname;
    const query = Object.fromEntries(url.searchParams);
    state.requests.push({ method: req.method, path, query });

    const json = (body, status = 200) => {
      res.writeHead(status, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify(body));
    };

    if (path === `/Users/${userId}/Views`) {
      return json({ Items: [{ Id: 'lib1', Name: 'Mock Library', CollectionType: 'movies' }] });
    }
    if (path === `/Users/${userId}/Items/Resume`) {
      return json({ Items: [] });
    }
    if (path === `/Users/${userId}/Items/Latest`) {
      return json(latest); // bare array, per the Jellyfin API (seed the Recently Added hub)
    }
    if (path === `/Users/${userId}/Items`) {
      // Search goes to the same endpoint with searchTerm and NO ParentId
      // (jellyfin.rs search()); a real server filters by name.
      if (query.searchTerm !== undefined) {
        const term = query.searchTerm.toLowerCase();
        return json({ Items: movies.filter((m) => m.name.toLowerCase().includes(term)).map(toJson) });
      }
      // Fail closed on the client's listing query contract (eh-12): a real
      // Jellyfin would return the wrong contents (or error) for a bad
      // ParentId or a missing IncludeItemTypes — an ignore-all mock would
      // hide exactly that class of regression.
      if (query.ParentId !== 'lib1' || !(query.IncludeItemTypes ?? '').includes('Movie')) {
        state.contractViolations.push({ path, query });
        return json({ error: 'query contract violation' }, 400);
      }
      return json({ Items: movies.map(toJson) });
    }
    const single = /^\/Users\/[^/]+\/Items\/([^/]+)$/.exec(path);
    if (single && path.startsWith(`/Users/${userId}/`) && byId[single[1]]) {
      return json(toJson(byId[single[1]]));
    }
    const played = /^\/Users\/[^/]+\/PlayedItems\/([^/]+)$/.exec(path);
    if (played && path.startsWith(`/Users/${userId}/`) && byId[played[1]]) {
      if (req.method === 'POST') state.userData[played[1]].played = true;
      if (req.method === 'DELETE') state.userData[played[1]].played = false;
      return json({});
    }
    const pbinfo = /^\/Items\/([^/]+)\/PlaybackInfo$/.exec(path);
    if (pbinfo && byId[pbinfo[1]]) {
      return json({
        MediaSources: [{ Id: `ms-${pbinfo[1]}`, SupportsDirectPlay: true, SupportsDirectStream: true }],
        PlaySessionId: `ps-${pbinfo[1]}`,
      });
    }
    if (path.startsWith('/Sessions/Playing')) {
      const endpoint = path.slice('/Sessions/Playing'.length) || '/Start';
      let raw = '';
      req.on('data', (c) => (raw += c));
      req.on('end', () => {
        let body = {};
        try {
          body = JSON.parse(raw);
        } catch {}
        state.checkins.push({ endpoint, body });
        // A real server records the reported position; Stopped is the
        // authoritative final one that a refetch must reflect.
        if (endpoint === '/Stopped' && typeof body.PositionTicks === 'number' && state.userData[body.ItemId]) {
          state.userData[body.ItemId].positionTicks = body.PositionTicks;
        }
        json({});
      });
      return;
    }
    const stream = /^\/Videos\/([^/]+)\/stream$/.exec(path);
    if (stream && byId[stream[1]]?.mediaFile) {
      return serveRange(req, res, byId[stream[1]].mediaFile);
    }
    return json({ error: 'not mocked' }, 404); // images etc.: no-art fallback
  });

  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      resolve({
        port: server.address().port,
        state,
        userId,
        close: () => new Promise((r) => server.close(r)),
      });
    });
  });
}
