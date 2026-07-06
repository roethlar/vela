<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { onMount, onDestroy } from "svelte";
  import Icon from "$lib/Icon.svelte";

  type Source = { id: string; name: string; kind: string };
  type LocalFolder = { id: string; name: string; path: string; kind: string };
  type SmbFolder = {
    id: string;
    name: string;
    path: string;
    relativePath: string;
    kind: string;
  };
  type SmbMount = {
    id: string;
    name: string;
    server: string;
    share: string;
    mountpoint: string;
    folders: SmbFolder[];
  };
  type SmbDirectory = { name: string; path: string };
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

  type SshfsStatus = { found: boolean; path: string | null; platform: string };

  type MpvInfo = {
    available: boolean;
    path: string | null;
    configuredPath: string | null;
    canAutoInstall: boolean;
    installCommand: string | null;
    installDescription: string;
    installUrl: string;
  };

  type AutocropMode = "off" | "manual" | "auto";
  type MpvAdvanced = {
    extraArgs: string;
    useOwnConfig: boolean;
    autocrop: AutocropMode;
  };

  let {
    onClose,
    onChanged,
    onLinkPlex,
    onMpvChanged,
  }: {
    onClose: () => void;
    onChanged: () => void;
    onLinkPlex: () => void;
    onMpvChanged?: (m: MpvInfo) => void;
  } = $props();

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
  // Mirrors backend LOCAL_FAMILY_KINDS (src-tauri/src/source/local.rs). These
  // synthetic sources are managed via the folder / SMB / SSH rows below — NOT
  // the registered-source list, whose Remove calls remove_source, which rejects
  // local-family ids and errors. So they must never leak a source row (with a
  // dead-end Remove button) into Connected. smb/ssh were added 2026-07-04 and
  // leaked through the old `kind !== "local"` filter (Bug 5 P1).
  const LOCAL_FAMILY_KINDS = ["local", "smb", "ssh"];
  let sshfs = $state<SshfsStatus | null>(null);
  let busy = $state(false);
  let err = $state<string | null>(null);

  // Vertical settings tabs — split the (formerly very long) panel into sections.
  type TabId = "connected" | "servers" | "folders" | "player" | "appearance";
  const tabs: { id: TabId; label: string }[] = [
    { id: "connected", label: "Connected" },
    { id: "servers", label: "Servers" },
    { id: "folders", label: "Folders" },
    { id: "player", label: "Player" },
    { id: "appearance", label: "Appearance" },
  ];
  let activeTab = $state<TabId>("connected");

  // Theme catalog (mirrors the blocks in app.css). Persisted to localStorage and
  // applied to <html data-theme>; app.html applies the saved value before paint.
  type ThemeMode = "dark" | "light";
  type ThemeDef = { id: string; label: string; mode: ThemeMode; swatch: [string, string, string] };
  const THEMES: ThemeDef[] = [
    { id: "dark", label: "Vela Dark", mode: "dark", swatch: ["#0b0d10", "#e5a00d", "#20262f"] },
    { id: "dracula", label: "Dracula", mode: "dark", swatch: ["#282a36", "#bd93f9", "#50fa7b"] },
    { id: "nord", label: "Nord", mode: "dark", swatch: ["#2e3440", "#88c0d0", "#a3be8c"] },
    { id: "solarized-dark", label: "Solarized Dark", mode: "dark", swatch: ["#002b36", "#268bd2", "#859900"] },
    { id: "gruvbox-dark", label: "Gruvbox Dark", mode: "dark", swatch: ["#282828", "#fabd2f", "#b8bb26"] },
    { id: "solarized-light", label: "Solarized Light", mode: "light", swatch: ["#fdf6e3", "#268bd2", "#cb4b16"] },
    { id: "gruvbox-light", label: "Gruvbox Light", mode: "light", swatch: ["#fbf1c7", "#d79921", "#98971a"] },
    { id: "catppuccin-latte", label: "Catppuccin Latte", mode: "light", swatch: ["#eff1f5", "#1e66f5", "#8839ef"] },
    { id: "rose-pine-dawn", label: "Rosé Pine Dawn", mode: "light", swatch: ["#faf4ed", "#286983", "#d7827e"] },
    { id: "one-light", label: "One Light", mode: "light", swatch: ["#fafafa", "#4078f2", "#50a14f"] },
  ];
  const themeModes: ThemeMode[] = ["dark", "light"];
  let theme = $state<string>(readTheme());

  function readTheme(): string {
    try {
      const t = localStorage.getItem("vela-theme");
      if (t && THEMES.some((x) => x.id === t)) return t;
    } catch {}
    return "dark";
  }

  function setTheme(id: string) {
    theme = id;
    try {
      localStorage.setItem("vela-theme", id);
    } catch {}
    document.documentElement.setAttribute("data-theme", id);
  }

  // Add SMB share
  let smbServer = $state("");
  let smbShare = $state("");
  let smbUser = $state("");
  let smbPass = $state("");
  let smbDomain = $state("");
  let smbName = $state("");
  let smbBrowseMountId = $state("");
  let smbBrowsePath = $state("");
  let smbDirectories = $state<SmbDirectory[]>([]);
  let smbFolderKind = $state<"" | "movie" | "show">("");
  let smbBrowseBusy = $state(false);

  // Add SSH/SFTP folder
  let sshHost = $state("");
  let sshPort = $state("22");
  let sshUser = $state("");
  let sshPath = $state("");
  let sshKey = $state("");
  let sshKind = $state<"" | "movie" | "show">("");
  let sshName = $state("");

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

  // mpv player location
  let mpv = $state<MpvInfo | null>(null);
  let mpvPathInput = $state("");
  let mpvBusy = $state(false);
  let installingMpv = $state(false);

  // Advanced mpv options (free-form args + own-config toggle).
  let mpvExtraArgs = $state("");
  let mpvUseOwnConfig = $state(false);
  let mpvAutocrop = $state<AutocropMode>("off");
  let mpvAdvBusy = $state(false);
  let showMpvHelp = $state(false);

  // Canned starting points shown in the contextual help. Each "Insert" appends its
  // options to the textarea so users can tweak from a working baseline.
  const mpvPresets: { label: string; args: string; help: string }[] = [
    {
      label: "Smooth playback on older / weak GPUs",
      args: "--vo=gpu\n--profile=fast\n--hdr-compute-peak=no",
      help: "Drops the heavy gpu-next renderer, high-quality scaling, and per-frame peak detection. Fixes stutter on old GPUs at the cost of some image quality. Note: this disables HDR.",
    },
    {
      label: "Force a specific GPU backend",
      args: "--gpu-api=vulkan",
      help: "Pin the graphics API (vulkan, d3d11, or opengl) when the auto-picked one misbehaves on your drivers.",
    },
    {
      label: "Sharper upscaling (strong GPUs only)",
      args: "--scale=ewa_lanczossharp\n--cscale=ewa_lanczossharp",
      help: "Higher-quality scaling for capable GPUs. Adds GPU load — skip it if you're already dropping frames.",
    },
    {
      label: "Smoother motion (reduce judder)",
      args: "--video-sync=display-resample\n--interpolation=yes\n--tscale=oversample",
      help: "Resamples frame timing to your display's refresh rate. Smoother panning, more GPU work, and not to everyone's taste.",
    },
    {
      label: "Always show the stats overlay",
      args: "--osd-level=3",
      help: "Show mpv's on-screen stats from the start (codec, hwdec, dropped frames). You can also just press Shift+I during playback.",
    },
  ];

  // Guards against a slow initial load resolving after a later add/remove
  // refresh and overwriting the panel with stale lists.
  let loadSeq = 0;

  load();

  async function load() {
    const seq = ++loadSeq;
    try {
      const [s, f, m, ssh, mp, adv, sfs] = await Promise.all([
        invoke<Source[]>("get_sources"),
        invoke<LocalFolder[]>("list_local_folders"),
        invoke<SmbMount[]>("list_smb_mounts"),
        invoke<SshMount[]>("list_ssh_mounts"),
        invoke<MpvInfo>("check_mpv"),
        invoke<MpvAdvanced>("get_mpv_advanced"),
        invoke<SshfsStatus>("sshfs_status"),
      ]);
      if (seq !== loadSeq) return;
      sources = s;
      folders = f;
      smbMounts = m;
      sshMounts = ssh;
      sshfs = sfs;
      if (smbBrowseMountId && !m.some((mount) => mount.id === smbBrowseMountId)) {
        smbBrowseMountId = "";
        smbBrowsePath = "";
        smbDirectories = [];
      }
      if (!smbBrowseMountId && m.length > 0) {
        smbBrowseMountId = m[0].id;
      }
      mpv = mp;
      mpvPathInput = mp.configuredPath ?? "";
      mpvExtraArgs = adv.extraArgs;
      mpvUseOwnConfig = adv.useOwnConfig;
      mpvAutocrop = adv.autocrop;
    } catch (e) {
      if (seq === loadSeq) err = String(e);
    }
  }

  // Local folders fed by remote mounts are managed via the mount, not directly.
  let remoteFolderIds = $derived(new Set(sshMounts.map((m) => m.mountpoint)));

  async function mountSmb() {
    if (!smbServer.trim() || !smbShare.trim()) {
      err = "Server and share are required.";
      return;
    }
    const mountedServer = smbServer.trim();
    const mountedShare = smbShare.trim();
    busy = true;
    err = null;
    try {
      await invoke("mount_smb", {
        server: smbServer,
        share: smbShare,
        username: smbUser,
        password: smbPass,
        domain: smbDomain.trim() || null,
        name: smbName.trim() || null,
      });
      smbServer = smbShare = smbUser = smbPass = smbDomain = smbName = "";
      await load();
      const mounted = smbMounts.find(
        (mount) =>
          mount.server.toLowerCase() === mountedServer.toLowerCase() &&
          mount.share.toLowerCase() === mountedShare.toLowerCase()
      );
      if (mounted) {
        await loadSmbDirectories(mounted.id, "");
      }
      onChanged();
    } catch (e) {
      err = String(e);
    } finally {
      busy = false;
    }
  }

  async function loadSmbDirectories(id = smbBrowseMountId, path = smbBrowsePath) {
    if (!id) return;
    smbBrowseBusy = true;
    err = null;
    try {
      smbDirectories = await invoke<SmbDirectory[]>("list_smb_directories", {
        id,
        path: path || null,
      });
      smbBrowseMountId = id;
      smbBrowsePath = path;
    } catch (e) {
      smbDirectories = [];
      err = String(e);
    } finally {
      smbBrowseBusy = false;
    }
  }

  async function addSmbFolder() {
    if (!smbBrowseMountId) {
      err = "Add an SMB share first.";
      return;
    }
    busy = true;
    err = null;
    try {
      await invoke("add_smb_folder", {
        id: smbBrowseMountId,
        path: smbBrowsePath,
        kind: smbFolderKind || null,
      });
      await load();
      onChanged();
    } catch (e) {
      err = String(e);
    } finally {
      busy = false;
    }
  }

  async function removeSmbFolder(id: string, folderId: string) {
    // Removing a share's last folder would leave a zombie zero-folder mount (an
    // invisible share that browses nothing). Cascade to a full unmount instead:
    // the UX ruling forbids an erroring/dead-end click, and a share with no
    // folders is useless. The backend also refuses to empty a mount as a guard
    // (Bug 5 P1).
    const mount = smbMounts.find((m) => m.id === id);
    if (mount && mount.folders.length <= 1) {
      await unmountSmb(id);
      return;
    }
    busy = true;
    err = null;
    try {
      await invoke("remove_smb_folder", { id, folderId });
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
        name: sshName.trim() || null,
      });
      sshHost = sshUser = sshPath = sshKey = sshName = "";
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

  async function unlinkPlex() {
    if (!confirm("Disconnect the Plex account? You'll need to re-link to use it again.")) return;
    busy = true;
    err = null;
    try {
      await invoke("unlink_plex");
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

  function applyMpv(m: MpvInfo) {
    mpv = m;
    mpvPathInput = m.configuredPath ?? "";
    onMpvChanged?.(m);
  }

  async function browseMpv() {
    err = null;
    const picked = await openDialog({
      directory: false,
      multiple: false,
      title: "Locate the mpv executable",
      // On Windows mpv is an .exe; elsewhere it has no extension, so allow all.
      filters:
        navigatorIsWindows()
          ? [{ name: "mpv", extensions: ["exe"] }]
          : undefined,
    });
    if (!picked || Array.isArray(picked)) return;
    mpvPathInput = picked;
    await saveMpvPath();
  }

  async function saveMpvPath() {
    mpvBusy = true;
    err = null;
    try {
      const m = await invoke<MpvInfo>("set_mpv_path", {
        path: mpvPathInput.trim() || null,
      });
      applyMpv(m);
    } catch (e) {
      err = String(e);
    } finally {
      mpvBusy = false;
    }
  }

  async function clearMpvPath() {
    mpvPathInput = "";
    await saveMpvPath();
  }

  async function installMpv() {
    if (installingMpv) return;
    installingMpv = true;
    err = null;
    try {
      const m = await invoke<MpvInfo>("install_mpv");
      applyMpv(m);
    } catch (e) {
      err = String(e);
    } finally {
      installingMpv = false;
    }
  }

  async function saveMpvAdvanced() {
    mpvAdvBusy = true;
    err = null;
    try {
      await invoke("set_mpv_advanced", {
        extraArgs: mpvExtraArgs,
        useOwnConfig: mpvUseOwnConfig,
        autocrop: mpvAutocrop,
      });
    } catch (e) {
      err = String(e);
    } finally {
      mpvAdvBusy = false;
    }
  }

  // Append a preset's options to the textarea so the user tweaks from a baseline
  // rather than replacing what they already typed.
  function insertPreset(args: string) {
    const cur = mpvExtraArgs.trimEnd();
    mpvExtraArgs = (cur ? cur + "\n" : "") + args + "\n";
  }

  function navigatorIsWindows() {
    return typeof navigator !== "undefined" && /Win/i.test(navigator.platform);
  }

  function parentSmbPath(path: string) {
    const parts = path.split("/").filter(Boolean);
    parts.pop();
    return parts.join("/");
  }

  function smbPathLabel(path: string) {
    return path || "Share root";
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
    aria-label="Settings"
    tabindex="-1"
    bind:this={panel}
    onclick={(e) => e.stopPropagation()}
    onkeydown={trapFocus}
  >
    <header>
      <h2>Settings</h2>
      <button class="x" bind:this={closeBtn} onclick={onClose} aria-label="Close"><Icon name="close" size={17} /></button>
    </header>

    {#if err}
      <div class="err" role="alert">{err}</div>
    {/if}

    <div class="tabwrap">
      <div class="tabs" role="tablist" aria-orientation="vertical" aria-label="Settings sections">
        {#each tabs as t}
          <button
            role="tab"
            aria-selected={activeTab === t.id}
            class:active={activeTab === t.id}
            onclick={() => (activeTab = t.id)}
          >{t.label}</button>
        {/each}
      </div>

      <div class="tabcontent" role="tabpanel">

    {#if activeTab === "connected"}
    <section>
      <h3>Connected</h3>
      {#if sources.length === 0 && folders.length === 0}
        <p class="muted">No sources yet. Add one below.</p>
      {/if}
      <!-- The synthetic local-family sources (local/smb/ssh) are managed via the
           folder/SMB/SSH rows below, not here (remove_source rejects their ids).
           Excluding the whole family drops the leaked smb/ssh rows AND their
           erroring Remove button (Bug 5 P1). -->
      {#each sources.filter((s) => !LOCAL_FAMILY_KINDS.includes(s.kind)) as s (s.id)}
        <div class="row">
          <span class="badge">{s.kind}</span>
          <span class="name">{s.name}</span>
          {#if s.kind === "plex"}
            <button class="rm" disabled={busy} onclick={unlinkPlex}>Disconnect</button>
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
          <button class="rm" disabled={busy} onclick={() => unmountSmb(m.id)}>Remove</button>
        </div>
        {#each m.folders as f (f.id)}
          <div class="row subrow">
            <span class="badge">{f.kind || "auto"}</span>
            <span class="name">{f.name}<span class="muted small"> · {smbPathLabel(f.relativePath)}</span></span>
            <button class="rm" disabled={busy} onclick={() => removeSmbFolder(m.id, f.id)}>Remove</button>
          </div>
        {/each}
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
    {/if}

    {#if activeTab === "player"}
    <section>
      <h3>mpv player</h3>
      {#if mpv}
        <p class="muted small">
          {#if mpv.available}
            ✓ Found mpv at <code class="path">{mpv.path}</code>
          {:else}
            ✗ mpv wasn't found. Install it below, or point Vela at an existing mpv.
          {/if}
        </p>
      {/if}
      <div class="form">
        <div class="field">
          <label for="mpv-path">mpv executable path (optional override)</label>
          <input
            id="mpv-path"
            placeholder={navigatorIsWindows() ? "C:\\Program Files\\mpv\\mpv.exe" : "/usr/local/bin/mpv"}
            bind:value={mpvPathInput}
          />
        </div>
        <div class="btnrow">
          <button class="primary" disabled={mpvBusy} onclick={saveMpvPath}>
            {mpvBusy ? "Saving…" : "Save path"}
          </button>
          <button disabled={mpvBusy} onclick={browseMpv}>Browse…</button>
          {#if mpv?.configuredPath}
            <button class="rm" disabled={mpvBusy} onclick={clearMpvPath}>Clear</button>
          {/if}
        </div>
        {#if mpv?.canAutoInstall}
          <button class="primary" disabled={installingMpv} onclick={installMpv}>
            {installingMpv ? "Installing mpv…" : "Install mpv automatically"}
          </button>
          <p class="muted small">
            {mpv.installDescription}. Needs an internet connection.
          </p>
        {/if}

        <div class="warn">
          <b>⚠ Advanced — requires mpv knowledge.</b>
          These options are passed straight to mpv, exactly as written. Wrong or
          unsupported options can degrade quality or stop playback. If you're not
          comfortable with mpv's command-line options, leave this blank — Vela's
          defaults are already tuned for HDR.
        </div>

        <div class="field">
          <label for="mpv-extra">Advanced mpv options</label>
          <textarea
            id="mpv-extra"
            rows="4"
            spellcheck="false"
            placeholder={"One option per line, e.g.\n--vo=gpu\n--profile=fast"}
            bind:value={mpvExtraArgs}
          ></textarea>
          <p class="muted small">
            One mpv option per line, appended when launching mpv — so these override
            Vela's defaults. Vela's playback tracking (its IPC socket) and the media
            URL are protected and can't be overridden. Lines starting with
            <code>#</code> are ignored. A bad option just makes mpv fail to start.
          </p>
        </div>

        <label class="check">
          <input type="checkbox" bind:checked={mpvUseOwnConfig} />
          Use my own mpv config (<code>~/.config/mpv/mpv.conf</code>)
        </label>
        <p class="muted small">
          By default Vela launches mpv with <code>--no-config</code> for a predictable
          setup. Tick this to load your own <code>mpv.conf</code> instead — it can then
          change anything, including settings that disable HDR or break playback.
        </p>

        <div class="field">
          <label for="mpv-autocrop">Black-bar cropping (mpv autocrop)</label>
          <select id="mpv-autocrop" bind:value={mpvAutocrop}>
            <option value="off">Off</option>
            <option value="manual">Manual — press Shift+C to crop</option>
            <option value="auto">Automatic — crop every video</option>
          </select>
          <p class="muted small">
            Loads mpv's bundled <code>autocrop.lua</code> to remove black bars.
            <b>Manual</b> only crops when you press <code>Shift+C</code> during
            playback. <b>Automatic</b> crops every video on its own.
          </p>
          {#if mpvAutocrop === "auto"}
            <p class="warn small">
              ⚠ Automatic cropping runs at the start of every video and can be
              unreliable on HDR content — on some GPU/Wayland setups it may
              occasionally hang mpv (unkillable). If playback freezes, switch back to
              Off or Manual.
            </p>
          {/if}
        </div>

        <div class="btnrow">
          <button class="primary" disabled={mpvAdvBusy} onclick={saveMpvAdvanced}>
            {mpvAdvBusy ? "Saving…" : "Save mpv options"}
          </button>
          <button onclick={() => (showMpvHelp = !showMpvHelp)}>
            {showMpvHelp ? "Hide examples" : "Show examples"}
          </button>
        </div>

        {#if showMpvHelp}
          <div class="presets">
            {#each mpvPresets as p}
              <div class="preset">
                <div class="preset-head">
                  <b>{p.label}</b>
                  <button class="ins" onclick={() => insertPreset(p.args)}>Insert</button>
                </div>
                <code class="preset-args">{p.args.split("\n").join("   ")}</code>
                <p class="muted small">{p.help}</p>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </section>
    {/if}

    {#if activeTab === "servers"}
    <section>
      <h3>Add a Plex account</h3>
      <button class="primary" onclick={() => { onLinkPlex(); onClose(); }}>Link Plex…</button>
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
          {busy ? "Adding…" : "Add"}
        </button>
        <p class="muted small">
          Add as many servers as you like — connect one, then fill this in again for the
          next. Each appears under <b>Connected</b> and is browsed alongside the rest.
        </p>
      </div>
    </section>
    {/if}

    {#if activeTab === "folders"}
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
          <label for="smb-name">Name (optional)</label>
          <input id="smb-name" placeholder="Shown in the sidebar (defaults to the share name)" bind:value={smbName} />
        </div>
        <button class="primary" disabled={busy} onclick={mountSmb}>
          {busy ? "Connecting…" : "Add share"}
        </button>
        <p class="muted small">
          On Linux, Vela connects to the share directly — no mount, no root,
          nothing else to set up; enter the server, share, and credentials.
          macOS and Windows attach the share through the OS.
        </p>

        {#if smbMounts.length > 0}
          <div class="field">
            <label for="smb-mounted">Share</label>
            <select
              id="smb-mounted"
              bind:value={smbBrowseMountId}
              onchange={(e) => loadSmbDirectories((e.currentTarget as HTMLSelectElement).value, "")}
            >
              {#each smbMounts as mount (mount.id)}
                <option value={mount.id}>//{mount.server}/{mount.share}</option>
              {/each}
            </select>
          </div>

          <div class="smb-browser">
            <div class="browser-head">
              <div>
                <b>{smbPathLabel(smbBrowsePath)}</b>
                {#if smbBrowseMountId}
                  <span class="muted small"> · {smbMounts.find((mount) => mount.id === smbBrowseMountId)?.name}</span>
                {/if}
              </div>
              <div class="btnrow compact">
                {#if smbBrowsePath}
                  <button disabled={smbBrowseBusy} onclick={() => loadSmbDirectories(smbBrowseMountId, parentSmbPath(smbBrowsePath))}>
                    Up
                  </button>
                {/if}
                <button disabled={smbBrowseBusy || !smbBrowseMountId} onclick={() => loadSmbDirectories(smbBrowseMountId, smbBrowsePath)}>
                  {smbBrowseBusy ? "Loading…" : "Refresh"}
                </button>
              </div>
            </div>

            <div class="field">
              <label for="smb-folder-kind">Contains</label>
              <select id="smb-folder-kind" bind:value={smbFolderKind}>
                <option value="">Auto-detect</option>
                <option value="movie">Movies</option>
                <option value="show">TV Shows</option>
              </select>
            </div>
            <button class="primary" disabled={busy || !smbBrowseMountId} onclick={addSmbFolder}>
              Add this folder
            </button>

            <div class="dirlist">
              {#if smbDirectories.length === 0}
                <p class="muted small">No child folders loaded.</p>
              {:else}
                {#each smbDirectories as dir (dir.path)}
                  <div class="dirrow">
                    <span>{dir.name}</span>
                    <button disabled={smbBrowseBusy} onclick={() => loadSmbDirectories(smbBrowseMountId, dir.path)}>Open</button>
                  </div>
                {/each}
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </section>

    <section>
      <h3>Add an SSH / SFTP folder</h3>
      {#if sshfs && !sshfs.found}
        <div class="warn">
          <b>sshfs is not installed</b> — SSH folders need it.
          {#if sshfs.platform === "macos"}
            Plain <code>brew install sshfs</code> does not work on macOS. The working
            route: <code>brew install --cask macfuse</code>, approve its system
            extension in System Settings (Apple Silicon Macs must first allow
            third-party kernel extensions via Recovery) and restart, then
            <code>brew install gromgit/fuse/sshfs-mac</code>. Heads-up: these builds
            can be unstable on recent macOS.
          {:else if sshfs.platform === "linux"}
            Install <code>sshfs</code> with your package manager (apt, dnf, pacman, …).
          {:else}
            SSH/SFTP folders currently require sshfs on Linux or macOS.
          {/if}
        </div>
      {:else if sshfs?.found && sshfs.path}
        <p class="muted small">sshfs detected: {sshfs.path}</p>
      {/if}
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
        <div class="field">
          <label for="ssh-name">Name (optional)</label>
          <input id="ssh-name" placeholder="Shown in the sidebar (defaults to the folder name)" bind:value={sshName} />
        </div>
        <button class="primary" disabled={busy} onclick={mountSsh}>
          {busy ? "Mounting…" : "Mount & add"}
        </button>
        <p class="muted small">
          SSH/SFTP uses sshfs with your SSH keys, agent, and ~/.ssh/config. Vela does not
          store SSH passwords. Connect to new hosts once with plain <code>ssh</code>
          first so the host key is trusted — mounts can't answer that prompt.
        </p>
      </div>
    </section>
    {/if}

    {#if activeTab === "appearance"}
    <section>
      <h3>Theme</h3>
      {#each themeModes as mode (mode)}
        <div class="themegroup">{mode === "dark" ? "Dark" : "Light"}</div>
        <div class="themegrid">
          {#each THEMES.filter((t) => t.mode === mode) as t (t.id)}
            <button
              class="themecard"
              class:active={theme === t.id}
              aria-pressed={theme === t.id}
              onclick={() => setTheme(t.id)}
            >
              <span class="swatch" aria-hidden="true">
                {#each t.swatch as c}<span style="background:{c}"></span>{/each}
              </span>
              <span class="themename">{t.label}</span>
            </button>
          {/each}
        </div>
      {/each}
      <p class="muted small">Color themes from popular open-source palettes.</p>
    </section>
    {/if}
      </div>
    </div>
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
    animation: vela-fade 0.16s var(--ease);
  }
  .panel {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    width: min(780px, 100%);
    padding: 1.2rem 1.4rem 1.6rem;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    cursor: default;
    animation: vela-pop 0.18s var(--ease);
  }
  .tabwrap {
    display: flex;
    gap: 1.1rem;
    align-items: flex-start;
  }
  .tabs {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    flex: 0 0 130px;
    border-right: 1px solid var(--border);
    padding-right: 0.6rem;
  }
  .tabs button {
    text-align: left;
    background: none;
    border: none;
    color: var(--text-2);
    padding: 0.5rem 0.7rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.9rem;
  }
  .tabs button:hover {
    background: var(--surface-2);
  }
  .tabs button.active {
    background: var(--surface-2);
    color: var(--text-bright);
  }
  .tabcontent {
    flex: 1;
    min-width: 0;
    max-height: 68vh;
    overflow-y: auto;
    padding-right: 0.4rem;
  }
  .themegroup {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-dim);
    margin: 0.7rem 0 0.45rem;
  }
  .themegrid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 0.5rem;
  }
  .themecard {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    background: var(--surface-sunken);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.5rem 0.6rem;
    cursor: pointer;
    color: var(--text);
    text-align: left;
    transition: border-color 0.15s var(--ease);
  }
  .themecard:hover {
    border-color: var(--border-strong);
  }
  .themecard.active {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .swatch {
    display: inline-flex;
    border-radius: 5px;
    overflow: hidden;
    border: 1px solid var(--border);
    flex: none;
  }
  .swatch span {
    width: 14px;
    height: 22px;
    display: block;
  }
  .themename {
    font-size: 0.85rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* First section in a tab shouldn't show the divider line / top gap. */
  .tabcontent section:first-child h3 {
    border-top: none;
    margin-top: 0;
    padding-top: 0;
  }
  .warn {
    background: var(--warn-bg);
    color: var(--warn-text);
    border: 1px solid var(--warn-border);
    border-radius: 6px;
    padding: 0.5rem 0.7rem;
    font-size: 0.85rem;
    line-height: 1.4;
    margin-bottom: 0.6rem;
  }
  .warn b {
    color: var(--warn-text);
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
    color: var(--text-2);
    margin: 1.2rem 0 0.5rem;
    border-top: 1px solid var(--border);
    padding-top: 1rem;
  }
  .x {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.3rem;
    border-radius: 0.4rem;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition:
      color 0.15s var(--ease),
      background 0.15s var(--ease);
  }
  .x:hover {
    color: var(--text-bright);
    background: var(--surface-2);
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
  .subrow {
    padding-left: 1.6rem;
  }
  .badge {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    background: var(--border);
    color: var(--text-2);
    padding: 0.15rem 0.45rem;
    border-radius: 4px;
  }
  .muted {
    color: var(--text-muted);
  }
  .small {
    font-size: 0.8rem;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .smb-browser {
    background: var(--surface-sunken);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.65rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .browser-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }
  .btnrow.compact {
    gap: 0.35rem;
  }
  .btnrow.compact button:not(.primary):not(.rm),
  .dirrow button {
    background: var(--surface-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.35rem 0.65rem;
    cursor: pointer;
  }
  .dirlist {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .dirrow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
    padding: 0.35rem 0;
    border-top: 1px solid var(--border);
  }
  .dirrow span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  label {
    font-size: 0.8rem;
    color: var(--text-2);
  }
  input,
  select,
  textarea {
    background: var(--surface-sunken);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.5rem 0.6rem;
    color: var(--text);
    font-size: 0.9rem;
  }
  textarea {
    font-family: ui-monospace, monospace;
    resize: vertical;
    min-height: 4.5rem;
    line-height: 1.4;
  }
  .presets {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    margin-top: 0.2rem;
  }
  .preset {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.5rem 0.65rem;
  }
  .preset-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .preset-args {
    display: block;
    margin: 0.3rem 0;
    font-family: ui-monospace, monospace;
    font-size: 0.8rem;
    color: var(--info);
    white-space: pre-wrap;
    word-break: break-word;
  }
  button.ins {
    background: var(--surface-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.25rem 0.7rem;
    font-size: 0.8rem;
    cursor: pointer;
    flex: none;
  }
  .check {
    flex-direction: row;
    align-items: center;
    gap: 0.4rem;
    display: flex;
  }
  button.primary {
    background: var(--accent);
    color: var(--on-accent);
    border: none;
    border-radius: 6px;
    padding: 0.55rem 1.1rem;
    font-weight: 700;
    cursor: pointer;
    align-self: flex-start;
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
  button.primary:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .btnrow {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
  }
  .btnrow button.primary {
    align-self: auto;
  }
  /* Secondary buttons (e.g. Browse…) inside the panel. */
  .btnrow button:not(.primary):not(.rm) {
    background: var(--surface-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.55rem 1.1rem;
    cursor: pointer;
  }
  .btnrow.compact button:not(.primary):not(.rm) {
    padding: 0.35rem 0.65rem;
  }
  .btnrow button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  code.path {
    word-break: break-all;
    background: var(--surface-sunken);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.05rem 0.3rem;
  }
  .rm {
    background: var(--danger-bg);
    color: var(--danger-text);
    border: 1px solid var(--danger-border);
    border-radius: 6px;
    padding: 0.3rem 0.7rem;
    cursor: pointer;
  }
  .err {
    background: var(--danger-bg);
    color: var(--danger-text);
    padding: 0.5rem 0.8rem;
    border-radius: 6px;
    font-size: 0.85rem;
    margin: 0.5rem 0;
  }
</style>
