<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "$lib/Icon.svelte";
  import { detailKeyOf, type Detail, type Item, type PersonRef, type PlayIntent } from "$lib/types";

  // Full-screen info page for a single item (movie / video). Renders the
  // listing data immediately and enriches in place when `get_item_detail`
  // resolves; a failed fetch (e.g. a backend without item_detail yet) keeps
  // the sparse listing render silently — never an error page (owner
  // amendment 2026-07-08). Callers remount per item via {#key}.
  let {
    item,
    posterSrc,
    onPlay,
    onMenu,
    onPerson,
  }: {
    item: Item;
    posterSrc: (p: string) => string;
    onPlay: (item: Item, intent?: PlayIntent) => void;
    onMenu: (e: MouseEvent, item: Item) => void;
    // Person browse: cast cards and director/writer names are clickable when
    // the backend identified the person; absent keys render plain text.
    onPerson?: (key: string, kind: "actor" | "director" | "writer", name: string) => void;
  } = $props();

  let detail = $state<Detail | null>(null);
  let gen = 0; // guards a stale response if the item prop ever swaps in place

  $effect(() => {
    const key = detailKeyOf(item);
    const g = ++gen;
    invoke<Detail>("get_item_detail", { ratingKey: key })
      .then((d) => {
        if (g === gen) detail = d;
      })
      .catch(() => {
        /* sparse fallback: the listing fields below already render */
      });
  });

  // Every display field prefers the rich detail, falling back to the card.
  let title = $derived(detail?.title ?? item.title);
  let year = $derived(detail?.year ?? item.year);
  let summary = $derived(detail?.summary ?? item.summary);
  let poster = $derived(detail?.poster ?? item.poster);
  let backdrop = $derived(detail?.backdrop ?? item.backdrop);
  let durationMs = $derived(detail?.durationMs ?? item.durationMs);
  let viewOffsetMs = $derived(detail?.viewOffsetMs ?? item.viewOffsetMs);
  let inProgress = $derived((viewOffsetMs ?? 0) > 0);
  let played = $derived(detail?.played ?? item.played);
  let playable = $derived(item.mediaType !== "show" && item.mediaType !== "season");
  let posterFailed = $state(false);
  let pct = $derived(
    viewOffsetMs && durationMs
      ? Math.round(Math.min(100, (100 * viewOffsetMs) / durationMs))
      : null
  );

  function runtimeLabel(ms: number): string {
    const mins = Math.round(ms / 60000);
    const h = Math.floor(mins / 60);
    return h > 0 ? `${h}h ${mins % 60}m` : `${mins}m`;
  }

  // "2160p · HEVC · HDR · MKV" for one media version.
  function versionLabel(v: NonNullable<Detail["media"]>[number]): string {
    const res = v.videoResolution
      ? /^\d+$/.test(v.videoResolution)
        ? `${v.videoResolution}p`
        : v.videoResolution.toUpperCase()
      : v.height
        ? `${v.height}p`
        : null;
    return [
      res,
      v.videoCodec?.toUpperCase(),
      v.hdr ? "HDR" : null,
      v.container?.toUpperCase(),
    ]
      .filter(Boolean)
      .join(" · ");
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

{#snippet castBody(c: NonNullable<Detail["cast"]>[number])}
  {#if c.thumb}
    <img
      class="headshot"
      src={posterSrc(c.thumb)}
      alt={c.name}
      loading="lazy"
      onerror={(e) => ((e.currentTarget as HTMLImageElement).style.visibility = "hidden")}
    />
  {:else}
    <div class="headshot placeholder" aria-hidden="true"><Icon name="film" size={22} stroke={1.5} /></div>
  {/if}
  <div class="castname">{c.name}</div>
  {#if c.role}<div class="castrole">{c.role}</div>{/if}
{/snippet}

<div class="detail" role="region" aria-label="Item details">
  {#if backdrop}
    <div class="backdrop" aria-hidden="true">
      <img src={posterSrc(backdrop)} alt="" onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = "none")} />
    </div>
  {/if}
  <div class="body">
    <!-- Back lives in the page-level crumb bar (the info surface is one more
         drill level), not inside the component. -->
    <div class="hero">
      <div class="postercol">
        <button
          class="posterframe"
          class:noplay={!playable}
          onclick={() => playable && onPlay(item, "resume")}
          oncontextmenu={(e) => onMenu(e, item)}
          aria-label={playable ? `${inProgress ? "Resume" : "Play"} ${title}` : title}
        >
          {#if poster && !posterFailed}
            <img src={posterSrc(poster)} alt={title} onerror={() => (posterFailed = true)} />
          {:else}
            <div class="noart">{title}</div>
          {/if}
          {#if playable}
            <div class="playoverlay" aria-hidden="true">
              <span class="playbtn"><Icon name="play" size={26} /></span>
            </div>
          {/if}
          {#if pct !== null}
            <div class="progress" aria-hidden="true"><div class="bar" style="width:{pct}%"></div></div>
          {/if}
        </button>
        {#if playable}
          <div class="playactions">
            <button class="primary playwide" onclick={() => onPlay(item, "resume")}>
              <Icon name="play" size={16} />
              {inProgress ? "Resume" : "Play"}
            </button>
            {#if inProgress}
              <button class="playwide" onclick={() => onPlay(item, "beginning")}>
                Play from Beginning
              </button>
            {/if}
          </div>
        {/if}
      </div>
      <div class="info">
        <h1>{title}</h1>
        {#if detail?.tagline}
          <p class="tagline">{detail.tagline}</p>
        {/if}
        <div class="metarow">
          {#if year}<span>{year}</span>{/if}
          {#if durationMs}<span>{runtimeLabel(durationMs)}</span>{/if}
          {#if detail?.contentRating}<span class="chip">{detail.contentRating}</span>{/if}
          {#if detail?.rating != null}<span class="metric" title="Rating"><Icon name="star" size={13} /><span class="sr-only">Rating: </span>{detail.rating.toFixed(1)}</span>{/if}
          {#if detail?.audienceRating != null}<span class="metric" title="Audience rating"><Icon name="heart" size={13} /><span class="sr-only">Audience rating: </span>{detail.audienceRating.toFixed(1)}</span>{/if}
          {#if played === true && pct === null}<span class="chip watched"><Icon name="check" size={12} stroke={2.5} /> Watched</span>{/if}
        </div>
        {#if detail?.genres?.length}
          <div class="genres">
            {#each detail.genres as g (g)}<span class="chip">{g}</span>{/each}
          </div>
        {/if}
        {#if summary}
          <p class="summary">{summary}</p>
        {/if}
        <div class="credits">
          {#if detail?.directors?.length}
            <div><span class="credlabel">Directed by</span> {@render people(detail.directors, "director")}</div>
          {/if}
          {#if detail?.writers?.length}
            <div><span class="credlabel">Written by</span> {@render people(detail.writers, "writer")}</div>
          {/if}
          {#if detail?.studio}
            <div><span class="credlabel">Studio</span> {detail.studio}</div>
          {/if}
          {#if detail?.originallyAvailableAt}
            <div><span class="credlabel">Released</span> {detail.originallyAvailableAt}</div>
          {/if}
        </div>
      </div>
    </div>

    {#if detail?.cast?.length}
      <section class="section">
        <h2>Cast</h2>
        <div class="castrow">
          {#each detail.cast as c (c.name + (c.role ?? ""))}
            {#if c.personKey && onPerson}
              <button
                class="castcard clickable"
                onclick={() => onPerson!(c.personKey!, "actor", c.name)}
                title="Browse titles with {c.name}"
              >
                {@render castBody(c)}
              </button>
            {:else}
              <div class="castcard">{@render castBody(c)}</div>
            {/if}
          {/each}
        </div>
      </section>
    {/if}

    {#if detail?.media?.length}
      <section class="section">
        <h2>Media</h2>
        {#each detail.media as v, i (i)}
          {@const audio = streamsOf(v, 2)}
          {@const subs = streamsOf(v, 3)}
          <div class="version">
            <div class="vlabel">{versionLabel(v)}</div>
            {#if audio.length}
              <div class="vstreams"><span class="credlabel">Audio</span> {audio.join(" · ")}</div>
            {/if}
            {#if subs.length}
              <div class="vstreams"><span class="credlabel">Subtitles</span> {subs.join(" · ")}</div>
            {/if}
          </div>
        {/each}
      </section>
    {/if}
  </div>
</div>

<style>
  .detail {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .backdrop {
    position: absolute;
    inset: 0 0 auto 0;
    height: 22rem;
    overflow: hidden;
    pointer-events: none;
    mask-image: linear-gradient(to bottom, rgba(0, 0, 0, 0.55), transparent);
    -webkit-mask-image: linear-gradient(to bottom, rgba(0, 0, 0, 0.55), transparent);
  }
  .backdrop img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: blur(2px) saturate(1.05);
  }
  .body {
    position: relative;
    padding: 1rem 1.5rem 3rem;
    max-width: 68rem;
  }
  .hero {
    display: flex;
    gap: 1.5rem;
    align-items: flex-start;
  }
  .postercol {
    flex: 0 0 218px;
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }
  .posterframe {
    position: relative;
    display: block;
    width: 218px;
    aspect-ratio: 2 / 3;
    padding: 0;
    border: 1px solid var(--border-subtle);
    border-radius: 0.6rem;
    overflow: hidden;
    cursor: pointer;
    background: var(--surface-sunken);
    box-shadow: var(--shadow-lg);
  }
  .posterframe.noplay {
    cursor: default;
  }
  .posterframe img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .posterframe:hover .playoverlay,
  .posterframe:focus-visible .playoverlay {
    opacity: 1;
  }
  .posterframe:hover .playbtn,
  .posterframe:focus-visible .playbtn {
    transform: scale(1);
  }
  .playactions .playwide:not(.primary) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.45rem;
    padding: 0.55rem 0.9rem;
    border-radius: 0.5rem;
    font-weight: 600;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
  }
  .playactions {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }
  .info {
    flex: 1;
    min-width: 0;
    padding-top: 0.4rem;
  }
  h1 {
    margin: 0 0 0.25rem;
    font-size: 1.9rem;
    letter-spacing: -0.02em;
    color: var(--text-bright);
  }
  .tagline {
    margin: 0 0 0.5rem;
    color: var(--text-muted);
    font-style: italic;
  }
  .metarow {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.7rem;
    color: var(--text-2);
    margin-bottom: 0.6rem;
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
  .genres {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-bottom: 0.8rem;
  }
  .summary {
    margin: 0 0 0.9rem;
    max-width: 46rem;
    line-height: 1.55;
    color: var(--text);
  }
  .credits {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    color: var(--text-2);
    font-size: 0.92rem;
  }
  .credlabel {
    color: var(--text-muted);
    margin-right: 0.35rem;
  }
  .section {
    margin-top: 1.7rem;
  }
  .section h2 {
    font-size: 1.15rem;
    font-weight: 700;
    letter-spacing: -0.02em;
    margin: 0 0 0.7rem;
  }
  .castrow {
    display: flex;
    gap: 0.8rem;
    overflow-x: auto;
    padding-bottom: 0.5rem;
    scrollbar-width: thin;
  }
  .castcard {
    flex: 0 0 92px;
    text-align: center;
  }
  .headshot {
    width: 92px;
    height: 92px;
    border-radius: 50%;
    object-fit: cover;
    border: 1px solid var(--border-subtle);
    background: var(--surface-sunken);
  }
  .headshot.placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-dim);
  }
  .castname {
    margin-top: 0.35rem;
    font-size: 0.82rem;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .castrole {
    font-size: 0.76rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Person-browse affordances: identified people are clickable. */
  button.castcard {
    appearance: none;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: inherit;
    cursor: pointer;
  }
  .castcard.clickable:hover .castname,
  .castcard.clickable:focus-visible .castname {
    color: var(--accent);
    text-decoration: underline;
  }
  .version {
    border: 1px solid var(--border-subtle);
    border-radius: 0.5rem;
    padding: 0.6rem 0.8rem;
    margin-bottom: 0.6rem;
    background: var(--bg-blur);
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
</style>
