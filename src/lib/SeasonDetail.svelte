<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "$lib/Icon.svelte";
  import { detailKeyOf, type Detail, type Item, type PersonRef, type PlayIntent } from "$lib/types";

  // The shared episode info page (binding UX ruling): ONE page per season —
  // an episode list plus a detail panel bound to the selection; selecting an
  // episode updates the panel in place. `seasonKey` is the season whose
  // children to list (already detail-routed by the caller); when null the
  // page runs in single-episode mode around `seed` (an episode reached from
  // a context with no season key, e.g. a home rail or search result).
  //
  // idv-4 (binding, both halves):
  // (a) the episode list loads ALL get_children pages, not the first window;
  // (b) a fast episode switch must never paint stale detail — solved
  //     structurally: the panel renders from a per-episode cache keyed by the
  //     SELECTED episode, so a late response can only fill its own slot.
  let {
    seasonKey,
    seed,
    initialSelKey,
    posterSrc,
    onPlay,
    onMenu,
    onShow,
    onSeason,
    onPerson,
  }: {
    seasonKey: string | null;
    seed: Item;
    initialSelKey?: string;
    posterSrc: (p: string) => string;
    onPlay: (item: Item, intent?: PlayIntent) => void;
    onMenu: (e: MouseEvent, item: Item) => void;
    // Heading navigation (owner playtest 2026-07-08): the show title links to
    // the show's seasons drill; the season title links to the full season
    // page when this page isn't already listing it (single-episode mode).
    onShow?: (key: string, title: string) => void;
    onSeason?: (key: string, seed: Item, selKey?: string) => void;
    // Person browse: episode director/writer names are clickable when the
    // backend identified the person; absent keys render plain text.
    onPerson?: (key: string, kind: "director" | "writer", name: string) => void;
  } = $props();

  const PAGE = 60;

  let episodes = $state<Item[]>([]);
  let loadingList = $state(true);
  let listError = $state(false);
  let selKey = $state<string | null>(null);
  // Per-episode detail cache: Detail = enriched, null = fetch failed (render
  // sparse from the listing item). Missing key = not fetched yet.
  let details = $state<Record<string, Detail | null>>({});
  const inflight = new Set<string>();
  let gen = 0; // guards the page loop if the component is torn down mid-load

  $effect(() => {
    const key = seasonKey;
    const seedItem = seed;
    const initSel = initialSelKey ?? null;
    const g = ++gen;
    if (!key) {
      // Single-episode mode: an episode reached with no season key.
      episodes = [seedItem];
      selKey = initSel ?? seedItem.ratingKey;
      loadingList = false;
      return;
    }
    selKey = initSel;
    (async () => {
      // Load every page (idv-4a): a long season must list past PAGE. The
      // accumulator keeps this loop free of reactive reads; each pass
      // publishes, so long lists paint progressively.
      const acc: Item[] = [];
      try {
        for (;;) {
          const page = await invoke<Item[]>("get_children", { ratingKey: key, start: acc.length, size: PAGE });
          if (g !== gen) return;
          if (acc.length === 0 && initSel === null && page.length > 0) {
            selKey = page[0].ratingKey;
          }
          acc.push(...page);
          episodes = [...acc];
          if (page.length < PAGE) break;
        }
      } catch {
        if (g === gen) listError = true; // degraded: seed-only, no error page
      } finally {
        if (g === gen) {
          loadingList = false;
          // A season that lists nothing still shows the seed episode when we
          // have one, keeping the page clean rather than empty.
          if (acc.length === 0 && seedItem.mediaType === "episode") {
            episodes = [seedItem];
            selKey = initSel ?? seedItem.ratingKey;
          }
        }
      }
    })();
    return () => {
      gen++;
    };
  });

  let selected = $derived(episodes.find((e) => e.ratingKey === selKey) ?? null);
  // The panel reads the cache slot of the CURRENT selection only (idv-4b).
  let detail = $derived(selected ? (details[selected.ratingKey] ?? null) : null);

  // Fetch the selected episode's detail once; failures cache null → sparse.
  $effect(() => {
    const ep = selected;
    if (!ep) return;
    const slot = ep.ratingKey;
    if (slot in details || inflight.has(slot)) return;
    inflight.add(slot);
    invoke<Detail>("get_item_detail", { ratingKey: detailKeyOf(ep) })
      .then((d) => {
        details[slot] = d;
      })
      .catch(() => {
        details[slot] = null;
      })
      .finally(() => {
        inflight.delete(slot);
      });
  });

  // Panel fields prefer rich detail, falling back to the listing episode.
  let panelSummary = $derived(detail?.summary ?? selected?.summary);
  let panelStill = $derived(detail?.poster ?? selected?.poster);
  let panelDuration = $derived(detail?.durationMs ?? selected?.durationMs);
  let panelViewOffset = $derived(detail?.viewOffsetMs ?? selected?.viewOffsetMs);
  let panelInProgress = $derived((panelViewOffset ?? 0) > 0);
  let panelPct = $derived(
    panelViewOffset && panelDuration
      ? Math.round(Math.min(100, (100 * panelViewOffset) / panelDuration))
      : null
  );
  let stillFailed = $state(false);
  $effect(() => {
    void selKey;
    stillFailed = false; // each selection gets a fresh chance at its art
  });

  let showTitle = $derived(
    selected?.grandparentTitle ?? seed.grandparentTitle ?? seed.parentTitle ?? ""
  );
  let seasonTitle = $derived.by(() => {
    const idx = selected?.parentIndex ?? seed.parentIndex;
    if (idx != null) return `Season ${idx}`;
    return seed.mediaType === "season" ? seed.title : (seed.parentTitle ?? "");
  });

  // Heading-link targets, when the listing data carries container keys: the
  // show is an episode's grandparent or a season seed's parent; the season
  // link appears only when it would navigate somewhere new (single-episode /
  // degraded mode — clicking it opens the full season page).
  let showKey = $derived(
    selected?.grandparentRatingKey ??
      seed.grandparentRatingKey ??
      (seed.mediaType === "season" ? seed.parentRatingKey : undefined)
  );
  let seasonLinkKey = $derived.by(() => {
    // Only an EPISODE's parent key names a season. A season seed's parent is
    // its SHOW (see showKey above) — linking that here would re-target this
    // page at the show and list seasons as episodes (idv-s4 review r1, the
    // idv-s2 routing guard).
    const k =
      selected?.parentRatingKey ??
      (seed.mediaType === "episode" ? seed.parentRatingKey : undefined);
    return k && k !== seasonKey ? k : undefined;
  });

  function epTag(e: Item): string {
    return e.index != null ? `E${e.index}` : "";
  }
  function runtimeLabel(ms: number): string {
    const mins = Math.round(ms / 60000);
    const h = Math.floor(mins / 60);
    return h > 0 ? `${h}h ${mins % 60}m` : `${mins}m`;
  }
  function pctOf(e: Item): number | null {
    return e.viewOffsetMs && e.durationMs
      ? Math.round(Math.min(100, (100 * e.viewOffsetMs) / e.durationMs))
      : null;
  }
  function streamsOf(v: NonNullable<Detail["media"]>[number], type: number): string[] {
    return (v.streams ?? [])
      .filter((s) => s.streamType === type)
      .map((s) => s.displayTitle ?? [s.language, s.codec?.toUpperCase()].filter(Boolean).join(" "))
      .filter((s) => s.length > 0);
  }
</script>

{#snippet people(refs: PersonRef[], kind: "director" | "writer")}
  {#each refs as p, i (p.name + i)}{#if i > 0}{", "}{/if}{#if p.personKey && onPerson}<button class="personlink" onclick={() => onPerson!(p.personKey!, kind, p.name)}>{p.name}</button>{:else}<span>{p.name}</span>{/if}{/each}
{/snippet}

<div class="season" role="region" aria-label="Season details">
  <!-- Back lives in the page-level crumb bar (the info surface is one more
       drill level); the heading is the page title plus show/season links. -->
  <div class="topbar">
    <div class="heading">
      {#if showTitle}
        {#if showKey && onShow}
          <button class="show navlink" title="Open show" onclick={() => onShow!(showKey!, showTitle)}>{showTitle}</button>
        {:else}
          <span class="show">{showTitle}</span>
        {/if}
      {/if}
      {#if seasonTitle}
        {#if seasonLinkKey && onSeason}
          <button class="sea navlink" title="Open season" onclick={() => onSeason!(seasonLinkKey!, seed, selected?.ratingKey)}>{seasonTitle}</button>
        {:else}
          <span class="sea">{seasonTitle}</span>
        {/if}
      {/if}
    </div>
  </div>
  <div class="split">
    <div class="eplist" aria-label="Episodes">
      {#if loadingList && episodes.length === 0}
        {#each Array(6) as _, i (i)}
          <div class="eprow skel" aria-hidden="true"><div class="epthumb"></div><div class="epmeta"><span class="skel-line" style="width:70%"></span></div></div>
        {/each}
      {:else}
        {#each episodes as e (e.ratingKey)}
          {@const pct = pctOf(e)}
          <button
            class="eprow"
            class:selected={e.ratingKey === selKey}
            onclick={() => (selKey = e.ratingKey)}
            oncontextmenu={(ev) => onMenu(ev, e)}
            aria-current={e.ratingKey === selKey}
          >
            <div class="epthumb">
              {#if e.poster}
                <img src={posterSrc(e.poster)} alt="" loading="lazy" onerror={(ev) => ((ev.currentTarget as HTMLImageElement).style.visibility = "hidden")} />
              {/if}
              {#if pct !== null}
                <div class="progress" aria-hidden="true"><div class="bar" style="width:{pct}%"></div></div>
              {/if}
            </div>
            <div class="epmeta">
              <span class="eptitle"
                >{#if epTag(e)}<span class="eptag">{epTag(e)}</span>{/if}{e.title}</span
              >
              <span class="epsub">
                {#if e.durationMs}{runtimeLabel(e.durationMs)}{/if}
                {#if e.played === true && pct === null}<span class="watchedmark" title="Watched"><Icon name="check" size={11} stroke={2.75} /><span class="sr-only">Watched</span></span>{/if}
              </span>
            </div>
          </button>
        {/each}
        {#if loadingList}
          <div class="listnote">Loading more…</div>
        {:else if listError && episodes.length <= 1}
          <div class="listnote">Couldn't load the full episode list.</div>
        {/if}
      {/if}
    </div>
    <div class="panel">
      {#if selected}
        <div class="stillwrap">
          {#if panelStill && !stillFailed}
            <img class="still" src={posterSrc(panelStill)} alt="" onerror={() => (stillFailed = true)} />
          {:else}
            <div class="still noart">{selected.title}</div>
          {/if}
        </div>
        <div class="paneltitle">
          {#if selected.parentIndex != null && selected.index != null}
            <span class="epcode">S{selected.parentIndex} · E{selected.index}</span>
          {/if}
          <h1>{selected.title}</h1>
        </div>
        <div class="metarow">
          {#if detail?.originallyAvailableAt}<span>{detail.originallyAvailableAt}</span>{/if}
          {#if panelDuration}<span>{runtimeLabel(panelDuration)}</span>{/if}
          {#if detail?.rating != null}<span class="metric" title="Rating"><Icon name="star" size={13} /><span class="sr-only">Rating: </span>{detail.rating.toFixed(1)}</span>{/if}
          {#if selected.played === true && panelPct === null}<span class="chip watched"><Icon name="check" size={12} stroke={2.5} /> Watched</span>{/if}
        </div>
        <div class="playactions">
          <button class="primary playwide" onclick={() => onPlay(selected!, "resume")}>
            <Icon name="play" size={16} />
            {panelInProgress ? "Resume" : "Play"}
          </button>
          {#if panelInProgress}
            <button class="playwide" onclick={() => onPlay(selected!, "beginning")}>
              Play from Beginning
            </button>
          {/if}
        </div>
        {#if panelSummary}
          <p class="summary">{panelSummary}</p>
        {/if}
        {#if detail?.directors?.length || detail?.writers?.length}
          <div class="credits">
            {#if detail?.directors?.length}
              <div><span class="credlabel">Directed by</span> {@render people(detail.directors, "director")}</div>
            {/if}
            {#if detail?.writers?.length}
              <div><span class="credlabel">Written by</span> {@render people(detail.writers, "writer")}</div>
            {/if}
          </div>
        {/if}
        {#if detail?.media?.length}
          <div class="mediaspecs">
            {#each detail.media as v, i (i)}
              {@const audio = streamsOf(v, 2)}
              {@const subs = streamsOf(v, 3)}
              <div class="version">
                <div class="vlabel">
                  {[
                    v.videoResolution ? (/^\d+$/.test(v.videoResolution) ? `${v.videoResolution}p` : v.videoResolution.toUpperCase()) : null,
                    v.videoCodec?.toUpperCase(),
                    v.hdr ? "HDR" : null,
                    v.container?.toUpperCase(),
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </div>
                {#if audio.length}<div class="vstreams"><span class="credlabel">Audio</span> {audio.join(" · ")}</div>{/if}
                {#if subs.length}<div class="vstreams"><span class="credlabel">Subtitles</span> {subs.join(" · ")}</div>{/if}
              </div>
            {/each}
          </div>
        {/if}
      {:else}
        <div class="panelempty">Select an episode.</div>
      {/if}
    </div>
  </div>
</div>

<style>
  .season {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 1rem 1.5rem 0;
  }
  .topbar {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 0.9rem;
  }
  .heading {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    min-width: 0;
  }
  /* Heading titles rendered as navigation: the button reset comes FIRST so
     the .show/.sea typography rules below win over `font: inherit`. */
  .heading .navlink {
    appearance: none;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .heading .navlink:hover,
  .heading .navlink:focus-visible {
    color: var(--accent);
    text-decoration: underline;
  }
  .heading .show {
    font-size: 1.3rem;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--text-bright);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .heading .sea {
    color: var(--text-muted);
    white-space: nowrap;
  }
  .split {
    flex: 1;
    min-height: 0;
    display: flex;
    gap: 1.25rem;
  }
  .eplist {
    flex: 0 0 21rem;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding-bottom: 3rem;
  }
  .eprow {
    display: flex;
    gap: 0.65rem;
    align-items: center;
    padding: 0.4rem;
    border: 1px solid transparent;
    border-radius: 0.5rem;
    background: none;
    cursor: pointer;
    text-align: left;
    color: var(--text);
  }
  .eprow:hover {
    background: var(--bg-blur);
  }
  .eprow.selected {
    border-color: var(--accent);
    background: var(--accent-tint);
  }
  .epthumb {
    position: relative;
    flex: 0 0 96px;
    width: 96px;
    aspect-ratio: 16 / 9;
    border-radius: 0.35rem;
    overflow: hidden;
    background: var(--surface-sunken);
  }
  .epthumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .epmeta {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }
  .eptitle {
    font-size: 0.92rem;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .eptag {
    color: var(--text-muted);
    font-weight: 600;
    margin-right: 0.4rem;
  }
  .epsub {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.8rem;
    color: var(--text-muted);
  }
  .watchedmark {
    color: var(--accent);
    display: inline-flex;
  }
  .listnote {
    padding: 0.5rem;
    color: var(--text-muted);
    font-size: 0.85rem;
  }
  .panel {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 3rem;
  }
  .stillwrap {
    max-width: 34rem;
  }
  .still {
    width: 100%;
    aspect-ratio: 16 / 9;
    border-radius: 0.6rem;
    border: 1px solid var(--border-subtle);
  }
  .still:not(.noart) {
    object-fit: cover;
    background: var(--surface-sunken);
    display: block;
  }
  .still.noart {
    display: flex;
    padding: 1rem;
  }
  .paneltitle {
    margin-top: 0.8rem;
  }
  .epcode {
    color: var(--text-muted);
    font-weight: 600;
    font-size: 0.9rem;
  }
  h1 {
    margin: 0.1rem 0 0.4rem;
    font-size: 1.5rem;
    letter-spacing: -0.02em;
    color: var(--text-bright);
  }
  .metarow {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.7rem;
    color: var(--text-2);
    margin-bottom: 0.7rem;
  }
  .metric {
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
  .playactions .playwide:not(.primary) {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.5rem 0.9rem;
    border-radius: 0.5rem;
    font-weight: 600;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
  }
  .playactions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.45rem;
    margin-bottom: 0.9rem;
  }
  .summary {
    margin: 0 0 0.9rem;
    max-width: 44rem;
    line-height: 1.55;
  }
  .credits {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    color: var(--text-2);
    font-size: 0.92rem;
    margin-bottom: 0.9rem;
  }
  .credlabel {
    color: var(--text-muted);
    margin-right: 0.35rem;
  }
  .version {
    border: 1px solid var(--border-subtle);
    border-radius: 0.5rem;
    padding: 0.6rem 0.8rem;
    margin-bottom: 0.6rem;
    background: var(--bg-blur);
    max-width: 34rem;
  }
  .vlabel {
    font-weight: 600;
    color: var(--text-bright);
    margin-bottom: 0.2rem;
  }
  .vstreams {
    font-size: 0.88rem;
    color: var(--text-2);
  }
  .panelempty {
    color: var(--text-muted);
    padding: 2rem 0;
  }
  .eprow.skel .epthumb {
    background: var(--surface-sunken);
  }
  .skel-line {
    display: inline-block;
    height: 0.7rem;
    border-radius: 0.3rem;
    background: var(--surface-sunken);
  }
</style>
