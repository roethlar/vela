<script module lang="ts">
  type PlayerWindowMode = "remember" | "fullscreen" | "default";
  type VideoPlayer = { player: "external" | "embedded"; offered: boolean; note: string | null };
  type ContinuePlayingMode = import("$lib/types").ContinuePlayingMode;
  type PlaybackPreferences = import("$lib/types").PlaybackPreferences;
  type PlaybackSourcePolicy = import("$lib/types").PlaybackSourcePolicy;

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

  // Each backend setting has one confirmation and one write queue for the
  // app's lifetime (pw-7). A reply must arrive before the next write starts;
  // otherwise reply order need not be the order the backend stored them.
  function createSetting<T>(initial: T) {
    const confirmed = $state({ saves: 0, known: false, value: initial });
    const status = $state({ pending: 0, saved: false });
    const feedback = $state({ error: null as string | null });
    let savedTimer: ReturnType<typeof setTimeout> | undefined;
    const followers = new Set<(value: T) => void>();
    const waitingForLoad = new Set<{ resolve: () => void; reject: (error: unknown) => void }>();
    let loadError: { error: unknown } | undefined;
    let reads = 0;
    let tail: Promise<void> | undefined;
    function show(value: T) {
      confirmed.known = true;
      confirmed.value = value;
      for (const follower of [...followers]) follower(value);
      for (const waiting of waitingForLoad) waiting.resolve();
      waitingForLoad.clear();
    }
    return {
      confirmed,
      status,
      feedback,
      follow(show: (value: T) => void) {
        followers.add(show);
        return () => { followers.delete(show); };
      },
      beginLoad() {
        loadError = undefined;
        return { read: ++reads, saves: confirmed.saves };
      },
      loaded(value: T, ticket: { read: number; saves: number }) {
        if (ticket.read === reads && ticket.saves === confirmed.saves) show(value);
      },
      loadFailed(error: unknown, ticket: { read: number; saves: number }) {
        if (ticket.read !== reads || ticket.saves !== confirmed.saves || confirmed.known) return;
        loadError = { error };
        for (const waiting of waitingForLoad) waiting.reject(error);
        waitingForLoad.clear();
      },
      ready() {
        if (confirmed.known) return Promise.resolve();
        if (loadError) return Promise.reject(loadError.error);
        return new Promise<void>((resolve, reject) => waitingForLoad.add({ resolve, reject }));
      },
      confirm(value: T) {
        confirmed.saves += 1;
        show(value);
      },
      queue<R>(write: () => Promise<R>): Promise<R> {
        clearTimeout(savedTimer);
        status.saved = false;
        status.pending += 1;
        const pending = tail ? tail.then(write) : write();
        const tracked = pending.then((value) => {
          // Outcomes belong to the queue, so closing an opening cannot hide
          // a refusal, and a later success clears it in every opening.
          feedback.error = null;
          status.pending -= 1;
          if (status.pending === 0) {
            status.saved = true;
            savedTimer = setTimeout(() => { status.saved = false; }, 2000);
          }
          return value;
        }, (error) => {
          feedback.error = String(error);
          status.pending -= 1;
          throw error;
        });
        const settled = tracked.then(() => {}, () => {});
        tail = settled;
        void settled.then(() => {
          if (tail === settled) tail = undefined;
        });
        return tracked;
      },
    };
  }

  // Drafts belong to one opening. Track edits, including choosing a default
  // before the first load answers. A load/another opening's save updates only
  // untouched fields; finishing our save settles only the edits it submitted.
  function createDraft<T extends object>(setting: ReturnType<typeof createSetting<T>>) {
    const value = $state({ ...setting.confirmed.value });
    const edits = new Map<keyof T, number>();
    const submittedEdits = new Map<keyof T, number>();
    const dirty = new Set<keyof T>();
    const keys = Object.keys(value) as (keyof T)[];
    const unfollow = setting.follow((confirmed) => {
      for (const key of keys) if (!dirty.has(key)) value[key] = confirmed[key];
    });
    return {
      value,
      feedback: setting.feedback,
      unfollow,
      edit(key: keyof T) {
        edits.set(key, (edits.get(key) ?? 0) + 1);
        dirty.add(key);
      },
      needsSave(key: keyof T, normalize: (value: T[keyof T]) => unknown) {
        if (!dirty.has(key) || edits.get(key) === submittedEdits.get(key)) return false;
        if (setting.confirmed.known && !setting.status.pending && normalize(value[key]) === normalize(setting.confirmed.value[key])) {
          dirty.delete(key);
          return false;
        }
        return true;
      },
      capture(submit = true, fields = keys) {
        const changes: Partial<T> = {};
        for (const key of fields) {
          changes[key] = value[key];
          if (submit && edits.has(key)) submittedEdits.set(key, edits.get(key)!);
        }
        return { value: { ...value }, changes, fields, edits: new Map(edits) };
      },
      finish(submitted: { fields: (keyof T)[]; edits: Map<keyof T, number> }, keepDraft?: keyof T) {
        for (const key of submitted.fields) {
          if (edits.get(key) !== submitted.edits.get(key)) continue;
          if (key === keepDraft && dirty.has(key) && value[key] !== setting.confirmed.value[key]) continue;
          dirty.delete(key);
          value[key] = setting.confirmed.value[key];
        }
      },
    };
  }

  function changeDraft<T extends object, K extends keyof T>(
    draft: ReturnType<typeof createDraft<T>>, key: K, value: T[K], save: (key: K) => Promise<void>,
  ) {
    // Svelte calls an explicit change handler before updating bind:value.
    draft.value[key] = value;
    draft.edit(key);
    return save(key);
  }

  const playerWindowSetting = createSetting({ mode: "remember" as PlayerWindowMode });
  const tlsSetting = createSetting({ skip: false });
  const continuePlayingSetting = createSetting({ mode: "only-tv" as ContinuePlayingMode });
  const videoPlayerSetting = createSetting<VideoPlayer>({ player: "external", offered: false, note: null });
  const mpvSetting = createSetting({ info: null as MpvInfo | null, path: "" });
  // A reopened select needs the catalog for its already-confirmed tier.
  let qualityTiers = $state<QualityTier[]>([]);
  const advancedSetting = createSetting<Omit<MpvAdvanced, "qualityTiers">>({
    extraArgs: "", useOwnConfig: false, autocrop: "off", playbackQuality: "original",
    skipIntros: "button", skipCredits: "button", skipCommercials: "button",
  });
  const playbackSetting = createSetting({
    policy: "best" as PlaybackSourcePolicy, resolutionOverride: "", hdrOverride: "",
    preferences: null as PlaybackPreferences | null,
  });
  function mpvValues(info: MpvInfo) {
    return { info, path: info.configuredPath ?? "" };
  }
  function playbackValues(preferences: PlaybackPreferences) {
    return {
      preferences, policy: preferences.policy,
      resolutionOverride: preferences.resolutionOverride ?? "",
      hdrOverride: preferences.hdrOverride ?? "",
    };
  }
</script>

<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { onMount, onDestroy } from "svelte";
  import Icon from "$lib/Icon.svelte";
  import ServerLogo from "$lib/ServerLogo.svelte";
  import type { DisplayProfile } from "$lib/types";

  type Source = { id: string; name: string; kind: string };

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

  onMount(() => {
    void loadPlayerTls();
  });

  async function loadPlayerTls() {
    const read = tlsSetting.beginLoad();
    try {
      const verify = await invoke<boolean>("get_mpv_tls_verify");
      tlsSetting.loaded({ skip: !verify }, read);
    } catch {
      tlsLoadError = "Could not read the certificate setting. Close Settings and try again.";
    }
  }

  async function savePlayerTls() {
    if (!tlsSetting.confirmed.known) return;
    const submitted = tlsDraft.capture();
    try {
      await tlsSetting.queue(async () => {
        const verify = await invoke<boolean>("set_mpv_tls_verify", { verify: !submitted.value.skip });
        tlsSetting.confirm({ skip: !verify });
      });
    } catch {
      // The queue exposes the refusal even after this opening closes.
    } finally {
      tlsDraft.finish(submitted);
    }
  }

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
  const mpvDraft = createDraft(mpvSetting);
  const mpvPath = mpvDraft.value;
  let mpv = $derived(mpvPath.info);
  onDestroy(mpvDraft.unfollow);
  let installingMpv = $state(false);

  // The defaults in the module match the backend's missing-field defaults.
  const advancedDraft = createDraft(advancedSetting);
  const advanced = advancedDraft.value;
  onDestroy(advancedDraft.unfollow);
  let showMpvHelp = $state(false);

  // What a cleanly-finished single item or exhausted playlist should do next.
  // Missing values use the product default. Unknown persisted values block
  // normal app use at the durable settings boundary.
  const continueDraft = createDraft(continuePlayingSetting);
  const continuation = continueDraft.value;
  onDestroy(continueDraft.unfollow);

  const videoPlayerDraft = createDraft(videoPlayerSetting);
  const videoPlayer = videoPlayerDraft.value;
  onDestroy(videoPlayerDraft.unfollow);

  // How the first mpv window after Vela starts opens. The control starts on
  // the mode the backend last confirmed to the app (see the module script).
  const playerWindowDraft = createDraft(playerWindowSetting);
  const playerWindow = playerWindowDraft.value;
  onDestroy(playerWindowDraft.unfollow);

  const tlsDraft = createDraft(tlsSetting);
  const tls = tlsDraft.value;
  let tlsLoadError = $state<string | null>(null);
  onDestroy(tlsDraft.unfollow);
  // Only accepted shared values make an earlier read failure obsolete.
  onDestroy(tlsSetting.follow(() => { tlsLoadError = null; }));

  // Duplicate-copy selection and the display profile used by Compatible.
  const playbackDraft = createDraft(playbackSetting);
  const playback = playbackDraft.value;
  let playbackPreferences = $derived(playback.preferences);
  onDestroy(playbackDraft.unfollow);

  // One save status for the whole page, shown in its header where it is
  // always in view. A status beside each control would be out of sight for a
  // control at the foot of a long section, and seven of them were noise.
  let saving = $derived(
    [playbackSetting, continuePlayingSetting, videoPlayerSetting, playerWindowSetting, tlsSetting, mpvSetting, advancedSetting]
      .some((setting) => setting.status.pending > 0),
  );
  let justSaved = $derived(
    [playbackSetting, continuePlayingSetting, videoPlayerSetting, playerWindowSetting, tlsSetting, mpvSetting, advancedSetting]
      .some((setting) => setting.status.saved),
  );
  // A refused save is explained in its own section; the header says there is one.
  let notSaved = $derived(
    tlsLoadError !== null || [playbackDraft, continueDraft, videoPlayerDraft, playerWindowDraft, tlsDraft, mpvDraft, advancedDraft]
      .some((draft) => draft.feedback.error !== null),
  );

  // Teardown covers Close, Escape, the overlay and removal by the parent.
  // Captured writes and their confirmations outlive this opening in the queues.
  onDestroy(flushTextDrafts);
  function flushTextDrafts() {
    void saveMpvPath();
    void blurMpvAdvanced();
  }

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
    // A load reads the mode at one moment and applies it later. A save
    // confirmed in between, from this opening or any other, is newer than
    // what the load read (review finding pw-7).
    const windowRead = playerWindowSetting.beginLoad();
    const mpvRead = mpvSetting.beginLoad();
    const advancedRead = advancedSetting.beginLoad();
    const continueRead = continuePlayingSetting.beginLoad();
    const videoPlayerRead = videoPlayerSetting.beginLoad();
    const playbackRead = playbackSetting.beginLoad();
    try {
      const [s, mp, adv, continueMode, windowMode, playbackPrefs, player] = await Promise.all([
        invoke<Source[]>("get_sources"),
        invoke<MpvInfo>("check_mpv"),
        invoke<MpvAdvanced>("get_mpv_advanced"),
        invoke<ContinuePlayingMode>("get_continue_playing"),
        invoke<PlayerWindowMode>("get_mpv_window_mode"),
        invoke<PlaybackPreferences>("get_playback_preferences"),
        invoke<VideoPlayer>("get_player"),
      ]);
      if (seq !== loadSeq) return;
      sources = s;
      mpvSetting.loaded(mpvValues(mp), mpvRead);
      const { qualityTiers: tiers, ...options } = adv;
      // The catalog is not a saved choice; even an overtaken read can supply it.
      qualityTiers = tiers;
      advancedSetting.loaded(options, advancedRead);
      continuePlayingSetting.loaded({ mode: continueMode }, continueRead);
      playerWindowSetting.loaded({ mode: windowMode }, windowRead);
      playbackSetting.loaded(playbackValues(playbackPrefs), playbackRead);
      videoPlayerSetting.loaded(player, videoPlayerRead);
    } catch (e) {
      advancedSetting.loadFailed(e, advancedRead);
      playbackSetting.loadFailed(e, playbackRead);
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

  function writeMpv(write: () => Promise<MpvInfo>) {
    return mpvSetting.queue(async () => {
      try {
        const m = await write();
        mpvSetting.confirm(mpvValues(m));
        onMpvChanged?.(m);
      } catch (e) {
        // Both path changes and installation can store the path before their
        // final mpv check fails. Re-read while still owning the write queue.
        try {
          const m = await invoke<MpvInfo>("check_mpv");
          mpvSetting.confirm(mpvValues(m));
          onMpvChanged?.(m);
        } catch {}
        throw e;
      }
    });
  }

  async function browseMpv() {
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
    mpvDraft.edit("path");
    mpvPath.path = picked;
    await saveMpvPath();
  }

  async function saveMpvPath() {
    if (!mpvDraft.needsSave("path", (value) => String(value).trim())) return;
    const submitted = mpvDraft.capture();
    const path = submitted.value.path.trim() || null;
    try {
      await writeMpv(() => invoke<MpvInfo>("set_mpv_path", { path }));
    } catch {} finally {
      mpvDraft.finish(submitted);
    }
  }

  async function clearMpvPath() {
    mpvDraft.edit("path");
    mpvPath.path = "";
    await saveMpvPath();
  }

  async function installMpv() {
    if (installingMpv) return;
    installingMpv = true;
    const submitted = mpvDraft.capture(false);
    try {
      await writeMpv(() => invoke<MpvInfo>("install_mpv"));
    } catch {} finally {
      mpvDraft.finish(submitted);
      installingMpv = false;
    }
  }

  async function saveMpvAdvanced(key: keyof Omit<MpvAdvanced, "qualityTiers">) {
    const submitted = advancedDraft.capture(true, [key]);
    let saved = false;
    try {
      await advancedSetting.queue(async () => {
        // Only this action's field is queued. Siblings come from the accepted
        // load and preceding confirmations, including other dialog openings.
        await advancedSetting.ready();
        const options = { ...advancedSetting.confirmed.value, ...submitted.changes };
        await invoke("set_mpv_advanced", options);
        // This setter acknowledges all fields, but returns no DTO. Its only
        // normalization is trimming the extra arguments.
        advancedSetting.confirm({ ...options, extraArgs: options.extraArgs.trim() });
      });
      saved = true;
    } catch {} finally {
      // Keep the author's formatting after success, as before; rollback uses
      // the trimmed text actually stored. New edits always remain drafts.
      advancedDraft.finish(submitted, saved ? "extraArgs" : undefined);
    }
  }

  async function saveContinuePlaying() {
    const submitted = continueDraft.capture();
    const mode = submitted.value.mode;
    try {
      await continuePlayingSetting.queue(async () => {
        const normalized = await invoke<ContinuePlayingMode>("set_continue_playing", { mode });
        continuePlayingSetting.confirm({ mode: normalized });
        onContinuePlayingChanged?.(normalized);
      });
    } catch {} finally {
      continueDraft.finish(submitted);
    }
  }

  async function saveVideoPlayer() {
    const submitted = videoPlayerDraft.capture();
    const player = submitted.value.player;
    try {
      await videoPlayerSetting.queue(async () => {
        try {
          videoPlayerSetting.confirm(await invoke<VideoPlayer>("set_player", { player }));
        } catch (e) {
          // The save may have succeeded before its status check failed.
          // Re-read while still owning the queue, just like the mpv path.
          try {
            videoPlayerSetting.confirm(await invoke<VideoPlayer>("get_player"));
          } catch {}
          throw e;
        }
      });
    } catch {} finally {
      videoPlayerDraft.finish(submitted);
    }
  }

  async function savePlayerWindowMode() {
    const submitted = playerWindowDraft.capture();
    const mode = submitted.value.mode;
    try {
      await playerWindowSetting.queue(async () => {
        const saved = await invoke<PlayerWindowMode>("set_mpv_window_mode", { mode });
        playerWindowSetting.confirm({ mode: saved });
      });
    } catch {} finally {
      playerWindowDraft.finish(submitted);
    }
  }

  async function savePlaybackPreferences(key: "policy" | "resolutionOverride" | "hdrOverride") {
    const submitted = playbackDraft.capture(true, [key]);
    try {
      await playbackSetting.queue(async () => {
        await playbackSetting.ready();
        const { policy, resolutionOverride, hdrOverride } = { ...playbackSetting.confirmed.value, ...submitted.changes };
        await invoke("set_playback_preferences", {
          policy, resolutionOverride: resolutionOverride || null, hdrOverride: hdrOverride || null,
        });
        // The write is confirmed even if refreshing the detected display
        // fails. Keep the existing display status while refreshing it.
        playbackSetting.confirm({
          policy, resolutionOverride, hdrOverride,
          preferences: playbackSetting.confirmed.value.preferences,
        });
        const read = playbackSetting.beginLoad();
        try {
          const preferences = await invoke<PlaybackPreferences>("get_playback_preferences");
          playbackSetting.loaded(playbackValues(preferences), read);
        } catch (e) {
          playbackSetting.loaded({ ...playbackSetting.confirmed.value, preferences: null }, read);
          throw e;
        }
      });
    } catch {} finally {
      playbackDraft.finish(submitted);
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
    advancedDraft.edit("extraArgs");
    const cur = advanced.extraArgs.trimEnd();
    advanced.extraArgs = (cur ? cur + "\n" : "") + args + "\n";
    return saveMpvAdvanced("extraArgs");
  }

  function blurMpvAdvanced() {
    if (advancedDraft.needsSave("extraArgs", (value) => String(value).trim())) {
      return saveMpvAdvanced("extraArgs");
    }
  }

  function navigatorIsWindows() {
    return typeof navigator !== "undefined" && /Win/i.test(navigator.platform);
  }
</script>

{#snippet saveError(error: string | null)}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
{/snippet}

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
      <span class="save-status" class:failed={!saving && notSaved} aria-live="polite">
        {saving ? "Saving…" : notSaved ? "Not saved" : justSaved ? "Saved" : ""}
      </span>
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
          <ServerLogo kind={s.kind} />
          <span class="badge">{s.kind}</span>
          <span class="name">{s.name}</span>
          <!-- Owner ruling 2026-11-12: Emby's experimental status (decision
               2026-07-15) is stated here and nowhere in the main window. -->
          {#if s.kind === "emby"}<span class="badge experimental">Experimental</span>{/if}
          <button class="rm" disabled={busy} onclick={() => removeSource(s.id)}>Remove</button>
        </div>
      {/each}
    </section>
    {/if}

    {#if activeTab === "player"}
    <section>
      <h3>Duplicate playback source</h3>
      {@render saveError(playbackDraft.feedback.error)}
      <div class="form">
        <p class="muted small">
          This chooses <b>which copy</b> plays, not how it is delivered — that's
          Playback quality, further down. It does nothing when a title exists in
          only one place.
        </p>
        <fieldset class="policygrid">
          <legend>When the same title exists on more than one server</legend>
          {#each playbackPolicies as policy}
            <label class:active={playback.policy === policy.value} class="policycard">
              <input
                type="radio"
                name="playback-source-policy"
                value={policy.value}
                bind:group={playback.policy}
                onchange={(e) => changeDraft(playbackDraft, "policy", e.currentTarget.value as PlaybackSourcePolicy, savePlaybackPreferences)}
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
              <select id="playback-resolution-override" bind:value={playback.resolutionOverride} onchange={(e) => changeDraft(playbackDraft, "resolutionOverride", e.currentTarget.value, savePlaybackPreferences)}>
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
              <select id="playback-hdr-override" bind:value={playback.hdrOverride} onchange={(e) => changeDraft(playbackDraft, "hdrOverride", e.currentTarget.value, savePlaybackPreferences)}>
                <option value="">Auto (detected)</option>
                <option value="enabled">HDR enabled</option>
                <option value="disabled">SDR / HDR disabled</option>
              </select>
            </div>
          </div>
        </details>

      </div>
    </section>

    <section>
      <h3>Continue Playing</h3>
      {@render saveError(continueDraft.feedback.error)}
      <div class="form">
        <div class="field">
          <label for="continue-playing">After a video or playlist finishes</label>
          <select id="continue-playing" bind:value={continuation.mode} onchange={(e) => changeDraft(continueDraft, "mode", e.currentTarget.value as ContinuePlayingMode, saveContinuePlaying)}>
            <option value="off">Off</option>
            <option value="on">On — continue through Continue Watching</option>
            <option value="only-tv">TV only — play the next episode</option>
          </select>
          <p class="muted small">
            TV only follows the show's episode order and rolls into the next season.
            On walks the same Continue Watching list shown on Home.
          </p>
        </div>
      </div>
    </section>

    {#if videoPlayer.offered}
    <section>
      <h3>Video player <span class="badge experimental">Experimental</span></h3>
      {@render saveError(videoPlayerDraft.feedback.error)}
      <div class="form">
        <div class="field">
          <label for="video-player">Videos play in</label>
          <select id="video-player" bind:value={videoPlayer.player} onchange={(e) => changeDraft(videoPlayerDraft, "player", e.currentTarget.value as VideoPlayer["player"], saveVideoPlayer)}>
            <option value="external">mpv's own window</option>
            <option value="embedded">Vela's window (experimental)</option>
          </select>
          <p class="muted small">
            Vela's window shows video inside Vela with Vela's own controls,
            using a copy of mpv that comes with Vela.
            It is experimental and works only on Linux with Wayland.
            Switching to mpv's own window applies to the next video.
            To use Vela's window, restart Vela if this run started without embedded playback.
            When it cannot be used, videos play in mpv's own window and a line
            on screen says why.
          </p>
          {#if videoPlayer.note}
            <p class="muted small">{videoPlayer.note}</p>
          {/if}
        </div>
      </div>
    </section>
    {/if}

    <section>
      <h3>Player window</h3>
      {@render saveError(playerWindowDraft.feedback.error)}
      <div class="form">
        <div class="field">
          <label for="player-window">After Vela starts, the first video opens</label>
          <select id="player-window" bind:value={playerWindow.mode} onchange={(e) => changeDraft(playerWindowDraft, "mode", e.currentTarget.value as PlayerWindowMode, savePlayerWindowMode)}>
            <option value="remember">As you last left it</option>
            <option value="fullscreen">Full screen</option>
            <option value="default">Default — as mpv and your options say</option>
          </select>
          <p class="muted small">
            Each later video opens the way you left the one before it: full
            screen, maximized, its screen and its size. This only chooses how
            the first one opens. It applies to mpv's own window.
          </p>
        </div>
      </div>
    </section>

    <section>
      <h3>mpv player</h3>
      <div class="form">
        <label class="check">
          <input
            type="checkbox"
            id="player-skip-certificate-checks"
            bind:checked={tls.skip}
            onchange={(e) => changeDraft(tlsDraft, "skip", e.currentTarget.checked, savePlayerTls)}
            disabled={!tlsSetting.confirmed.known}
            aria-describedby="player-certificate-help"
          />
          Skip server certificate checks
        </label>
        <p id="player-certificate-help" class="muted small">
          The player checks HTTPS server certificates by default. Turning this check off
          sends your sign-in token without knowing who receives it. For a server signed
          by a private authority, you can instead name that authority's certificate file
          to mpv with <code>tls-ca-file</code> in Advanced options or your own mpv configuration.
          Applies to the next video; plain HTTP is unchanged.
        </p>
        {@render saveError(tlsDraft.feedback.error ?? tlsLoadError)}
      </div>
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
            bind:value={mpvPath.path}
            oninput={() => mpvDraft.edit("path")}
            onblur={saveMpvPath}
            onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); void saveMpvPath(); } }}
          />
        </div>
        <div class="btnrow">
          <button onclick={browseMpv}>Browse…</button>
          {#if mpv?.configuredPath}
            <button class="rm" onclick={clearMpvPath}>Clear</button>
          {/if}
        </div>
        {@render saveError(mpvDraft.feedback.error)}
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
          {@render saveError(advancedDraft.feedback.error)}
          <textarea
            id="mpv-extra"
            rows="4"
            spellcheck="false"
            placeholder={"One option per line, e.g.\n--vo=gpu\n--profile=fast"}
            bind:value={advanced.extraArgs}
            oninput={() => advancedDraft.edit("extraArgs")}
            onblur={blurMpvAdvanced}
          ></textarea>
          <p class="muted small">
            One mpv option per line, appended when launching mpv — so these override
            Vela's defaults. Vela's playback tracking (its IPC socket) and the media
            URL are protected and can't be overridden. Lines starting with
            <code>#</code> are ignored. A bad option just makes mpv fail to start.
          </p>
        </div>

        <label class="check">
          <input type="checkbox" bind:checked={advanced.useOwnConfig} onchange={(e) => changeDraft(advancedDraft, "useOwnConfig", e.currentTarget.checked, saveMpvAdvanced)} />
          Use my own mpv config (<code>~/.config/mpv/mpv.conf</code>)
        </label>
        <p class="muted small">
          By default Vela launches mpv with <code>--no-config</code> for a predictable
          setup. Tick this to load your own <code>mpv.conf</code> instead — it can then
          change anything, including settings that disable HDR or break playback.
        </p>

        <div class="field">
          <label for="mpv-autocrop">Black-bar cropping (mpv autocrop)</label>
          <select id="mpv-autocrop" bind:value={advanced.autocrop} onchange={(e) => changeDraft(advancedDraft, "autocrop", e.currentTarget.value as AutocropMode, saveMpvAdvanced)}>
            <option value="off">Off</option>
            <option value="manual">Manual — press Shift+C to crop</option>
            <option value="auto">Automatic — crop every video</option>
          </select>
          <p class="muted small">
            Loads mpv's bundled <code>autocrop.lua</code> to remove black bars.
            <b>Manual</b> only crops when you press <code>Shift+C</code> during
            playback. <b>Automatic</b> crops every video on its own.
            In Vela's window, cropping is done by Vela's own script (the Crop
            button or <code>Shift+C</code>) and does not crop HDR video.
          </p>
          {#if advanced.autocrop === "auto"}
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
          <select id="playback-quality" bind:value={advanced.playbackQuality} onchange={(e) => changeDraft(advancedDraft, "playbackQuality", e.currentTarget.value, saveMpvAdvanced)}>
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
          {#if sources.some((s) => s.kind === "emby")}
            <!-- Owner ruling 2026-07-25: Emby transcoding is best-effort and
                 must be labelled limited rather than claimed. The Jellyfin and
                 Emby paths share an implementation that has only ever been
                 exercised against Jellyfin. Shown only to users who actually
                 have an Emby server connected. -->
            <p class="muted small">
              <span class="inlineicon"><Icon name="alert" size={14} stroke={2.25} /></span
              >Converting is <b>unverified on Emby</b>. It shares Jellyfin's
              implementation and may work, but nothing here has been tested
              against a real Emby server — please report what you see.
            </p>
          {/if}
        </div>

        <div class="field">
          <label for="skip-intros">Skip intros</label>
          <select id="skip-intros" bind:value={advanced.skipIntros} onchange={(e) => changeDraft(advancedDraft, "skipIntros", e.currentTarget.value as SkipPolicy, saveMpvAdvanced)}>
            <option value="off">Off</option>
            <option value="button">Button — ask on screen</option>
            <option value="autoskip">Auto-skip</option>
          </select>
        </div>

        <div class="field">
          <label for="skip-credits">Skip credits</label>
          <select id="skip-credits" bind:value={advanced.skipCredits} onchange={(e) => changeDraft(advancedDraft, "skipCredits", e.currentTarget.value as SkipPolicy, saveMpvAdvanced)}>
            <option value="off">Off</option>
            <option value="button">Button — ask on screen</option>
            <option value="autoskip">Auto-skip</option>
          </select>
        </div>

        <div class="field">
          <label for="skip-commercials">Skip commercials</label>
          <select id="skip-commercials" bind:value={advanced.skipCommercials} onchange={(e) => changeDraft(advancedDraft, "skipCommercials", e.currentTarget.value as SkipPolicy, saveMpvAdvanced)}>
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
  /* Beside the Close button; its width is kept so the header does not move. */
  .save-status {
    margin: 0 0.75rem 0 auto;
    min-width: 5.5em;
    text-align: right;
    color: var(--text-muted);
    font-size: 0.85rem;
  }
  .save-status.failed {
    color: var(--danger-text);
  }
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
  .badge.experimental {
    color: var(--accent);
  }
  .row :global(svg[role="img"]) {
    flex: none;
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
