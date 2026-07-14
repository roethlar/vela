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
    served: [], // { method, path, status } in RESPONSE order (see json())
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
    failNextItems: false, // one-shot: 500 the next listing (a doomed loadMore)
    // one-shot: 401 the next listing. A 401 is the ONE failure a listing and a
    // scan report with the SAME text (both surface as RECONNECT_REQUIRED, which
    // friendlyError maps to one constant sentence) — which is what makes the
    // banner-ownership case writable (codex r12; refresh case 25).
    unauthNextItems: false,
    // one-shot: 401 the next watch-state edit (PlayedItems POST/DELETE). A 401 is
    // the one failure a LISTING and a NON-listing writer report with the SAME
    // rendered text (both surface as RECONNECT_REQUIRED, which friendlyError maps
    // to one constant sentence) — which is what makes the banner-ownership case
    // writable at all (grok r17; refresh case 25).
    unauthNextPlayed: false,
    playedDelayMs: 0, // one-shot delay for the next watch-state edit
    itemsDelayMs: 0, // one-shot delay for the next listing
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
    // one-shot: 401 the next scan — the same RECONNECT_REQUIRED a 401 listing
    // reports (see unauthNextItems).
    unauthNextItemRefresh: false,
    itemRefreshDelayMs: 0, // one-shot delay for the next POST /Items/{id}/Refresh
    // Fail EVERY PlaybackInfo until reset (not a one-shot: a scenario that needs several
    // failed plays should not have to re-arm between them). This is the only deterministic
    // way to fail a Play: `play_by_key` resolves the stream BEFORE it spawns mpv
    // (commands.rs:2247), and a bogus `mpv_path` does NOT work — `resolve_mpv` validates it
    // and silently falls back to mpv on PATH (playback.rs:207).
    failPlaybackInfo: false,
    playbackInfoDelayMs: 0, // one-shot: park the next PlaybackInfo (bound at arrival)
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
  // A view's item TYPE follows its collectionType, because Vela asks each
  // library for the types that library holds (jellyfin.rs items(): a "show"
  // section is listed with IncludeItemTypes=Series, a "movie" one with
  // Movie). A movies-only mock behaves exactly as before; a tvshows view
  // lets a scenario stand up a NON-Movies provider (mergedrefresh needs one
  // so the merged Movies type can disappear when its sole provider fails).
  const itemTypeOf = (view) =>
    view.collectionType === "tvshows" ? "Series" : "Movie";
  const typeOfItem = (id) => {
    const view = state.views.find((v) => v.movies.some((m) => m.id === id));
    return view ? itemTypeOf(view) : "Movie";
  };
  const toJson = (m) => ({
    Id: m.id,
    Name: m.name,
    Type: typeOfItem(m.id),
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

  // The admin-gated scan routes are the ONLY writes Vela makes, and they carry
  // admin-capable credentials — so the mock demands them. Without this, dropping
  // the auth headers from the production scan request would leave every scan
  // guard green while a real Jellyfin/Emby answered 401 and Scan library became
  // unusable (codex r7). Jellyfin sends Authorization, Emby X-Emby-Token.
  const authed = (req) =>
    (req.headers.authorization ?? "").includes('Token="') ||
    !!req.headers["x-emby-token"];

  const server = http.createServer((req, res) => {
    const url = new URL(req.url, "http://mock");
    const path = url.pathname;
    const query = Object.fromEntries(url.searchParams);
    state.requests.push({ method: req.method, path, query });

    const json = (body, status = 200) => {
      // Record the response as it goes OUT, not just the request as it came in. A
      // scenario that parks a response and then asserts an ABSENCE (no banner) needs a
      // positive witness that the client has actually been given something to react to
      // — a wait measured from the request's ARRIVAL proves nothing about when the
      // parked answer was delivered, so the assertion can simply run too early and pass
      // (codex + grok, r21).
      //
      // Recorded AFTER the write, so it cannot claim delivery for a response still
      // sitting in this handler (codex r23). It is still a SERVER-dispatch witness, not
      // proof the client processed it — the scenarios pair it with a held window for
      // that, and say so.
      res.writeHead(status, { "Content-Type": "application/json" });
      res.end(JSON.stringify(body));
      state.served.push({ method: req.method, path, status });
    };
    const unauthorized = (p) => {
      state.contractViolations.push({ path: p, query: { auth: "missing" } });
      return json({ error: "unauthenticated" }, 401);
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
        // A search re-run is a LISTING for one-shot purposes. `refreshWatchState()`
        // re-enters a search root through `runSearch(rerun)`, so an edit made in
        // search results recovers through HERE — and until this branch honoured the
        // one-shots, that recovery could not be parked or failed, which left the whole
        // search root untestable for delayed-publication races (r20, pagefail case 7).
        // Bound at ARRIVAL, exactly like the listing branch below.
        const failSearch = state.failNextItems;
        if (failSearch) state.failNextItems = false;
        const unauthSearch = state.unauthNextItems;
        if (unauthSearch) state.unauthNextItems = false;
        const searchDelay = state.itemsDelayMs;
        if (searchDelay > 0) state.itemsDelayMs = 0;
        const term = query.searchTerm.toLowerCase();
        const respondSearch = () => {
          if (failSearch) return json({ error: "mock search failure" }, 500);
          if (unauthSearch) return json({ error: "unauthenticated" }, 401);
          return json({
            Items: allMovies()
              .filter(
                (m) =>
                  query.IncludeItemTypes.includes(typeOfItem(m.id)) &&
                  m.name.toLowerCase().includes(term),
              )
              .map(toJson),
          });
        };
        if (searchDelay > 0) {
          setTimeout(respondSearch, searchDelay);
          return;
        }
        return respondSearch();
      }
      // Fail closed on the client's listing query contract (eh-12): a real
      // Jellyfin would return the wrong contents (or error) for a bad
      // ParentId or a missing IncludeItemTypes — an ignore-all mock would
      // hide exactly that class of regression.
      const view = state.views.find((v) => v.id === query.ParentId);
      if (!view || !(query.IncludeItemTypes ?? "").includes(itemTypeOf(view))) {
        state.contractViolations.push({ path, query });
        return json({ error: "query contract violation" }, 400);
      }
      // One-shots bound at ARRIVAL (like the Latest flags): a scenario can park
      // a listing and make it FAIL, to exercise an ordinary in-flight load that
      // dies while a refresh is running (codex r3).
      const failItems = state.failNextItems;
      if (failItems) state.failNextItems = false;
      const unauthItems = state.unauthNextItems;
      if (unauthItems) state.unauthNextItems = false;
      const itemsDelay = state.itemsDelayMs;
      if (itemsDelay > 0) state.itemsDelayMs = 0;
      // HONOR StartIndex/Limit (library-refresh-scan plan): with an
      // ignore-pagination mock, a refresh that APPENDS instead of replacing
      // (or reloads from a stale offset) would still pass exact-set
      // assertions downstream.
      const start = Number(query.StartIndex ?? 0);
      const end =
        query.Limit !== undefined ? start + Number(query.Limit) : undefined;
      const respondItems = () => {
        if (failItems) return json({ error: "mock listing failure" }, 500);
        if (unauthItems) return json({ error: "unauthenticated" }, 401);
        return json({ Items: view.movies.slice(start, end).map(toJson) });
      };
      if (itemsDelay > 0) {
        setTimeout(respondItems, itemsDelay);
        return;
      }
      return respondItems();
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
      // Bound at ARRIVAL. The edit's own server call is the longest wait in
      // `setWatched` and the user can leave DURING it — so a scenario has to be able
      // to park the edit itself, not just its recovery repaint (r20, pagefail case 9).
      const unauthPlayed = state.unauthNextPlayed;
      if (unauthPlayed) state.unauthNextPlayed = false; // one-shot
      const playedDelay = state.playedDelayMs;
      if (playedDelay > 0) state.playedDelayMs = 0; // one-shot
      const respondPlayed = () => {
        if (unauthPlayed) return json({ error: "unauthenticated" }, 401);
        // Real servers reset the resume point on BOTH transitions (Jellyfin
        // MarkPlayed/MarkUnplayed zero PlaybackPositionTicks; Plex scrobble/
        // unscrobble clears the view offset) — keeping it would let a stale
        // resume point survive a "full reset" and pass the old assertions.
        if (req.method === "POST")
          state.userData[played[1]] = { played: true, positionTicks: 0 };
        if (req.method === "DELETE")
          state.userData[played[1]] = { played: false, positionTicks: 0 };
        return json({});
      };
      if (playedDelay > 0) {
        setTimeout(respondPlayed, playedDelay);
        return;
      }
      return respondPlayed();
    }
    const pbinfo = /^\/Items\/([^/]+)\/PlaybackInfo$/.exec(path);
    if (pbinfo && findMovie(pbinfo[1]) && state.failPlaybackInfo) {
      const delay = state.playbackInfoDelayMs;
      if (delay > 0) state.playbackInfoDelayMs = 0; // one-shot, bound at ARRIVAL
      const respond = () => json({ error: "mock: cannot resolve a stream" }, 500);
      if (delay > 0) {
        setTimeout(respond, delay);
        return;
      }
      return respond();
    }
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
      if (!authed(req)) return unauthorized(path);
      if (state.failNextVirtualFolders) {
        state.failNextVirtualFolders = false; // one-shot
        return json({ error: "mock: admin required" }, 403);
      }
      return json(state.virtualFolders);
    }
    const refresh = /^\/Items\/([^/]+)\/Refresh$/.exec(path);
    if (refresh && req.method === "POST") {
      if (!authed(req)) return unauthorized(path);
      // BOTH one-shots are consumed at ARRIVAL so they bind to THIS request.
      // Binding the failure at RESPOND time instead let a fast scan B steal
      // the 403 armed for a parked scan A — making the stale-FAILURE ordering
      // case unwritable (lrs-5; same class as the Latest flags, codex r1 f2).
      const fail = state.failNextItemRefresh;
      if (fail) state.failNextItemRefresh = false; // one-shot
      const unauth = state.unauthNextItemRefresh;
      if (unauth) state.unauthNextItemRefresh = false; // one-shot
      const delay = state.itemRefreshDelayMs;
      if (delay > 0) state.itemRefreshDelayMs = 0; // one-shot
      const respond = () => {
        if (fail) return json({ error: "mock: admin required" }, 403);
        if (unauth) return json({ error: "unauthenticated" }, 401);
        // The request (with its full scan query) is already in state.requests;
        // scenarios assert on that log, not on this body. But the RESPONSE still has to
        // be recorded, or `state.served` has a hole exactly where a scenario parks a
        // scan — this is the one success path that does not go through json().
        res.writeHead(204);
        res.end();
        state.served.push({ method: req.method, path, status: 204 });
        return;
      };
      if (delay > 0) {
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
