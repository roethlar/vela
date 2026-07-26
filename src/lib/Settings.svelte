<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { onMount, onDestroy } from "svelte";
  import Icon from "$lib/Icon.svelte";
  import type {
    ContinuePlayingMode,
    DisplayProfile,
    PlaybackPreferences,
    PlaybackSourcePolicy,
  } from "$lib/types";

  type Source = { id: string; name: string; kind: string };

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
  type SkipPolicy = "off" | "button" | "autoskip";
  type QualityTier = {
    id: string;
    label: string;
    bitrateKbps: number;
    width: number;
    height: number;
  };
  type MpvAdvanced = {
    extraArgs: string;
    useOwnConfig: boolean;
    autocrop: AutocropMode;
    skipIntros: SkipPolicy;
    skipCredits: SkipPolicy;
    skipCommercials: SkipPolicy;
    playbackQuality: string;
    qualityTiers: QualityTier[];
  };

  let {
    onClose,
    onChanged,
    onLinkPlex,
    onMpvChanged,
    onContinuePlayingChanged,
  }: {
    onClose: () => void;
    onChanged: () => void;
    onLinkPlex: () => void;
    onMpvChanged?: (m: MpvInfo) => void;
    onContinuePlayingChanged?: (mode: ContinuePlayingMode) => void;
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
  let busy = $state(false);
  let err = $state<string | null>(null);

  // Vertical settings tabs — split the (formerly very long) panel into sections.
  type TabId = "connected" | "servers" | "player" | "appearance";
  const tabs: { id: TabId; label: string }[] = [
    { id: "connected", label: "Connected" },
    { id: "servers", label: "Servers" },
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
    { id: "oled", label: "OLED Black", mode: "dark", swatch: ["#000000", "#c58a0b", "#0e0e0e"] },
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

  // Add Jellyfin/Emby form
  let kind = $state<"jellyfin" | "emby">("jellyfin");
  let url = $state("");
  let username = $state("");
  let password = $state("");
  let useApiKey = $state(false);
  let apiKey = $state("");
  let userId = $state("");

  // mpv player location
  let mpv = $state<MpvInfo | null>(null);
  let mpvPathInput = $state("");
  let mpvBusy = $state(false);
  let installingMpv = $state(false);

  // Advanced mpv options (free-form args + own-config toggle).
  let mpvExtraArgs = $state("");
  let mpvUseOwnConfig = $state(false);
  let mpvAutocrop = $state<AutocropMode>("off");
  // Button matches the backend's missing-field default, so a load failure
  // cannot present a stronger setting than the user actually has.
  // Original matches the backend's missing-field default, so a failed load can
  // never present a converted setting the user never chose.
  let playbackQuality = $state("original");
  let qualityTiers = $state<QualityTier[]>([]);
  let skipIntros = $state<SkipPolicy>("button");
  let skipCredits = $state<SkipPolicy>("button");
  let skipCommercials = $state<SkipPolicy>("button");
  let mpvAdvBusy = $state(false);
  let showMpvHelp = $state(false);

  // What a cleanly-finished single item or exhausted playlist should do next.
  // Missing values use the product default. Unknown persisted values block
  // normal app use at the durable settings boundary.
  let continuePlaying = $state<ContinuePlayingMode>("only-tv");
  let continuePlayingBusy = $state(false);

  // Duplicate-copy selection and the display profile used by Compatible.
  let playbackPreferences = $state<PlaybackPreferences | null>(null);
  let playbackSourcePolicy = $state<PlaybackSourcePolicy>("best");
  let playbackResolutionOverride = $state("");
  let playbackHdrOverride = $state("");
  let playbackPreferencesBusy = $state(false);

  const playbackPolicies: {
    value: PlaybackSourcePolicy;
    label: string;
    summary: string;
  }[] = [
    {
      value: "best",
      label: "Prefer Best",
      summary: "Highest resolution first, then HDR within that resolution, then bitrate.",
    },
    {
      value: "compatible",
      label: "Prefer Compatible",
      summary:
        "Pick the copy that best matches this machine's display resolution and HDR state.",
    },
    {
      value: "fastest",
      label: "Prefer Fastest Source",
      summary: "Same machine first, then local network, then internet; best quality breaks ties.",
    },
    {
      value: "ask",
      label: "Ask Every Time",
      summary: "Ask which server copy to use whenever a manual play has duplicates.",
    },
  ];

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
      const [s, mp, adv, continueMode, playbackPrefs] = await Promise.all([
        invoke<Source[]>("get_sources"),
        invoke<MpvInfo>("check_mpv"),
        invoke<MpvAdvanced>("get_mpv_advanced"),
        invoke<ContinuePlayingMode>("get_continue_playing"),
        invoke<PlaybackPreferences>("get_playback_preferences"),
      ]);
      if (seq !== loadSeq) return;
      sources = s;
      mpv = mp;
      mpvPathInput = mp.configuredPath ?? "";
      mpvExtraArgs = adv.extraArgs;
      mpvUseOwnConfig = adv.useOwnConfig;
      mpvAutocrop = adv.autocrop;
      playbackQuality = adv.playbackQuality;
      qualityTiers = adv.qualityTiers;
      skipIntros = adv.skipIntros;
      skipCredits = adv.skipCredits;
      skipCommercials = adv.skipCommercials;
      continuePlaying = continueMode;
      applyPlaybackPreferences(playbackPrefs);
    } catch (e) {
      if (seq === loadSeq) err = String(e);
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
        skipIntros,
        skipCredits,
        skipCommercials,
        playbackQuality,
      });
    } catch (e) {
      err = String(e);
    } finally {
      mpvAdvBusy = false;
    }
  }

  async function saveContinuePlaying() {
    if (continuePlayingBusy) return;
    continuePlayingBusy = true;
    err = null;
    try {
      const normalized = await invoke<ContinuePlayingMode>("set_continue_playing", {
        mode: continuePlaying,
      });
      continuePlaying = normalized;
      onContinuePlayingChanged?.(normalized);
    } catch (e) {
      err = String(e);
    } finally {
      continuePlayingBusy = false;
    }
  }

  function applyPlaybackPreferences(preferences: PlaybackPreferences) {
    playbackPreferences = preferences;
    playbackSourcePolicy = preferences.policy;
    playbackResolutionOverride = preferences.resolutionOverride ?? "";
    playbackHdrOverride = preferences.hdrOverride ?? "";
  }

  async function savePlaybackPreferences() {
    if (playbackPreferencesBusy) return;
    playbackPreferencesBusy = true;
    err = null;
    try {
      await invoke("set_playback_preferences", {
        policy: playbackSourcePolicy,
        resolutionOverride: playbackResolutionOverride || null,
        hdrOverride: playbackHdrOverride || null,
      });
      applyPlaybackPreferences(
        await invoke<PlaybackPreferences>("get_playback_preferences")
      );
    } catch (e) {
      err = String(e);
    } finally {
      playbackPreferencesBusy = false;
    }
  }

  function displaySummary(display: DisplayProfile): string {
    const resolution =
      display.widthPx > 0 && display.heightPx > 0
        ? `${display.widthPx} × ${display.heightPx}`
        : "resolution unknown";
    const hdr =
      display.hdr === "enabled"
        ? "HDR enabled"
        : display.hdr === "disabled"
          ? "SDR / HDR disabled"
          : "HDR state unknown";
    return `${display.name || "Unknown display"} — ${resolution}, ${hdr}`;
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
      {#if sources.length === 0}
        <p class="muted">No servers yet. Add one under Servers.</p>
      {/if}
      {#each sources as s (s.id)}
        <div class="row">
          <span class="badge">{s.kind}</span>
          <span class="name">{s.name}</span>
          <button class="rm" disabled={busy} onclick={() => removeSource(s.id)}>Remove</button>
        </div>
      {/each}
    </section>
    {/if}

    {#if activeTab === "player"}
    <section>
      <h3>Duplicate playback source</h3>
      <div class="form">
        <p class="muted small">
          This chooses <b>which copy</b> plays, not how it is delivered — that's
          Playback quality, further down. It does nothing when a title exists in
          only one place.
        </p>
        <fieldset class="policygrid">
          <legend>When the same title exists on more than one server</legend>
          {#each playbackPolicies as policy}
            <label class:active={playbackSourcePolicy === policy.value} class="policycard">
              <input
                type="radio"
                name="playback-source-policy"
                value={policy.value}
                bind:group={playbackSourcePolicy}
              />
              <span>
                <b>{policy.label}</b>
                <small>{policy.summary}</small>
              </span>
            </label>
          {/each}
        </fieldset>

        <div class="helpbox">
          <b>How Vela decides</b>
          <p><b>Best:</b> resolution → HDR within that resolution → bitrate. A 4K SDR copy beats a 1080p HDR copy.</p>
          <p><b>Compatible:</b> stay at or below the playback display's resolution and match its current HDR state when possible.</p>
          <p><b>Fastest:</b> this machine → local network → internet, then Best within that tier.</p>
          <p>
            <b>Manual override:</b> <b>Play Version</b> on a title's menu permanently chooses a server for the three automatic modes.
            In Ask Every Time it answers only that play and is never saved. During a playlist or TV continuation, Ask keeps the first
            choice only for that playback session and asks again if the chosen server lacks a later item.
          </p>
        </div>

        {#if playbackPreferences}
          <div class="displaystatus" aria-live="polite">
            <b>Detected playback display</b>
            <span>{displaySummary(playbackPreferences.detectedDisplay)}</span>
            {#if playbackPreferences.detectedDisplay.evidence === "mpv-observed"}
              <small>Observed from the current or most recent mpv playback output.</small>
            {:else}
              <small>Detected from the monitor containing Vela's window.</small>
            {/if}
            {#if playbackPreferences.effectiveDisplay.evidence === "manual-override"}
              <small><b>Compatible will use:</b> {displaySummary(playbackPreferences.effectiveDisplay)}</small>
            {/if}
          </div>
        {/if}

        <details class="advanced-display">
          <summary>Advanced display override</summary>
          <p class="muted small">
            Leave both on Auto unless native detection is unavailable or wrong. Resolution and HDR can be overridden independently.
          </p>
          <div class="overridegrid">
            <div class="field">
              <label for="playback-resolution-override">Resolution</label>
              <select id="playback-resolution-override" bind:value={playbackResolutionOverride}>
                <option value="">Auto (detected)</option>
                <option value="720p">1280 × 720</option>
                <option value="1080p">1920 × 1080</option>
                <option value="1440p">2560 × 1440</option>
                <option value="2160p">3840 × 2160 (4K)</option>
                <option value="4320p">7680 × 4320 (8K)</option>
              </select>
            </div>
            <div class="field">
              <label for="playback-hdr-override">HDR</label>
              <select id="playback-hdr-override" bind:value={playbackHdrOverride}>
                <option value="">Auto (detected)</option>
                <option value="enabled">HDR enabled</option>
                <option value="disabled">SDR / HDR disabled</option>
              </select>
            </div>
          </div>
        </details>

        <div class="btnrow">
          <button class="primary" disabled={playbackPreferencesBusy} onclick={savePlaybackPreferences}>
            {playbackPreferencesBusy ? "Saving…" : "Save playback source preference"}
          </button>
        </div>
      </div>
    </section>

    <section>
      <h3>Continue Playing</h3>
      <div class="form">
        <div class="field">
          <label for="continue-playing">After a video or playlist finishes</label>
          <select id="continue-playing" bind:value={continuePlaying}>
            <option value="off">Off</option>
            <option value="on">On — continue through Continue Watching</option>
            <option value="only-tv">TV only — play the next episode</option>
          </select>
          <p class="muted small">
            TV only follows the show's episode order and rolls into the next season.
            On walks the same Continue Watching list shown on Home.
          </p>
        </div>
        <div class="btnrow">
          <button class="primary" disabled={continuePlayingBusy} onclick={saveContinuePlaying}>
            {continuePlayingBusy ? "Saving…" : "Save Continue Playing"}
          </button>
        </div>
      </div>
    </section>

    <section>
      <h3>mpv player</h3>
      {#if mpv}
        <p class="muted small">
          {#if mpv.available}
            <span class="inlineicon"><Icon name="check" size={14} stroke={2.5} /></span>Found mpv at <code class="path">{mpv.path}</code>
          {:else}
            <span class="inlineicon"><Icon name="close" size={14} stroke={2.5} /></span>mpv wasn't found. Install it below, or point Vela at an existing mpv.
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
          <b><span class="inlineicon"><Icon name="alert" size={14} stroke={2.25} /></span>Advanced — requires mpv knowledge.</b>
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
              <span class="inlineicon"><Icon name="alert" size={14} stroke={2.25} /></span>Automatic cropping runs at the start of every video and can be
              unreliable on HDR content — on some GPU/Wayland setups it may
              occasionally hang mpv (unkillable). If playback freezes, switch back to
              Off or Manual.
            </p>
          {/if}
        </div>

        <div class="field">
          <label for="playback-quality">Playback quality</label>
          <select id="playback-quality" bind:value={playbackQuality}>
            <option value="original">Original — play the file as it is</option>
            {#each qualityTiers as tier (tier.id)}
              <!-- Two tiers share the label "Convert to 1080p HD" and differ
                   only by bitrate, so the bitrate is not decoration. -->
              <option value={tier.id}>
                {tier.label} — {tier.bitrateKbps >= 1000
                  ? `${tier.bitrateKbps / 1000} Mbps`
                  : `${tier.bitrateKbps} kbps`}
              </option>
            {/each}
            <!-- The tr-8 gate is withdrawn: Automatic is implemented as of
                 slice 5 — playback is sampled for decoder drops and a starving
                 cache, and steps down a tier when either persists. -->
            <option value="automatic">Automatic</option>
          </select>
          <p class="muted small">
            How the copy you play is <b>delivered</b>. Original streams the file
            untouched and is the only setting that keeps HDR. Anything else asks
            your server to convert it, which costs HDR and container chapters.
            <b>Automatic</b> starts at Original and drops a step only if playback
            can't keep up — at most twice, and never back up. Set this to suit
            where you are — a slow connection now, a fast one later — and change
            it whenever that changes; it isn't remembered per title. A title's
            own right-click menu can override it for one play.
          </p>
        </div>

        <div class="field">
          <label for="skip-intros">Skip intros</label>
          <select id="skip-intros" bind:value={skipIntros}>
            <option value="off">Off</option>
            <option value="button">Button — ask on screen</option>
            <option value="autoskip">Auto-skip</option>
          </select>
        </div>

        <div class="field">
          <label for="skip-credits">Skip credits</label>
          <select id="skip-credits" bind:value={skipCredits}>
            <option value="off">Off</option>
            <option value="button">Button — ask on screen</option>
            <option value="autoskip">Auto-skip</option>
          </select>
        </div>

        <div class="field">
          <label for="skip-commercials">Skip commercials</label>
          <select id="skip-commercials" bind:value={skipCommercials}>
            <option value="off">Off</option>
            <option value="button">Button — ask on screen</option>
            <option value="autoskip">Auto-skip</option>
          </select>
          <p class="muted small">
            Uses the intro, credits and commercial markers your media server
            publishes — Vela never guesses where they are, so titles without
            markers are unaffected. <b>Button</b> shows a skip button on the
            video: click it, or press <code>Space</code> while it is visible.
            <b>Auto-skip</b> jumps past the range on its own.
          </p>
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
      <p class="muted small">Choose a palette for your screen and room.</p>
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
    /* A neutral black scrim must dim both light and dark themes; unlike a
       surface color, its job is to suppress the whole app behind the modal. */
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
    box-shadow: 0 20px 60px var(--shadow-lg);
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
    background: var(--accent-tint);
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
  .policygrid {
    border: 0;
    padding: 0;
    margin: 0;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.5rem;
  }
  .policygrid legend {
    color: var(--text-2);
    font-size: 0.8rem;
    margin-bottom: 0.4rem;
  }
  .policycard {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.65rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-sunken);
    cursor: pointer;
  }
  .policycard.active {
    border-color: var(--accent);
    background: var(--accent-tint);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .policycard input {
    margin: 0.15rem 0 0;
    flex: none;
  }
  .policycard span {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .policycard b {
    color: var(--text);
  }
  .policycard small {
    color: var(--text-muted);
    line-height: 1.35;
  }
  .helpbox,
  .displaystatus {
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 0.65rem 0.75rem;
    background: var(--surface);
    font-size: 0.82rem;
    line-height: 1.4;
  }
  .helpbox p {
    margin: 0.35rem 0 0;
  }
  .displaystatus {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .displaystatus small {
    color: var(--text-muted);
  }
  .advanced-display {
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 0.55rem 0.65rem;
    background: var(--surface-sunken);
  }
  .advanced-display summary {
    cursor: pointer;
    color: var(--text-2);
    font-size: 0.85rem;
    font-weight: 600;
  }
  .overridegrid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.6rem;
  }
  @media (max-width: 680px) {
    .policygrid,
    .overridegrid {
      grid-template-columns: 1fr;
    }
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
    background: var(--accent-tint);
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
  .inlineicon {
    display: inline-flex;
    margin-right: 0.22rem;
    vertical-align: -0.17em;
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
    align-self: flex-start;
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
  .btnrow button:disabled:not(.primary) {
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
