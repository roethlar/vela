<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "$lib/Icon.svelte";
  import { friendlyError } from "$lib/errors";
  import type { Item, ServerPlaylist } from "$lib/types";

  let {
    playlist,
    refreshVersion = 0,
    posterSrc,
    onBack,
  }: {
    playlist: ServerPlaylist;
    refreshVersion?: number;
    posterSrc: (poster: string) => string;
    onBack: () => void;
  } = $props();

  type Status = { text: string; failed: boolean };
  let items = $state<Item[]>([]);
  let loading = $state(true);
  let playing = $state(false);
  let status = $state<Status | null>(null);
  let observedLoad = $state("");
  let loadAttempt = 0;
  let playAttempt = 0;

  $effect(() => {
    const key = playlist.key;
    const loadKey = `${key}:${refreshVersion}`;
    if (loadKey === observedLoad) return;
    observedLoad = loadKey;
    void loadItems(key);
  });

  async function loadItems(key: string): Promise<void> {
    const attempt = ++loadAttempt;
    playAttempt++;
    loading = true;
    playing = false;
    status = null;
    items = [];
    try {
      const loaded = await invoke<Item[]>("get_server_playlist_items", { key });
      if (attempt === loadAttempt && playlist.key === key) items = loaded;
    } catch (error) {
      if (attempt === loadAttempt && playlist.key === key) {
        status = {
          text: `Couldn't load “${playlist.title}” — ${String(error)}`,
          failed: true,
        };
      }
    } finally {
      if (attempt === loadAttempt) loading = false;
    }
  }

  async function play(index: number, beginning: boolean): Promise<void> {
    const item = items[index];
    if (!item || loading || playing) return;
    const key = playlist.key;
    const attempt = ++playAttempt;
    playing = true;
    status = null;
    try {
      await invoke("server_playlist_play", {
        key,
        startIndex: index,
        startFromBeginning: beginning,
      });
      if (attempt === playAttempt && playlist.key === key) {
        status = { text: `Playing “${item.title}”.`, failed: false };
      }
    } catch (error) {
      if (attempt === playAttempt && playlist.key === key) {
        status = {
          text: `Couldn't play “${item.title}” — ${String(error)}`,
          failed: true,
        };
      }
    } finally {
      if (attempt === playAttempt) playing = false;
    }
  }

  function subtitle(item: Item): string {
    const episode =
      item.parentIndex != null && item.index != null
        ? `S${item.parentIndex} · E${item.index}`
        : null;
    return [item.grandparentTitle, episode, item.year]
      .filter((part) => part != null && String(part).length > 0)
      .join(" · ");
  }
</script>

<section class="serverplaylist" aria-label="Server playlist">
  <div class="toolbar">
    <button class="back" onclick={onBack}><Icon name="back" size={15} /> Playlists</button>
  </div>
  <div class="heading">
    <div>
      <p class="eyebrow">Read-only server playlist</p>
      <h1>{playlist.title}</h1>
      <p class="muted">{playlist.sourceName}</p>
    </div>
    {#if items.length > 0}
      <button class="primary" disabled={loading || playing} onclick={() => play(0, false)}>
        <Icon name="play" size={15} /> Play playlist
      </button>
    {/if}
  </div>

  {#if status}
    <div class:failure={status.failed} class="status" role={status.failed ? "alert" : "status"}>
      {status.failed ? friendlyError(status.text) : status.text}
    </div>
  {/if}

  {#if loading}
    <div class="empty" aria-busy="true">Loading server playlist…</div>
  {:else if items.length === 0 && !status?.failed}
    <div class="empty">This server playlist is empty.</div>
  {:else if items.length > 0}
    <ol class="entries" aria-label="Server playlist items">
      {#each items as item, index (`${item.ratingKey}:${index}`)}
        {@const art = item.poster ?? item.seriesPoster}
        {@const inProgress = (item.viewOffsetMs ?? 0) > 0}
        <li>
          <span class="position" aria-hidden="true">{index + 1}</span>
          <div class="thumb" aria-hidden="true">
            {#if art}
              <img src={posterSrc(art)} alt="" onerror={(event) => ((event.currentTarget as HTMLImageElement).style.display = "none")} />
            {:else}
              <Icon name="film" size={20} stroke={1.5} />
            {/if}
          </div>
          <div class="entrymeta">
            <strong>{item.title}</strong>
            {#if subtitle(item)}<span>{subtitle(item)}</span>{/if}
          </div>
          <div class="entryactions" aria-label="Playback for {item.title}">
            <button disabled={playing} onclick={() => play(index, false)}>
              {inProgress ? "Resume" : "Play"}
            </button>
            {#if inProgress}
              <button disabled={playing} onclick={() => play(index, true)}>Beginning</button>
            {/if}
          </div>
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .serverplaylist { flex: 1; overflow-y: auto; padding: 1.2rem 1.5rem 3rem; }
  .toolbar { margin-bottom: 0.8rem; }
  button:not(.primary) { font: inherit; }
  .back, .entryactions button { background: var(--surface); border: 1px solid var(--border); color: var(--text-2); border-radius: 7px; padding: 0.4rem 0.65rem; cursor: pointer; }
  .back { display: inline-flex; align-items: center; gap: 0.3rem; border: none; }
  button:hover:not(:disabled):not(.primary) { color: var(--text-bright); background: var(--surface-2); }
  button:disabled:not(.primary) { opacity: 0.45; cursor: default; }
  .heading { display: flex; align-items: flex-end; justify-content: space-between; gap: 2rem; margin-bottom: 1rem; }
  .heading h1 { font-family: var(--font-display); font-size: clamp(1.8rem, 4vw, 3rem); margin: 0.1rem 0; letter-spacing: -0.035em; }
  .eyebrow { color: var(--accent); text-transform: uppercase; letter-spacing: 0.1em; font-size: 0.7rem; font-weight: 700; margin: 0; }
  .muted { color: var(--text-muted); margin: 0.2rem 0 0; }
  .status { color: var(--text-2); background: var(--surface); border-left: 3px solid var(--accent); border-radius: 5px; padding: 0.55rem 0.75rem; margin: 0.8rem 0; }
  .status.failure { color: var(--danger-text); border-color: var(--danger-border); background: var(--danger-bg); }
  .empty { color: var(--text-muted); border: 1px dashed var(--border); border-radius: 12px; padding: 2rem; text-align: center; }
  .entries { list-style: none; padding: 0; margin: 1rem 0; display: flex; flex-direction: column; gap: 0.5rem; }
  .entries li { display: grid; grid-template-columns: 2rem 3rem minmax(8rem, 1fr) auto; gap: 0.7rem; align-items: center; padding: 0.55rem; background: var(--surface); border: 1px solid var(--border-subtle); border-radius: 10px; }
  .position { color: var(--text-dim); text-align: center; font-variant-numeric: tabular-nums; }
  .thumb { width: 3rem; height: 3rem; border-radius: 6px; background: var(--surface-2); color: var(--text-dim); overflow: hidden; display: grid; place-items: center; }
  .thumb img { width: 100%; height: 100%; object-fit: cover; }
  .entrymeta { min-width: 0; display: flex; flex-direction: column; gap: 0.15rem; }
  .entrymeta strong, .entrymeta span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .entrymeta span { color: var(--text-muted); font-size: 0.8rem; }
  .entryactions { display: flex; justify-content: flex-end; gap: 0.35rem; }
  .entryactions button { font-size: 0.78rem; padding: 0.35rem 0.5rem; }
  @media (max-width: 860px) {
    .heading { align-items: stretch; flex-direction: column; gap: 1rem; }
    .entries li { grid-template-columns: 1.6rem 2.6rem 1fr; }
    .thumb { width: 2.6rem; height: 2.6rem; }
    .entryactions { grid-column: 2 / -1; justify-content: flex-start; }
  }
</style>
