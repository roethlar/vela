// Minimal stateful mock Jellyfin server for hermetic server-flow scenarios.
// Serves exactly the endpoint surface Vela's Jellyfin client uses (see
// src-tauri/src/source/jellyfin.rs) with PascalCase JSON, records every
// request for assertions, and lets scenarios read/flip watch state directly
// (it runs in the runner process — no control endpoint needed).
import fs from 'node:fs';
import http from 'node:http';

export function startMockJellyfin({
  userId = 'u1',
  runTimeTicks = 6_000_000_000, // 10 min in 100ns ticks
  mediaFile = null, // when set, /Videos/m1/stream serves this file (Range-capable)
} = {}) {
  const state = {
    played: false, // UserData.Played for the single movie
    positionTicks: 0, // UserData.PlaybackPositionTicks; Stopped check-ins update it
    requests: [], // { method, path, query } in arrival order
    checkins: [], // parsed /Sessions/Playing* bodies: { endpoint, body }
    contractViolations: [], // Items requests whose query broke the client contract
  };

  const movie = () => ({
    Id: 'm1',
    Name: 'Mock Movie',
    Type: 'Movie',
    ProductionYear: 2020,
    RunTimeTicks: runTimeTicks,
    Overview: 'A film that exists only for the harness.',
    UserData: { Played: state.played, PlaybackPositionTicks: state.positionTicks },
  });

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
      return json([]); // bare array, per the Jellyfin API
    }
    if (path === `/Users/${userId}/Items`) {
      // Fail closed on the client's query contract (eh-12): a real Jellyfin
      // would return the wrong contents (or error) for a bad ParentId or a
      // missing IncludeItemTypes — an ignore-all mock would hide exactly
      // that class of regression.
      if (query.ParentId !== 'lib1' || !(query.IncludeItemTypes ?? '').includes('Movie')) {
        state.contractViolations.push({ path, query });
        return json({ error: 'query contract violation' }, 400);
      }
      return json({ Items: [movie()] });
    }
    if (path === `/Users/${userId}/Items/m1`) {
      return json(movie());
    }
    if (path === `/Users/${userId}/PlayedItems/m1`) {
      if (req.method === 'POST') state.played = true;
      if (req.method === 'DELETE') state.played = false;
      return json({});
    }
    if (path === '/Items/m1/PlaybackInfo') {
      return json({
        MediaSources: [{ Id: 'ms1', SupportsDirectPlay: true, SupportsDirectStream: true }],
        PlaySessionId: 'ps1',
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
        if (endpoint === '/Stopped' && typeof body.PositionTicks === 'number') {
          state.positionTicks = body.PositionTicks;
        }
        json({});
      });
      return;
    }
    if (path === '/Videos/m1/stream' && mediaFile) {
      // Minimal Range support: mpv seeks over HTTP with byte ranges.
      const size = fs.statSync(mediaFile).size;
      const range = /bytes=(\d+)-(\d*)/.exec(req.headers.range ?? '');
      const start = range ? Number(range[1]) : 0;
      const end = range && range[2] ? Number(range[2]) : size - 1;
      res.writeHead(range ? 206 : 200, {
        'Content-Type': 'video/mp4',
        'Accept-Ranges': 'bytes',
        'Content-Length': end - start + 1,
        ...(range ? { 'Content-Range': `bytes ${start}-${end}/${size}` } : {}),
      });
      fs.createReadStream(mediaFile, { start, end }).pipe(res);
      return;
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
