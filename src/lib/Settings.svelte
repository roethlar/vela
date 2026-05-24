<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { onMount, onDestroy } from "svelte";

  type Source = { id: string; name: string; kind: string };
  type LocalFolder = { id: string; name: string; path: string; kind: string };
  type SmbMount = { id: string; name: string; server: string; share: string; mountpoint: string };
  type SshMount = {
    id: string;
    name: string;
    host: string;
    port: number;
    username: string;
    remotePath: string;
    identityFile: string;
    mountpoint: string;
  };

  let {
    onClose,
    onChanged,
    onLinkPlex,
  }: { onClose: () => void; onChanged: () => void; onLinkPlex: () => void } = $props();

  // Modal focus management: move focus into the dialog on open, trap Tab inside,
  // and restore focus to the trigger on close.
  let panel: HTMLDivElement | undefined;
  let closeBtn: HTMLButtonElement | undefined;
  let prevFocus: HTMLElement | null = null;

  onMount(() => {
    prevFocus = document.activeElement as HTMLElement | null;
    closeBtn?.focus();
  });
  onDestroy(() => prevFocus?.focus());

  function trapFocus(e: KeyboardEvent) {
    if (e.key !== "Tab" || !panel) return;
    const f = panel.querySelectorAll<HTMLElement>(
      'a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])'
    );
    if (f.length === 0) return;
    const first = f[0];
    const last = f[f.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  let sources = $state<Source[]>([]);
  let folders = $state<LocalFolder[]>([]);
  let smbMounts = $state<SmbMount[]>([]);
  let sshMounts = $state<SshMount[]>([]);
  let busy = $state(false);
  let err = $state<string | null>(null);

  // Add SMB share
  let smbServer = $state("");
  let smbShare = $state("");
  let smbUser = $state("");
  let smbPass = $state("");
  let smbDomain = $state("");
  let smbKind = $state<"" | "movie" | "show">("");

  // Add SSH/SFTP folder
  let sshHost = $state("");
  let sshPort = $state("22");
  let sshUser = $state("");
  let sshPath = $state("");
  let sshKey = $state("");
  let sshKind = $state<"" | "movie" | "show">("");

  // Add Jellyfin/Emby form
  let kind = $state<"jellyfin" | "emby">("jellyfin");
  let url = $state("");
  let username = $state("");
  let password = $state("");
  let useApiKey = $state(false);
  let apiKey = $state("");
  let userId = $state("");

  // Add local folder
  let folderKind = $state<"" | "movie" | "show">("");

  // Guards against a slow initial load resolving after a later add/remove
  // refresh and overwriting the panel with stale lists.
  let loadSeq = 0;

  load();

  async function load() {
    const seq = ++loadSeq;
    try {
      const [s, f, m, ssh] = await Promise.all([
        invoke<Source[]>("get_sources"),
        invoke<LocalFolder[]>("list_local_folders"),
        invoke<SmbMount[]>("list_smb_mounts"),
        invoke<SshMount[]>("list_ssh_mounts"),
      ]);
      if (seq !== loadSeq) return;
      sources = s;
      folders = f;
      smbMounts = m;
      sshMounts = ssh;
    } catch (e) {
      if (seq === loadSeq) err = String(e);
    }
  }

  // Local folders fed by remote mounts are managed via the mount, not directly.
  let remoteFolderIds = $derived(new Set([...smbMounts, ...sshMounts].map((m) => m.mountpoint)));

  async function mountSmb() {
    if (!smbServer.trim() || !smbShare.trim()) {
      err = "Server and share are required.";
      return;
    }
    busy = true;
    err = null;
    try {
      await invoke("mount_smb", {
        server: smbServer,
        share: smbShare,
        username: smbUser,
        password: smbPass,
        domain: smbDomain.trim() || null,
        kind: smbKind || null,
      });
      smbServer = smbShare = smbUser = smbPass = smbDomain = "";
      await load();
      onChanged();
    } catch (e) {
      err = String(e);
    } finally {
      busy = false;
    }
  }

  async function unmountSmb(id: string) {
    busy = true;
    err = null;
    try {
      await invoke("unmount_smb", { id });
      await load();
      onChanged();
    } catch (e) {
      err = String(e);
    } finally {
      busy = false;
    }
  }

  async function mountSsh() {
    if (!sshHost.trim() || !sshPath.trim()) {
      err = "Host and remote path are required.";
      return;
    }
    const parsedPort = Number(sshPort || "22");
    if (!Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65535) {
      err = "SSH port must be between 1 and 65535.";
      return;
    }
    busy = true;
    err = null;
    try {
      await invoke("mount_ssh", {
        host: sshHost,
        port: parsedPort,
        username: sshUser,
        remotePath: sshPath,
        identityFile: sshKey.trim() || null,
        kind: sshKind || null,
      });
      sshHost = sshUser = sshPath = sshKey = "";
      sshPort = "22";
      await load();
      onChanged();
    } catch (e) {
      err = String(e);
    } finally {
      busy = false;
    }
  }

  async function unmountSsh(id: string) {
    busy = true;
    err = null;
    try {
      await invoke("unmount_ssh", { id });
      await load();
      onChanged();
    } catch (e) {
      err = String(e);
    } finally {
      busy = false;
    }
  }

  async function addServer() {
    if (!url.trim()) {
      err = "Server URL is required.";
      return;
    }
    busy = true;
    err = null;
    try {
      if (useApiKey) {
        await invoke("connect_jellyfin_token", {
          kind,
          baseUrl: url,
          apiKey,
          userId: userId.trim() || null,
        });
      } else {
        await invoke("connect_jellyfin", { kind, baseUrl: url, username, password });
      }
      url = username = password = apiKey = userId = "";
      await load();
      onChanged();
    } catch (e) {
      err = String(e);
    } finally {
      busy = false;
    }
  }

  async function addFolder() {
    err = null;
    const picked = await openDialog({ directory: true, multiple: false, title: "Choose a media folder" });
    if (!picked || Array.isArray(picked)) return;
    busy = true;
    try {
      await invoke("add_local_folder", { path: picked, kind: folderKind || null });
      await load();
      onChanged();
    } catch (e) {
      err = String(e);
    } finally {
      busy = false;
    }
  }

  async function removeSource(id: string) {
    busy = true;
    err = null;
    try {
      await invoke("remove_source", { id });
      await load();
      onChanged();
    } catch (e) {
      err = String(e);
    } finally {
      busy = false;
    }
  }

  async function removeFolder(id: string) {
    busy = true;
    err = null;
    try {
      await invoke("remove_local_folder", { id });
      await load();
      onChanged();
    } catch (e) {
      err = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="overlay" role="presentation" onclick={onClose}>
  <!-- Stop propagation so clicks inside the panel don't close it. -->
  <div
    class="panel"
    role="dialog"
    aria-modal="true"
    aria-label="Sources"
    tabindex="-1"
    bind:this={panel}
    onclick={(e) => e.stopPropagation()}
    onkeydown={trapFocus}
  >
    <header>
      <h2>Sources</h2>
      <button class="x" bind:this={closeBtn} onclick={onClose} aria-label="Close">✕</button>
    </header>

    {#if err}
      <div class="err" role="alert">{err}</div>
    {/if}

    <section>
      <h3>Connected</h3>
      {#if sources.length === 0 && folders.length === 0}
        <p class="muted">No sources yet. Add one below.</p>
      {/if}
      <!-- The synthetic "local" source is managed via the folder/SMB rows below,
           not here (remove_source only handles Jellyfin/Emby). -->
      {#each sources.filter((s) => s.kind !== "local") as s (s.id)}
        <div class="row">
          <span class="badge">{s.kind}</span>
          <span class="name">{s.name}</span>
          {#if s.kind === "plex"}
            <span class="muted small">linked</span>
          {:else}
            <button class="rm" disabled={busy} onclick={() => removeSource(s.id)}>Remove</button>
          {/if}
        </div>
      {/each}
      {#each folders.filter((f) => !remoteFolderIds.has(f.path)) as f (f.id)}
        <div class="row">
          <span class="badge">local</span>
          <span class="name">{f.name}<span class="muted small"> · {f.path}</span></span>
          <button class="rm" disabled={busy} onclick={() => removeFolder(f.id)}>Remove</button>
        </div>
      {/each}
      {#each smbMounts as m (m.id)}
        <div class="row">
          <span class="badge">smb</span>
          <span class="name">{m.name}<span class="muted small"> · //{m.server}/{m.share}</span></span>
          <button class="rm" disabled={busy} onclick={() => unmountSmb(m.id)}>Unmount</button>
        </div>
      {/each}
      {#each sshMounts as m (m.id)}
        <div class="row">
          <span class="badge">ssh</span>
          <span class="name">
            {m.name}<span class="muted small"> · {m.username ? `${m.username}@` : ""}{m.host}:{m.remotePath}</span>
          </span>
          <button class="rm" disabled={busy} onclick={() => unmountSsh(m.id)}>Unmount</button>
        </div>
      {/each}
    </section>

    <section>
      <h3>Add a Jellyfin / Emby server</h3>
      <div class="form">
        <div class="field">
          <label for="srv-kind">Type</label>
          <select id="srv-kind" bind:value={kind}>
            <option value="jellyfin">Jellyfin</option>
            <option value="emby">Emby</option>
          </select>
        </div>
        <div class="field">
          <label for="srv-url">Server URL</label>
          <input id="srv-url" placeholder="http://192.168.1.10:8096" bind:value={url} />
        </div>
        {#if useApiKey}
          <div class="field">
            <label for="srv-key">API key</label>
            <input id="srv-key" bind:value={apiKey} />
          </div>
          <div class="field">
            <label for="srv-uid">User ID (optional)</label>
            <input id="srv-uid" bind:value={userId} />
          </div>
        {:else}
          <div class="field">
            <label for="srv-user">Username</label>
            <input id="srv-user" bind:value={username} />
          </div>
          <div class="field">
            <label for="srv-pass">Password</label>
            <input id="srv-pass" type="password" bind:value={password} placeholder="(blank if none)" />
          </div>
        {/if}
        <label class="check">
          <input type="checkbox" bind:checked={useApiKey} /> Use an API key instead
        </label>
        <button class="primary" disabled={busy} onclick={addServer}>
          {busy ? "Connecting…" : "Connect"}
        </button>
      </div>
    </section>

    <section>
      <h3>Add a Plex account</h3>
      <button class="primary" onclick={() => { onLinkPlex(); onClose(); }}>Link Plex…</button>
    </section>

    <section>
      <h3>Add a local / mounted folder</h3>
      <div class="form">
        <div class="field">
          <label for="fld-kind">Contains</label>
          <select id="fld-kind" bind:value={folderKind}>
            <option value="">Auto-detect</option>
            <option value="movie">Movies</option>
            <option value="show">TV Shows</option>
          </select>
        </div>
        <button class="primary" disabled={busy} onclick={addFolder}>Choose folder…</button>
      </div>
    </section>

    <section>
      <h3>Add an SMB / network share</h3>
      <div class="form">
        <div class="field">
          <label for="smb-server">Server</label>
          <input id="smb-server" placeholder="192.168.1.10 or nas.local" bind:value={smbServer} />
        </div>
        <div class="field">
          <label for="smb-share">Share</label>
          <input id="smb-share" placeholder="Media" bind:value={smbShare} />
        </div>
        <div class="field">
          <label for="smb-user">Username</label>
          <input id="smb-user" bind:value={smbUser} />
        </div>
        <div class="field">
          <label for="smb-pass">Password</label>
          <input id="smb-pass" type="password" bind:value={smbPass} placeholder="(blank for guest)" />
        </div>
        <div class="field">
          <label for="smb-domain">Domain (optional)</label>
          <input id="smb-domain" bind:value={smbDomain} />
        </div>
        <div class="field">
          <label for="smb-kind">Contains</label>
          <select id="smb-kind" bind:value={smbKind}>
            <option value="">Auto-detect</option>
            <option value="movie">Movies</option>
            <option value="show">TV Shows</option>
          </select>
        </div>
        <button class="primary" disabled={busy} onclick={mountSmb}>
          {busy ? "Mounting…" : "Mount & add"}
        </button>
        <p class="muted small">
          On Linux, Vela uses your desktop's user-space SMB mount from KIO-FUSE or GVfs.
          Open the share in your file manager first if setup cannot find a readable path.
        </p>
      </div>
    </section>

    <section>
      <h3>Add an SSH / SFTP folder</h3>
      <div class="form">
        <div class="field">
          <label for="ssh-host">Host</label>
          <input id="ssh-host" placeholder="media.example.com" bind:value={sshHost} />
        </div>
        <div class="field">
          <label for="ssh-port">Port</label>
          <input id="ssh-port" inputmode="numeric" bind:value={sshPort} />
        </div>
        <div class="field">
          <label for="ssh-user">Username</label>
          <input id="ssh-user" bind:value={sshUser} />
        </div>
        <div class="field">
          <label for="ssh-path">Remote path</label>
          <input id="ssh-path" placeholder="/srv/media" bind:value={sshPath} />
        </div>
        <div class="field">
          <label for="ssh-key">Identity file (optional)</label>
          <input id="ssh-key" placeholder="~/.ssh/id_ed25519" bind:value={sshKey} />
        </div>
        <div class="field">
          <label for="ssh-kind">Contains</label>
          <select id="ssh-kind" bind:value={sshKind}>
            <option value="">Auto-detect</option>
            <option value="movie">Movies</option>
            <option value="show">TV Shows</option>
          </select>
        </div>
        <button class="primary" disabled={busy} onclick={mountSsh}>
          {busy ? "Mounting…" : "Mount & add"}
        </button>
        <p class="muted small">
          SSH/SFTP uses sshfs with your SSH keys, agent, and ~/.ssh/config. Vela does not store SSH passwords.
        </p>
      </div>
    </section>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 4vh 1rem;
    z-index: 50;
    overflow-y: auto;
  }
  .panel {
    background: #16181d;
    border: 1px solid #2a2e37;
    border-radius: 12px;
    width: min(560px, 100%);
    padding: 1.2rem 1.4rem 1.6rem;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    cursor: default;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.5rem;
  }
  h2 {
    margin: 0;
    font-size: 1.3rem;
  }
  h3 {
    font-size: 0.95rem;
    color: #b9c0cc;
    margin: 1.2rem 0 0.5rem;
    border-top: 1px solid #2a2e37;
    padding-top: 1rem;
  }
  .x {
    background: none;
    border: none;
    color: #8a93a0;
    font-size: 1rem;
    cursor: pointer;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.4rem 0;
  }
  .row .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    background: #2a2e37;
    color: #b9c0cc;
    padding: 0.15rem 0.45rem;
    border-radius: 4px;
  }
  .muted {
    color: #8a93a0;
  }
  .small {
    font-size: 0.8rem;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  label {
    font-size: 0.8rem;
    color: #b9c0cc;
  }
  input,
  select {
    background: #0f1115;
    border: 1px solid #2a2e37;
    border-radius: 6px;
    padding: 0.5rem 0.6rem;
    color: #eaeef5;
    font-size: 0.9rem;
  }
  .check {
    flex-direction: row;
    align-items: center;
    gap: 0.4rem;
    display: flex;
  }
  button.primary {
    background: #e5a00d;
    color: #1a1205;
    border: none;
    border-radius: 6px;
    padding: 0.55rem 1.1rem;
    font-weight: 700;
    cursor: pointer;
    align-self: flex-start;
  }
  button.primary:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .rm {
    background: #2a1d1d;
    color: #ffb4b4;
    border: 1px solid #4a2a2a;
    border-radius: 6px;
    padding: 0.3rem 0.7rem;
    cursor: pointer;
  }
  .err {
    background: #3a1d1d;
    color: #ffb4b4;
    padding: 0.5rem 0.8rem;
    border-radius: 6px;
    font-size: 0.85rem;
    margin: 0.5rem 0;
  }
</style>
