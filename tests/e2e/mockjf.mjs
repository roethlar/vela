// Minimal stateful mock Jellyfin server for hermetic server-flow scenarios.
// Serves exactly the endpoint surface Vela's Jellyfin client uses (see
// src-tauri/src/source/jellyfin.rs) with PascalCase JSON, records every
// request for assertions, and lets scenarios read/flip watch state directly
// (it runs in the runner process — no control endpoint needed).
import http from 'node:http';

export function startMockJellyfin({ userId = 'u1' } = {}) {
  const state = {
    played: false, // UserData.Played for the single movie
    requests: [], // { method, path } in arrival order
  };

  const movie = () => ({
    Id: 'm1',
    Name: 'Mock Movie',
    Type: 'Movie',
    ProductionYear: 2020,
    RunTimeTicks: 6_000_000_000, // 10 min in 100ns ticks
    Overview: 'A film that exists only for the harness.',
    UserData: { Played: state.played, PlaybackPositionTicks: 0 },
  });

  const server = http.createServer((req, res) => {
    const path = new URL(req.url, 'http://mock').pathname;
    state.requests.push({ method: req.method, path });

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
    if (path === `/Users/${userId}/Items` || path === `/Users/${userId}/Items/m1`) {
      return json(path.endsWith('/m1') ? movie() : { Items: [movie()] });
    }
    if (path === `/Users/${userId}/PlayedItems/m1`) {
      if (req.method === 'POST') state.played = true;
      if (req.method === 'DELETE') state.played = false;
      return json({});
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
