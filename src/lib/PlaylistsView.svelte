<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "$lib/Icon.svelte";
  import { friendlyError } from "$lib/errors";
  import type { Playlist, PlaylistEntry, PlaylistSummary } from "$lib/types";

  let {
    sourceVersion = 0,
    posterSrc,
  }: {
    sourceVersion?: number;
    posterSrc: (poster: string) => string;
  } = $props();

  type Status = { text: string; failed: boolean };

  let playlists = $state<PlaylistSummary[]>([]);
  let selectedId = $state<string | null>(null);
  let playlist = $state<Playlist | null>(null);
  let loadingList = $state(true);
  let loadingDetail = $state(false);
  let listBusy = $state(false);
  let detailBusy = $state(false);
  let listStatus = $state<Status | null>(null);
  let detailStatus = $state<Status | null>(null);
  let createName = $state("");
  let renameName = $state("");
  let confirmDelete = $state(false);
  let listLoadAttempt = 0;
  let listActionAttempt = 0;
  let detailLoadAttempt = 0;
  let detailActionAttempt = 0;
  let observedSourceVersion = $state(-1);

  $effect(() => {
    if (sourceVersion === observedSourceVersion) return;
    observedSourceVersion = sourceVersion;
    void loadSummaries();
    if (selectedId) void loadDetail(selectedId);
  });

  async function loadSummaries(): Promise<void> {
    const attempt = ++listLoadAttempt;
    loadingList = playlists.length === 0;
    listStatus = null;
    try {
      const loaded = await invoke<PlaylistSummary[]>("playlist_list");
      if (attempt === listLoadAttempt) playlists = loaded;
    } catch (error) {
      if (attempt === listLoadAttempt) {
        listStatus = { text: String(error), failed: true };
      }
    } finally {
      if (attempt === listLoadAttempt) loadingList = false;
    }
  }

  async function loadDetail(id: string): Promise<void> {
    const attempt = ++detailLoadAttempt;
    loadingDetail = playlist?.id !== id;
    detailStatus = null;
    try {
      const loaded = await invoke<Playlist>("playlist_get", { id });
      if (attempt === detailLoadAttempt && selectedId === id) {
        playlist = loaded;
        renameName = loaded.name;
      }
    } catch (error) {
      if (attempt === detailLoadAttempt && selectedId === id) {
        detailStatus = { text: String(error), failed: true };
      }
    } finally {
      if (attempt === detailLoadAttempt) loadingDetail = false;
    }
  }

  function openPlaylist(id: string): void {
    detailActionAttempt++;
    detailStatus = null;
    confirmDelete = false;
    selectedId = id;
    playlist = null;
    void loadDetail(id);
  }

  function backToList(): void {
    detailLoadAttempt++;
    detailActionAttempt++;
    selectedId = null;
    playlist = null;
    detailStatus = null;
    confirmDelete = false;
    void loadSummaries();
  }

  async function createPlaylist(): Promise<void> {
    const name = createName.trim();
    if (!name || listBusy) return;
    const attempt = ++listActionAttempt;
    listBusy = true;
    listStatus = null;
    try {
      const created = await invoke<{ id: string }>("playlist_create", { name });
      if (attempt !== listActionAttempt) return;
      createName = "";
      await loadSummaries();
      openPlaylist(created.id);
    } catch (error) {
      if (attempt === listActionAttempt) {
        listStatus = { text: String(error), failed: true };
      }
    } finally {
      if (attempt === listActionAttempt) listBusy = false;
    }
  }

  async function runDetailAction(
    command: string,
    args: Record<string, unknown>,
    success: string,
  ): Promise<void> {
    const id = selectedId;
    if (!id || detailBusy) return;
    const attempt = ++detailActionAttempt;
    detailBusy = true;
    detailStatus = null;
    try {
      await invoke(command, args);
      if (attempt !== detailActionAttempt || selectedId !== id) return;
      await loadDetail(id);
      if (attempt === detailActionAttempt && selectedId === id) {
        detailStatus = { text: success, failed: false };
      }
      await loadSummaries();
    } catch (error) {
      if (attempt === detailActionAttempt && selectedId === id) {
        detailStatus = { text: String(error), failed: true };
      }
    } finally {
      if (attempt === detailActionAttempt) detailBusy = false;
    }
  }

  function renamePlaylist(): void {
    const id = selectedId;
    const name = renameName.trim();
    if (!id || !name) return;
    void runDetailAction("playlist_rename", { id, name }, "Playlist renamed.");
  }

  function removeEntry(entry: PlaylistEntry): void {
    if (!selectedId) return;
    void runDetailAction(
      "playlist_remove_item",
      { id: selectedId, entryId: entry.id },
      `Removed “${entry.item.title}”.`,
    );
  }

  function moveEntry(entry: PlaylistEntry, toIndex: number): void {
    if (!selectedId) return;
    void runDetailAction(
      "playlist_reorder",
      { id: selectedId, entryId: entry.id, toIndex },
      "Playlist order updated.",
    );
  }

  async function deletePlaylist(): Promise<void> {
    const id = selectedId;
    if (!id || detailBusy) return;
    const attempt = ++detailActionAttempt;
    detailBusy = true;
    detailStatus = null;
    try {
      await invoke("playlist_delete", { id });
      if (attempt !== detailActionAttempt || selectedId !== id) return;
      selectedId = null;
      playlist = null;
      confirmDelete = false;
      await loadSummaries();
      listStatus = { text: "Playlist deleted.", failed: false };
    } catch (error) {
      if (attempt === detailActionAttempt && selectedId === id) {
        detailStatus = { text: String(error), failed: true };
      }
    } finally {
      if (attempt === detailActionAttempt) detailBusy = false;
    }
  }

  async function playEntry(index: number, beginning: boolean): Promise<void> {
    const id = selectedId;
    const entry = playlist?.items[index];
    if (!id || !entry || !entry.available || detailBusy) return;
    const attempt = ++detailActionAttempt;
    detailBusy = true;
    detailStatus = null;
    try {
      await invoke("playlist_play", {
        id,
        startIndex: index,
        startFromBeginning: beginning,
      });
      if (attempt === detailActionAttempt && selectedId === id) {
        detailStatus = { text: `Playing “${entry.item.title}”.`, failed: false };
      }
    } catch (error) {
      if (attempt === detailActionAttempt && selectedId === id) {
        detailStatus = {
          text: `Couldn't play “${entry.item.title}” — ${String(error)}`,
          failed: true,
        };
      }
    } finally {
      if (attempt === detailActionAttempt) detailBusy = false;
    }
  }

  function subtitle(entry: PlaylistEntry): string {
    const item = entry.item;
    const episode =
      item.parentIndex != null && item.index != null
        ? `S${item.parentIndex} · E${item.index}`
        : null;
    return [item.grandparentTitle, episode, item.year, entry.sourceName]
      .filter((part) => part != null && String(part).length > 0)
      .join(" · ");
  }
</script>

<section class="playlists" aria-label="Playlists">
  {#if selectedId}
    <div class="toolbar">
      <button class="back" onclick={backToList}><Icon name="back" size={15} /> Playlists</button>
    </div>
    {#if detailStatus}
      <div class:failure={detailStatus.failed} class="status" role={detailStatus.failed ? "alert" : "status"}>
        {detailStatus.failed ? friendlyError(detailStatus.text) : detailStatus.text}
      </div>
    {/if}
    {#if loadingDetail && !playlist}
      <div class="empty" aria-busy="true">Loading playlist…</div>
    {:else if playlist}
      <div class="heading">
        <div>
          <p class="eyebrow">Vela playlist</p>
          <h1>{playlist.name}</h1>
          <p class="muted">{playlist.items.length} {playlist.items.length === 1 ? "item" : "items"}</p>
        </div>
        <div class="rename">
          <label for="playlist-rename">Name</label>
          <div class="inline">
            <input id="playlist-rename" aria-label="Playlist name" bind:value={renameName} maxlength="120" />
            <button disabled={detailBusy || !renameName.trim()} onclick={renamePlaylist}>Save</button>
          </div>
        </div>
      </div>

      {#if playlist.items.length === 0}
        <div class="empty">This playlist is empty. Add items from any title's context menu.</div>
      {:else}
        <ol class="entries" aria-label="Playlist items">
          {#each playlist.items as entry, index (entry.id)}
            {@const art = entry.item.poster ?? entry.item.seriesPoster}
            {@const inProgress = (entry.item.viewOffsetMs ?? 0) > 0}
            <li class:unavailable={!entry.available}>
              <span class="position" aria-hidden="true">{index + 1}</span>
              <div class="thumb" aria-hidden="true">
                {#if art}
                  <img src={posterSrc(art)} alt="" onerror={(event) => ((event.currentTarget as HTMLImageElement).style.display = "none")} />
                {:else}
                  <Icon name="film" size={20} stroke={1.5} />
                {/if}
              </div>
              <div class="entrymeta">
                <strong>{entry.item.title}</strong>
                {#if subtitle(entry)}<span>{subtitle(entry)}</span>{/if}
                {#if !entry.available}<span class="dead">Unavailable</span>{/if}
              </div>
              <div class="entryactions" aria-label="Actions for {entry.item.title}">
                <button disabled={detailBusy || !entry.available} onclick={() => playEntry(index, false)}>
                  {inProgress ? "Resume" : "Play"}
                </button>
                {#if inProgress}
                  <button disabled={detailBusy || !entry.available} onclick={() => playEntry(index, true)}>Beginning</button>
                {/if}
                <button aria-label="Move {entry.item.title} up" disabled={detailBusy || index === 0} onclick={() => moveEntry(entry, index - 1)}>Up</button>
                <button aria-label="Move {entry.item.title} down" disabled={detailBusy || index === playlist!.items.length - 1} onclick={() => moveEntry(entry, index + 1)}>Down</button>
                <button class="dangertext" disabled={detailBusy} onclick={() => removeEntry(entry)}>Remove</button>
              </div>
            </li>
          {/each}
        </ol>
      {/if}

      <div class="deletezone">
        {#if confirmDelete}
          <span>Delete “{playlist.name}”?</span>
          <button class="danger" disabled={detailBusy} onclick={deletePlaylist}>Delete permanently</button>
          <button disabled={detailBusy} onclick={() => (confirmDelete = false)}>Cancel</button>
        {:else}
          <button class="dangertext" disabled={detailBusy} onclick={() => (confirmDelete = true)}>Delete playlist…</button>
        {/if}
      </div>
    {/if}
  {:else}
    <div class="heading listheading">
      <div>
        <p class="eyebrow">Across every server</p>
        <h1>Playlists</h1>
        <p class="muted">Durable Vela playlists can mix Plex, Jellyfin, and Emby items.</p>
      </div>
      <form class="create" onsubmit={(event) => { event.preventDefault(); void createPlaylist(); }}>
        <label for="playlist-create">New playlist</label>
        <div class="inline">
          <input id="playlist-create" aria-label="New playlist name" placeholder="Playlist name" bind:value={createName} maxlength="120" />
          <button class="primary" disabled={listBusy || !createName.trim()} type="submit"><Icon name="plus" size={15} /> Create</button>
        </div>
      </form>
    </div>
    {#if listStatus}
      <div class:failure={listStatus.failed} class="status" role={listStatus.failed ? "alert" : "status"}>
        {listStatus.failed ? friendlyError(listStatus.text) : listStatus.text}
      </div>
    {/if}
    {#if loadingList && playlists.length === 0}
      <div class="empty" aria-busy="true">Loading playlists…</div>
    {:else if playlists.length === 0}
      <div class="empty">No playlists yet. Create one here, then add titles from their context menus.</div>
    {:else}
      <div class="playlistgrid">
        {#each playlists as saved (saved.id)}
          <button onclick={() => openPlaylist(saved.id)} aria-label={`Open ${saved.name}, ${saved.itemCount} items`}>
            <span class="listicon"><Icon name="playlist" size={22} /></span>
            <strong>{saved.name}</strong>
            <span>{saved.itemCount} {saved.itemCount === 1 ? "item" : "items"}</span>
          </button>
        {/each}
      </div>
    {/if}
  {/if}
</section>

<style>
  .playlists { flex: 1; overflow-y: auto; padding: 1.2rem 1.5rem 3rem; }
  .toolbar { margin-bottom: 0.8rem; }
  button { font: inherit; }
  .back, .entryactions button, .rename button, .deletezone button { background: var(--surface); border: 1px solid var(--border); color: var(--text-2); border-radius: 7px; padding: 0.4rem 0.65rem; cursor: pointer; }
  .back { display: inline-flex; align-items: center; gap: 0.3rem; border: none; }
  button:hover:not(:disabled) { color: var(--text-bright); background: var(--surface-2); }
  button:disabled { opacity: 0.45; cursor: default; }
  .heading { display: flex; align-items: flex-end; justify-content: space-between; gap: 2rem; margin-bottom: 1rem; }
  .heading h1 { font-family: var(--font-display); font-size: clamp(1.8rem, 4vw, 3rem); margin: 0.1rem 0; letter-spacing: -0.035em; }
  .eyebrow { color: var(--accent); text-transform: uppercase; letter-spacing: 0.1em; font-size: 0.7rem; font-weight: 700; margin: 0; }
  .muted { color: var(--text-muted); margin: 0.2rem 0 0; }
  .create, .rename { min-width: min(100%, 22rem); }
  label { display: block; color: var(--text-muted); font-size: 0.78rem; margin-bottom: 0.3rem; }
  .inline { display: flex; gap: 0.45rem; }
  input { min-width: 0; flex: 1; background: var(--surface); border: 1px solid var(--border); color: var(--text); border-radius: 8px; padding: 0.5rem 0.65rem; }
  input:focus { outline: none; border-color: var(--accent); box-shadow: 0 0 0 3px rgba(229, 160, 13, 0.15); }
  .primary { display: inline-flex; align-items: center; gap: 0.3rem; border: none; background: var(--accent); color: var(--on-accent); border-radius: 8px; padding: 0.5rem 0.8rem; cursor: pointer; white-space: nowrap; }
  .status { color: var(--text-2); background: var(--surface); border-left: 3px solid var(--accent); border-radius: 5px; padding: 0.55rem 0.75rem; margin: 0.8rem 0; }
  .status.failure { color: #ffb4ad; border-color: #e25d52; background: rgba(120, 24, 20, 0.24); }
  .empty { color: var(--text-muted); border: 1px dashed var(--border); border-radius: 12px; padding: 2rem; text-align: center; }
  .playlistgrid { display: grid; grid-template-columns: repeat(auto-fill, minmax(13rem, 1fr)); gap: 0.8rem; }
  .playlistgrid > button { display: grid; grid-template-columns: auto 1fr; grid-template-rows: auto auto; gap: 0.2rem 0.7rem; align-items: center; text-align: left; background: var(--surface); color: var(--text); border: 1px solid var(--border); border-radius: 12px; padding: 1rem; cursor: pointer; }
  .playlistgrid .listicon { grid-row: 1 / 3; color: var(--accent); }
  .playlistgrid span:last-child { color: var(--text-muted); font-size: 0.82rem; }
  .entries { list-style: none; padding: 0; margin: 1rem 0; display: flex; flex-direction: column; gap: 0.5rem; }
  .entries li { display: grid; grid-template-columns: 2rem 3rem minmax(8rem, 1fr) auto; gap: 0.7rem; align-items: center; padding: 0.55rem; background: var(--surface); border: 1px solid var(--border-subtle); border-radius: 10px; }
  .entries li.unavailable { opacity: 0.72; }
  .position { color: var(--text-dim); text-align: center; font-variant-numeric: tabular-nums; }
  .thumb { width: 3rem; height: 3rem; border-radius: 6px; background: var(--surface-2); color: var(--text-dim); overflow: hidden; display: grid; place-items: center; }
  .thumb img { width: 100%; height: 100%; object-fit: cover; }
  .entrymeta { min-width: 0; display: flex; flex-direction: column; gap: 0.15rem; }
  .entrymeta strong, .entrymeta span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .entrymeta span { color: var(--text-muted); font-size: 0.8rem; }
  .entrymeta .dead { color: #ffb4ad; font-weight: 650; }
  .entryactions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 0.35rem; }
  .entryactions button { font-size: 0.78rem; padding: 0.35rem 0.5rem; }
  .dangertext { color: #ff9e96 !important; }
  .danger { background: #a62e29 !important; border-color: #c94a44 !important; color: white !important; }
  .deletezone { display: flex; align-items: center; gap: 0.5rem; justify-content: flex-end; margin-top: 1.5rem; color: var(--text-muted); font-size: 0.85rem; }
  @media (max-width: 860px) {
    .heading { align-items: stretch; flex-direction: column; gap: 1rem; }
    .create, .rename { width: 100%; }
    .entries li { grid-template-columns: 1.6rem 2.6rem 1fr; }
    .thumb { width: 2.6rem; height: 2.6rem; }
    .entryactions { grid-column: 2 / -1; justify-content: flex-start; }
  }
</style>
