<script lang="ts">
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { onMount, onDestroy, tick } from "svelte";
  import Settings from "$lib/Settings.svelte";

  // Poster URLs that 404'd; fall back to the title placeholder for these.
  let failedPosters = $state(new Set<string>());
  // Tracked timers, cleared on destroy / when superseded.
  let copyTimer: ReturnType<typeof setTimeout> | undefined;
  let pollTimer: ReturnType<typeof setTimeout> | undefined;
  onDestroy(() => {
    if (copyTimer) clearTimeout(copyTimer);
    if (pollTimer) clearTimeout(pollTimer);
    if (queueTimer) clearInterval(queueTimer);
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
  type Item = {
    ratingKey: string;
    title: string;
    year?: number;
    poster?: string;
    mediaType?: string;
    durationMs?: number;
    viewOffsetMs?: number;
    grandparentTitle?: string;
    parentTitle?: string;
    index?: number;
    parentIndex?: number;
    played?: boolean | null;
  };
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
    installCommand: string;
    installUrl: string;
  };
  let mpvInfo = $state<MpvInfo | null>(null);
  let copied = $state(false);
  let installingMpv = $state(false);

  // One-click mpv install via the OS package manager (winget/brew). On success
  // the backend returns refreshed status, which clears the prompt.
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

  // Switch the active source (null = All), then reload home scoped to it.
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
      const h = await invoke<Hub[]>("get_hubs", { sourceId: activeSource });
      if (gen === homeGen) hubs = h;
    } catch (e) {
      if (gen === homeGen) error = String(e);
    } finally {
      if (gen === homeGen) loading = false;
    }
  }

  function goHome() {
    loadGen++; // invalidate any in-flight browse load so it can't append after we leave
    loadingMore = false;
    loading = false; // a stale browse load won't clear this (its gen is stale); do it here
    error = null; // don't carry a browse/search error banner onto Home
    searchTerm = "";
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
    mode = "browse";
    searchTerm = "";
    active = section;
    crumbs = [{ title: section.title, ratingKey: null }];
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
    if (!here || (!here.ratingKey && !active)) return;
    loadingMore = true;
    try {
      const page = here.ratingKey
        ? await invoke<Item[]>("get_children", { ratingKey: here.ratingKey, start: offset, size: PAGE })
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
    if (item.mediaType === "show" || item.mediaType === "season") {
      if (mode === "home") {
        // Drilling out of a hub: start a fresh crumb trail rooted at this item.
        active = null;
        crumbs = [{ title: item.title, ratingKey: item.ratingKey }];
      } else {
        crumbs = [...crumbs, { title: item.title, ratingKey: item.ratingKey }];
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
      if (queueOpen) refreshQueue();
    } catch (e) {
      error = String(e);
      // A failure may mean mpv went missing — re-check so the install prompt shows.
      invoke<MpvInfo>("check_mpv").then((m) => (mpvInfo = m)).catch(() => {});
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
  let menu = $state<{ x: number; y: number; item: Item } | null>(null);
  function openMenu(e: MouseEvent, item: Item) {
    e.preventDefault();
    // Clamp so the menu stays on screen near the right/bottom edges.
    menu = { x: Math.min(e.clientX, window.innerWidth - 200), y: Math.min(e.clientY, window.innerHeight - 160), item };
  }
  function closeMenu() {
    menu = null;
  }

  async function setWatched(item: Item, played: boolean) {
    closeMenu();
    try {
      await invoke("set_watched", { ratingKey: item.ratingKey, played });
      // Reflect immediately (deep-reactive $state). Scrobble/unscrobble both clear
      // the resume position, so drop the in-progress bar too — leaving a clean
      // watched (✓) or unwatched state instead of a contradictory bar + badge.
      item.played = played;
      item.viewOffsetMs = 0;
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="app">
  <header>
    <span class="brand">Ve<b>la</b></span>
    {#if authenticated && sources.length > 1}
      <div class="srcswitch">
        <button class:active={activeSource === null} onclick={() => selectSource(null)}>All</button>
        {#each sources as src (src.id)}
          <button class:active={activeSource === src.id} onclick={() => selectSource(src.id)}>{src.name}</button>
        {/each}
      </div>
    {/if}
    <nav>
      {#if authenticated}
        <button class:active={mode === "home"} onclick={goHome}>Home</button>
        {#each sections as s (s.key)}
          <button class:active={mode === "browse" && active?.key === s.key} onclick={() => select(s)}>
            {s.title}{#if activeSource === null && sources.length > 1 && s.sourceName}<span class="srctag"> · {s.sourceName}</span>{/if}
          </button>
        {/each}
      {/if}
    </nav>
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
      ☰{#if queue.items.length > 0}<span class="qcount">{queue.items.length}</span>{/if}
    </button>
    <button class="gear" title="Settings" aria-label="Settings" onclick={() => (showSettings = true)}>⚙</button>
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
          Install it automatically, or point Vela at an existing mpv in Settings → Player.
        {:else}
          Install it, then restart Vela.
        {/if}
        <code>{mpvInfo.installCommand}</code>
      </div>
      <div class="mpvactions">
        {#if mpvInfo.canAutoInstall}
          <button class="primary" disabled={installingMpv} onclick={installMpv}>
            {installingMpv ? "Installing…" : "Install mpv"}
          </button>
        {/if}
        <button onclick={() => copyText(mpvInfo!.installCommand)}>{copied ? "Copied!" : "Copy"}</button>
        <button onclick={() => (showSettings = true)}>Set path…</button>
        <button onclick={() => openExternal(mpvInfo!.installUrl)}>Get mpv</button>
      </div>
    </div>
  {/if}

  {#snippet poster(item: Item)}
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
      class:landscape={item.mediaType === "episode" || item.mediaType === "video"}
      class:watched={item.played === true && pct === null}
      onclick={() => open(item)}
      oncontextmenu={(e) => openMenu(e, item)}
      title={baseName}
      aria-label={label}
    >
      <div class="art">
        {#if item.played === true && pct === null}
          <!-- Fully watched: marked played AND not mid-resume (pct is the resume %). -->
          <div class="watchedbadge" aria-hidden="true">✓</div>
        {/if}
        {#if item.poster && !failedPosters.has(item.ratingKey)}
          <img
            src={posterSrc(item.poster)}
            alt={item.title}
            loading="lazy"
            onerror={() => {
              failedPosters.add(item.ratingKey);
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
      </div>
    </button>
  {/snippet}

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
      <h2>No sources connected</h2>
      <button class="primary" onclick={() => (showSettings = true)}>Add Source</button>
    </div>
  {:else if mode === "home"}
    {#if loading && hubs.length === 0}
      <div class="muted center">Loading…</div>
    {:else if hubs.length === 0}
      <div class="muted center">Nothing on your home screen yet — pick a library above.</div>
    {:else}
      <div class="home">
        {#each hubs as hub (hub.sourceId + ":" + hub.hubIdentifier)}
          <section class="rail">
            <h2>{hub.title}{#if activeSource === null && sources.length > 1 && hub.sourceName}<span class="srctag"> · {hub.sourceName}</span>{/if}</h2>
            <div class="row">
              {#each hub.items as item (item.ratingKey)}
                {@render poster(item)}
              {/each}
            </div>
          </section>
        {/each}
      </div>
    {/if}
  {:else if loading && items.length === 0}
    <div class="muted center">Loading…</div>
  {:else}
    <div class="crumbs">
      <button class="back" onclick={back}>← Back</button>
      {#each crumbs as c, i (i)}
        {#if i > 0}<span class="sep">/</span>{/if}
        <button class="crumb" class:current={i === crumbs.length - 1} onclick={() => goCrumb(i)}>{c.title}</button>
      {/each}
      {#if active && crumbs.length === 1}
        <select class="sort" bind:value={sort} onchange={changeSort}>
          {#each SORTS as s (s.key)}
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
        {#each items as item (item.ratingKey)}
          {@render poster(item)}
        {/each}
      </main>
    {/if}
  {/if}

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
    <button role="menuitem" onclick={() => playNext(mi)}>Play next</button>
    <button role="menuitem" onclick={() => addToQueue(mi)}>Add to queue</button>
    {#if mi.played != null && !fullyWatched}
      <button role="menuitem" onclick={() => setWatched(mi, true)}>Mark watched</button>
    {/if}
    {#if mi.played != null && (mi.played === true || inProgress)}
      <button role="menuitem" onclick={() => setWatched(mi, false)}>Mark unwatched</button>
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
        <button class="drawerclose" aria-label="Close queue" onclick={toggleQueue}>✕</button>
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
            <button class="drawerremove" title="Remove from queue" aria-label="Remove from queue" onclick={() => queueRemove(i)}>✕</button>
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
    font-weight: 700;
    letter-spacing: 0.5px;
  }
  .brand b {
    color: var(--accent);
    font-weight: 800;
  }
  nav {
    display: flex;
    gap: 0.25rem;
    flex-wrap: wrap;
  }
  nav button {
    background: transparent;
    border: none;
    color: var(--text-muted);
    padding: 0.4rem 0.8rem;
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.92rem;
  }
  nav button:hover {
    color: var(--text-bright);
    background: var(--surface);
  }
  nav button.active {
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
  .srcswitch {
    display: flex;
    gap: 0.2rem;
    background: var(--surface-sunken);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.15rem;
  }
  .srcswitch button {
    background: transparent;
    border: none;
    color: var(--text-muted);
    padding: 0.25rem 0.6rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .srcswitch button.active {
    color: var(--on-accent);
    background: var(--accent);
    font-weight: 700;
  }
  .srctag {
    color: var(--text-dim);
    font-weight: 400;
    font-size: 0.85em;
  }
  .gear {
    margin-left: auto;
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 1.1rem;
    cursor: pointer;
    padding: 0.2rem 0.4rem;
  }
  .gear:hover {
    color: var(--text-bright);
  }
  .search:focus {
    outline: none;
    border-color: var(--accent);
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
    padding: 0.3rem 0.7rem;
    border-radius: 7px;
    cursor: pointer;
    margin-right: 0.5rem;
    font-size: 0.85rem;
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
    color: var(--border-strong);
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
    transition: transform 0.15s ease;
  }
  .poster.landscape {
    max-width: 210px;
  }
  .poster:hover {
    transform: translateY(-4px);
  }
  .art {
    position: relative;
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: 7px;
    overflow: hidden;
    background: var(--surface);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.45);
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
    padding: 0.5rem;
    font-size: 0.85rem;
    color: var(--text-dim);
    text-align: center;
  }
  .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 4px;
    background: rgba(0, 0, 0, 0.55);
  }
  .progress .bar {
    height: 100%;
    background: var(--accent);
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
    font-size: 1.05rem;
    font-weight: 600;
    margin: 0 0 0.7rem;
  }
  .row {
    display: flex;
    gap: 0.8rem;
    overflow-x: auto;
    padding-bottom: 0.6rem;
    scrollbar-width: thin;
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
  .empty h2 {
    margin: 0;
    color: var(--text-2);
    font-size: 1.2rem;
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
  }
  button.primary:hover {
    background: var(--accent-hover);
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
    font-size: 0.8rem;
    font-weight: 800;
    line-height: 1.25rem;
    text-align: center;
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
    font-size: 1rem;
    line-height: 1;
    padding: 0.3rem 0.45rem;
    border-radius: 0.35rem;
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
    font-size: 0.9rem;
    padding: 0 0.5rem;
    border-radius: 0.35rem;
  }
  .drawerremove:hover {
    color: var(--danger-text);
    background: var(--border-subtle);
  }
</style>
