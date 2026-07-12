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
import fs from "node:fs";
import http from "node:http";

export function startMockJellyfin({
  userId = "u1",
  movies = [{ id: "m1", name: "Mock Movie", year: 2020 }],
  // Optional multi-view seed: [{ id, name, collectionType, movies }].
  // Defaults to the original single 'lib1' view over `movies`, so
  // pre-existing scenarios are unaffected (library-refresh-scan plan).
  views = null,
  latest = [], // items for /Users/{userId}/Items/Latest → the "Recently Added" Home hub
  // Real servers don't persist sub-threshold positions (Plex's resume
  // minimum is ~60s); a Stopped report below this many ticks is accepted
  // but stores no resume point. Scenarios that must prove Vela's own
  // recents stamp works WITHOUT server help (br-1) set it above the clip.
  minResumeTicks = 0,
  // Opt-in faithful Resume hub: /Users/{u}/Items/Resume returns in-progress
  // unplayed movies (real-server behavior). Default stays the hardcoded
  // empty list so pre-existing scenarios keep a recents-only hero feed and
  // their EMPTY_HOME assertions.
  serveResume = false,
} = {}) {
  const initialViews = views ?? [
    { id: "lib1", name: "Mock Library", collectionType: "movies", movies },
  ];

  const state = {
    // Per-movie UserData; Stopped check-ins update positionTicks like a real
    // server, and PlayedItems POST/DELETE flip played.
    userData: Object.fromEntries(
      initialViews
        .flatMap((v) => v.movies)
        .map((m) => [m.id, { played: false, positionTicks: 0 }]),
    ),
    requests: [], // { method, path, query } in arrival order
    checkins: [], // parsed /Sessions/Playing* bodies: { endpoint, body }
    contractViolations: [], // Items requests whose query broke the client contract
    // Scenario-mutable machinery (library-refresh-scan plan). Handlers read
    // these LIVE, so scenarios may add/remove views or reseed the Latest
    // rail mid-session; the one-shot flags are consumed by the next matching
    // request, while viewsDelayMs is a knob that stays until reset.
    views: initialViews,
    latest: [...latest], // mutable Latest rail seed
    failNextViews: false, // one-shot: 500 the next /Users/{id}/Views
    viewsDelayMs: 0,
    failNextLatest: false, // one-shot: 500 the next /Users/{id}/Items/Latest
    delayNextLatestMs: 0, // one-shot delay for the next Latest request
    // Scan-trigger machinery (library-refresh-scan plan): VirtualFolders is
    // seeded from the served views but kept SEPARATE — a grouped view added
    // to state.views without a matching entry here reproduces the real
    // grouped-folder shape (view id ∉ VirtualFolders ItemIds).
    virtualFolders: initialViews.map((v) => ({
      Name: v.name,
      ItemId: v.id,
      CollectionType: v.collectionType ?? "movies",
      Locations: [`/media/${v.id}`],
    })),
    failNextVirtualFolders: false, // one-shot: 403 the next VirtualFolders GET
    failNextItemRefresh: false, // one-shot: 403 the next POST /Items/{id}/Refresh
    itemRefreshDelayMs: 0, // one-shot delay for the next POST /Items/{id}/Refresh
    // Mutation helpers that keep `userData` coherent — pushing on the raw
    // array alone would leave toJson reading missing userData and crash the
    // next listing that includes the new movie.
    addMovie(viewId, movie) {
      const view = state.views.find((v) => v.id === viewId);
      if (!view) throw new Error(`mockjf: no view '${viewId}' to add to`);
      view.movies.push(movie);
      state.userData[movie.id] ??= { played: false, positionTicks: 0 };
    },
    removeMovie(viewId, id) {
      const view = state.views.find((v) => v.id === viewId);
      if (!view) throw new Error(`mockjf: no view '${viewId}' to remove from`);
      view.movies = view.movies.filter((m) => m.id !== id);
      // The userData entry stays: a re-added movie keeps its watch state.
    },
  };

  const allMovies = () => state.views.flatMap((v) => v.movies);
  const findMovie = (id) => allMovies().find((m) => m.id === id);
  const toJson = (m) => ({
    Id: m.id,
    Name: m.name,
    Type: "Movie",
    ProductionYear: m.year ?? 2020,
    RunTimeTicks: m.runTimeTicks ?? 6_000_000_000, // default 10 min in 100ns ticks
    Overview: "A film that exists only for the harness.",
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
    const m = /^bytes=(\d*)-(\d*)$/.exec(req.headers.range ?? "");
    let start = 0;
    let end = size - 1;
    if (m && (m[1] !== "" || m[2] !== "")) {
      if (m[1] === "") {
        start = Math.max(0, size - Number(m[2])); // suffix: last N bytes
      } else {
        start = Number(m[1]);
        if (m[2] !== "") end = Math.min(Number(m[2]), size - 1);
      }
      if (start >= size || start > end) {
        // start>end covers reversed ranges like bytes=50-40 — unsatisfiable.
        res.writeHead(416, { "Content-Range": `bytes */${size}` });
        return res.end();
      }
      res.writeHead(206, {
        "Content-Type": "video/mp4",
        "Accept-Ranges": "bytes",
        "Content-Length": end - start + 1,
        "Content-Range": `bytes ${start}-${end}/${size}`,
      });
    } else {
      res.writeHead(200, {
        "Content-Type": "video/mp4",
        "Accept-Ranges": "bytes",
        "Content-Length": size,
      });
    }
    fs.createReadStream(mediaFile, { start, end }).pipe(res);
  };

  const server = http.createServer((req, res) => {
    const url = new URL(req.url, "http://mock");
    const path = url.pathname;
    const query = Object.fromEntries(url.searchParams);
    state.requests.push({ method: req.method, path, query });

    const json = (body, status = 200) => {
      res.writeHead(status, { "Content-Type": "application/json" });
      res.end(JSON.stringify(body));
    };

    if (path === `/Users/${userId}/Views`) {
      const respond = () => {
        if (state.failNextViews) {
          state.failNextViews = false; // one-shot, consumed at respond time
          return json({ error: "mock Views failure" }, 500);
        }
        return json({
          Items: state.views.map((v) => ({
            Id: v.id,
            Name: v.name,
            CollectionType: v.collectionType ?? "movies",
          })),
        });
      };
      if (state.viewsDelayMs > 0) {
        setTimeout(respond, state.viewsDelayMs);
        return;
      }
      return respond();
    }
    if (path === `/Users/${userId}/Items/Resume`) {
      if (!serveResume) return json({ Items: [] });
      return json({
        Items: allMovies()
          .filter(
            (m) =>
              state.userData[m.id].positionTicks > 0 &&
              !state.userData[m.id].played,
          )
          .map(toJson),
      });
    }
    if (path === `/Users/${userId}/Items/Latest`) {
      // BOTH one-shot flags are captured at ARRIVAL so they bind to THIS
      // request: consuming failure at respond time would hand a delayed older
      // request's failure to a newer concurrent one (codex code review r1,
      // finding 2).
      const fail = state.failNextLatest;
      if (fail) state.failNextLatest = false; // one-shot
      const delay = state.delayNextLatestMs;
      if (delay > 0) state.delayNextLatestMs = 0; // one-shot
      const respond = () => {
        if (fail) return json({ error: "mock Latest failure" }, 500);
        return json(state.latest); // bare array, per the Jellyfin API (Recently Added hub)
      };
      if (delay > 0) {
        setTimeout(respond, delay);
        return;
      }
      return respond();
    }
    if (path === `/Users/${userId}/Items`) {
      // Search goes to the same endpoint with searchTerm and NO ParentId
      // (jellyfin.rs search()); a real server filters by name. The client
      // always sends Recursive + IncludeItemTypes — their absence is a
      // client regression (eh-12 class, br-2), while a NARROWED type set is
      // answered the way a real server answers: filtered, so a search that
      // stopped asking for movies gets none back instead of a false pass.
      if (query.searchTerm !== undefined) {
        if (
          query.Recursive !== "true" ||
          query.IncludeItemTypes === undefined
        ) {
          state.contractViolations.push({ path, query });
          return json({ error: "query contract violation" }, 400);
        }
        if (!query.IncludeItemTypes.includes("Movie"))
          return json({ Items: [] });
        const term = query.searchTerm.toLowerCase();
        return json({
          Items: allMovies()
            .filter((m) => m.name.toLowerCase().includes(term))
            .map(toJson),
        });
      }
      // Fail closed on the client's listing query contract (eh-12): a real
      // Jellyfin would return the wrong contents (or error) for a bad
      // ParentId or a missing IncludeItemTypes — an ignore-all mock would
      // hide exactly that class of regression.
      const view = state.views.find((v) => v.id === query.ParentId);
      if (!view || !(query.IncludeItemTypes ?? "").includes("Movie")) {
        state.contractViolations.push({ path, query });
        return json({ error: "query contract violation" }, 400);
      }
      // HONOR StartIndex/Limit (library-refresh-scan plan): with an
      // ignore-pagination mock, a refresh that APPENDS instead of replacing
      // (or reloads from a stale offset) would still pass exact-set
      // assertions downstream.
      const start = Number(query.StartIndex ?? 0);
      const end =
        query.Limit !== undefined ? start + Number(query.Limit) : undefined;
      return json({ Items: view.movies.slice(start, end).map(toJson) });
    }
    const single = /^\/Users\/[^/]+\/Items\/([^/]+)$/.exec(path);
    if (
      single &&
      path.startsWith(`/Users/${userId}/`) &&
      findMovie(single[1])
    ) {
      return json(toJson(findMovie(single[1])));
    }
    const played = /^\/Users\/[^/]+\/PlayedItems\/([^/]+)$/.exec(path);
    if (
      played &&
      path.startsWith(`/Users/${userId}/`) &&
      findMovie(played[1])
    ) {
      // Real servers reset the resume point on BOTH transitions (Jellyfin
      // MarkPlayed/MarkUnplayed zero PlaybackPositionTicks; Plex scrobble/
      // unscrobble clears the view offset) — keeping it would let a stale
      // resume point survive a "full reset" and pass the old assertions.
      if (req.method === "POST")
        state.userData[played[1]] = { played: true, positionTicks: 0 };
      if (req.method === "DELETE")
        state.userData[played[1]] = { played: false, positionTicks: 0 };
      return json({});
    }
    const pbinfo = /^\/Items\/([^/]+)\/PlaybackInfo$/.exec(path);
    if (pbinfo && findMovie(pbinfo[1])) {
      return json({
        MediaSources: [
          {
            Id: `ms-${pbinfo[1]}`,
            SupportsDirectPlay: true,
            SupportsDirectStream: true,
          },
        ],
        PlaySessionId: `ps-${pbinfo[1]}`,
      });
    }
    if (path.startsWith("/Sessions/Playing")) {
      const endpoint = path.slice("/Sessions/Playing".length) || "/Start";
      let raw = "";
      req.on("data", (c) => (raw += c));
      req.on("end", () => {
        let body = {};
        try {
          body = JSON.parse(raw);
        } catch {}
        state.checkins.push({ endpoint, body });
        // A real server records the reported position; Stopped is the
        // authoritative final one that a refetch must reflect — unless it
        // falls under the server's resume minimum (see minResumeTicks).
        if (
          endpoint === "/Stopped" &&
          typeof body.PositionTicks === "number" &&
          state.userData[body.ItemId] &&
          body.PositionTicks >= minResumeTicks
        ) {
          state.userData[body.ItemId].positionTicks = body.PositionTicks;
        }
        json({});
      });
      return;
    }
    const stream = /^\/Videos\/([^/]+)\/stream$/.exec(path);
    if (stream && findMovie(stream[1])?.mediaFile) {
      return serveRange(req, res, findMovie(stream[1]).mediaFile);
    }
    // Scan-trigger endpoints (library-refresh-scan plan). VirtualFolders is
    // the JF bare admin route (a bare array, not an Items envelope); a 403
    // here is exactly what a real non-admin token gets, since the route is
    // elevation-gated before any refresh POST is reached.
    if (path === "/Library/VirtualFolders" && req.method === "GET") {
      if (state.failNextVirtualFolders) {
        state.failNextVirtualFolders = false; // one-shot
        return json({ error: "mock: admin required" }, 403);
      }
      return json(state.virtualFolders);
    }
    const refresh = /^\/Items\/([^/]+)\/Refresh$/.exec(path);
    if (refresh && req.method === "POST") {
      const respond = () => {
        if (state.failNextItemRefresh) {
          state.failNextItemRefresh = false; // one-shot
          return json({ error: "mock: admin required" }, 403);
        }
        // The request (with its Recursive/RegenerateTrickplay query) is
        // already in state.requests; scenarios assert on that log, not on
        // this body.
        res.writeHead(204);
        return res.end();
      };
      // One-shot delay (scanlib out-of-order case): consumed at ARRIVAL so a
      // second scan issued while this one is parked responds immediately.
      const delay = state.itemRefreshDelayMs;
      if (delay > 0) {
        state.itemRefreshDelayMs = 0; // one-shot
        setTimeout(respond, delay);
        return;
      }
      return respond();
    }
    return json({ error: "not mocked" }, 404); // images etc.: no-art fallback
  });

  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      resolve({
        port: server.address().port,
        state,
        userId,
        close: () => new Promise((r) => server.close(r)),
      });
    });
  });
}
