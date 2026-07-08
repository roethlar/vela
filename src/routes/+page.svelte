<script lang="ts">
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy, tick } from "svelte";
  import Settings from "$lib/Settings.svelte";
  import Icon from "$lib/Icon.svelte";
  import ItemDetail from "$lib/ItemDetail.svelte";
  import SeasonDetail from "$lib/SeasonDetail.svelte";
  import { detailKeyOf, type Item } from "$lib/types";

  // Poster URLs that 404'd; fall back to the title placeholder for these.
  let failedPosters = $state(new Set<string>());
  // Tracked timers, cleared on destroy / when superseded.
  let copyTimer: ReturnType<typeof setTimeout> | undefined;
  let pollTimer: ReturnType<typeof setTimeout> | undefined;
  let unlistenPlaybackEnded: (() => void) | undefined;
  onDestroy(() => {
    if (copyTimer) clearTimeout(copyTimer);
    if (pollTimer) clearTimeout(pollTimer);
    if (queueTimer) clearInterval(queueTimer);
    unlistenPlaybackEnded?.();
    linkGen++; // invalidate any in-flight link_poll so it won't reschedule after unmount
  });

  // The scrollable browse grid, so we can keep loading until it actually scrolls.
  let gridEl: HTMLElement | undefined = $state();

  // Posters are either an https URL (server/online art) or a local file path
  // (local sidecar art) — the latter needs the Tauri asset protocol to load.
  function posterSrc(p: string): string {
    return /^https?:\/\//.test(p) ? p : convertFileSrc(p);
  }

  type Section = { key: string; title: string; sectionType: string; sourceName?: string };
  // `Item` (the listing-card DTO mirror) lives in $lib/types, shared with the
  // detail components.
  type Hub = { title: string; hubIdentifier: string; hubType: string; items: Item[]; sourceId: string; sourceName?: string };
  type Crumb = { title: string; ratingKey: string | null };
  type Source = { id: string; name: string; kind: string };

  let sources = $state<Source[]>([]);
  let activeSource = $state<string | null>(null); // null = All sources (unified)
  let showSettings = $state(false);

  let authenticated = $state(false);
  let mode = $state<"home" | "browse">("home");
  let sections = $state<Section[]>([]);
  let hubs = $state<Hub[]>([]);
  let active = $state<Section | null>(null);
  // The All view's consolidated Library: a content-type listing merged across
  // sources (rework Phase B). Mutually exclusive with `active`.
  let activeType = $state<string | null>(null);
  const TYPE_LABELS: Record<string, string> = { movie: "Movies", show: "TV Shows", video: "Videos" };
  let typeTabs = $derived(
    ["movie", "show", "video"].filter((t) => sections.some((s) => s.sectionType === t))
  );
  // Merged listings only honor fields items carry on the DTO across sources:
  // title, year (release date), date added, last played. `rating` has no DTO
  // field, so it stays a per-source (server-side) sort only.
  const TYPE_SORTS = new Set([
    "titleSort:asc",
    "year:desc",
    "originallyAvailableAt:desc",
    "addedAt:desc",
    "lastViewedAt:desc",
  ]);
  function sourceNameOf(id: string): string {
    return sources.find((s) => s.id === id)?.name ?? "";
  }
  let crumbs = $state<Crumb[]>([]);
  let items = $state<Item[]>([]);
  let loading = $state(false);
  let loadingMore = $state(false);
  let offset = $state(0);
  let hasMore = $state(true);
  let error = $state<string | null>(null);
  let sort = $state("titleSort:asc");
  let searchQuery = $state("");
  let searchTerm = $state(""); // the query backing the current search results view
  const PAGE = 60;

  const SORTS = [
    { key: "titleSort:asc", label: "Title (A–Z)" },
    { key: "year:desc", label: "Year (newest)" },
    { key: "addedAt:desc", label: "Recently added" },
    { key: "originallyAvailableAt:desc", label: "Release date" },
    { key: "rating:desc", label: "Rating" },
    { key: "lastViewedAt:desc", label: "Recently played" },
  ];

  // Device-link state
  type Pin = {
    id: string;
    code: string;
    clientIdentifier: string;
    authUrl: string;
    qrSvg: string;
  };
  let pin = $state<Pin | null>(null);

  // mpv availability (for the install prompt). null = not checked yet.
  type MpvInfo = {
    available: boolean;
    path: string | null;
    configuredPath: string | null;
    canAutoInstall: boolean;
    installCommand: string | null;
    installDescription: string;
    installUrl: string;
  };
  let mpvInfo = $state<MpvInfo | null>(null);
  let copied = $state(false);
  let installingMpv = $state(false);

  // One-click mpv install. The backend chooses the concrete method for this OS.
  // On success it returns refreshed status, which clears the prompt.
  async function installMpv() {
    if (installingMpv) return;
    installingMpv = true;
    error = null;
    try {
      mpvInfo = await invoke<MpvInfo>("install_mpv");
    } catch (e) {
      error = String(e);
    } finally {
      installingMpv = false;
    }
  }

  // Version/build identity for the footer.
  type AppInfo = { version: string; buildDate: string; repoUrl: string };
  let appInfo = $state<AppInfo | null>(null);

  onMount(boot);

  // Refresh watch state the moment a playback session ends. The backend emits
  // `playback-ended` after its final server check-in, so the re-fetch below is
  // guaranteed to see the updated progress/played state (the hubs fetch itself
  // is live and uncached).
  // Shared by the playback-ended event and watched-state edits: anything
  // that changes watch state re-fetches hubs + recents so the hero flow and
  // progress bars reflect it without a restart.
  function refreshWatchState() {
    heroPos = 0; // the most recent change should be front and center
    if (mode === "home") {
      loadHome(++homeGen);
    } else {
      // The hidden Home hubs are stale now; empty them so goHome() re-fetches.
      hubs = [];
      // Refresh the visible listing so its progress bars / played badges update.
      if (searchTerm) runSearch(searchTerm);
      else resetAndLoad();
    }
  }

  onMount(() => {
    listen("playback-ended", refreshWatchState).then((un) => (unlistenPlaybackEnded = un));
  });

  async function boot() {
    // Dev flag for the not-yet-flipped detail surface (see openInfo below):
    // on in dev builds, opt-in via localStorage in release builds.
    devDetail = import.meta.env.DEV || localStorage.getItem("vela.devDetail") === "1";
    // Check mpv up front so we can prompt to install before the user hits play.
    invoke<MpvInfo>("check_mpv").then((m) => (mpvInfo = m)).catch(() => {});
    invoke<AppInfo>("get_app_info").then((a) => (appInfo = a)).catch(() => {});
    refreshQueue(); // so the header chip shows the count from launch

    try {
      await loadSourceList();
      authenticated = sources.length > 0;
      if (authenticated) await loadEverything();
    } catch (e) {
      error = String(e);
    }
  }

  async function loadSourceList() {
    try {
      sources = await invoke<Source[]>("get_sources");
    } catch {
      /* non-fatal: switcher just won't show extra sources */
    }
  }

  // Switch the active source (null = All), then reload home scoped to it. The
  // empty-scoped-Home → content routing is reactive (see the $effect below), so
  // it is NOT duplicated here.
  async function selectSource(id: string | null) {
    if (activeSource === id && mode === "home") return;
    activeSource = id;
    goHome();
    await loadEverything();
  }

  // Re-sync after sources are added/removed in Settings.
  async function onSourcesChanged() {
    await loadSourceList();
    if (!sources.some((s) => s.id === activeSource)) activeSource = null;
    authenticated = sources.length > 0;
    if (authenticated) {
      linkGen++; // abandon any in-flight Plex link poll tied to the old pin
      pin = null;
      await loadEverything();
    } else {
      // Last source removed — clear stale content and show the neutral empty state.
      sourceGen++; // discard any in-flight section load
      homeGen++; // and any in-flight home load
      loadGen++; // and any in-flight browse/search load
      linkGen++; // and any in-flight Plex link poll
      pin = null;
      hubs = [];
      sections = [];
      items = [];
      crumbs = [];
      active = null;
      activeType = null;
      detailView = null;
      mode = "home";
      loading = false;
    }
  }

  // Make the backend's reconnect signal human-readable.
  function friendlyError(e: string): string {
    return e.includes("RECONNECT_REQUIRED")
      ? "A server needs reconnecting — open Settings (⚙) and reconnect it."
      : e;
  }

  // Open a URL in the system browser via the backend (webview would navigate away).
  async function openExternal(url: string) {
    try {
      await invoke("open_url", { url });
    } catch (e) {
      error = String(e);
    }
  }

  async function copyText(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => (copied = false), 1500);
    } catch {
      /* clipboard may be unavailable; the text is visible to copy manually */
    }
  }

  // Row policy per hub (curation decision 2026-07-04): continue-watching AND
  // On Deck hubs fold into the hero cover-flow, interleaved by recency; every
  // other hub is a uniform 2:3 poster row (episodes show series art there).
  // Keyed on hub identifiers: Plex passes its own through (e.g.
  // "home.continue", plus Vela's synthetic "vela.ondeck"), Jellyfin/Emby use
  // "resume".
  function hubPolicy(h: Hub): "hero" | "landscape" | "portrait" {
    const id = h.hubIdentifier.toLowerCase();
    if (id.includes("continue") || id === "resume" || id.includes("ondeck")) return "hero";
    return "portrait";
  }

  // Which artwork a card shows, given its row's shape: landscape cards keep
  // episode stills but use backdrops for movies/shows; portrait cards use the
  // series poster for episodes. Falls back to `poster` when the richer field
  // is missing.
  function isEpisodic(item: Item): boolean {
    return item.mediaType === "episode" || item.mediaType === "video";
  }
  function artFor(item: Item, landscape: boolean): string | undefined {
    if (landscape) return isEpisodic(item) ? item.poster : (item.backdrop ?? item.poster);
    return isEpisodic(item) ? (item.seriesPoster ?? item.poster) : item.poster;
  }

  // Continue Watching cover-flow (design decision 2026-07-04): ONE hero fed
  // by Vela's own recents ("recently played and not finished") merged with
  // the server continue hubs, newest first, deduped. Index 0 = newest;
  // higher indices are older and fan behind-left of the centered card.
  let recents = $state<Item[]>([]);
  // Keys the user removed from Continue Watching; suppressed from both hero
  // feeds even while a server hub still carries them.
  let continueTombstones = $state<string[]>([]);
  let heroPos = $state(0);
  let heroItems = $derived.by(() => {
    const scoped = activeSource ? recents.filter((r) => r.sourceId === activeSource) : recents;
    const hubItems = hubs.filter((h) => hubPolicy(h) === "hero").flatMap((h) => h.items);
    const hidden = new Set(continueTombstones);
    const seen = new Set<string>();
    const out: Item[] = [];
    // Recents iterate first so the local copy wins the dedup — it carries the
    // freshest position and its own recency stamp.
    for (const it of [...scoped, ...hubItems]) {
      if (!seen.has(it.ratingKey) && !hidden.has(it.ratingKey)) {
        seen.add(it.ratingKey);
        out.push(it);
      }
    }
    // Interleave by recency (curation decision 2026-07-04): newest watch
    // activity first, from either feed. Unstamped items keep their relative
    // order after all stamped ones (sort() is stable; missing = -1).
    return out.sort((a, b) => (b.lastWatchedAtMs ?? -1) - (a.lastWatchedAtMs ?? -1));
  });
  function heroClamp(i: number): number {
    return Math.min(Math.max(i, 0), Math.max(heroItems.length - 1, 0));
  }

  // Bug 3 (owner UX ruling 2026-07-05): a scoped source's per-source Home must
  // never terminate on the "Nothing on your home screen yet" dead-end. A
  // local/SMB/SSH source contributes no Home hubs and a fresh mount has no
  // recents, so its Home settles empty even though its library sections are in
  // the sidebar. When that happens (and the source has sections), land on its
  // content by opening the first section.
  //
  // This is REACTIVE rather than a tail of selectSource() so it fires on every
  // path that can reach an empty scoped Home — clicking the source, the Home
  // button, or Back from a top-level section (back() → goHome()); a
  // selectSource-only fix left Home/Back dead-ending, and re-clicking the source
  // early-returns (codex r1, finding 1). It is gated on `!loading` so a pending
  // or superseded Home load can't misfire: a slow server source whose hubs
  // haven't arrived yet is still loading, so we never force-browse it — its Home
  // rails are kept once the load settles (codex r1, finding 2 + finding 3).
  // Keyed on the empty-Home STATE, not "any non-null source".
  $effect(() => {
    if (
      mode === "home" &&
      activeSource !== null &&
      !loading &&
      hubs.length === 0 &&
      heroItems.length === 0 &&
      sections.length > 0
    ) {
      // select() sets mode = "browse" synchronously, so the condition is false
      // on the next run — no loop, no double-open.
      select(sections[0]);
    }
  });

  // Section (nav) loads are invalidated only by a source switch (`sourceGen`);
  // home/hub loads are also invalidated by leaving home for a browse/search
  // (`homeGen`), so a stale hub response can't clear `loading` mid-browse — but
  // a pending section refresh survives a browse and still populates the tabs.
  let sourceGen = 0;
  let homeGen = 0;

  async function loadEverything() {
    loadGen++; // a full home reload supersedes any in-flight browse/search load
    // Clear the previous source's content immediately so stale rails/tabs can't
    // be clicked (and browse/play the old source) while the new load is pending.
    hubs = [];
    sections = [];
    failedPosters = new Set(); // bounded to the current view's posters
    const sg = ++sourceGen;
    const hg = ++homeGen;
    await Promise.all([loadSections(sg), loadHome(hg)]);
  }

  async function loadSections(gen: number = sourceGen) {
    try {
      const s = await invoke<Section[]>("get_sections", { sourceId: activeSource });
      if (gen === sourceGen) sections = s;
    } catch (e) {
      if (gen === sourceGen) error = String(e);
    }
  }

  async function loadHome(gen: number = homeGen) {
    mode = "home";
    loading = true;
    error = null;
    try {
      const [h, r, t] = await Promise.all([
        invoke<Hub[]>("get_hubs", { sourceId: activeSource }),
        // Recents failing must not blank Home; the hero degrades to hub data.
        invoke<Item[]>("get_recents").catch(() => [] as Item[]),
        // Tombstones failing must not blank Home either; worst case a
        // removed item reappears until the next successful load.
        invoke<string[]>("get_continue_tombstones").catch(() => [] as string[]),
      ]);
      if (gen === homeGen) {
        hubs = h;
        recents = r;
        continueTombstones = t;
      }
    } catch (e) {
      if (gen === homeGen) error = String(e);
    } finally {
      if (gen === homeGen) loading = false;
    }
  }

  function goHome() {
    detailView = null;
    loadGen++; // invalidate any in-flight browse load so it can't append after we leave
    loadingMore = false;
    loading = false; // a stale browse load won't clear this (its gen is stale); do it here
    error = null; // don't carry a browse/search error banner onto Home
    searchTerm = "";
    activeType = null;
    mode = "home";
    // Entering a browse earlier may have discarded an in-flight hub load (via the
    // homeGen bump). If we have no hubs, re-fetch so Home isn't stuck empty.
    if (hubs.length === 0) loadHome(++homeGen);
  }

  // Bumped on each begin/link so a superseded poll loop stops touching the
  // (global) pin — no duplicate polling or stale errors from an old attempt.
  let linkGen = 0;

  async function beginLink() {
    const gen = ++linkGen;
    if (pollTimer) clearTimeout(pollTimer); // drop any pending poll from a prior attempt
    pin = null; // abandon any previously shown code immediately, so a failed
    // (or superseded) begin can't leave a dead, unpolled code on screen
    try {
      const p = await invoke<Pin>("link_begin");
      if (gen !== linkGen) return; // a newer attempt started while we were requesting
      pin = p;
      pollLink(gen);
    } catch (e) {
      // e.g. invoked from Settings while offline — surface it instead of an
      // unhandled rejection, but only if this attempt is still the current one.
      if (gen === linkGen) error = String(e);
    }
  }

  async function pollLink(gen: number) {
    if (gen !== linkGen || !pin) return;
    try {
      const ok = await invoke<boolean>("link_poll", {
        pinId: pin.id,
        clientIdentifier: pin.clientIdentifier,
      });
      if (gen !== linkGen) return; // a newer link attempt superseded this one
      if (ok) {
        pin = null;
        authenticated = true;
        await loadSourceList(); // surface the new Plex source in the switcher
        await loadEverything();
        return;
      }
    } catch (e) {
      // Terminal error (expired/rate-limited/server failure) — stop polling and
      // clear the dead code so the UI doesn't keep showing it with no poll loop.
      if (gen === linkGen) {
        error = String(e);
        pin = null;
      }
      return;
    }
    if (gen === linkGen) pollTimer = setTimeout(() => pollLink(gen), 2000);
  }

  async function select(section: Section) {
    detailView = null;
    mode = "browse";
    searchTerm = "";
    active = section;
    activeType = null;
    crumbs = [{ title: section.title, ratingKey: null }];
    await resetAndLoad();
  }

  // Open a consolidated content-type listing (All view's Library).
  async function selectType(t: string) {
    detailView = null;
    mode = "browse";
    searchTerm = "";
    active = null;
    activeType = t;
    if (!TYPE_SORTS.has(sort)) sort = "titleSort:asc";
    crumbs = [{ title: TYPE_LABELS[t] ?? t, ratingKey: null }];
    await resetAndLoad();
  }

  // Bumped on every navigation; in-flight loads from an older generation are discarded.
  let loadGen = 0;

  async function resetAndLoad() {
    homeGen++; // leaving home: invalidate any in-flight home/sections load
    const myGen = ++loadGen;
    loadingMore = false; // abandon any in-flight load (its results are now stale)
    offset = 0;
    hasMore = true;
    items = [];
    failedPosters = new Set(); // bounded to the current view's posters
    loading = true;
    error = null;
    await loadMore(myGen);
    if (myGen === loadGen) loading = false;
  }

  // Load the next page for the current level (section root or a parent's children)
  // and append it. Drives infinite scroll. Discards results if navigation moved on.
  async function loadMore(myGen: number = loadGen) {
    if (loadingMore || !hasMore || myGen !== loadGen) return;
    const here = crumbs[crumbs.length - 1];
    if (!here || (!here.ratingKey && !active && !activeType)) return;
    loadingMore = true;
    try {
      const page = here.ratingKey
        ? await invoke<Item[]>("get_children", { ratingKey: here.ratingKey, start: offset, size: PAGE })
        : activeType
          ? await invoke<Item[]>("get_type_listing", {
              sectionType: activeType,
              sort,
              start: offset,
              size: PAGE,
            })
          : await invoke<Item[]>("get_items", {
              sectionKey: active!.key,
              sectionType: active!.sectionType,
              sort,
              start: offset,
              size: PAGE,
            });
      if (myGen !== loadGen) return; // navigated away while awaiting; drop these
      items = [...items, ...page];
      offset += page.length;
      hasMore = page.length >= PAGE;
    } catch (e) {
      if (myGen === loadGen) {
        error = String(e);
        hasMore = false;
      }
    } finally {
      // Only release the in-flight guard for the current generation; a stale load
      // finishing must not unlock a newer one mid-flight.
      if (myGen === loadGen) loadingMore = false;
    }
    // If the page we just appended doesn't make the grid scrollable yet, keep
    // loading: on tall / hi-dpi (4K) displays a single page can fit without a
    // scrollbar, so onScroll would never fire and we'd be stuck with more to load.
    // Bounded — each pass advances offset and clears hasMore on a short page.
    await tick();
    if (myGen === loadGen && hasMore && gridEl && gridEl.scrollHeight <= gridEl.clientHeight) {
      await loadMore(myGen);
    }
  }

  function onScroll(e: Event) {
    const el = e.currentTarget as HTMLElement;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 600) loadMore();
  }

  // Drill into shows/seasons; play episodes/movies. Works from the home rails too.
  async function open(item: Item) {
    detailView = null;
    if (item.mediaType === "show" || item.mediaType === "season") {
      // Merged shows drill through the metadata-richest backing (idv-5) so
      // seasons/episodes come from — and play on — the rich server source;
      // non-merged items carry no detailKey, so nothing changes for them.
      const key = detailKeyOf(item);
      if (mode === "home") {
        // Drilling out of a hub: start a fresh crumb trail rooted at this item.
        active = null;
        crumbs = [{ title: item.title, ratingKey: key }];
      } else {
        crumbs = [...crumbs, { title: item.title, ratingKey: key }];
      }
      mode = "browse";
      await resetAndLoad();
    } else {
      await play(item);
    }
  }

  async function changeSort() {
    await resetAndLoad();
  }

  async function runSearch(query: string = searchQuery) {
    const q = query.trim();
    if (q.length < 2) {
      error = "Search needs at least 2 characters.";
      if (searchTerm) {
        items = [];
        crumbs = [];
        active = null;
        searchTerm = "";
      }
      return;
    }
    homeGen++; // leaving home: invalidate any in-flight home/sections load
    const myGen = ++loadGen; // invalidate any in-flight load; guard our own result
    loadingMore = false;
    detailView = null;
    mode = "browse";
    active = null; // search results aren't a section, so no pagination
    searchTerm = q;
    crumbs = [{ title: `Search: "${q}"`, ratingKey: null }];
    items = [];
    hasMore = false;
    loading = true;
    error = null;
    try {
      const results = await invoke<Item[]>("search", { query: q, sourceId: activeSource });
      if (myGen !== loadGen) return; // user navigated away while searching
      items = results;
    } catch (e) {
      if (myGen === loadGen) error = String(e);
    } finally {
      if (myGen === loadGen) loading = false;
    }
  }

  function back() {
    if (crumbs.length > 1) goCrumb(crumbs.length - 2);
    else goHome();
  }

  async function goCrumb(index: number) {
    crumbs = crumbs.slice(0, index + 1);
    const here = crumbs[crumbs.length - 1];
    // Returning to a search root (no section, no rating key) re-runs the search.
    if (!here.ratingKey && !active && searchTerm) {
      await runSearch(searchTerm);
    } else {
      await resetAndLoad();
    }
  }

  // Play queue (Phase B). The backend owns the queue; we just send actions and
  // pull a snapshot for the drawer. queueItemFromItem captures the display
  // fields the drawer needs (title/poster/subtitle), so right-clicking an item
  // in any view enqueues the same poster we'd show in the grid.
  type QueueItem = {
    ratingKey: string;
    title: string;
    durationMs: number | null;
    poster: string | null;
    subtitle: string | null;
  };
  type QueueSnapshot = { items: QueueItem[]; currentIndex: number | null };

  function queueItemFromItem(item: Item): QueueItem {
    const subtitle = item.grandparentTitle
      ? item.parentIndex != null && item.index != null
        ? `S${item.parentIndex} · E${item.index}`
        : item.title
      : item.year != null
        ? `${item.year}`
        : null;
    return {
      ratingKey: item.ratingKey,
      title: item.grandparentTitle ?? item.title,
      durationMs: item.durationMs ?? null,
      poster: item.poster ?? null,
      subtitle,
    };
  }

  let queue = $state<QueueSnapshot>({ items: [], currentIndex: null });
  let queueOpen = $state(false);
  let queueTimer: ReturnType<typeof setInterval> | undefined;

  async function refreshQueue() {
    try {
      queue = await invoke<QueueSnapshot>("queue_list");
    } catch {
      // Backend unreachable mid-startup is fine to ignore here.
    }
  }
  function toggleQueue() {
    queueOpen = !queueOpen;
    if (queueOpen) {
      refreshQueue();
      // While the drawer is visible, poll lightly so auto-advances (which run
      // entirely in the backend on mpv EOF) show up without a user action.
      queueTimer = setInterval(refreshQueue, 3000);
    } else if (queueTimer) {
      clearInterval(queueTimer);
      queueTimer = undefined;
    }
  }

  async function play(item: Item) {
    try {
      await invoke("play_item", { item: queueItemFromItem(item) });
      // Snapshot into Vela's recents only after the session actually
      // launched (play_item resolves at mpv spawn): a FAILED play must not
      // create a recents entry or clear a remove-from-continue tombstone.
      invoke("record_recent", { item }).catch(() => {});
      if (queueOpen) refreshQueue();
    } catch (e) {
      error = String(e);
      // A failure may mean mpv went missing — re-check so the install prompt shows.
      invoke<MpvInfo>("check_mpv").then((m) => (mpvInfo = m)).catch(() => {});
    }
  }

  // Play a merged title from a specific backing source and remember the
  // choice for this title (it wins over the default ranking from now on).
  async function playFrom(item: Item, b: { sourceId: string; ratingKey: string }) {
    closeMenu();
    try {
      if (item.canonicalId) {
        await invoke("set_merged_override", { canonicalId: item.canonicalId, sourceId: b.sourceId });
      }
      await play({ ...item, ratingKey: b.ratingKey, sourceId: b.sourceId });
    } catch (e) {
      error = String(e);
    }
  }

  async function playNext(item: Item) {
    closeMenu();
    try {
      await invoke("queue_play_next", { item: queueItemFromItem(item) });
      refreshQueue();
    } catch (e) {
      error = String(e);
    }
  }
  async function addToQueue(item: Item) {
    closeMenu();
    try {
      await invoke("queue_append", { item: queueItemFromItem(item) });
      refreshQueue();
    } catch (e) {
      error = String(e);
    }
  }
  async function queueJumpTo(index: number) {
    try {
      await invoke("queue_play_at", { index });
      refreshQueue();
    } catch (e) {
      error = String(e);
    }
  }
  async function queueRemove(index: number) {
    try {
      await invoke("queue_remove", { index });
      refreshQueue();
    } catch (e) {
      error = String(e);
    }
  }
  async function queueClearAll() {
    try {
      await invoke("queue_clear");
      refreshQueue();
    } catch (e) {
      error = String(e);
    }
  }

  // Right-click context menu for per-item actions.
  let menu = $state<{ x: number; y: number; item: Item; hero: boolean } | null>(null);
  function openMenu(e: MouseEvent, item: Item, hero = false) {
    e.preventDefault();
    // Clamp so the menu stays on screen near the right/bottom edges.
    menu = { x: Math.min(e.clientX, window.innerWidth - 200), y: Math.min(e.clientY, window.innerHeight - 160), item, hero };
  }
  function closeMenu() {
    menu = null;
  }

  async function setWatched(item: Item, played: boolean) {
    closeMenu();
    try {
      // Merged cards may front a local file while a server backing owns the
      // watch state — route the action where it can actually be recorded.
      await invoke("set_watched", { ratingKey: item.watchKey ?? item.ratingKey, played });
      // Reflect immediately (deep-reactive $state). Scrobble/unscrobble both clear
      // the resume position, so drop the in-progress bar too — leaving a clean
      // watched (✓) or unwatched state instead of a contradictory bar + badge.
      item.played = played;
      item.viewOffsetMs = 0;
      // Curate the hero without a restart: mark-watched drops the item from
      // recents (backend) and the re-fetch drops the server hub copy;
      // mark-unwatched just re-fetches (the hub decides if it returns).
      refreshWatchState();
    } catch (e) {
      error = String(e);
    }
  }

  // Explicit hero curation: tombstone + recents drop (backend), then the
  // standard re-fetch. No watched-state change.
  async function removeFromContinue(item: Item) {
    closeMenu();
    try {
      await invoke("remove_from_continue", { ratingKey: item.ratingKey });
      refreshWatchState();
    } catch (e) {
      error = String(e);
    }
  }

  // ---- Detail / info surface (item-detail-view, amended slice 2) ----------
  // Reached only via the dev-flagged context-menu "Info" entry until the
  // navigation flips (amended slice 3): set localStorage "vela.devDetail" to
  // "1". The view layers over home/browse without touching their state, so
  // closing it returns exactly where the user was.
  type DetailView =
    | { kind: "item"; item: Item }
    | { kind: "season"; seasonKey: string | null; seed: Item; initialSelKey?: string };
  let detailView = $state<DetailView | null>(null);
  let devDetail = $state(false);

  function closeDetail() {
    detailView = null;
  }

  // A season key for an episode's shared page is only trustworthy when it
  // names the list the episode actually came from: the already-open shared
  // page itself, or a browse grid whose children include this episode. A
  // bare crumb is NOT enough — with a season page open above a seasons
  // grid, the crumb still points at the show, and get_children(show) would
  // list seasons in the episode list (idv-s2 review r1). Anything else
  // degrades to single-episode mode rather than a wrong list.
  function seasonKeyFor(ep: Item): string | null {
    if (detailView) {
      return detailView.kind === "season" ? detailView.seasonKey : null;
    }
    if (mode === "browse" && !searchTerm) {
      const here = crumbs[crumbs.length - 1];
      if (here?.ratingKey && items.some((i) => i.ratingKey === ep.ratingKey)) {
        return here.ratingKey;
      }
    }
    return null;
  }

  // Route an item to its info surface (owner UX ruling): movie/video → the
  // full-screen item page; season → the shared episode page; episode → the
  // shared page for its season (see seasonKeyFor). Shows keep their seasons
  // drill, so no entry is offered for them.
  function openInfo(item: Item) {
    closeMenu();
    if (item.mediaType === "season") {
      detailView = { kind: "season", seasonKey: detailKeyOf(item), seed: item };
    } else if (item.mediaType === "episode") {
      detailView = {
        kind: "season",
        seasonKey: seasonKeyFor(item),
        seed: item,
        initialSelKey: item.ratingKey,
      };
    } else {
      detailView = { kind: "item", item };
    }
  }
</script>

<div class="app">
  <div class="grain" aria-hidden="true"></div>
  <header>
    <span class="brand">Ve<b>la</b></span>
    {#if authenticated}
      <input
        class="search"
        type="search"
        placeholder="Search…"
        aria-label="Search your libraries"
        bind:value={searchQuery}
        onkeydown={(e) => e.key === "Enter" && runSearch()}
      />
    {/if}
    <button
      class="queuechip"
      class:has-items={queue.items.length > 0}
      class:active={queueOpen}
      title="Play queue"
      aria-label="Play queue ({queue.items.length} item{queue.items.length === 1 ? '' : 's'})"
      onclick={toggleQueue}
    >
      <Icon name="queue" size={17} />{#if queue.items.length > 0}<span class="qcount">{queue.items.length}</span>{/if}
    </button>
    <button class="gear" title="Settings" aria-label="Settings" onclick={() => (showSettings = true)}>
      <Icon name="settings" />
    </button>
  </header>

  {#if error}
    <div class="error" role="alert">{friendlyError(error)}</div>
  {/if}

  {#if showSettings}
    <Settings
      onClose={() => (showSettings = false)}
      onChanged={onSourcesChanged}
      onLinkPlex={beginLink}
      onMpvChanged={(m) => (mpvInfo = m)}
    />
  {/if}

  {#if mpvInfo && !mpvInfo.available}
    <div class="mpvbar">
      <div class="mpvtext">
        <b>mpv is required for playback</b> and wasn't found.
        {#if mpvInfo.canAutoInstall}
          Install it automatically ({mpvInfo.installDescription}), or point Vela at an existing mpv in Settings → Player.
        {:else}
          Install it, then restart Vela.
        {/if}
        {#if mpvInfo.installCommand}
          <code>{mpvInfo.installCommand}</code>
        {/if}
      </div>
      <div class="mpvactions">
        {#if mpvInfo.canAutoInstall}
          <button class="primary" disabled={installingMpv} onclick={installMpv}>
            {installingMpv ? "Installing…" : "Install mpv"}
          </button>
        {/if}
        {#if mpvInfo.installCommand}
          <button onclick={() => copyText(mpvInfo!.installCommand!)}>{copied ? "Copied!" : "Copy"}</button>
        {/if}
        <button onclick={() => (showSettings = true)}>Set path…</button>
        <button onclick={() => openExternal(mpvInfo!.installUrl)}>Get mpv</button>
      </div>
    </div>
  {/if}

  {#snippet poster(item: Item, i: number, shape: "auto" | "portrait" | "landscape" = "auto")}
    {@const landscape = shape === "landscape" || (shape === "auto" && isEpisodic(item))}
    {@const art = artFor(item, landscape)}
    {@const pct =
      item.viewOffsetMs && item.durationMs
        ? Math.round(Math.min(100, (100 * item.viewOffsetMs) / item.durationMs))
        : null}
    {@const baseName = item.grandparentTitle ?? item.title}
    {@const playable = item.mediaType !== "show" && item.mediaType !== "season"}
    {@const parts = item.grandparentTitle
      ? [
          item.grandparentTitle,
          ...(item.parentIndex != null && item.index != null
            ? [`S${item.parentIndex} · E${item.index}`]
            : []),
          item.title, // episode title (also the poster's alt / no-art text)
        ]
      : [item.title, ...(item.year != null ? [`${item.year}`] : [])]}
    {@const label = `${parts.join(" — ")}${pct !== null ? ` — ${pct}% watched` : ""}`}
    <button
      class="poster"
      class:landscape
      class:watched={item.played === true && pct === null}
      style="animation-delay: {Math.min(i, 14) * 22}ms;"
      onclick={() => open(item)}
      oncontextmenu={(e) => openMenu(e, item)}
      title={baseName}
      aria-label={label}
    >
      <div class="art">
        {#if item.played === true && pct === null}
          <!-- Fully watched: marked played AND not mid-resume (pct is the resume %). -->
          <div class="watchedbadge" aria-hidden="true"><Icon name="check" size={13} stroke={2.75} /></div>
        {/if}
        {#if art && !failedPosters.has(art)}
          <img
            src={posterSrc(art)}
            alt={item.title}
            loading="lazy"
            onerror={() => {
              failedPosters.add(art);
              failedPosters = failedPosters; // trigger reactivity → show placeholder
            }}
          />
        {:else}
          <div class="noart">{item.title}</div>
        {/if}
        {#if playable}
          <div class="playoverlay" aria-hidden="true">
            <span class="playbtn"><Icon name="play" size={20} /></span>
          </div>
        {/if}
        {#if pct !== null}
          <!-- Decorative: the percentage is exposed on the button's aria-label
               above (a progressbar role here would be flattened by the button). -->
          <div class="progress" aria-hidden="true"><div class="bar" style="width:{pct}%"></div></div>
        {/if}
      </div>
      <div class="meta">
        <span class="t">{item.grandparentTitle ?? item.title}</span>
        {#if item.grandparentTitle}
          <span class="y">{item.parentIndex != null && item.index != null ? `S${item.parentIndex} · E${item.index}` : item.title}</span>
        {:else if item.year}
          <span class="y">{item.year}</span>
        {/if}
        {#if activeType && mode === "browse" && item.sourceId}
          <!-- Merged entries: name the source, or the count when several back it. -->
          <span class="y srctag"
            >· {(item.backing?.length ?? 0) > 1
              ? `${item.backing!.length} sources`
              : sourceNameOf(item.backing?.[0]?.sourceId ?? item.sourceId)}</span
          >
        {/if}
      </div>
    </button>
  {/snippet}

  {#snippet heroFlow()}
    {@const idx = heroClamp(heroPos)}
    {@const center = heroItems[idx]}
    {@const centerEp =
      center.parentIndex != null && center.index != null
        ? `S${center.parentIndex} · E${center.index} – ${center.title}`
        : center.title}
    <section class="rail">
      <h2>Continue Watching</h2>
      <div class="flow" role="group" aria-label="Continue watching">
        {#each heroItems as it, i (it.ratingKey)}
          {@const d = i - idx}
          {@const art = it.backdrop ?? artFor(it, true)}
          {@const pct =
            d === 0 && it.viewOffsetMs && it.durationMs
              ? Math.round(Math.min(100, (100 * it.viewOffsetMs) / it.durationMs))
              : null}
          {#if Math.abs(d) <= 4}
            <!-- Older items (higher index) fan behind-left, newer behind-right. -->
            <button
              class="flowcard"
              class:center={d === 0}
              style="z-index:{30 - Math.abs(d)}; transform: translateX(calc(-50% + {d * -17}%)) rotateY({d === 0 ? 0 : d > 0 ? 18 : -18}deg) scale({d === 0 ? 1 : Math.max(0.6, 0.86 - (Math.abs(d) - 1) * 0.06)}); filter: brightness({d === 0 ? 1 : Math.max(0.35, 0.6 - (Math.abs(d) - 1) * 0.12)});"
              onclick={() => (d === 0 ? open(it) : (heroPos = i))}
              oncontextmenu={(e) => openMenu(e, it, true)}
              title={it.grandparentTitle ?? it.title}
              aria-label={d === 0 ? `Play ${it.grandparentTitle ?? it.title}` : `Show ${it.grandparentTitle ?? it.title}`}
            >
              <div class="art">
                {#if art && !failedPosters.has(art)}
                  <img
                    src={posterSrc(art)}
                    alt=""
                    onerror={() => {
                      failedPosters.add(art);
                      failedPosters = failedPosters;
                    }}
                  />
                {:else}
                  <div class="noart">{it.grandparentTitle ?? it.title}</div>
                {/if}
                {#if d === 0}
                  <div class="playoverlay" aria-hidden="true">
                    <span class="playbtn"><Icon name="play" size={24} /></span>
                  </div>
                {/if}
                {#if pct !== null}
                  <div class="progress" aria-hidden="true"><div class="bar" style="width:{pct}%"></div></div>
                {/if}
              </div>
            </button>
          {/if}
        {/each}
        {#if heroItems.length > 1}
          <!-- Always visible (hover-reveal read as "no controls" in playtest). -->
          <button class="heroarrow left" aria-label="Older" disabled={idx >= heroItems.length - 1} onclick={() => (heroPos = heroClamp(idx + 1))}>
            <Icon name="back" size={18} />
          </button>
          <button class="heroarrow right" aria-label="Newer" disabled={idx <= 0} onclick={() => (heroPos = heroClamp(idx - 1))}>
            <Icon name="chevron" size={18} />
          </button>
        {/if}
      </div>
      <div class="meta flowmeta">
        <span class="t">{center.grandparentTitle ?? center.title}</span>
        {#if center.grandparentTitle}
          <span class="y">{centerEp}</span>
        {:else if center.year}
          <span class="y">{center.year}</span>
        {/if}
        {#if activeSource === null && sources.length > 1 && center.sourceId}
          <span class="y srctag">· {sourceNameOf(center.sourceId)}</span>
        {/if}
      </div>
    </section>
  {/snippet}

  {#snippet skelCard()}
    <div class="poster skel" aria-hidden="true">
      <div class="art skel-art"></div>
      <div class="meta"><span class="skel-line" style="width:80%"></span></div>
    </div>
  {/snippet}

  {#snippet skelRails()}
    <div class="home" aria-busy="true" aria-label="Loading">
      {#each Array(3) as _, r (r)}
        <section class="rail">
          <span class="skel-line skel-title"></span>
          <div class="row">
            {#each Array(8) as _, i (i)}{@render skelCard()}{/each}
          </div>
        </section>
      {/each}
    </div>
  {/snippet}

  {#snippet skelGrid()}
    <div class="grid skelgrid" aria-busy="true" aria-label="Loading">
      {#each Array(18) as _, i (i)}{@render skelCard()}{/each}
    </div>
  {/snippet}

  <div class="shell">
    {#if authenticated && !pin}
      <!-- Library navigation lives in a left sidebar (Infuse reference):
           Home, the Library entries for the current scope, and the source
           scopes — freeing the vertical space the top nav used to take. -->
      <aside class="sidebar">
        <nav class="sidenav" aria-label="Library">
          <button class="sideitem" class:active={mode === "home"} onclick={goHome}>Home</button>
          <div class="sidegroup">Library</div>
          {#if activeSource === null && sources.length > 1}
            {#each typeTabs as t (t)}
              <button class="sideitem" class:active={mode === "browse" && activeType === t} onclick={() => selectType(t)}>
                {TYPE_LABELS[t] ?? t}
              </button>
            {/each}
          {:else}
            {#each sections as s (s.key)}
              <button class="sideitem" class:active={mode === "browse" && active?.key === s.key} onclick={() => select(s)}>
                {s.title}
              </button>
            {/each}
          {/if}
          {#if sources.length > 1}
            <div class="sidegroup">Sources</div>
            <button class="sideitem" class:active={activeSource === null} onclick={() => selectSource(null)}>All</button>
            {#each sources as src (src.id)}
              <button class="sideitem" class:active={activeSource === src.id} onclick={() => selectSource(src.id)}>{src.name}</button>
            {/each}
          {/if}
        </nav>
      </aside>
    {/if}
    <div class="content">
  {#if pin}
    <div class="link">
      <h2>Link this device</h2>
      <p class="muted">Scan with your phone, or open Plex to authorize.</p>
      {#if pin.qrSvg}
        <button class="qr" onclick={() => openExternal(pin!.authUrl)} title="Open Plex to authorize">
          <img src={pin.qrSvg} alt="Plex device-link QR code" />
        </button>
      {/if}
      <button class="primary authbtn" onclick={() => openExternal(pin!.authUrl)}>
        Open Plex to authorize
      </button>
      <p class="muted small">
        Or go to <b>plex.tv/link</b> and enter this code:
      </p>
      <div class="code">{pin.code}</div>
      <p class="muted">Waiting for you to authorize…</p>
    </div>
  {:else if !authenticated}
    <div class="empty">
      <div class="empty-icon" aria-hidden="true"><Icon name="film" size={46} stroke={1.5} /></div>
      <h2>Welcome to Vela</h2>
      <p class="muted empty-sub">
        Connect Plex, Jellyfin, or Emby to start browsing your library in HDR.
      </p>
      <button class="primary" onclick={() => (showSettings = true)}>Add a source</button>
    </div>
  {:else if detailView}
    <!-- The info surface replaces the content area but leaves home/browse
         state untouched underneath — Back returns exactly where you were. -->
    {#if detailView.kind === "item"}
      {#key detailView.item.ratingKey}
        <ItemDetail item={detailView.item} {posterSrc} onBack={closeDetail} onPlay={play} onMenu={openMenu} />
      {/key}
    {:else}
      {#key detailView.seed.ratingKey}
        <SeasonDetail
          seasonKey={detailView.seasonKey}
          seed={detailView.seed}
          initialSelKey={detailView.initialSelKey}
          {posterSrc}
          onBack={closeDetail}
          onPlay={play}
          onMenu={openMenu}
        />
      {/key}
    {/if}
  {:else if mode === "home"}
    {#if loading && hubs.length === 0 && heroItems.length === 0}
      {@render skelRails()}
    {:else if hubs.length === 0 && heroItems.length === 0}
      <!-- The hero is fed by Vela's own recents, independent of hubs: a
           local-only setup with an unfinished play must still show
           Continue Watching (2026-07-04 hero decision). -->
      <div class="muted center">Nothing on your home screen yet — pick a library from the sidebar.</div>
    {:else}
      <div class="home">
        {#if heroItems.length > 0}
          {@render heroFlow()}
        {/if}
        <!-- Hero-policy hubs feed the cover-flow above; the rest stay rows. -->
        {#each hubs.filter((h) => hubPolicy(h) !== "hero") as hub (hub.sourceId + ":" + hub.hubIdentifier)}
          {@const policy = hubPolicy(hub)}
          <section class="rail">
            <h2>{hub.title}{#if activeSource === null && sources.length > 1 && hub.sourceName}<span class="srctag"> · {hub.sourceName}</span>{/if}</h2>
            <div class="row">
              {#each hub.items as item, i (item.ratingKey)}
                {@render poster(item, i, policy === "landscape" ? "landscape" : "portrait")}
              {/each}
            </div>
          </section>
        {/each}
      </div>
    {/if}
  {:else if loading && items.length === 0}
    {@render skelGrid()}
  {:else}
    <div class="crumbs">
      <button class="back" onclick={back}><Icon name="back" size={15} /> Back</button>
      {#each crumbs as c, i (i)}
        {#if i > 0}<span class="sep"><Icon name="chevron" size={13} /></span>{/if}
        <button class="crumb" class:current={i === crumbs.length - 1} onclick={() => goCrumb(i)}>{c.title}</button>
      {/each}
      {#if (active || activeType) && crumbs.length === 1}
        <select class="sort" bind:value={sort} onchange={changeSort}>
          {#each activeType ? SORTS.filter((s) => TYPE_SORTS.has(s.key)) : SORTS as s (s.key)}
            <option value={s.key}>{s.label}</option>
          {/each}
        </select>
      {/if}
    </div>
    {#if items.length === 0}
      <div class="muted center">
        {searchTerm ? "No matches found." : "Nothing in this view yet."}
      </div>
    {:else}
      <main class="grid" bind:this={gridEl} onscroll={onScroll}>
        {#each items as item, i (item.ratingKey)}
          <!-- Search results can mix movies and episodes; force one shape there.
               Container drill-downs are naturally uniform, so "auto" stands. -->
          {@render poster(item, i, searchTerm ? "portrait" : "auto")}
        {/each}
      </main>
    {/if}
  {/if}

    </div>
  </div>

  {#if appInfo}
    <footer class="buildinfo" title="Built {appInfo.buildDate}">
      Vela v{appInfo.version} · {appInfo.buildDate} ·
      <button class="ghlink" onclick={() => openExternal(appInfo!.repoUrl)}>GitHub</button>
    </footer>
  {/if}
</div>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") {
      if (menu) closeMenu();
      else if (queueOpen) toggleQueue();
      else if (detailView) closeDetail();
    }
  }}
/>

{#if menu}
  {@const mi = menu.item}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="menubackdrop" role="presentation" onclick={closeMenu} oncontextmenu={(e) => { e.preventDefault(); closeMenu(); }}></div>
  {@const inProgress = (mi.viewOffsetMs ?? 0) > 0}
  {@const fullyWatched = mi.played === true && !inProgress}
  <div class="ctxmenu" style="left:{menu.x}px; top:{menu.y}px;" role="menu">
    <button role="menuitem" onclick={() => { closeMenu(); play(mi); }}>Play</button>
    {#if devDetail && mi.mediaType !== "show"}
      <!-- Dev-flagged until the nav flip (amended slice 3) ungates it; it
           then stays as the info path for carousel items (CW click = play). -->
      <button role="menuitem" onclick={() => openInfo(mi)}>Info</button>
    {/if}
    <button role="menuitem" onclick={() => playNext(mi)}>Play next</button>
    <button role="menuitem" onclick={() => addToQueue(mi)}>Add to queue</button>
    {#if mi.played != null && !fullyWatched}
      <button role="menuitem" onclick={() => setWatched(mi, true)}>Mark watched</button>
    {/if}
    {#if mi.played != null && (mi.played === true || inProgress)}
      <button role="menuitem" onclick={() => setWatched(mi, false)}>Mark unwatched</button>
    {/if}
    {#if menu.hero}
      <button role="menuitem" onclick={() => removeFromContinue(mi)}>Remove from Continue Watching</button>
    {/if}
    {#if (mi.backing?.length ?? 0) > 1 && mi.canonicalId}
      <!-- Merged title: pick which source plays it (persists for this title).
           Keyed by the full backing identity — sourceId alone can collide. -->
      {#each mi.backing! as b (b.sourceId + ":" + b.ratingKey)}
        <button role="menuitem" onclick={() => playFrom(mi, b)}>
          Play from {sourceNameOf(b.sourceId)}
        </button>
      {/each}
    {/if}
  </div>
{/if}

{#if queueOpen}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="drawerbackdrop" role="presentation" onclick={toggleQueue}></div>
  <aside class="drawer" aria-label="Play queue">
    <header class="drawerhead">
      <div class="drawertitle">Up Next{#if queue.items.length > 0}<span class="qcount inline">{queue.items.length}</span>{/if}</div>
      <div class="drawerhead-actions">
        {#if queue.items.length > 0}
          <button class="drawerlink" onclick={queueClearAll}>Clear</button>
        {/if}
        <button class="drawerclose" aria-label="Close queue" onclick={toggleQueue}><Icon name="close" size={16} /></button>
      </div>
    </header>
    {#if queue.items.length === 0}
      <div class="drawerempty">Nothing queued. Right-click an item to add it here.</div>
    {:else}
      <ol class="drawerlist">
        {#each queue.items as qi, i (qi.ratingKey + ":" + i)}
          <li class="drawerrow" class:current={i === queue.currentIndex}>
            <button class="drawerplay" title="Play this item" onclick={() => queueJumpTo(i)}>
              {#if qi.poster}
                <img class="drawerthumb" src={posterSrc(qi.poster)} alt="" loading="lazy" onerror={(e) => { (e.currentTarget as HTMLImageElement).style.visibility = 'hidden'; }} />
              {:else}
                <div class="drawerthumb noart small">{qi.title}</div>
              {/if}
              <div class="drawerinfo">
                <div class="drawerinfotitle">{qi.title}</div>
                {#if qi.subtitle}<div class="drawerinfosub">{qi.subtitle}</div>{/if}
              </div>
            </button>
            <button class="drawerremove" title="Remove from queue" aria-label="Remove from queue" onclick={() => queueRemove(i)}><Icon name="close" size={15} /></button>
          </li>
        {/each}
      </ol>
    {/if}
  </aside>
{/if}

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
  }
  /* Small, fixed build-info tag in the bottom-right — easy to find, out of the way. */
  .buildinfo {
    position: fixed;
    right: 0.6rem;
    bottom: 0.45rem;
    z-index: 20;
    font-size: 0.7rem;
    line-height: 1;
    color: var(--text-muted);
    background: var(--bg-blur);
    padding: 0.3rem 0.55rem;
    border-radius: 0.4rem;
    pointer-events: auto;
    user-select: none;
  }
  .buildinfo .ghlink {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--text-2);
    text-decoration: underline;
    cursor: pointer;
  }
  .buildinfo .ghlink:hover {
    color: var(--accent);
    text-decoration: underline;
  }
  header {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    padding: 0.75rem 1.25rem;
    border-bottom: 1px solid var(--border-subtle);
    position: sticky;
    top: 0;
    background: var(--bg-blur);
    backdrop-filter: blur(8px);
    z-index: 10;
  }
  .brand {
    font-family: var(--font-display);
    font-size: 1.3rem;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .brand b {
    color: var(--accent);
    font-weight: 700;
  }
  /* Left library sidebar (Infuse reference): Home / Library / Sources. */
  .shell {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .sidebar {
    flex: 0 0 212px;
    width: 212px;
    overflow-y: auto;
    border-right: 1px solid var(--border-subtle);
    padding: 0.8rem 0.55rem 1.2rem;
  }
  .content {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .sidenav {
    display: flex;
    flex-direction: column;
    gap: 0.12rem;
  }
  .sidegroup {
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-dim);
    margin: 0.95rem 0.65rem 0.3rem;
    user-select: none;
  }
  .sideitem {
    background: transparent;
    border: none;
    color: var(--text-muted);
    text-align: left;
    width: 100%;
    padding: 0.42rem 0.65rem;
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.92rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sideitem:hover {
    color: var(--text-bright);
    background: var(--surface);
  }
  .sideitem.active {
    color: var(--text-bright);
    background: var(--surface-2);
  }
  .search {
    margin-left: auto;
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 0.4rem 0.7rem;
    border-radius: 8px;
    font-size: 0.9rem;
    width: 200px;
  }
  .srctag {
    color: var(--text-dim);
    font-weight: 400;
    font-size: 0.85em;
  }
  .gear {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.35rem;
    border-radius: 0.45rem;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition:
      color 0.15s var(--ease),
      background 0.15s var(--ease);
  }
  .gear:hover {
    color: var(--text-bright);
    background: var(--border-subtle);
  }
  .search:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px rgba(229, 160, 13, 0.15);
  }
  .crumbs {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.7rem 1.25rem 0;
    flex-wrap: wrap;
  }
  .crumbs .sort {
    margin-left: auto;
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text-2);
    padding: 0.3rem 0.5rem;
    border-radius: 7px;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .crumbs .back {
    background: var(--surface);
    border: none;
    color: var(--text-2);
    padding: 0.3rem 0.7rem 0.3rem 0.55rem;
    border-radius: 7px;
    cursor: pointer;
    margin-right: 0.5rem;
    font-size: 0.85rem;
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
  }
  .crumbs .back:hover {
    background: var(--surface-2);
    color: var(--text-bright);
  }
  .crumbs .crumb {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.9rem;
    padding: 0.2rem 0.1rem;
  }
  .crumbs .crumb:hover {
    color: var(--text-bright);
  }
  .crumbs .crumb.current {
    color: var(--text);
    font-weight: 600;
    cursor: default;
  }
  .crumbs .sep {
    color: var(--text-dim);
    display: inline-flex;
    align-items: center;
  }
  .grid {
    flex: 1;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(116px, 150px));
    justify-content: start;
    gap: 0.95rem;
    padding: 1.25rem;
    /* Extra bottom clearance so the last row scrolls clear of the fixed footer. */
    padding-bottom: 3rem;
    align-content: start;
  }
  .poster {
    appearance: none;
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
    text-align: left;
    color: inherit;
    font: inherit;
    line-height: normal;
    display: flex;
    flex-direction: column;
    gap: 0.38rem;
    width: 100%;
    max-width: 150px;
    min-width: 0;
    transition: transform 0.15s var(--ease);
    animation: vela-rise 0.4s var(--ease) backwards;
  }
  .poster.landscape {
    max-width: 210px;
  }
  .poster:hover {
    transform: translateY(-4px);
  }
  .poster:hover .art {
    box-shadow: var(--shadow-card-hover);
    border-color: var(--border-strong);
    transform: scale(1.03);
  }
  .poster:focus-visible {
    outline: none;
  }
  .poster:focus-visible .art {
    border-color: var(--accent);
    box-shadow: var(--shadow-card), 0 0 0 2px var(--accent);
  }
  .art {
    position: relative;
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-card);
    transition:
      box-shadow 0.18s var(--ease),
      border-color 0.18s var(--ease),
      transform 0.2s var(--ease);
  }
  .playoverlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.45), rgba(0, 0, 0, 0.05) 55%);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.18s var(--ease);
  }
  .playbtn {
    width: 2.9rem;
    height: 2.9rem;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    display: flex;
    align-items: center;
    justify-content: center;
    padding-left: 0.15rem;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.45);
    transform: scale(0.82);
    transition: transform 0.18s var(--ease);
  }
  .poster:hover .playoverlay,
  .poster:focus-visible .playoverlay {
    opacity: 1;
  }
  .poster:hover .playbtn,
  .poster:focus-visible .playbtn {
    transform: scale(1);
  }
  .poster.landscape .art {
    aspect-ratio: 16 / 9;
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .noart {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0.6rem;
    font-size: 0.82rem;
    font-weight: 600;
    line-height: 1.3;
    color: var(--text-2);
    text-align: center;
    background: linear-gradient(150deg, var(--surface-2), var(--surface-sunken));
  }
  .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 5px;
    background: rgba(0, 0, 0, 0.5);
  }
  .progress .bar {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-hover));
  }
  /* Continue Watching cover-flow: the centered card is capped at ~30% of the
     window height; older items fan behind-left, newer behind-right
     (foobar2000 reference). Arrows are ALWAYS visible — hover-reveal read as
     "no controls" in the owner playtest. */
  .flow {
    position: relative;
    /* 30vh cap, but a 16:9 card must also fit narrow windows. */
    height: min(30vh, 46vw);
    perspective: 1200px;
    overflow: hidden;
  }
  .flowcard {
    appearance: none;
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
    color: inherit;
    font: inherit;
    position: absolute;
    top: 0;
    left: 50%;
    height: 100%;
    aspect-ratio: 16 / 9;
    transition:
      transform 0.32s var(--ease),
      filter 0.32s var(--ease);
  }
  .flowcard .art {
    width: 100%;
    height: 100%;
    aspect-ratio: auto;
  }
  .flowcard .progress {
    height: 6px;
  }
  .flowcard:focus-visible {
    outline: none;
  }
  .flowcard:focus-visible .art {
    border-color: var(--accent);
    box-shadow: var(--shadow-card), 0 0 0 2px var(--accent);
  }
  .flowcard.center:hover .art {
    box-shadow: var(--shadow-card-hover);
    border-color: var(--border-strong);
  }
  .flowcard.center:hover .playoverlay,
  .flowcard.center:focus-visible .playoverlay {
    opacity: 1;
  }
  .flowcard.center:hover .playbtn,
  .flowcard.center:focus-visible .playbtn {
    transform: scale(1);
  }
  .heroarrow {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    z-index: 40;
    width: 2.3rem;
    height: 2.3rem;
    border-radius: 50%;
    border: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.55);
    color: #fff;
    cursor: pointer;
    padding: 0;
    transition: background 0.15s var(--ease);
  }
  .heroarrow.left {
    left: 0.6rem;
  }
  .heroarrow.right {
    right: 0.6rem;
  }
  .heroarrow:hover {
    background: rgba(0, 0, 0, 0.75);
  }
  .heroarrow:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .flowmeta {
    align-items: center;
    text-align: center;
    margin-top: 0.5rem;
  }
  /* Home rails */
  .home {
    flex: 1;
    overflow-y: auto;
    padding: 0.25rem 1.25rem 2.5rem;
  }
  .rail {
    margin-top: 1.6rem;
  }
  .rail h2 {
    font-size: 1.24rem;
    font-weight: 700;
    letter-spacing: -0.02em;
    margin: 0 0 0.85rem;
  }
  .row {
    display: flex;
    gap: 0.8rem;
    overflow-x: auto;
    padding-bottom: 0.6rem;
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
  }
  .row:hover {
    scrollbar-color: var(--border-strong) transparent;
  }
  .row::-webkit-scrollbar {
    height: 6px;
  }
  .row::-webkit-scrollbar-thumb {
    background: transparent;
    border-radius: 3px;
  }
  .row:hover::-webkit-scrollbar-thumb {
    background: var(--border-strong);
  }
  .row .poster {
    width: 118px;
    max-width: 118px;
    flex: 0 0 118px;
    /* Without this, a flex item's min-width:auto = content min-size, so a wide
       landscape episode thumbnail overrides the 140px and blows the row up. */
    min-width: 0;
  }
  .row .poster.landscape {
    width: 190px;
    max-width: 190px;
    flex-basis: 190px;
  }
  .meta {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
  }
  .meta .t {
    font-size: 0.85rem;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta .y {
    font-size: 0.75rem;
    color: var(--text-muted);
  }
  .link {
    margin: auto;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.4rem;
  }
  .empty {
    margin: auto;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.9rem;
  }
  .empty-icon {
    color: var(--accent);
    line-height: 0;
    margin-bottom: 0.2rem;
  }
  .empty h2 {
    margin: 0;
    color: var(--text);
    font-size: 1.5rem;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .empty-sub {
    margin: 0;
    max-width: 24rem;
    line-height: 1.5;
  }

  /* Loading skeletons */
  .skel-art {
    border: 1px solid var(--border);
    background: linear-gradient(90deg, var(--surface) 25%, var(--surface-2) 37%, var(--surface) 63%);
    background-size: 200% 100%;
    animation: vela-shimmer 1.3s linear infinite;
  }
  .skel-line {
    display: block;
    height: 0.72rem;
    border-radius: 4px;
    background: linear-gradient(90deg, var(--surface) 25%, var(--surface-2) 37%, var(--surface) 63%);
    background-size: 200% 100%;
    animation: vela-shimmer 1.3s linear infinite;
  }
  .skel-title {
    width: 150px;
    height: 1.05rem;
    margin-bottom: 0.7rem;
  }
  .poster.skel {
    cursor: default;
  }
  .poster.skel:hover {
    transform: none;
  }
  .qr {
    background: #fff;
    padding: 10px;
    border: none;
    border-radius: 12px;
    cursor: pointer;
    line-height: 0;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.35);
    transition: transform 0.1s ease;
  }
  .qr:hover {
    transform: scale(1.02);
  }
  .qr img {
    width: 200px;
    height: 200px;
    display: block;
  }
  .authbtn {
    margin: 0.5rem 0 0.2rem;
  }
  .code {
    font-size: 2.5rem;
    letter-spacing: 0.4rem;
    font-weight: 800;
    color: var(--accent);
    margin: 0.4rem 0 0.8rem;
  }
  .muted {
    color: var(--text-muted);
  }
  .small {
    font-size: 0.85rem;
  }
  button.primary {
    background: var(--accent);
    color: var(--on-accent);
    border: none;
    border-radius: 6px;
    padding: 0.55rem 1.1rem;
    font-weight: 700;
    cursor: pointer;
    transition:
      background 0.15s var(--ease),
      transform 0.08s var(--ease);
  }
  button.primary:hover {
    background: var(--accent-hover);
  }
  button.primary:active {
    transform: translateY(1px);
  }
  .mpvbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
    background: var(--warn-bg);
    color: var(--warn-text);
    padding: 0.6rem 1rem;
    font-size: 0.9rem;
    border-bottom: 1px solid var(--warn-border);
    animation: vela-slide-down 0.2s var(--ease);
  }
  .mpvbar code {
    display: inline-block;
    margin-left: 0.5rem;
    background: #00000040;
    padding: 0.15rem 0.45rem;
    border-radius: 4px;
    font-family: ui-monospace, monospace;
    color: var(--warn-text);
  }
  .mpvactions {
    display: flex;
    gap: 0.5rem;
    flex-shrink: 0;
  }
  .mpvactions button {
    background: #00000030;
    color: var(--warn-text);
    border: 1px solid var(--warn-border);
    border-radius: 6px;
    padding: 0.4rem 0.9rem;
    cursor: pointer;
  }
  .mpvactions button.primary {
    background: var(--accent);
    color: var(--on-accent);
    border: none;
  }
  .center {
    margin: auto;
  }
  .error {
    background: var(--danger-bg);
    color: var(--danger-text);
    padding: 0.6rem 1rem;
    font-size: 0.85rem;
    animation: vela-slide-down 0.2s var(--ease);
  }

  /* Watched indicator + dimming */
  .watchedbadge {
    position: absolute;
    top: 0.35rem;
    right: 0.35rem;
    z-index: 2;
    width: 1.25rem;
    height: 1.25rem;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.5);
  }
  .poster.watched .art img,
  .poster.watched .noart {
    opacity: 0.55;
  }

  /* Right-click context menu */
  .menubackdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
  }
  .ctxmenu {
    position: fixed;
    z-index: 41;
    min-width: 11rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    padding: 0.3rem;
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.55);
    display: flex;
    flex-direction: column;
    transform-origin: top left;
    animation: vela-pop 0.13s var(--ease);
  }
  .ctxmenu button {
    background: none;
    border: none;
    color: var(--text);
    text-align: left;
    padding: 0.5rem 0.7rem;
    border-radius: 0.35rem;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .ctxmenu button:hover {
    background: var(--surface-2);
  }

  /* Header queue chip */
  .queuechip {
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-2);
    font: inherit;
    font-size: 0.9rem;
    padding: 0.3rem 0.55rem;
    border-radius: 0.45rem;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }
  .queuechip:hover {
    background: var(--border-subtle);
  }
  .queuechip.has-items {
    color: var(--text-bright);
  }
  .queuechip.active {
    background: var(--border-subtle);
    border-color: var(--border);
  }
  .qcount {
    background: var(--accent);
    color: var(--on-accent);
    font-size: 0.7rem;
    font-weight: 700;
    border-radius: 0.7rem;
    padding: 0.05rem 0.4rem;
    line-height: 1.1;
  }
  .qcount.inline {
    margin-left: 0.5rem;
  }

  /* Queue drawer */
  .drawerbackdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
    background: rgba(0, 0, 0, 0.25);
    animation: vela-fade 0.16s var(--ease);
  }
  .drawer {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: 360px;
    max-width: 92vw;
    z-index: 31;
    background: var(--surface-sunken);
    border-left: 1px solid var(--border);
    box-shadow: -10px 0 30px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    animation: vela-slide-right 0.22s var(--ease);
  }
  .drawerhead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.85rem 1rem;
    border-bottom: 1px solid var(--border-subtle);
  }
  .drawertitle {
    font-weight: 700;
    color: var(--text);
    display: inline-flex;
    align-items: center;
  }
  .drawerhead-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .drawerlink {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font: inherit;
    font-size: 0.85rem;
    padding: 0.2rem 0.4rem;
    border-radius: 0.35rem;
  }
  .drawerlink:hover {
    background: var(--border-subtle);
  }
  .drawerclose {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.35rem;
    border-radius: 0.35rem;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .drawerclose:hover {
    color: var(--text-bright);
    background: var(--border-subtle);
  }
  .drawerempty {
    padding: 1.25rem 1rem;
    color: var(--text-muted);
    font-size: 0.85rem;
  }
  .drawerlist {
    list-style: none;
    margin: 0;
    padding: 0.5rem;
    overflow-y: auto;
    flex: 1;
  }
  .drawerrow {
    display: flex;
    align-items: stretch;
    gap: 0.4rem;
    margin: 0;
    padding: 0;
  }
  .drawerrow + .drawerrow {
    margin-top: 0.25rem;
  }
  .drawerplay {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    background: none;
    border: 1px solid transparent;
    border-radius: 0.5rem;
    padding: 0.4rem 0.5rem;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    min-width: 0;
  }
  .drawerplay:hover {
    background: var(--border-subtle);
  }
  .drawerrow.current .drawerplay {
    border-color: var(--accent);
    background: rgba(229, 160, 13, 0.08);
  }
  .drawerthumb {
    width: 64px;
    height: 36px;
    object-fit: cover;
    border-radius: 0.3rem;
    background: var(--surface);
    flex-shrink: 0;
  }
  .drawerthumb.noart.small {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 0.65rem;
    padding: 0.2rem;
    text-align: center;
    overflow: hidden;
  }
  .drawerinfo {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  .drawerinfotitle {
    font-size: 0.88rem;
    font-weight: 600;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .drawerinfosub {
    font-size: 0.75rem;
    color: var(--text-muted);
  }
  .drawerremove {
    background: none;
    border: none;
    color: var(--text-dim);
    cursor: pointer;
    padding: 0 0.45rem;
    border-radius: 0.35rem;
    display: inline-flex;
    align-items: center;
  }
  .drawerremove:hover {
    color: var(--danger-text);
    background: var(--border-subtle);
  }
</style>
