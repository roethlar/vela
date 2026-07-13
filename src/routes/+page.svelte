<script lang="ts">
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy, tick } from "svelte";
  import Settings from "$lib/Settings.svelte";
  import Icon from "$lib/Icon.svelte";
  import ItemDetail from "$lib/ItemDetail.svelte";
  import SeasonDetail from "$lib/SeasonDetail.svelte";
  import { detailKeyOf, type Detail, type Item } from "$lib/types";

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

  type Section = { key: string; title: string; sectionType: string; sourceName?: string; sort?: string };
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

  // `showOnly` sorts exist only for show sections (server-side semantics:
  // Plex `episode.addedAt`, JF `DateLastContentAdded`); the select filters
  // them out elsewhere and select() resets a leaked one on section switch.
  const SORTS: { key: string; label: string; showOnly?: boolean }[] = [
    { key: "titleSort:asc", label: "Title (A–Z)" },
    { key: "year:desc", label: "Year (newest)" },
    { key: "addedAt:desc", label: "Recently added" },
    { key: "episodeAddedAt:desc", label: "Last episode added", showOnly: true },
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
      // Refresh the visible listing so its progress bars / played badges
      // update. The person root re-runs its own query, gated to the ROOT
      // level (plan-review r2): a drilled level under it refreshes through
      // resetAndLoad, whose crumb has a ratingKey.
      if (searchTerm) runSearch(searchTerm);
      else if (personView && crumbs.length === 1) runPersonView(personView);
      else resetAndLoad();
    }
  }

  onMount(() => {
    listen("playback-ended", refreshWatchState).then((un) => (unlistenPlaybackEnded = un));
  });

  async function boot() {
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
      // loadEverything() resets the view underneath the user — that IS
      // navigation. Without the bump, a refresh still in flight keeps owning
      // the epoch, so its gate would go on blocking the empty-Home redirect
      // (and swallowing errors) over a view it no longer has anything to do
      // with (codex r6).
      navEpoch++;
      await loadEverything();
    } else {
      // Last source removed — clear stale content and show the neutral empty state.
      sourceGen++; // discard any in-flight section load
      homeGen++; // and any in-flight home load
      loadGen++; // and any in-flight browse/search load
      navEpoch++; // and any pending refresh reconciliation
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
      // Deferral (library-refresh-scan plan): never redirect while a detail
      // is open — the forced-Home cleanup can land UNDER an open detail, and
      // select() would close it and open another library beneath the user.
      // Once the detail closes this effect re-evaluates and may fire.
      detailView === null &&
      // Never redirect MID-REFRESH. The refresh's Home leg deliberately does
      // not raise `loading` (a refresh must not blank the UI), so without this
      // the sections leg landing first — with a newly added library — fires the
      // redirect while the Home leg is still in flight: the user is thrown into
      // a grid instead of the rails that were about to arrive, and a Home leg
      // that then FAILS is discarded silently (its generation was superseded by
      // resetAndLoad). The action re-evaluates this effect when it settles, so a
      // Home that really is empty still redirects (codex r3). Deliberately NOT
      // `loading`: that would restore the skeleton flash the plan forbids.
      !(refreshing && refreshEpoch === navEpoch) &&
      activeSource !== null &&
      !loading &&
      hubs.length === 0 &&
      heroItems.length === 0 &&
      sections.length > 0
    ) {
      // select() sets mode = "browse" synchronously, so the condition is false
      // on the next run — no loop, no double-open. `auto`: this is the app
      // navigating, not the user (lrs-1).
      select(sections[0], { auto: true });
    }
  });

  // Section (nav) loads are invalidated only by a source switch (`sourceGen`);
  // home/hub loads are also invalidated by leaving home for a browse/search
  // (`homeGen`), so a stale hub response can't clear `loading` mid-browse — but
  // a pending section refresh survives a browse and still populates the tabs.
  let sourceGen = 0;
  let homeGen = 0;

  // Bumped by EVERY user navigation — select/selectType, goHome, running a
  // search, opening a person view, drilling into children, opening or closing
  // the detail surface, and source switches. The existing generations don't
  // cover all of navigation (in-source navigation bumps `loadGen` but not
  // `sourceGen`; detail open/close bumps none of them), and a delayed refresh
  // outcome must neither force Home nor publish a banner underneath a view
  // the user navigated to meanwhile (library-refresh-scan plan, slice 1).
  // `$state` because the empty-Home effect READS it: its gate short-circuits on
  // `refreshing && refreshEpoch === navEpoch`, so if navEpoch were a plain let,
  // the effect would register no dependency on navigation and would not re-run
  // when the user switched away from the refreshing root — leaving the redirect
  // asleep until the refresh settled, which is the very stranding the scoped
  // gate exists to prevent (codex r5).
  let navEpoch = $state(0);

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
    navEpoch++; // navigation: a delayed refresh outcome must not land on the new root
    detailView = null;
    loadGen++; // invalidate any in-flight browse load so it can't append after we leave
    loadingMore = false;
    loading = false; // a stale browse load won't clear this (its gen is stale); do it here
    error = null; // don't carry a browse/search error banner onto Home
    searchTerm = "";
    personView = null;
    activeType = null;
    mode = "home";
    // Entering a browse earlier may have discarded an in-flight hub load (via the
    // homeGen bump). If we have no hubs, re-fetch so Home isn't stuck empty.
    if (hubs.length === 0) loadHome(++homeGen);
  }

  // ---- Library refresh (library-refresh-scan plan, slice 1) ----------------
  // One user action that refreshes the section list AND the content the user
  // is looking at. Legs are ACTION-LOCAL: no leg writes or clears the shared
  // `error` banner; the action aggregates and publishes ONCE at settlement,
  // gated on the action-start `navEpoch` (navigation wins) and each failed
  // leg's claimed generation (a leg superseded by a newer same-root load
  // contributes neither data nor failure).
  let refreshing = $state(false);
  // Which navigation epoch the in-flight refresh belongs to. The empty-Home
  // redirect must yield to a refresh that owns the CURRENT root — but not to
  // one the user has already navigated away from: a slow source A refresh
  // would otherwise keep blocking source B's auto-open and Refresh control for
  // A's whole timeout, stranding a healthy source behind an unrelated one
  // (codex r5).
  let refreshEpoch = $state(-1);
  // A grid-root refresh owns the error banner for its duration (see below).
  let gridActionActive = $state(false);
  // The listing generation that was current when the action started: it may
  // silence THOSE loads (the ones it orphans), never newer ones.
  let gridActionBaseGen = $state(0);

  type RootKind = "home" | "section-grid" | "type-grid" | "search" | "person" | "drill" | "detail";

  // What the user is actually looking at — derived from visible state, never
  // residual state: goHome() leaves `active` set, and a search retains
  // `activeType`, so "has an active section/type" does not mean "is looking
  // at that grid".
  function visibleRootKind(): RootKind {
    if (detailView) return "detail";
    if (mode === "home") return "home";
    if (personView) return "person";
    if (searchTerm) return "search";
    const here = crumbs[crumbs.length - 1];
    if (here?.ratingKey) return "drill";
    if (active) return "section-grid";
    if (activeType) return "type-grid";
    // Browse mode always has one of the roots above; degrade to the
    // no-content-leg treatment if a new root family ever misses this map.
    return "drill";
  }

  // The browse root RIGHT NOW — visible, or hidden under an open detail —
  // when (and only when) it is a bare section grid, the one root the
  // disappearance fallback may reconcile. Home/search/person/drill roots
  // never qualify.
  function currentSectionRootKey(): string | null {
    if (mode === "home" || personView || searchTerm || !active) return null;
    const here = crumbs[crumbs.length - 1];
    return here?.ratingKey ? null : active.key;
  }

  // Forced-Home reconciliation for a browse root whose library disappeared
  // from a complete single-source sections response: goHome()'s reset MINUS
  // the detail surface (never touched — closing it must reveal Home, not the
  // orphan) and MINUS its hubs-empty conditional re-fetch, plus exactly one
  // unconditional Home re-fetch: cached rails may still feature the removed
  // library.
  function forceHomeForRemovedRoot() {
    navEpoch++; // the root changes: any pending refresh publication yields
    loadGen++; // invalidate any in-flight browse load
    loadingMore = false;
    loading = false;
    searchTerm = "";
    personView = null;
    activeType = null;
    mode = "home";
    loadHome(++homeGen);
  }

  async function refreshLibraries() {
    if (refreshing) return; // double-fire guard; correctness comes from the gens
    refreshing = true;
    try {
      // (a) the action owns its status: clear any prior banner immediately.
      error = null;
      // Snapshot what this action reconciles against.
      const epoch = navEpoch;
      refreshEpoch = epoch;
      const kind = visibleRootKind();
      const rootKey = kind === "section-grid" ? active!.key : null;
      // The fallback needs a COMPLETE sections response: a single-source
      // fetch either errors or returns that source's complete list. A merged
      // aggregate is partial by design (failing sources are skipped), so
      // absence proves nothing and no fallback may run there.
      const singleSource = activeSource !== null || sources.length === 1;
      // Failures aggregate action-locally; each records whether its leg's
      // claimed generation is STILL current at publication time — a
      // superseded leg contributes neither data nor failure.
      const legFailures: { msg: string; current: () => boolean }[] = [];

      // A grid-root action OWNS the status surface for its duration: an
      // ordinary listing load in flight (or one a scroll starts meanwhile)
      // publishes its failures DIRECTLY (loadMore's `else error = ...` path),
      // which would land a false error over the cards this action is about to
      // load — and the action, having no failure of its own, would have
      // nothing to clear it with (codex r3). Suppressing that publish is all
      // that was ever needed; the load itself is left to run, so nothing is
      // stranded or discarded (codex r5).
      gridActionActive = kind === "section-grid" || kind === "type-grid";
      // Only loads that already existed when we clicked are ours to silence. A
      // NEWER same-root load (playback-ended -> refreshWatchState -> resetAndLoad)
      // claims a higher generation, and OUR leg is the one that will be dropped
      // as stale — so swallowing its failure too would leave an empty grid with
      // no banner at all (codex r8).
      gridActionBaseGen = loadGen;

      // Sections leg (always). The swap is `sourceGen`-gated only — a fresher
      // section list is valid regardless of navigation. Unlike
      // loadEverything(), never blank `sections` first: a refresh must not
      // flash the sidebar empty.
      const sg = ++sourceGen;
      const sectionsLeg = (async (): Promise<Section[] | null> => {
        try {
          const s = await invoke<Section[]>("get_sections", { sourceId: activeSource });
          if (sg !== sourceGen) return null;
          sections = s;
          // Disappearance fallback — gated on settlement-time ROOT IDENTITY,
          // never the epoch: still rooted (bare grid, possibly under an open
          // detail) on a key missing from a complete refreshed list →
          // reconcile to Home; navigated elsewhere meanwhile → untouched.
          if (singleSource) {
            const rootNow = currentSectionRootKey();
            if (rootNow !== null && !s.some((sec) => sec.key === rootNow)) {
              forceHomeForRemovedRoot();
            }
          }
          return s;
        } catch (e) {
          if (sg === sourceGen) {
            legFailures.push({ msg: String(e), current: () => sg === sourceGen });
          }
          return null;
        }
      })();

      // Content leg — exactly one, chosen by the snapshot's visible root.
      let contentLeg: Promise<void> = Promise.resolve();
      if (kind === "home") {
        // Claim a Home generation like every Home load: an unclaimed leg
        // would let an older in-flight Home load overwrite the refreshed
        // rails — or publish its stale failure — after settlement, and a
        // newer same-root load (e.g. playback-ended) must supersede US.
        const hg = ++homeGen;
        contentLeg = (async () => {
          try {
            const [h, r, t] = await Promise.all([
              invoke<Hub[]>("get_hubs", { sourceId: activeSource }),
              invoke<Item[]>("get_recents").catch(() => [] as Item[]),
              invoke<string[]>("get_continue_tombstones").catch(() => [] as string[]),
            ]);
            if (hg === homeGen && epoch === navEpoch) {
              hubs = h;
              recents = r;
              continueTombstones = t;
            }
          } catch (e) {
            legFailures.push({ msg: String(e), current: () => hg === homeGen });
          } finally {
            // Claiming `homeGen` above orphans any in-flight plain Home load —
            // its finally is gen-gated and can no longer clear `loading`. This
            // leg owns the flag now and must release it, or a load pending at
            // click time strands the skeleton and blocks the empty-Home
            // redirect, which is gated on `!loading` (codex code review r1,
            // finding 1).
            if (hg === homeGen) loading = false;
          }
        })();
      } else if (kind === "section-grid" || kind === "type-grid") {
        // Grid roots reload from offset zero AFTER the current-generation
        // sections response lands (on a grid root the content leg DEPENDS on
        // the sections result), REPLACING the items — the reset half of the
        // listing machinery. `navEpoch`-gated: navigation meanwhile wins.
        //
        // The generation is claimed HERE, not at the click. Claiming it early
        // (the r3-2 design) invalidated whatever listing was already in
        // flight — and when this leg then returned early because the sections
        // fetch FAILED, that listing's result was discarded with nothing to
        // replace it: the library rendered as EMPTY ("Nothing in this view
        // yet"), unable to paginate, until the user navigated away (codex r5).
        // The in-flight load is left alone; it populates the grid normally,
        // and if this leg does reach its reset, the older generation is
        // discarded then — the machinery's ordinary behavior. What the early
        // claim was really for — an orphaned load publishing its own failure
        // banner over the action's result — is handled by `gridActionActive`
        // below instead, which costs nothing and breaks nothing.
        contentLeg = (async () => {
          let myGen = 0;
          try {
            const list = await sectionsLeg;
            if (list === null) return; // sections failed or superseded
            if (epoch !== navEpoch) return; // navigation wins
            if (kind === "section-grid" && !list.some((sec) => sec.key === rootKey)) {
              return; // root gone: the disappearance fallback owns this outcome
            }
            myGen = ++loadGen; // NOW we own the grid: discard any older load
            loadingMore = false;
            offset = 0;
            hasMore = true;
            items = [];
            failedPosters = new Set();
            loading = true;
            await loadMore(myGen, (msg) => {
              legFailures.push({ msg, current: () => myGen === loadGen });
            });
          } finally {
            // Only release what we claimed: an early return never touched the
            // in-flight load, which still owns (and will clear) these flags.
            if (myGen && myGen === loadGen) {
              loading = false;
              loadingMore = false;
            }
          }
        })();
      }
      // search/person/drill/detail roots: sidebar only — those views are
      // query-scoped, not library-list-scoped; reloading here would e.g.
      // replace filtered search results with a full type listing.

      await Promise.all([sectionsLeg, contentLeg]);

      // (b)+(c)+(d): publish the aggregate ONCE — only if the user hasn't
      // navigated since the click, and only failures whose leg is still the
      // current claimant of its generation.
      if (epoch === navEpoch) {
        const live = legFailures.filter((f) => f.current());
        if (live.length > 0) error = live.map((f) => f.msg).join("; ");
      }
    } finally {
      refreshing = false;
      refreshEpoch = -1;
      gridActionActive = false;
      gridActionBaseGen = 0;
    }
  }

  // Bumped on each begin/link so a superseded poll loop stops touching the
  // (global) pin — no duplicate polling or stale errors from an old attempt.
  let linkGen = 0;

  async function beginLink() {
    const gen = ++linkGen;
    // Linking REPLACES the visible root with the device-code screen, and its
    // completion calls loadEverything() — both reset the view underneath any
    // refresh already in flight. That is navigation: without the bump the
    // obsolete action still owns the epoch, so it could publish its old view's
    // error over the link screen, or keep the empty-Home redirect blocked after
    // the new source lands (codex r7).
    navEpoch++;
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
        navEpoch++; // the linked source resets the view (see beginLink)
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

  // `auto`: the APP is navigating (the empty-Home redirect below), not the
  // user. It must not bump `navEpoch` and must not clear `error`, or a
  // refresh whose sections leg failed while its Home leg came back empty
  // would redirect the user into a grid built from the STALE section list
  // and then read its own redirect as "the user navigated" — suppressing the
  // failure banner that explains the empty Home (codex code review r2,
  // lrs-1). Navigation-wins (contract (d)) is about USER navigation.
  async function select(section: Section, { auto = false }: { auto?: boolean } = {}) {
    if (!auto) navEpoch++; // navigation (see navEpoch)
    detailView = null;
    mode = "browse";
    searchTerm = "";
    personView = null;
    active = section;
    activeType = null;
    // Entering a library sets its sort deterministically: the persisted
    // per-library preference when valid for this section's type, else the
    // default. This also guarantees a show-only sort can never leak in from
    // the previously viewed section (the reset discipline selectType()
    // applies via TYPE_SORTS).
    const saved = SORTS.find((s) => s.key === section.sort);
    sort =
      saved && (!saved.showOnly || section.sectionType === "show")
        ? saved.key
        : "titleSort:asc";
    crumbs = [{ title: section.title, ratingKey: null }];
    await resetAndLoad({ keepError: auto });
  }

  // Open a consolidated content-type listing (All view's Library).
  async function selectType(t: string) {
    navEpoch++; // navigation (see navEpoch)
    detailView = null;
    mode = "browse";
    searchTerm = "";
    personView = null;
    active = null;
    activeType = t;
    if (!TYPE_SORTS.has(sort)) sort = "titleSort:asc";
    crumbs = [{ title: TYPE_LABELS[t] ?? t, ratingKey: null }];
    await resetAndLoad();
  }

  // Bumped on every navigation; in-flight loads from an older generation are discarded.
  let loadGen = 0;

  async function resetAndLoad({ keepError = false }: { keepError?: boolean } = {}) {
    homeGen++; // leaving home: invalidate any in-flight home/sections load
    const myGen = ++loadGen;
    loadingMore = false; // abandon any in-flight load (its results are now stale)
    offset = 0;
    hasMore = true;
    items = [];
    failedPosters = new Set(); // bounded to the current view's posters
    loading = true;
    // An auto-redirect keeps the banner: the refresh action publishes its
    // aggregate AFTER both legs settle, and Svelte's effect flush may land
    // either side of that — clearing here would race the publish away
    // (lrs-1). A user-driven select still clears, as before.
    if (!keepError) error = null;
    await loadMore(myGen);
    if (myGen === loadGen) loading = false;
  }

  // Load the next page for the current level (section root or a parent's children)
  // and append it. Drives infinite scroll. Discards results if navigation moved on.
  async function loadMore(myGen: number = loadGen, onError: ((msg: string) => void) | null = null) {
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
        // The refresh action aggregates its legs' failures action-locally
        // (library-refresh-scan plan); navigation loads keep the direct publish.
        if (onError) onError(String(e));
        // A grid-root refresh owns the banner while it runs: this load's own
        // failure must not land over the result the action is loading (r3-2),
        // and the action publishes its own legs' failures itself. It owns it for
        // its OWN root only — once the user navigates away the action's outcome
        // is discarded on the epoch mismatch, so it must not go on swallowing
        // the NEW view's errors, which would leave that view empty and silent
        // (codex r6).
        else if (
          !(
            gridActionActive &&
            refreshEpoch === navEpoch &&
            myGen <= gridActionBaseGen
          )
        )
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
      await loadMore(myGen, onError);
    }
  }

  function onScroll(e: Event) {
    const el = e.currentTarget as HTMLElement;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 600) loadMore();
  }

  // Library/home-rail click routing (owner UX ruling, nav flip): a show keeps
  // the seasons drill; everything else opens its info surface — movie/video →
  // the item page, season/episode → the shared episode page (openInfo).
  // Clicking never instant-plays here; only the Continue Watching flow and
  // the context menu play directly.
  async function open(item: Item) {
    if (item.mediaType !== "show") {
      openInfo(item);
      return;
    }
    navEpoch++; // drilling into children is navigation (see navEpoch)
    detailView = null;
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
  }

  async function changeSort() {
    // Per-library persistence (owner ask 2026-07-10): remember the choice on
    // the section AND in config, so reopening the library — this session or
    // after a restart — lands on it. Best-effort: a failed write must not
    // block the re-sort. The merged type view stays session-only.
    if (active) {
      active.sort = sort;
      const section = sections.find((s) => s.key === active!.key);
      if (section) section.sort = sort;
      invoke("set_section_sort", { sectionKey: active.key, sort }).catch(() => {});
    }
    await resetAndLoad();
  }

  async function runSearch(query: string = searchQuery) {
    const q = query.trim();
    if (q.length < 2) {
      error = "Search needs at least 2 characters.";
      if (searchTerm) {
        navEpoch++; // tearing down the search root is navigation (see navEpoch)
        items = [];
        crumbs = [];
        active = null;
        searchTerm = "";
      }
      return;
    }
    navEpoch++; // navigation (see navEpoch)
    homeGen++; // leaving home: invalidate any in-flight home/sections load
    const myGen = ++loadGen; // invalidate any in-flight load; guard our own result
    loadingMore = false;
    detailView = null;
    mode = "browse";
    active = null; // search results aren't a section, so no pagination
    personView = null;
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
    navEpoch++; // crumb moves are navigation (see navEpoch)
    crumbs = crumbs.slice(0, index + 1);
    const here = crumbs[crumbs.length - 1];
    // Returning to a search/person root (no section, no rating key) re-runs
    // its query — the root state survives child drills (searchTerm pattern).
    if (!here.ratingKey && !active && searchTerm) {
      await runSearch(searchTerm);
    } else if (!here.ratingKey && !active && personView) {
      await runPersonView(personView);
    } else {
      await resetAndLoad();
    }
  }

  // ---- Person browse (person-browse plan slice 2) --------------------------
  // A browse ROOT parallel to search: everything featuring a person, fetched
  // one-shot from the person's own source (tag ids are server-local). It
  // joins the mutually-exclusive root family (plan-review r1/r2, binding):
  // entering clears the other roots; root switches clear it; child drills
  // preserve it exactly as they preserve searchTerm.
  type PersonView = { key: string; kind: "actor" | "director" | "writer"; name: string };
  let personView = $state<PersonView | null>(null);

  function personLabel(p: PersonView): string {
    return p.kind === "actor"
      ? `With ${p.name}`
      : p.kind === "director"
        ? `Directed by ${p.name}`
        : `Written by ${p.name}`;
  }

  async function runPersonView(p: PersonView) {
    navEpoch++; // navigation (see navEpoch)
    homeGen++; // leaving home: invalidate any in-flight home/sections load
    const myGen = ++loadGen; // invalidate any in-flight load; guard our own result
    loadingMore = false;
    detailView = null;
    mode = "browse";
    active = null;
    activeType = null; // a stale type root must never repaint under this crumb
    searchTerm = "";
    personView = p;
    crumbs = [{ title: personLabel(p), ratingKey: null }];
    items = [];
    hasMore = false; // one-shot: the backend returns the full merged list
    loading = true;
    error = null;
    try {
      const results = await invoke<Item[]>("get_person_items", { personKey: p.key, kind: p.kind });
      if (myGen !== loadGen) return; // user navigated away while loading
      items = results;
    } catch (e) {
      if (myGen === loadGen) error = String(e);
    } finally {
      if (myGen === loadGen) loading = false;
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

  // The menu's Play entry must take `mi` as an argument BEFORE closing the
  // menu: `mi` is a template {@const} over `menu.item`, so an inline
  // `closeMenu(); play(mi)` nulls `menu` first and the `mi` read throws.
  function playFromCtx(item: Item) {
    closeMenu();
    play(item);
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
    sectionMenu = null; // only one context menu at a time (codex code review r1, finding 4)
    // Clamp so the menu stays on screen near the right/bottom edges.
    menu = { x: Math.min(e.clientX, window.innerWidth - 200), y: Math.min(e.clientY, window.innerHeight - 160), item, hero };
  }
  function closeMenu() {
    menu = null;
  }

  // Right-click menu on a sidebar library entry (slice 2 of the
  // library-refresh-scan plan): ask the section's server to rescan its
  // files. Merged type tabs get no menu — there's no single section to scan.
  let sectionMenu = $state<{ x: number; y: number; section: Section } | null>(null);
  function openSectionMenu(e: MouseEvent, section: Section) {
    e.preventDefault();
    menu = null; // only one context menu at a time (codex code review r1, finding 4)
    sectionMenu = { x: Math.min(e.clientX, window.innerWidth - 200), y: Math.min(e.clientY, window.innerHeight - 80), section };
  }
  function closeSectionMenu() {
    sectionMenu = null;
  }

  // Scan status: one neutral transient line ("Scan started — X"), auto-cleared
  // after a few seconds; failures use the standard error banner. There is ONE
  // published scan status, so ownership is a single GLOBAL attempt counter —
  // only the LATEST attempt (across ALL sections) may publish its outcome. A
  // per-key gate is not enough: scan lib1 (slow) then lib2 (fast) leaves
  // lib1's gen unsuperseded under its own key, and its stale completion would
  // overwrite lib2's newer notice. The auto-clear timer is attempt-owned too —
  // a timer armed by an earlier success must never wipe a newer attempt's
  // published notice. Per-key gens survive only for the `scanning` flag, so a
  // superseded attempt still re-enables its own menu entry.
  let scanNotice = $state<string | null>(null);
  let scanning = $state<Record<string, boolean>>({}); // menu-entry feedback only
  const scanGens: Record<string, number> = {};
  let scanAttempt = 0; // global publication ownership
  let scanNoticeOwner: number | null = null; // owning attempt of the visible notice
  let scanNoticeTimer: ReturnType<typeof setTimeout> | null = null;
  onDestroy(() => {
    if (scanNoticeTimer) clearTimeout(scanNoticeTimer);
  });

  async function scanSection(s: Section) {
    closeSectionMenu();
    const gen = (scanGens[s.key] = (scanGens[s.key] ?? 0) + 1);
    const attempt = ++scanAttempt;
    scanning[s.key] = true;
    // The action owns its status (refreshLibraries convention): clear any
    // prior banner/notice immediately, and cancel an armed auto-clear so it
    // can't fire mid-flight against the outcome we're about to publish.
    error = null;
    scanNotice = null;
    scanNoticeOwner = null;
    if (scanNoticeTimer) {
      clearTimeout(scanNoticeTimer);
      scanNoticeTimer = null;
    }
    try {
      await invoke("scan_section", { sectionKey: s.key });
      if (scanAttempt !== attempt) return; // superseded — stale outcome
      // No auto-refresh afterward: the scan runs asynchronously server-side
      // and completion is unknowable without polling (non-goal). The slice-1
      // refresh button is the companion action once the scan has landed.
      scanNotice = `Scan started — ${s.title}`;
      scanNoticeOwner = attempt;
      scanNoticeTimer = setTimeout(() => {
        // Only the owning attempt may clear — a timer armed by an earlier
        // success must not wipe a newer attempt's notice.
        if (scanNoticeOwner === attempt) {
          scanNotice = null;
          scanNoticeOwner = null;
        }
      }, 4000);
    } catch (e) {
      if (scanAttempt !== attempt) return;
      error = String(e);
    } finally {
      if (scanGens[s.key] === gen) scanning[s.key] = false;
    }
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
      // Curate the hero without a restart: the backend dropped the recents
      // entry and tombstoned the key (both directions); the re-fetch drops
      // any lingering server hub copy.
      refreshWatchState();
    } catch (e) {
      error = String(e);
      // The backend curates BEFORE the server call and rolls back on
      // failure — but an unrelated refresh (e.g. playback-ended) may have
      // rendered the transient curated state meanwhile. Re-fetch so the
      // rolled-back truth repaints; the error banner above still reports
      // the failed edit.
      refreshWatchState();
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

  // ---- Detail / info surface (item-detail-view) ----------------------------
  // Reached from library/home-rail clicks (open) and the context-menu "Info"
  // entry. The view layers over home/browse without touching their state, so
  // closing it returns exactly where the user was.
  type DetailView =
    | { kind: "item"; item: Item }
    | { kind: "season"; seasonKey: string | null; seed: Item; initialSelKey?: string };
  let detailView = $state<DetailView | null>(null);

  function closeDetail() {
    navEpoch++; // closing the detail surface is navigation (see navEpoch)
    detailView = null;
  }

  // The detail page's own crumb in the trail: a movie's title, a season
  // page's season name (mirrors SeasonDetail's heading derivation).
  function detailCrumbTitle(v: DetailView): string {
    if (v.kind === "item") return v.item.title;
    const s = v.seed;
    if (s.mediaType === "season") return s.title;
    if (s.parentIndex != null) return `Season ${s.parentIndex}`;
    return s.parentTitle ?? s.title;
  }

  // A season key for an episode's shared page: the episode's own parent key
  // is authoritative when the server sent one — it names the episode's season
  // no matter which surface (home rail, search, grid) the click came from.
  // Otherwise the key is only trustworthy when it names the list the episode
  // actually came from: the already-open shared page itself, or a browse grid
  // whose children include this episode. A bare crumb is NOT enough — with a
  // season page open above a seasons grid, the crumb still points at the
  // show, and get_children(show) would list seasons in the episode list
  // (idv-s2 review r1). Anything else degrades to single-episode mode rather
  // than a wrong list.
  function seasonKeyFor(ep: Item): string | null {
    if (ep.parentRatingKey) return ep.parentRatingKey;
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
    navEpoch++; // opening the detail surface is navigation (see navEpoch)
    closeMenu();
    if (item.mediaType === "season") {
      detailView = { kind: "season", seasonKey: detailKeyOf(item), seed: item };
    } else if (item.mediaType === "episode") {
      const seasonKey = seasonKeyFor(item);
      detailView = {
        kind: "season",
        seasonKey,
        seed: item,
        initialSelKey: item.ratingKey,
      };
      if (!seasonKey) {
        // A key-less episode (e.g. a hero recents snapshot from before
        // parent keys existed, kept stale by re-records) opens degraded;
        // the detail response carries the season key, so upgrade to the
        // full shared page when it arrives — unless the user navigated on.
        // Identity-compare against the $state PROXY read back after the
        // assignment: comparing against the raw pre-assignment object is
        // always false (deep $state proxies) and the upgrade never runs.
        const opened = detailView;
        invoke<Detail>("get_item_detail", { ratingKey: detailKeyOf(item) })
          .then((d) => {
            if (detailView === opened && detailView.kind === "season" && d.parentRatingKey) {
              detailView = { ...detailView, seasonKey: d.parentRatingKey };
            }
          })
          .catch(() => {
            /* deferred/sparse backends: the degraded page stands */
          });
      }
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

  {#if scanNotice}
    <!-- Transient scan acknowledgement (slice 2) — neutral, auto-clears;
         scan COMPLETION is unknowable without polling (non-goal). -->
    <div class="notice" role="status">{scanNotice}</div>
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
              onclick={() => (d === 0 ? play(it) : (heroPos = i))}
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
          <div class="sidegroup sidegroup-row">
            <span>Library</span>
            <!-- Slice 1 (library-refresh-scan plan): one action refreshes the
                 section list and the content the user is looking at. Disabled
                 while in flight; re-enable is the settled signal for E2E. -->
            <button
              class="refreshbtn"
              class:spinning={refreshing}
              aria-label="Refresh libraries"
              title="Refresh libraries"
              disabled={refreshing}
              onclick={refreshLibraries}
            >
              <Icon name="refresh" size={12} />
            </button>
          </div>
          {#if activeSource === null && sources.length > 1}
            {#each typeTabs as t (t)}
              <button class="sideitem" class:active={mode === "browse" && activeType === t} onclick={() => selectType(t)}>
                {TYPE_LABELS[t] ?? t}
              </button>
            {/each}
          {:else}
            {#each sections as s (s.key)}
              <button
                class="sideitem"
                class:active={mode === "browse" && active?.key === s.key}
                onclick={() => select(s)}
                oncontextmenu={(e) => openSectionMenu(e, s)}
              >
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
         state untouched underneath. It is one more drill level, so it
         carries the same crumb bar as browse (owner playtest 2026-07-08):
         ancestors close the detail and navigate; over Home, where there is
         no trail, the bar is just Back. -->
    <div class="crumbs">
      <button class="back" onclick={closeDetail}><Icon name="back" size={15} /> Back</button>
      {#if mode === "browse" && crumbs.length > 0}
        {#each crumbs as c, i (i)}
          {#if i > 0}<span class="sep"><Icon name="chevron" size={13} /></span>{/if}
          <button class="crumb" onclick={() => { closeDetail(); goCrumb(i); }}>{c.title}</button>
        {/each}
        <span class="sep"><Icon name="chevron" size={13} /></span>
        <span class="crumb current">{detailCrumbTitle(detailView)}</span>
      {/if}
    </div>
    {#if detailView.kind === "item"}
      {#key detailView.item.ratingKey}
        <ItemDetail
          item={detailView.item}
          {posterSrc}
          onPlay={play}
          onMenu={openMenu}
          onPerson={(key, kind, name) => runPersonView({ key, kind, name })}
        />
      {/key}
    {:else}
      {#key detailView.seed.ratingKey}
        <SeasonDetail
          seasonKey={detailView.seasonKey}
          seed={detailView.seed}
          initialSelKey={detailView.initialSelKey}
          {posterSrc}
          onPlay={play}
          onMenu={openMenu}
          onShow={(key, title) => open({ ratingKey: key, title, mediaType: "show" })}
          onSeason={(key, seed, sel) => {
            navEpoch++; // swapping the open detail surface is navigation
            detailView = { kind: "season", seasonKey: key, seed, initialSelKey: sel };
          }}
          onPerson={(key, kind, name) => runPersonView({ key, kind, name })}
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
          <!-- Merged type view: DTO-sortable keys only. Section view: hide
               show-only sorts outside show sections. -->
          {#each activeType
            ? SORTS.filter((s) => TYPE_SORTS.has(s.key))
            : SORTS.filter((s) => !s.showOnly || active?.sectionType === "show") as s (s.key)}
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
      // Menus first (topmost surfaces), then the drawer, then the detail —
      // Escape with the scan menu open must not close a detail underneath it
      // (codex code review r1, finding 4).
      if (menu) closeMenu();
      else if (sectionMenu) closeSectionMenu();
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
    <button role="menuitem" onclick={() => playFromCtx(mi)}>Play</button>
    {#if mi.mediaType !== "show"}
      <!-- The info path for the Continue Watching flow, where click plays;
           shows get no entry — their info surface is the seasons drill. -->
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

{#if sectionMenu}
  {@const sm = sectionMenu.section}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="menubackdrop" role="presentation" onclick={closeSectionMenu} oncontextmenu={(e) => { e.preventDefault(); closeSectionMenu(); }}></div>
  <div class="ctxmenu" style="left:{sectionMenu.x}px; top:{sectionMenu.y}px;" role="menu">
    <!-- Disabled-while-in-flight is feedback only; correctness comes from
         the per-section generation in scanSection. -->
    <button role="menuitem" disabled={scanning[sm.key]} onclick={() => scanSection(sm)}>Scan Library</button>
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
  .sidegroup-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.4rem;
  }
  .refreshbtn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    color: var(--text-dim);
    cursor: pointer;
    padding: 0.15rem;
    border-radius: 4px;
  }
  .refreshbtn:hover {
    color: var(--text-bright);
  }
  .refreshbtn:disabled {
    cursor: default;
  }
  .refreshbtn.spinning :global(svg) {
    animation: refresh-spin 0.9s linear infinite;
  }
  @keyframes refresh-spin {
    to {
      transform: rotate(360deg);
    }
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

  /* Neutral transient status (scan started) — same slot as .error, calmer. */
  .notice {
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-muted);
    padding: 0.6rem 1rem;
    font-size: 0.85rem;
    animation: vela-slide-down 0.2s var(--ease);
  }

  /* Watched indicator (checkmark only — owner ruling 2026-07-09: watched
     items are not dimmed) */
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
