<script lang="ts">
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy, tick } from "svelte";
  import EmptyState from "$lib/EmptyState.svelte";
  import Settings from "$lib/Settings.svelte";
  import Icon from "$lib/Icon.svelte";
  import ItemDetail from "$lib/ItemDetail.svelte";
  import PlaylistsView from "$lib/PlaylistsView.svelte";
  import ServerPlaylistView from "$lib/ServerPlaylistView.svelte";
  import SeasonDetail from "$lib/SeasonDetail.svelte";
  import { friendlyError } from "$lib/errors";
  import { imageReveal } from "$lib/imageReveal";
  import {
    detailKeyOf,
    type ContinuePlayingMode,
    type Detail,
    type Item,
    type PlaybackContinuation,
    type PlaybackSourceChoiceRequest,
    type PlaylistSummary,
    type PlayCommandResult,
    type PlayIntent,
    type ServerPlaylist,
    type ServerPlaylistGroup,
    type WatchStateMutation,
  } from "$lib/types";

  // Tracked timers, cleared on destroy / when superseded.
  let copyTimer: ReturnType<typeof setTimeout> | undefined;
  let pollTimer: ReturnType<typeof setTimeout> | undefined;
  let unlistenPlaybackEnded: (() => void) | undefined;
  let unlistenContinuePlaying: (() => void) | undefined;
  let unlistenSourceChoice: (() => void) | undefined;
  let unlistenDurableFault: (() => void) | undefined;
  onDestroy(() => {
    if (copyTimer) clearTimeout(copyTimer);
    if (pollTimer) clearTimeout(pollTimer);
    unlistenPlaybackEnded?.();
    unlistenContinuePlaying?.();
    unlistenSourceChoice?.();
    unlistenDurableFault?.();
    if (sourceChoiceRequest) {
      void invoke("cancel_playback_source_choice", {
        requestId: sourceChoiceRequest.requestId,
      });
    }
    linkGen++; // invalidate any in-flight link_poll so it won't reschedule after unmount
  });

  // The scrollable browse grid, so we can keep loading until it actually scrolls.
  let gridEl: HTMLElement | undefined = $state();

  // Plex art arrives as a credential-free opaque marker. Let Tauri spell the
  // custom-protocol URL for this platform (Windows uses an http localhost
  // origin; Unix/macOS use vela-artwork://). Other server art is already
  // http(s), and local sidecars need Tauri's asset protocol.
  function posterSrc(p: string): string {
    if (p.startsWith("vela-artwork:")) {
      return convertFileSrc(p.slice("vela-artwork:".length), "vela-artwork");
    }
    return /^https?:\/\//.test(p) ? p : convertFileSrc(p);
  }

  // `provenance`: opaque server-of-origin token, handed back with any action on
  // this section so the backend can refuse one whose key it no longer issues
  // (a menu open across a library refresh, a listing a failed refresh left on
  // screen). Never interpreted here.
  type Section = {
    key: string;
    title: string;
    sectionType: string;
    sourceName?: string;
    sort?: string;
    provenance?: string;
    // Which binding of the source issued this key (see sameSection). Sources
    // that cannot rebind always send 0.
    binding?: number;
  };
  // `Item` (the listing-card DTO mirror) lives in $lib/types, shared with the
  // detail components.
  type Hub = { title: string; hubIdentifier: string; hubType: string; items: Item[]; sourceId: string; sourceName?: string };
  type Crumb = {
    title: string;
    ratingKey: string | null;
    backing?: Item["backing"];
    canonicalId?: string;
    mediaType?: string;
  };
  type Source = { id: string; name: string; kind: string };
  type DurableStatusKind =
    | "ready"
    | "recoverable_invalid"
    | "unavailable"
    | "migration_blocked";
  type DurableFileStatus = {
    status: DurableStatusKind;
    layout: "post_split" | "legacy_combined";
    canRecover: boolean;
    rollbackVersions: DurableRollbackVersion[];
  };
  type DurableRollbackVersion = { id: string; createdAtUnixMs: number };
  type DurableStateStatus = {
    settings: DurableFileStatus;
    connections: DurableFileStatus;
  };
  type DurableRecoveryResult = {
    status: DurableStateStatus;
    recovered: boolean;
    backupFileName: string | null;
    reconnectRequired: boolean;
    restoredVersion: DurableRollbackVersion | null;
    error: string | null;
  };

  let sources = $state<Source[]>([]);
  let activeSource = $state<string | null>(null); // null = All sources (unified)
  let showSettings = $state(false);

  let authenticated = $state(false);
  let mode = $state<"home" | "browse" | "playlists">("home");
  // Invalidates the playlist view after source availability or an external
  // Add-to-Playlist mutation changes what it should render.
  let playlistVersion = $state(0);
  let serverPlaylistGroups = $state<ServerPlaylistGroup[]>([]);
  let selectedServerPlaylist = $state<ServerPlaylist | null>(null);
  let serverPlaylistVersion = $state(0);
  let serverPlaylistGen = 0;
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
  type SortDirection = "asc" | "desc";
  type SortFieldKey =
    | "titleSort"
    | "year"
    | "addedAt"
    | "episodeAddedAt"
    | "originallyAvailableAt"
    | "rating"
    | "lastViewedAt";
  type SortField = {
    key: SortFieldKey;
    label: string;
    showOnly?: boolean;
    merged?: boolean;
  };
  type ParsedSort = { field: SortField; direction: SortDirection };

  const DEFAULT_SORT = "titleSort:asc";
  // One direction-neutral field inventory drives both controls and the
  // section/view restrictions. Merged listings only honor fields items carry
  // on the DTO across sources; rating and leaf-added remain source-side only.
  const SORT_FIELDS: SortField[] = [
    { key: "titleSort", label: "Title", merged: true },
    { key: "year", label: "Year", merged: true },
    { key: "addedAt", label: "Date added", merged: true },
    { key: "episodeAddedAt", label: "Last episode added", showOnly: true },
    { key: "originallyAvailableAt", label: "Release date", merged: true },
    { key: "rating", label: "Rating" },
    { key: "lastViewedAt", label: "Last played", merged: true },
  ];

  function parseSort(value: string | null | undefined): ParsedSort | null {
    const parts = value?.split(":");
    if (!parts || parts.length !== 2) return null;
    const [fieldKey, direction] = parts;
    if (direction !== "asc" && direction !== "desc") return null;
    const field = SORT_FIELDS.find((candidate) => candidate.key === fieldKey);
    return field ? { field, direction } : null;
  }

  function composeSort(field: SortFieldKey, direction: SortDirection): string {
    return `${field}:${direction}`;
  }

  function sortAllowedForSection(
    value: string | null | undefined,
    sectionType: string,
  ): value is string {
    const parsed = parseSort(value);
    return parsed !== null && (!parsed.field.showOnly || sectionType === "show");
  }

  function sortAllowedForMerged(value: string): boolean {
    return parseSort(value)?.field.merged === true;
  }

  function sourceNameOf(id: string): string {
    return sources.find((s) => s.id === id)?.name ?? "";
  }
  let crumbs = $state<Crumb[]>([]);
  let items = $state<Item[]>([]);
  let loading = $state(false);
  let loadingMore = $state(false);
  let offset = $state(0);
  let hasMore = $state(true);
  // The banner can hold failures with DIFFERENT OWNERS at once. A listing failure is
  // owned by the load generation that produced it, and a refresh that SUPERSEDES that
  // load must retract it — fresh cards under a stale "couldn't load" is a lie (codex
  // r11). A non-listing failure — a failed edit, a failed search, the refresh's own
  // sections leg — is owned by NO generation, and no refresh may retract it.
  //
  // One tag cannot describe both. Combining the two into a single string under the
  // LISTING's tag handed the edit's failure to the retract: a later successful refresh
  // repaired the grid and erased the user's edit failure along with the listing
  // diagnostic it superseded, so they never learned their change had failed (codex +
  // grok, r20 — the exact loss the r19 fix existed to prevent, arriving through the
  // retract door instead of the publish door). So the banner is a LIST of owned parts,
  // and the retract drops only the parts it actually superseded.
  //
  // `gen: 0` means no listing generation owns this part. Every write goes through
  // setError/addError so a part's owner cannot outlive the message it describes. An
  // earlier version tried to keep the tag honest by remembering the text and trusting
  // it only while `error` still matched — which fails the moment two failures produce
  // the SAME string, and they do: a 401 on a listing and a 401 on a scan both surface
  // as `RECONNECT_REQUIRED`, which friendlyError maps to one constant sentence. The
  // refresh then retracted a scan failure it never superseded and the user was left
  // with no status at all (codex r12).
  // The view's banner. ONE writer class now: the listing, the refresh, the search — the
  // things that describe THIS view and die with it. Every other writer reports on its own
  // surface (the scan's, the edit's, the mpv bar's, the detail's), which is
  // what `.agents/plans/per-surface-status.md` was for.
  //
  // That is why there is no `owner` field here any more, no per-surface clear, and no
  // scope. All of that existed to referee writers with different lifetimes fighting over
  // one surface, and it never worked: eight consecutive review rounds, each fix opening
  // the next door, every one of them the same defect — a failure the user needed,
  // silently lost (library-refresh-scan log, r17-r24). The referee is gone because the
  // fight is gone.
  //
  // What REMAINS is view-vs-view, and it is load-scoped, not surface-scoped:
  //
  //   `gen`             the load generation that published this part (0 = no load owns it:
  //                     a search's validation message, the refresh's own sections leg).
  //   `retractThrough`  a refresh that REPLACED a load's cards retracts that load's
  //                     diagnostic and nothing else — fresh cards under a stale
  //                     "couldn't load" is a lie (codex r11), but a part no load owns was
  //                     never the refresh's to take back (codex + grok, r20).
  //   the weaker-claim  two listing failures can render the SAME sentence (a 401 on a
  //   merge in addError listing and a 401 on a page both collapse to one constant
  //                     RECONNECT_REQUIRED line — codex r12), so deduplicating on text
  //                     must not silently decide which load owns what is left (codex r21).
  //
  // Those are real rules about one surface with one kind of writer, and they stay.
  let errorParts = $state<{ msg: string; gen: number }[]>([]);
  const error = $derived(
    errorParts.length === 0 ? null : errorParts.map((p) => p.msg).join("; "),
  );
  function setError(msg: string | null, gen = 0) {
    if (msg === null) {
      errorParts = [];
      return;
    }
    // A newer load's failure supersedes the diagnostics of the loads it replaced, and
    // leaves alone the parts no load owns.
    errorParts = errorParts.filter((p) => p.gen === 0);
    addError(msg, gen);
  }
  // The view has been REPLACED (the device-code screen, a torn-down search root) rather
  // than reloaded: everything it was saying goes with it.
  function clearViewErrors() {
    errorParts = [];
  }
  // Say something without ERASING what is already there — the two failures are both true.
  // Identical text is not repeated: a retried offset can fail the same way twice.
  function addError(msg: string, gen = 0) {
    const at = errorParts.findIndex((p) => p.msg === msg);
    if (at === -1) {
      errorParts = [...errorParts, { msg, gen }];
      return;
    }
    // The same sentence, now true for a second load as well. The WEAKER claim wins: if
    // either reason is owned by no load, no refresh may take the line back; if both are
    // listing failures, it survives until the LATER one is superseded too. Keeping the
    // FIRST owner instead is how a failure became retractable by a refresh that had
    // superseded nothing (codex r21).
    const held = errorParts[at].gen;
    const weaker = held === 0 || gen === 0 ? 0 : Math.max(held, gen);
    if (weaker !== held)
      errorParts = errorParts.map((p, i) => (i === at ? { msg, gen: weaker } : p));
  }
  // Retract every part owned by a load through `claimedGen` — and nothing else. An
  // untagged part is not the refresh's to take back, and a NEWER load's failure
  // supersedes the refresh in turn.
  function retractThrough(claimedGen: number) {
    errorParts = errorParts.filter((p) => !(p.gen && p.gen <= claimedGen));
  }
  let sort = $state(DEFAULT_SORT);
  let parsedSort = $derived(
    parseSort(sort) ?? { field: SORT_FIELDS[0], direction: "asc" as SortDirection },
  );
  let sortDirectionLabel = $derived(
    parsedSort.direction === "asc"
      ? "Sort direction: ascending; activate for descending"
      : "Sort direction: descending; activate for ascending",
  );
  let searchQuery = $state("");
  let searchTerm = $state(""); // the query backing the current search results view
  const PAGE = 60;

  // Device-link state
  type Pin = {
    id: string;
    code: string;
    clientIdentifier: string;
    authUrl: string;
    qrSvg: string;
  };
  type PlexServerChoice = { machineIdentifier: string; name: string };
  type LinkPoll =
    | { status: "pending" }
    | { status: "chooseServer"; servers: PlexServerChoice[] }
    | { status: "connected"; source: Source };
  let pin = $state<Pin | null>(null);
  let plexServerChoices = $state<PlexServerChoice[]>([]);
  let selectingPlexServer = $state(false);

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
  let continuePlayingMode = $state<ContinuePlayingMode>("only-tv");
  let copied = $state(false);
  let installingMpv = $state(false);
  let durableStatus = $state<DurableStateStatus | null>(null);
  let retryingDurableState = $state(false);
  let recoveringInvalidFile = $state(false);
  let rollingBackVersionId = $state<string | null>(null);
  let durableRetryError = $state<string | null>(null);
  let durableBackupFileName = $state<string | null>(null);
  let durableRecoveryNotice = $state<string | null>(null);
  let durableBusy = $derived(retryingDurableState || recoveringInvalidFile);
  let normalBooted = false;
  let durableHeading: HTMLHeadingElement | undefined = $state();
  let normalRoot: HTMLDivElement | undefined = $state();

  // One-click mpv install. The backend chooses the concrete method for this OS.
  // On success it returns refreshed status, which clears the prompt.
  async function installMpv() {
    if (installingMpv) return;
    installingMpv = true;
    mpvStatus = null; // this attempt supersedes the last one's failure
    try {
      mpvInfo = await invoke<MpvInfo>("install_mpv");
    } catch (e) {
      // On the BAR's own surface, never the view's banner (slice 3). It sits with the
      // Retry button it is telling the user to press.
      mpvStatus = `Couldn't install mpv — ${String(e)}`;
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
  // Shared by the playback-ended event and explicit hero curation. Successful
  // watched-state edits use refreshAfterWatchEdit instead: a browse listing has
  // to retain its loaded depth and scroll while it revalidates server truth.
  // Returns the reload it kicked off, so a caller that must publish AFTER the
  // repaint can actually wait for it (an un-awaited `await` on a void function is
  // a no-op that looks correct — see setWatched, codex r18).
  function refreshWatchState(): Promise<unknown> {
    heroPos = 0; // the most recent change should be front and center
    if (mode === "home") {
      return loadHome(++homeGen);
    } else if (mode === "playlists") {
      // Vela playlist snapshots are durable curation, while a selected server
      // playlist is live server data. Refresh only that read-only detail and
      // keep either playlist root intact.
      hubs = [];
      if (selectedServerPlaylist) serverPlaylistVersion++;
      return Promise.resolve();
    } else {
      // The hidden Home hubs are stale now; empty them so goHome() re-fetches.
      hubs = [];
      // Refresh the visible listing so its progress bars / played badges
      // update. The person root re-runs its own query, gated to the ROOT
      // level (plan-review r2): a drilled level under it refreshes through
      // resetAndLoad, whose crumb has a ratingKey.
      // Both gated to the ROOT level. A drilled level BELOW a search or a person view
      // refreshes through resetAndLoad, whose crumb carries a ratingKey (plan-review r2
      // established this for the person root; the search root never got it). Ungated,
      // the search re-run REPLACES a multi-crumb drill trail with the one-crumb search
      // root — so changing watch state on a drilled item would yank the user back out to
      // the search results instead of repainting the drilled level (codex r21).
      if (searchTerm && crumbs.length === 1)
        return runSearch(searchTerm, { rerun: true });
      if (personView && crumbs.length === 1)
        return runPersonView(personView, { rerun: true });
      // PRESERVE: a failed re-fetch must not take the user's library away from them (see
      // resetAndLoad).
      return resetAndLoad({ preserve: true });
    }
  }

  // A failed edit creates no new browse truth: the backend rolls its temporary
  // curation back and the frontend has not mutated the clicked card. Re-entering
  // a browse/search/person/drill root here only blanks loaded cards, loses pages
  // and scroll, and manufactures a view failure while the server is unavailable.
  // Home is the exception because a load racing the curate-first backend call may
  // have rendered transient recents/tombstones. Invalidate hidden Home state, and
  // rebuild it immediately only when Home is the active underlying root.
  async function repairFailedWatchEdit(): Promise<void> {
    hubs = [];
    if (authenticated && mode === "home") await loadHome(++homeGen);
  }

  onMount(() => {
    listen<DurableStateStatus>("durable-state-fault", async ({ payload }) => {
      durableStatus = payload;
      durableRetryError = null;
      durableBackupFileName = null;
      await tick();
      durableHeading?.focus();
    }).then((un) => (unlistenDurableFault = un));
    listen("playback-ended", refreshWatchState).then((un) => (unlistenPlaybackEnded = un));
    listen<PlaybackContinuation>("continue-playing", handlePlaybackContinuation).then(
      (un) => (unlistenContinuePlaying = un),
    );
    listen<{ requestId: string }>("source-choice-required", handleSourceChoiceEvent).then(
      (un) => (unlistenSourceChoice = un),
    );
  });

  function durableReady(status: DurableStateStatus | null): boolean {
    return (
      status !== null &&
      status.settings.status === "ready" &&
      status.connections.status === "ready"
    );
  }

  function durableFault(status: DurableStateStatus): {
    file: "settings" | "connections";
    value: DurableFileStatus;
  } {
    return status.settings.status !== "ready"
      ? { file: "settings", value: status.settings }
      : { file: "connections", value: status.connections };
  }

  async function boot() {
    try {
      durableStatus = await invoke<DurableStateStatus>("get_durable_state_status");
    } catch {
      durableStatus = {
        settings: {
          status: "unavailable",
          layout: "post_split",
          canRecover: false,
          rollbackVersions: [],
        },
        connections: {
          status: "ready",
          layout: "post_split",
          canRecover: false,
          rollbackVersions: [],
        },
      };
    }
    await tick();
    durableHeading?.focus();
    if (!durableReady(durableStatus)) return;
    await bootNormal();
  }

  async function bootNormal() {
    if (normalBooted) return;
    normalBooted = true;
    // Check mpv up front so we can prompt to install before the user hits play.
    invoke<MpvInfo>("check_mpv").then((m) => (mpvInfo = m)).catch(() => {});
    invoke<AppInfo>("get_app_info").then((a) => (appInfo = a)).catch(() => {});
    invoke<ContinuePlayingMode>("get_continue_playing")
      .then((mode) => (continuePlayingMode = mode))
      .catch(() => {});
    try {
      await loadSourceList();
      authenticated = sources.length > 0;
      if (authenticated) {
        void loadServerPlaylists();
        await loadEverything();
      }
    } catch (e) {
      setError(String(e));
    }
  }

  async function retryDurableState() {
    if (durableBusy) return;
    retryingDurableState = true;
    durableRetryError = null;
    durableBackupFileName = null;
    try {
      durableStatus = await invoke<DurableStateStatus>("retry_durable_state");
      if (durableReady(durableStatus)) {
        await resumeAfterDurableReady();
      } else {
        await tick();
        durableHeading?.focus();
      }
    } catch {
      durableRetryError = "Vela could not safely retry the files.";
    } finally {
      retryingDurableState = false;
    }
  }

  async function resumeAfterDurableReady() {
    if (normalBooted) await onSourcesChanged();
    else await bootNormal();
    await tick();
    normalRoot?.focus();
  }

  async function recoverInvalidFile(file: "settings" | "connections") {
    if (durableBusy) return;
    recoveringInvalidFile = true;
    durableRetryError = null;
    durableBackupFileName = null;
    try {
      const result = await invoke<DurableRecoveryResult>("recover_invalid_file", { file });
      durableStatus = result.status;
      durableBackupFileName = result.backupFileName;
      durableRetryError = result.error;
      if (result.recovered && result.backupFileName) {
        durableRecoveryNotice = result.reconnectRequired
          ? `The damaged file was preserved as ${result.backupFileName}. Connect your servers again to continue.`
          : `The damaged file was preserved as ${result.backupFileName}. Your server connections were kept.`;
      }
      if (durableReady(durableStatus)) {
        await resumeAfterDurableReady();
      } else {
        await tick();
        durableHeading?.focus();
      }
    } catch {
      durableRetryError = "Vela could not safely recover the file.";
      await tick();
      durableHeading?.focus();
    } finally {
      recoveringInvalidFile = false;
    }
  }

  function formatRollbackDate(createdAtUnixMs: number): string {
    return new Date(createdAtUnixMs).toLocaleString([], {
      dateStyle: "medium",
      timeStyle: "short",
    });
  }

  async function rollbackInvalidFile(
    file: "settings" | "connections",
    version: DurableRollbackVersion,
  ) {
    if (durableBusy) return;
    recoveringInvalidFile = true;
    rollingBackVersionId = version.id;
    durableRetryError = null;
    durableBackupFileName = null;
    try {
      const result = await invoke<DurableRecoveryResult>("rollback_invalid_file", {
        file,
        versionId: version.id,
      });
      durableStatus = result.status;
      durableBackupFileName = result.backupFileName;
      durableRetryError = result.error;
      if (result.recovered && result.backupFileName && result.restoredVersion) {
        durableRecoveryNotice = `Restored the ${file} version from ${formatRollbackDate(
          result.restoredVersion.createdAtUnixMs,
        )}. The damaged file was preserved as ${result.backupFileName}.`;
      }
      if (durableReady(durableStatus)) {
        await resumeAfterDurableReady();
      } else {
        await tick();
        durableHeading?.focus();
      }
    } catch {
      durableRetryError = "Vela could not safely restore that version.";
      await tick();
      durableHeading?.focus();
    } finally {
      rollingBackVersionId = null;
      recoveringInvalidFile = false;
    }
  }

  function exitVela() {
    void invoke("exit_vela");
  }

  async function loadSourceList() {
    sources = await invoke<Source[]>("get_sources");
  }

  async function loadServerPlaylists(): Promise<void> {
    const gen = ++serverPlaylistGen;
    if (sources.length === 0) {
      serverPlaylistGroups = [];
      return;
    }
    try {
      const loaded = await invoke<ServerPlaylistGroup[]>("get_server_playlists");
      if (gen === serverPlaylistGen) serverPlaylistGroups = loaded;
    } catch {
      if (gen === serverPlaylistGen) {
        serverPlaylistGroups = sources.map((source) => ({
          sourceId: source.id,
          sourceName: source.name,
          sourceKind: source.kind,
          available: false,
          playlists: [],
        }));
      }
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
    // The source list is changing under the view: that IS navigation (a refresh
    // still in flight must stop owning the epoch — codex r6), and it has to be
    // declared BEFORE the first await, not after. A refresh can settle DURING
    // that await and publish against an epoch that still matches; if the last
    // source was just removed, the teardown below then leaves that banner
    // standing over the Welcome screen — a dead server's failure, with nothing
    // left on screen to explain it and no way to clear it (codex r14).
    navEpoch++;
    serverPlaylistGen++;
    await loadSourceList();
    // And AGAIN, now that the change has actually landed. The bump above
    // invalidates whatever was in flight when Settings called us — but Settings
    // does not await this, so the user can close it and hit Refresh DURING the
    // await, and that refresh would otherwise own the post-change epoch: it would
    // go on blocking the empty-Home redirect over a view its own request no longer
    // describes, stalling navigation until it settled or timed out (codex r15).
    // Same shape as the link flow's double bump (codex r8): declare the intent
    // before the await, and the fact after it.
    navEpoch++;
    if (!sources.some((s) => s.id === activeSource)) activeSource = null;
    authenticated = sources.length > 0;
    if (!sources.some((source) => source.id === selectedServerPlaylist?.sourceId)) {
      selectedServerPlaylist = null;
    }
    void loadServerPlaylists();
    // A scan in flight belongs to a library in the list that just changed — it may
    // be a library of a source that is no longer configured. Its outcome is about
    // to be meaningless whichever branch we take: dropping the LAST source leaves
    // it publishing over Welcome, and dropping ONE of several leaves it publishing
    // over whatever the user is looking at now, naming a library they no longer
    // have (codex r15, r16). Abandon it here, once, for both.
    scanAttempt++; // its publication check now fails
    clearScanStatus();
    // Same for a watch-state edit in flight: its outcome is about an item in a source list
    // that has just changed, and the last-source branch below leaves nothing it could
    // sensibly report on (the r14/r15 rule, applied to the edit's line).
    editAttempt++;
    clearEditStatus();
    playlistVersion++;
    serverPlaylistVersion++;
    if (mode === "playlists") {
      // Saved playlists remain available when the last source is removed;
      // the view reload marks those retained entries unavailable in place.
      sourceGen++;
      homeGen++;
      loadGen++;
      linkGen++;
      pin = null;
      hubs = [];
      continueHubs = [];
      items = [];
      loading = false;
      setError(null);
      if (authenticated) await loadSections(++sourceGen);
      else sections = [];
      return;
    }
    if (authenticated) {
      linkGen++; // abandon any in-flight Plex link poll tied to the old pin
      pin = null;
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
      continueHubs = [];
      sections = [];
      items = [];
      crumbs = [];
      active = null;
      activeType = null;
      detailView = null;
      mode = "home";
      loading = false;
      // The view this banner described no longer exists, and the Welcome screen
      // offers nothing that could clear it (codex r14). In-flight scans were
      // already abandoned above, for both branches.
      setError(null);
    }
  }

  // Open a URL in the system browser via the backend (webview would navigate away).
  async function openExternal(url: string) {
    try {
      await invoke("open_url", { url });
    } catch (e) {
      setError(String(e));
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
  // Retain the exact Home hub feed even while another surface invalidates its
  // visible Home cache. Playlist playback produces `playback-ended` between
  // entries; clearing the only copy there would make terminal continuation see
  // a recents-only list that the carousel never rendered.
  let continueHubs = $state<Hub[]>([]);
  // Keys the user removed from Continue Watching; suppressed from both hero
  // feeds even while a server hub still carries them.
  let continueTombstones = $state<string[]>([]);
  let heroPos = $state(0);
  let heroItems = $derived.by(() => {
    const scoped = activeSource ? recents.filter((r) => r.sourceId === activeSource) : recents;
    const hubItems = continueHubs
      .filter((h) => hubPolicy(h) === "hero")
      .flatMap((h) => h.items);
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

  // One automatic run at a time. A manual play has a different backend
  // session, so its terminal event starts a fresh seen set. An automatic play
  // stores the returned session; only that exact completion inherits the set.
  let continuationSession: string | null = null;
  let continuationSeen = new Set<string>();
  let continuationAttempt = 0;

  let sourceChoiceRequest = $state<PlaybackSourceChoiceRequest | null>(null);
  let sourceChoiceBusy = $state(false);
  let sourceChoiceDialog: HTMLDivElement | undefined = $state();
  let sourceChoiceOnStarted: ((sessionId: string) => void) | null = null;
  let sourceChoicePreviousFocus: HTMLElement | null = null;

  function localityLabel(locality: string): string {
    if (locality === "same-machine") return "This computer";
    if (locality === "lan") return "Local network";
    return "Internet";
  }

  function closeSourceChoiceUi(requestId?: string) {
    if (requestId && sourceChoiceRequest?.requestId !== requestId) return;
    sourceChoiceRequest = null;
    sourceChoiceBusy = false;
    sourceChoiceOnStarted = null;
    const previous = sourceChoicePreviousFocus;
    sourceChoicePreviousFocus = null;
    if (previous?.isConnected) void tick().then(() => previous.focus());
  }

  async function presentSourceChoice(
    request: PlaybackSourceChoiceRequest,
    onStarted: ((sessionId: string) => void) | null = null,
  ) {
    if (sourceChoiceRequest?.requestId === request.requestId) {
      if (onStarted) sourceChoiceOnStarted = onStarted;
      return;
    }
    if (sourceChoiceRequest) {
      void invoke("cancel_playback_source_choice", {
        requestId: sourceChoiceRequest.requestId,
      });
    }
    sourceChoicePreviousFocus = document.activeElement as HTMLElement | null;
    sourceChoiceRequest = request;
    sourceChoiceOnStarted = onStarted;
    sourceChoiceBusy = false;
    await tick();
    sourceChoiceDialog?.querySelector<HTMLButtonElement>("button.choice")?.focus();
  }

  async function handlePlayCommandResult(
    result: PlayCommandResult,
    onStarted: ((sessionId: string) => void) | null = null,
  ) {
    if (result.status === "started") {
      onStarted?.(result.sessionId);
    } else if (result.status === "sourceChoiceRequired") {
      await presentSourceChoice(result.request, onStarted);
    }
  }

  async function choosePlaybackSource(sourceId: string) {
    const request = sourceChoiceRequest;
    if (!request || sourceChoiceBusy) return;
    const onStarted = sourceChoiceOnStarted;
    sourceChoiceBusy = true;
    try {
      const result = await invoke<PlayCommandResult>("resolve_playback_source_choice", {
        requestId: request.requestId,
        sourceId,
      });
      if (sourceChoiceRequest?.requestId !== request.requestId) return;
      closeSourceChoiceUi(request.requestId);
      await handlePlayCommandResult(result, onStarted);
    } catch (error) {
      if (sourceChoiceRequest?.requestId !== request.requestId) return;
      closeSourceChoiceUi(request.requestId);
      const said = `Couldn't play “${request.title}” — ${String(error)}`;
      if (detailView) detailStatus = said;
      else setError(said);
    }
  }

  function cancelSourceChoice() {
    const requestId = sourceChoiceRequest?.requestId;
    if (!requestId || sourceChoiceBusy) return;
    closeSourceChoiceUi(requestId);
    void invoke("cancel_playback_source_choice", { requestId });
  }

  function handleSourceChoiceDialogKeydown(event: KeyboardEvent) {
    if (event.key !== "Tab" || !sourceChoiceDialog) return;
    const buttons = Array.from(
      sourceChoiceDialog.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"),
    );
    if (buttons.length === 0) {
      event.preventDefault();
      return;
    }
    const current = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (event.shiftKey && current <= 0) {
      event.preventDefault();
      buttons.at(-1)?.focus();
    } else if (!event.shiftKey && current === buttons.length - 1) {
      event.preventDefault();
      buttons[0].focus();
    }
  }

  async function handleSourceChoiceEvent(event: { payload: { requestId: string } }) {
    try {
      const request = await invoke<PlaybackSourceChoiceRequest>("get_playback_source_choice", {
        requestId: event.payload.requestId,
      });
      await presentSourceChoice(request);
    } catch {
      // The request may have expired or a newer manual play may have replaced it.
    }
  }

  function finishPlaybackRun(sessionId: string) {
    void invoke("finish_playback_run", { sessionId });
  }

  function invalidateContinuationRun() {
    continuationAttempt++;
    continuationSession = null;
    continuationSeen = new Set();
  }

  function resetContinuationRun() {
    const previousSession = continuationSession;
    invalidateContinuationRun();
    if (previousSession) finishPlaybackRun(previousSession);
  }

  function changeContinuePlayingMode(mode: ContinuePlayingMode) {
    continuePlayingMode = mode;
    resetContinuationRun();
  }

  async function handlePlaybackContinuation(event: { payload: PlaybackContinuation }) {
    const completed = event.payload;
    if (continuationSession !== completed.sessionId) {
      continuationSeen = new Set();
    }
    continuationSeen.add(completed.itemKey);
    continuationSession = null;
    const attempt = ++continuationAttempt;

    if (continuePlayingMode === "off") {
      finishPlaybackRun(completed.sessionId);
      return;
    }

    try {
      let next: Item | null = null;
      if (continuePlayingMode === "on") {
        // This is intentionally the literal derived list the Home carousel
        // renders: same active-source scope, tombstones, dedup, and ordering.
        next = heroItems.find((item) => !continuationSeen.has(item.ratingKey)) ?? null;
      } else if (completed.mediaType === "episode") {
        next = await invoke<Item | null>("next_episode", {
          itemKey: completed.itemKey,
          sessionId: completed.sessionId,
        });
      }
      if (attempt !== continuationAttempt || !next) {
        finishPlaybackRun(completed.sessionId);
        return;
      }
      if (continuationSeen.has(next.ratingKey)) {
        finishPlaybackRun(completed.sessionId);
        return;
      }
      const selectedNext = next;
      const result = await invoke<PlayCommandResult>("play_item", {
        item: selectedNext,
        startFromBeginning: false,
        expectedSession: completed.sessionId,
        seriesContinuation:
          continuePlayingMode === "only-tv" && selectedNext.mediaType === "episode",
        explicitSourceId: null,
      });
      await handlePlayCommandResult(result, (session) => {
        if (attempt !== continuationAttempt) return;
        continuationSeen.add(selectedNext.ratingKey);
        continuationSession = session;
      });
      await refreshWatchState();
    } catch (e) {
      if (attempt === continuationAttempt) {
        setError(`Couldn't continue playing — ${String(e)}`);
        continuationSession = null;
      }
      finishPlaybackRun(completed.sessionId);
    }
  }

  // Bug 3 (owner UX ruling 2026-07-05): a scoped source's per-source Home must
  // never terminate on the empty-Home dead-end. A connected media server can
  // expose library sections without contributing Home hubs or unfinished
  // recents, so its scoped Home settles empty even though browseable content is
  // in the sidebar. When that happens, land on its first section.
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
    continueHubs = [];
    sections = [];
    const sg = ++sourceGen;
    const hg = ++homeGen;
    await Promise.all([loadSections(sg), loadHome(hg)]);
  }

  async function loadSections(gen: number = sourceGen) {
    try {
      const s = await invoke<Section[]>("get_sections", { sourceId: activeSource });
      if (gen === sourceGen) sections = s;
    } catch (e) {
      if (gen === sourceGen && mode !== "playlists") setError(String(e));
    }
  }

  async function loadHome(gen: number = homeGen) {
    mode = "home";
    loading = true;
    // Clears the VIEW's parts only — the mpv bar and an open detail are not
    // this load's to silence (see setError).
    setError(null);
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
        continueHubs = h;
        recents = r;
        continueTombstones = t;
      }
    } catch (e) {
      if (gen === homeGen) setError(String(e));
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
    setError(null); // don't carry a browse/search error banner onto Home
    searchTerm = "";
    personView = null;
    activeType = null;
    mode = "home";
    // Entering a browse earlier may have discarded an in-flight hub load (via the
    // homeGen bump). If we have no hubs, re-fetch so Home isn't stuck empty.
    if (hubs.length === 0) loadHome(++homeGen);
  }

  function openPlaylists() {
    navEpoch++;
    detailView = null;
    detailStatus = null;
    homeGen++;
    loadGen++;
    loadingMore = false;
    loading = false;
    searchTerm = "";
    personView = null;
    active = null;
    activeType = null;
    crumbs = [];
    items = [];
    setError(null);
    closeMenu();
    closeSectionMenu();
    selectedServerPlaylist = null;
    mode = "playlists";
    void loadServerPlaylists();
  }

  function openServerPlaylist(playlist: ServerPlaylist) {
    openPlaylists();
    selectedServerPlaylist = playlist;
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
  // Failures the action silenced on the promise of replacing their result. If it
  // never claims the grid, that promise is broken and they must be published:
  // the load that failed is the reason the grid is empty (codex r16).
  let suppressedFailures: { msg: string; gen: number }[] = [];

  type RootKind = "home" | "playlists" | "section-grid" | "type-grid" | "search" | "person" | "drill" | "detail";

  // What the user is actually looking at — derived from visible state, never
  // residual state: goHome() leaves `active` set, and a search retains
  // `activeType`, so "has an active section/type" does not mean "is looking
  // at that grid".
  function visibleRootKind(): RootKind {
    if (detailView) return "detail";
    if (mode === "home") return "home";
    if (mode === "playlists") return "playlists";
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
  function currentSectionRoot(): Section | null {
    if (mode !== "browse" || personView || searchTerm || !active) return null;
    const here = crumbs[crumbs.length - 1];
    return here?.ratingKey ? null : active;
  }

  // Two sections are the SAME LIBRARY only if their key AND their binding match.
  // A Plex source that rebinds to a server it cannot prove is the one that issued
  // its keys (rediscovery on a server whose identity was never established)
  // reissues the same section NUMBERS for different libraries — so "the key is
  // still in the list" proves nothing, and the root the user is standing on may
  // now be a stranger's library under the old one's title (codex r12). Sources
  // that cannot rebind always issue binding 0, so this is exactly the old
  // key check for them.
  function sameSection(a: Section, b: Section): boolean {
    return a.key === b.key && (a.binding ?? 0) === (b.binding ?? 0);
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
      setError(null);
      suppressedFailures = [];
      // Snapshot what this action reconciles against.
      const epoch = navEpoch;
      refreshEpoch = epoch;
      const kind = visibleRootKind();
      // The root section ITSELF, not its key: identity is key + binding
      // (see sameSection).
      const rootSection = kind === "section-grid" ? active! : null;
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
            const rootNow = currentSectionRoot();
            if (rootNow !== null && !s.some((sec) => sameSection(sec, rootNow))) {
              forceHomeForRemovedRoot();
            }
          }
          // The root the user is standing on survived the refresh — but `active`
          // and its crumb are still the objects from the PREVIOUS list. If the
          // library was renamed on the server, the sidebar would show the new
          // title while the grid above it still carried the old one (codex r16).
          // Re-bind to the refreshed section: same library (sameSection), current
          // facts.
          const here = active;
          if (here) {
            const fresh = s.find((sec) => sameSection(sec, here));
            if (fresh) {
              active = fresh;
              if (crumbs.length > 0 && !crumbs[0].ratingKey) {
                crumbs = [{ ...crumbs[0], title: fresh.title }, ...crumbs.slice(1)];
              }
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
      // The listing generation the content leg claims, if it gets that far. Every
      // load at or below it is one this action SUPERSEDED, so a failure banner
      // one of them left behind is ours to retract at settlement.
      let claimedGen = 0;
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
              continueHubs = h;
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
        // replace it: the library rendered an authoritative empty state,
        // unable to paginate, until the user navigated away (codex r5).
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
            if (
              kind === "section-grid" &&
              !list.some((sec) => sameSection(sec, rootSection!))
            ) {
              return; // root gone: the disappearance fallback owns this outcome
            }
            myGen = claimedGen = ++loadGen; // NOW we own the grid: discard any older load
            loadingMore = false;
            offset = 0;
            hasMore = true;
            items = [];
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
        // A load we silenced but never replaced still owns the grid. We only
        // silenced it because we meant to take that grid over; if our content leg
        // never claimed it (the sections leg failed, so it returned early), the
        // grid is empty BECAUSE that load failed, and no one else is going to say
        // so (codex r16).
        const abandoned = claimedGen
          ? []
          : suppressedFailures.filter((f) => f.gen === loadGen);
        if (live.length > 0 || abandoned.length > 0) {
          // The click cleared the banner, so ANY part still here was published during
          // our run, by someone else: a newer listing load, a failed search, a failed
          // edit. None of them are ours to erase — and the one that owns the grid is
          // usually the only thing explaining why it is empty (grok r17). Say our
          // piece AFTER theirs, each part under its own owner.
          //
          // Our own legs failed, and no listing generation owns a sections/home leg —
          // they are untagged, so no future refresh can retract them. But a listing
          // failure we SILENCED keeps the generation that produced it: if a later
          // refresh replaces that grid, its diagnostic must go with the cards it
          // described.
          for (const f of live) addError(f.msg);
          for (const f of abandoned) addError(f.msg, f.gen);
        }
        // Nothing of ours failed — but a load we SUPERSEDED may have published a
        // banner after the click cleared the surface. Its cards are gone, replaced by
        // ours, so its failure no longer describes anything on screen: fresh cards
        // under a stale "couldn't load" message (codex r11). Retract it, and ONLY it —
        // a NEWER load supersedes US in turn and its failure is the one the user
        // needs, and an UNTAGGED part (an edit's failure) was never ours to take back
        // at all (codex + grok, r20).
        else if (claimedGen) retractThrough(claimedGen);
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
    plexServerChoices = [];
    selectingPlexServer = false;
    // (or superseded) begin can't leave a dead, unpolled code on screen
    try {
      const p = await invoke<Pin>("link_begin");
      if (gen !== linkGen) return; // a newer attempt started while we were requesting
      pin = p;
      // The device-code screen has REPLACED the view, so everything the old view was
      // saying goes with it — including a failure published during this very await,
      // while the grid was still up (codex r21).
      clearViewErrors();
      // The bump at the top of beginLink() invalidates whatever was in flight
      // THEN — but Settings closes immediately, so the user can start a Refresh
      // while link_begin is still awaiting. THIS is the moment the PIN screen
      // replaces the view, so it is the moment that refresh must stop owning it
      // (codex r8).
      navEpoch++;
      pollLink(gen);
    } catch (e) {
      // e.g. invoked from Settings while offline — surface it instead of an
      // unhandled rejection, but only if this attempt is still the current one.
      // No pin is coming: the user is left on the view they were already on, which is
      // still theirs and still described by whatever it was saying.
      if (gen === linkGen) setError(String(e));
    }
  }

  async function pollLink(gen: number) {
    if (gen !== linkGen || !pin) return;
    try {
      const result = await invoke<LinkPoll>("link_poll", {
        pinId: pin.id,
        clientIdentifier: pin.clientIdentifier,
      });
      if (gen !== linkGen) return; // a newer link attempt superseded this one
      if (result.status === "connected") {
        await finishLink(gen);
        return;
      }
      if (result.status === "chooseServer") {
        plexServerChoices = result.servers;
        return;
      }
    } catch (e) {
      // Terminal error (expired/rate-limited/server failure) — stop polling and
      // clear the dead code so the UI doesn't keep showing it with no poll loop.
      if (gen === linkGen) {
        setError(String(e));
        pin = null;
        plexServerChoices = [];
      }
      return;
    }
    if (gen === linkGen) pollTimer = setTimeout(() => pollLink(gen), 2000);
  }

  async function finishLink(gen: number) {
    if (gen !== linkGen) return;
    pin = null;
    plexServerChoices = [];
    authenticated = true;
    navEpoch++; // the linked source resets the view (see beginLink)
    await loadSourceList(); // surface the new Plex source in the switcher
    await loadEverything();
  }

  async function selectPlexServer(machineIdentifier: string) {
    if (!pin || selectingPlexServer) return;
    const gen = linkGen;
    const currentPin = pin;
    selectingPlexServer = true;
    try {
      await invoke<Source>("link_select_server", {
        pinId: currentPin.id,
        clientIdentifier: currentPin.clientIdentifier,
        machineIdentifier,
      });
      await finishLink(gen);
    } catch (e) {
      if (gen === linkGen) setError(String(e));
    } finally {
      if (gen === linkGen) selectingPlexServer = false;
    }
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
    // per-library preference when its field and direction are both valid for
    // this section's type, else the default. This also guarantees a show-only
    // field can never leak in from the previously viewed section.
    sort = sortAllowedForSection(section.sort, section.sectionType)
      ? section.sort
      : DEFAULT_SORT;
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
    if (!sortAllowedForMerged(sort)) sort = DEFAULT_SORT;
    crumbs = [{ title: TYPE_LABELS[t] ?? t, ratingKey: null }];
    await resetAndLoad();
  }

  // Bumped on every navigation; in-flight loads from an older generation are discarded.
  let loadGen = 0;

  // Immutable coordinates for one paginated browse request. Besides making
  // loadMore independent of reactive state after its request starts, this is
  // the identity a successful watched-state edit must still own before its
  // buffered revalidation may publish.
  type ListingRequest =
    | {
        kind: "children";
        ratingKey: string;
        backing?: Item["backing"];
        canonicalId?: string;
        mediaType?: string;
      }
    | { kind: "type"; sectionType: string; sourceId: string | null; sort: string }
    | {
        kind: "section";
        sectionKey: string;
        sectionType: string;
        binding: number;
        sort: string;
      };

  function currentListingRequest(): ListingRequest | null {
    if (mode !== "browse") return null;
    const here = crumbs[crumbs.length - 1];
    if (here?.ratingKey) {
      return {
        kind: "children",
        ratingKey: here.ratingKey,
        backing: here.backing,
        canonicalId: here.canonicalId,
        mediaType: here.mediaType,
      };
    }
    if (searchTerm || personView) return null;
    if (activeType) {
      return { kind: "type", sectionType: activeType, sourceId: activeSource, sort };
    }
    if (active) {
      return {
        kind: "section",
        sectionKey: active.key,
        sectionType: active.sectionType,
        binding: active.binding ?? 0,
        sort,
      };
    }
    return null;
  }

  function sameListingRequest(a: ListingRequest, b: ListingRequest | null): boolean {
    if (!b || a.kind !== b.kind) return false;
    if (a.kind === "children" && b.kind === "children") {
      return (
        a.ratingKey === b.ratingKey &&
        a.canonicalId === b.canonicalId &&
        a.mediaType === b.mediaType &&
        JSON.stringify(a.backing ?? []) === JSON.stringify(b.backing ?? [])
      );
    }
    if (a.kind === "type" && b.kind === "type") {
      return (
        a.sectionType === b.sectionType &&
        a.sourceId === b.sourceId &&
        a.sort === b.sort
      );
    }
    if (a.kind === "section" && b.kind === "section") {
      return (
        a.sectionKey === b.sectionKey &&
        a.sectionType === b.sectionType &&
        a.binding === b.binding &&
        a.sort === b.sort
      );
    }
    return false;
  }

  function fetchListingPage(request: ListingRequest, start: number, size: number): Promise<Item[]> {
    if (request.kind === "children") {
      return invoke<Item[]>("get_children", {
        ratingKey: request.ratingKey,
        backing: request.backing,
        canonicalId: request.canonicalId,
        mediaType: request.mediaType,
        start,
        size,
      });
    }
    if (request.kind === "type") {
      return invoke<Item[]>("get_type_listing", {
        sectionType: request.sectionType,
        sort: request.sort,
        start,
        size,
      });
    }
    return invoke<Item[]>("get_items", {
      sectionKey: request.sectionKey,
      sectionType: request.sectionType,
      sort: request.sort,
      start,
      size,
    });
  }

  async function resetAndLoad({
    keepError = false,
    preserve = false,
  }: { keepError?: boolean; preserve?: boolean } = {}) {
    homeGen++; // leaving home: invalidate any in-flight home/sections load
    const myGen = ++loadGen;
    loadingMore = false; // abandon any in-flight load (its results are now stale)
    // `preserve`: this is a RE-ENTRY of the root the user is already standing on (a
    // watch-state repaint), not a navigation to a new one. Blanking the grid up front is
    // right when you are LEAVING a view — the old cards would be a lie — but here the user
    // asked to change one item's watch state, and if the re-fetch fails, emptying their
    // library is not an outcome they asked for or can undo. It cost the owner their whole
    // view on a failed mark-watched against a stopped server (playtest, 0.1.47).
    const held = preserve ? { items, offset, hasMore } : null;
    offset = 0;
    hasMore = true;
    // The blank stays, even when preserving: `loadMore` APPENDS, so keeping the old cards
    // here makes a SUCCESSFUL repaint show every item twice (it did — markwatched and
    // watchstate both went red). What must not survive is the EMPTY RESULT of a failure.
    items = [];
    loading = true;
    // An auto-redirect keeps the banner: the refresh action publishes its
    // aggregate AFTER both legs settle, and Svelte's effect flush may land
    // either side of that — clearing here would race the publish away
    // (lrs-1). A user-driven select still clears, as before.
    if (!keepError) setError(null);
    const ok = await loadMore(myGen);
    if (myGen === loadGen) {
      if (held && !ok) {
        // Put the view back exactly as it was. The failure is still reported — the user
        // is told — but they keep the library they were looking at.
        items = held.items;
        offset = held.offset;
        hasMore = held.hasMore;
      }
      loading = false;
    }
  }

  // Load the next page for the current level (section root or a parent's children)
  // and append it. Drives infinite scroll. Discards results if navigation moved on.
  // Returns false ONLY if the request failed. A no-op (nothing to ask for, already
  // loading, superseded) is not a failure — the caller must not read it as one.
  async function loadMore(
    myGen: number = loadGen,
    onError: ((msg: string) => void) | null = null,
  ): Promise<boolean> {
    if (loadingMore || !hasMore || myGen !== loadGen) return true;
    let failed = false;
    const request = currentListingRequest();
    if (!request) return true;
    loadingMore = true;
    try {
      const page = await fetchListingPage(request, offset, PAGE);
      if (myGen !== loadGen) return true; // navigated away while awaiting; drop these
      items = [...items, ...page];
      offset += page.length;
      hasMore = page.length >= PAGE;
      // This generation just loaded successfully, so any failure we were HOLDING
      // for it no longer describes anything: the cards are here. Settlement would
      // otherwise publish a diagnostic for a page that arrived (codex r17).
      if (suppressedFailures.length > 0) {
        suppressedFailures = suppressedFailures.filter((f) => f.gen !== myGen);
      }
    } catch (e) {
      failed = true;
      if (myGen === loadGen) {
        // The refresh action aggregates its legs' failures action-locally
        // (library-refresh-scan plan); navigation loads keep the direct publish.
        if (onError) {
          onError(String(e));
          hasMore = false; // the action's own leg: it reports this itself
        }
        // A grid-root refresh owns the banner while it runs: this load's own
        // failure must not land over the result the action is loading (r3-2),
        // and the action publishes its own legs' failures itself. It owns it for
        // its OWN root only — once the user navigates away the action's outcome
        // is discarded on the epoch mismatch, so it must not go on swallowing
        // the NEW view's errors, which would leave that view empty and silent
        // (codex r6).
        else if (
          gridActionActive &&
          refreshEpoch === navEpoch &&
          myGen <= gridActionBaseGen
        ) {
          // Silenced only because the action EXPECTS to replace this load's
          // result. It may not get that far — if its sections leg fails, its
          // content leg returns before ever claiming the grid, and then nothing
          // replaced this load: the grid is empty (or truncated) precisely
          // BECAUSE this load failed, and the user is owed that reason. Hold the
          // failure; settlement publishes it if the action never took the grid
          // (codex r16).
          suppressedFailures.push({ msg: String(e), gen: myGen });
          // And do NOT declare the library finished. We silenced this failure on
          // the promise of replacing it — but if the user opens a detail while we
          // wait on sections, the epoch moves, the content leg never claims, and
          // settlement drops the held failure as belonging to a view the user has
          // left. They press Back to a library that is truncated, cannot
          // paginate, and says nothing about why. Killing `hasMore` here is what
          // makes that silence permanent; leaving it lets a scroll retry the page
          // we never actually replaced (r8-4, declined once, OVERTURNED on
          // independent re-adjudication). If the action does claim the grid it
          // resets `hasMore` itself, so this costs the happy path nothing.
        } else {
          // The ONLY tagged write: a refresh that goes on to SUPERSEDE this
          // load owns its cards, so it must take this message down with it
          // (see refreshLibraries settlement, codex r11).
          setError(String(e), myGen);
          hasMore = false;
        }
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
    //
    // NOT after a failure. A failed page advances neither, and a SILENCED one no
    // longer clears `hasMore` either (r8-4), so on a viewport tall enough to fit
    // the cards this would re-request the same failing offset as fast as the
    // server can refuse it, for as long as the refresh runs — a request storm,
    // one held failure pushed per pass (codex r17).
    await tick();
    if (
      !failed &&
      myGen === loadGen &&
      hasMore &&
      gridEl &&
      gridEl.scrollHeight <= gridEl.clientHeight
    ) {
      return await loadMore(myGen, onError);
    }
    return !failed;
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
    // The face key still supplies the immediate route, while merged hierarchy
    // coordinates on the crumb make the backend fetch every show copy.
    const key = detailKeyOf(item);
    if (mode === "home") {
      // Drilling out of a hub: start a fresh crumb trail rooted at this item.
      active = null;
      crumbs = [{
        title: item.title,
        ratingKey: key,
        backing: item.backing,
        canonicalId: item.canonicalId,
        mediaType: item.mediaType,
      }];
    } else {
      crumbs = [...crumbs, {
        title: item.title,
        ratingKey: key,
        backing: item.backing,
        canonicalId: item.canonicalId,
        mediaType: item.mediaType,
      }];
    }
    mode = "browse";
    await resetAndLoad();
  }

  async function applySort(field: SortFieldKey, direction: SortDirection) {
    const next = composeSort(field, direction);
    if (next === sort) return;
    sort = next;
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

  async function changeSortField(event: Event) {
    const key = (event.currentTarget as HTMLSelectElement).value;
    const field = SORT_FIELDS.find((candidate) => candidate.key === key);
    if (!field) return;
    if (activeType ? !field.merged : field.showOnly && active?.sectionType !== "show") return;
    await applySort(field.key, parsedSort.direction);
  }

  async function toggleSortDirection() {
    await applySort(
      parsedSort.field.key,
      parsedSort.direction === "asc" ? "desc" : "asc",
    );
  }

  // `rerun`: refreshWatchState() re-enters the CURRENT root to pick up new watch
  // state. The user has not navigated — the visible root is identical — so it
  // must not bump `navEpoch`, or an in-flight refresh would read it as
  // navigation and silently drop its own failure: the spinner would stop with a
  // stale sidebar and no error at all (codex r9).
  async function runSearch(query: string = searchQuery, { rerun = false } = {}) {
    const q = query.trim();
    if (q.length < 2) {
      if (searchTerm) {
        // This tears the search root DOWN and replaces it. Anything the old root was
        // saying — a failed edit made in those results, its own listing diagnostic —
        // describes a view that is gone, and `setError` alone would keep every untagged
        // part (gen 0 says no LOAD owns it, not that it belongs here).
        clearViewErrors();
        navEpoch++; // tearing down the search root is navigation (see navEpoch)
        items = [];
        crumbs = [];
        active = null;
        searchTerm = "";
      }
      setError("Search needs at least 2 characters.");
      return;
    }
    if (!rerun) navEpoch++; // navigation (see navEpoch); a re-run is not (r9)
    homeGen++; // leaving home: invalidate any in-flight home/sections load
    const myGen = ++loadGen; // invalidate any in-flight load; guard our own result
    loadingMore = false;
    detailView = null;
    mode = "browse";
    active = null; // search results aren't a section, so no pagination
    personView = null;
    searchTerm = q;
    crumbs = [{ title: `Search: "${q}"`, ratingKey: null }];
    // A RE-RUN re-enters the root the user is already standing on (a watch-state repaint),
    // so a failure must not take their results away — same rule as resetAndLoad's
    // `preserve`, same reason (playtest, 0.1.47).
    const held = rerun ? items : null;
    if (!rerun) items = [];
    hasMore = false;
    loading = !rerun;
    setError(null);
    try {
      const results = await invoke<Item[]>("search", { query: q, sourceId: activeSource });
      if (myGen !== loadGen) return; // user navigated away while searching
      items = results;
    } catch (e) {
      if (myGen === loadGen) {
        if (held) items = held; // put the view back; the failure is still reported
        setError(String(e));
      }
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

  async function runPersonView(p: PersonView, { rerun = false } = {}) {
    if (!rerun) navEpoch++; // navigation (see navEpoch); a re-run is not (r9)
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
    // A RE-RUN re-enters the root the user is standing on: a failure must not empty it
    // (same rule as resetAndLoad's `preserve`).
    const held = rerun ? items : null;
    if (!rerun) items = [];
    hasMore = false; // one-shot: the backend returns the full merged list
    loading = !rerun;
    setError(null);
    try {
      const results = await invoke<Item[]>("get_person_items", { personKey: p.key, kind: p.kind });
      if (myGen !== loadGen) return; // user navigated away while loading
      items = results;
    } catch (e) {
      if (myGen === loadGen) {
        if (held) items = held; // put the view back; the failure is still reported
        setError(String(e));
      }
    } finally {
      if (myGen === loadGen) loading = false;
    }
  }

  // The browse root that issued a manual watch edit. `set_watched` can take
  // long enough for the user to navigate elsewhere, so completion may only
  // repaint when these exact coordinates still describe the current root.
  type WatchEditBrowseOrigin =
    | { kind: "listing"; epoch: number; request: ListingRequest }
    | { kind: "search"; epoch: number; query: string; sourceId: string | null }
    | { kind: "person"; epoch: number; person: PersonView };

  function watchEditBrowseOrigin(): WatchEditBrowseOrigin | null {
    if (mode !== "browse") return null;
    if (searchTerm && crumbs.length === 1) {
      return { kind: "search", epoch: navEpoch, query: searchTerm, sourceId: activeSource };
    }
    if (personView && crumbs.length === 1) {
      return { kind: "person", epoch: navEpoch, person: { ...personView } };
    }
    const request = currentListingRequest();
    return request ? { kind: "listing", epoch: navEpoch, request } : null;
  }

  function ownsWatchEditOrigin(origin: WatchEditBrowseOrigin): boolean {
    if (mode !== "browse" || navEpoch !== origin.epoch) return false;
    if (origin.kind === "listing") {
      return sameListingRequest(origin.request, currentListingRequest());
    }
    if (origin.kind === "search") {
      return (
        crumbs.length === 1 &&
        searchTerm === origin.query &&
        activeSource === origin.sourceId
      );
    }
    return (
      crumbs.length === 1 &&
      personView?.key === origin.person.key &&
      personView.kind === origin.person.kind
    );
  }

  function restoreGridScroll(top: number) {
    if (!gridEl) return;
    gridEl.scrollTop = Math.min(top, Math.max(0, gridEl.scrollHeight - gridEl.clientHeight));
  }

  // Revalidate a paginated listing without ever unmounting its grid. Starting
  // at zero is an authority boundary, not just a pagination choice: merged
  // continuation pages reuse the backend's existing immutable snapshot.
  async function reloadBrowseAfterWatchEdit(
    origin: Extract<WatchEditBrowseOrigin, { kind: "listing" }>,
  ): Promise<void> {
    if (!ownsWatchEditOrigin(origin)) return;
    const targetDepth = Math.max(offset, items.length);
    if (targetDepth === 0) return;
    const savedScrollTop = gridEl?.scrollTop ?? 0;
    homeGen++; // invalidate any hidden Home load before clearing its cached rows
    const myGen = ++loadGen;
    loadingMore = true; // stale pagination cannot append while the buffer is built
    setError(null);
    const buffered: Item[] = [];
    let start = 0;
    let refreshedHasMore = true;
    try {
      while (buffered.length < targetDepth && refreshedHasMore) {
        const page = await fetchListingPage(origin.request, start, PAGE);
        if (myGen !== loadGen || !ownsWatchEditOrigin(origin)) return;
        buffered.push(...page);
        start += page.length;
        refreshedHasMore = page.length >= PAGE;
        if (page.length === 0) break;
      }
      if (myGen !== loadGen || !ownsWatchEditOrigin(origin)) return;
      items = buffered;
      offset = buffered.length;
      hasMore = refreshedHasMore;
      await tick();
      if (myGen !== loadGen || !ownsWatchEditOrigin(origin)) return;
      restoreGridScroll(savedScrollTop);
    } catch (e) {
      // The server edit already succeeded. This is a listing-revalidation
      // failure, so retain the confirmed local card and the entire old grid.
      if (myGen === loadGen && ownsWatchEditOrigin(origin)) {
        setError(String(e), myGen);
      }
    } finally {
      if (myGen === loadGen) loadingMore = false;
    }
  }

  async function rerunQueryAfterWatchEdit(
    origin: Exclude<WatchEditBrowseOrigin, { kind: "listing" }>,
  ): Promise<void> {
    if (!ownsWatchEditOrigin(origin)) return;
    const savedScrollTop = gridEl?.scrollTop ?? 0;
    const rerun =
      origin.kind === "search"
        ? runSearch(origin.query, { rerun: true })
        : runPersonView(origin.person, { rerun: true });
    // Both rerun functions claim their generation synchronously before their
    // first await, so this is the exact result whose DOM update we may restore.
    const myGen = loadGen;
    await rerun;
    if (myGen !== loadGen || !ownsWatchEditOrigin(origin)) return;
    await tick();
    if (myGen !== loadGen || !ownsWatchEditOrigin(origin)) return;
    restoreGridScroll(savedScrollTop);
  }

  function refreshAfterWatchEdit(origin: WatchEditBrowseOrigin | null): Promise<unknown> {
    if (mode === "home" || mode === "playlists") return refreshWatchState();
    // Home is hidden under every browse root. Never let pre-edit hubs become
    // authoritative when the user returns there.
    hubs = [];
    if (!origin || !ownsWatchEditOrigin(origin)) return Promise.resolve();
    return origin.kind === "listing"
      ? reloadBrowseAfterWatchEdit(origin)
      : rerunQueryAfterWatchEdit(origin);
  }

  function hasResume(item: Pick<Item, "viewOffsetMs">): boolean {
    return (item.viewOffsetMs ?? 0) > 0;
  }

  // `quality`: a one-off choice from the title's own menu. Null means "use the
  // setting"; the backend never stores whatever is passed here.
  async function play(
    item: Item,
    intent: PlayIntent = "resume",
    explicitSourceId: string | null = null,
    quality: string | null = null,
  ) {
    // A user choice always starts a new run. The backend's expected-session
    // check independently prevents an already-awaited continuation from
    // replacing this play, including plays launched inside playlist views.
    invalidateContinuationRun();
    try {
      const seriesContinuation =
        continuePlayingMode === "only-tv" && item.mediaType === "episode";
      const result = await invoke<PlayCommandResult>("play_item", {
        item,
        startFromBeginning: intent === "beginning",
        expectedSession: null,
        seriesContinuation,
        explicitSourceId,
        quality,
      });
      await handlePlayCommandResult(result, (sessionId) => {
        if (seriesContinuation) {
          continuationSeen.add(item.ratingKey);
          continuationSession = sessionId;
        }
      });
    } catch (e) {
      // A Play started from an open detail is reported ON that detail, which survives a
      // search teardown underneath it — so the view's clear is not its owner (slice 4).
      // From the grid, the view IS the surface, and its banner is right.
      const said = `Couldn't play “${item.title}” — ${String(e)}`;
      if (detailView) detailStatus = said;
      else setError(said);
      // A failure may mean mpv went missing — re-check so the install prompt shows.
      invoke<MpvInfo>("check_mpv").then((m) => (mpvInfo = m)).catch(() => {});
    }
  }

  // Play a merged title from a specific backing source. The backend persists
  // this title override in automatic modes and keeps it one-shot in Ask mode.
  async function playFrom(
    item: Item,
    b: { sourceId: string; ratingKey: string },
    intent: PlayIntent = "resume",
  ) {
    closeMenu();
    await play(
      { ...item, ratingKey: b.ratingKey, sourceId: b.sourceId },
      intent,
      b.sourceId,
    );
  }

  // The menu's Play entry must take `mi` as an argument BEFORE closing the
  // menu: `mi` is a template {@const} over `menu.item`, so an inline
  // `closeMenu(); play(mi)` nulls `menu` first and the `mi` read throws.
  function playFromCtx(item: Item, intent: PlayIntent) {
    closeMenu();
    play(item, intent);
  }

  // Right-click context menu for per-item actions.
  let menu = $state<{ x: number; y: number; item: Item; hero: boolean } | null>(null);
  let addMenuOpen = $state(false);
  let versionMenuOpen = $state(false);
  let addMenuLoading = $state(false);
  let addMenuPlaylists = $state<PlaylistSummary[]>([]);
  let addMenuStatus = $state<{ text: string; failed: boolean } | null>(null);
  let addMenuAttempt = 0;

  function openMenu(e: MouseEvent, item: Item, hero = false) {
    e.preventDefault();
    sectionMenu = null; // only one context menu at a time (codex code review r1, finding 4)
    addMenuAttempt++;
    addMenuOpen = false;
    versionMenuOpen = false;
    addMenuLoading = false;
    addMenuPlaylists = [];
    addMenuStatus = null;
    closeQualityMenu();
    // The playlist submenu can be much taller than the old fixed action list.
    menu = {
      x: Math.max(8, Math.min(e.clientX, window.innerWidth - 290)),
      y: Math.max(8, Math.min(e.clientY, window.innerHeight - 420)),
      item,
      hero,
    };
  }

  function closeMenu() {
    addMenuAttempt++;
    addMenuOpen = false;
    versionMenuOpen = false;
    addMenuLoading = false;
    addMenuStatus = null;
    closeQualityMenu();
    menu = null;
  }

  function closeAddMenu() {
    addMenuAttempt++;
    addMenuOpen = false;
    addMenuLoading = false;
    addMenuStatus = null;
  }

  async function toggleAddMenu() {
    if (addMenuOpen) {
      closeAddMenu();
      return;
    }
    versionMenuOpen = false;
    addMenuOpen = true;
    addMenuLoading = true;
    addMenuStatus = null;
    const attempt = ++addMenuAttempt;
    try {
      const loaded = await invoke<PlaylistSummary[]>("playlist_list");
      if (attempt === addMenuAttempt && menu) addMenuPlaylists = loaded;
    } catch (error) {
      if (attempt === addMenuAttempt && menu) {
        addMenuStatus = { text: String(error), failed: true };
      }
    } finally {
      if (attempt === addMenuAttempt) addMenuLoading = false;
    }
  }

  function toggleVersionMenu() {
    if (versionMenuOpen) {
      versionMenuOpen = false;
      return;
    }
    closeAddMenu();
    versionMenuOpen = true;
  }

  // The per-title one-off quality choice. Nested under version when a title has
  // several copies, offered directly when it has one; never both, per the
  // 2026-07-25 ruling. Nothing here is persisted — the choice applies to the
  // play it starts and the next play reads the setting again.
  type QualityTier = { id: string; label: string; bitrateKbps: number };
  type QualityOptions = {
    canDirectPlay: boolean;
    sourceBitrateKbps: number;
    sourceHeight: number;
    tiers: QualityTier[];
  };
  // Keyed by the backing it belongs to, so a multi-version title can have one
  // copy's list open without discarding another's.
  let qualityMenuFor = $state<string | null>(null);
  let qualityLoading = $state(false);
  let qualityOptions = $state<QualityOptions | null>(null);
  let qualityError = $state<string | null>(null);
  let qualityAttempt = 0;

  function closeQualityMenu() {
    qualityAttempt++;
    qualityMenuFor = null;
    qualityLoading = false;
    qualityOptions = null;
    qualityError = null;
  }

  const backingKey = (b: { sourceId: string; ratingKey: string }) =>
    b.sourceId + ":" + b.ratingKey;

  // Resolved when the submenu OPENS, never when the context menu does: for Plex
  // this is a decision round trip per version, and paying it on every
  // right-click would make the menu feel slow for everyone who never converts.
  async function toggleQualityMenu(item: Item, b: { sourceId: string; ratingKey: string }) {
    const key = backingKey(b);
    if (qualityMenuFor === key) {
      closeQualityMenu();
      return;
    }
    closeQualityMenu();
    qualityMenuFor = key;
    qualityLoading = true;
    const attempt = ++qualityAttempt;
    try {
      const loaded = await invoke<QualityOptions>("quality_options", {
        itemKey: b.ratingKey,
        // Pins the answer to the version that will actually play rather than
        // the source's default one (or-6). The backend ignores it when the
        // policy's winner is a different copy than this row.
        item,
        versionId: null,
      });
      if (attempt === qualityAttempt && menu) qualityOptions = loaded;
    } catch (error) {
      if (attempt === qualityAttempt && menu) qualityError = String(error);
    } finally {
      if (attempt === qualityAttempt) qualityLoading = false;
    }
  }

  function qualityLabel(tier: QualityTier) {
    // Two tiers share the label "Convert to 1080p HD", so the bitrate is not
    // decoration — it is the only thing telling them apart.
    const rate =
      tier.bitrateKbps >= 1000
        ? `${tier.bitrateKbps / 1000} Mbps`
        : `${tier.bitrateKbps} kbps`;
    return `${tier.label} — ${rate}`;
  }

  // One play at one quality. `quality` reaches play_item for this launch only.
  async function playAtQuality(
    item: Item,
    b: { sourceId: string; ratingKey: string },
    quality: string,
  ) {
    closeMenu();
    await play(
      { ...item, ratingKey: b.ratingKey, sourceId: b.sourceId },
      "resume",
      b.sourceId,
      quality,
    );
  }

  async function addToPlaylist(saved: PlaylistSummary) {
    const item = menu?.item;
    if (!item || addMenuLoading) return;
    const attempt = ++addMenuAttempt;
    addMenuLoading = true;
    addMenuStatus = null;
    try {
      await invoke("playlist_add_items", { id: saved.id, items: [item] });
      if (attempt === addMenuAttempt && menu) {
        addMenuStatus = { text: `Added to “${saved.name}”.`, failed: false };
        playlistVersion++;
      }
    } catch (error) {
      if (attempt === addMenuAttempt && menu) {
        addMenuStatus = {
          text: `Couldn't add “${item.title}” — ${String(error)}`,
          failed: true,
        };
      }
    } finally {
      if (attempt === addMenuAttempt) addMenuLoading = false;
    }
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
  // A scan's status lives on its OWN surface — never on the view's error banner.
  // Sharing one slot cost three separate defects (a scan erasing a listing's
  // failure; a scan's failure destroying that diagnostic permanently; a scan
  // completing after its source was removed and republishing over Welcome —
  // codex r14, r15). The view's banner explains the view; a scan explains itself.
  // `failed` picks the surface: an alert that stays, or a neutral notice that
  // auto-clears.
  // A watch-state edit reports on ITS OWN line, never the view's banner (owner ruling
  // 2026-07-14, "its own line"; plan .agents/plans/per-surface-status.md). It is an action
  // the user took, not a fact about the grid — and the two writers sharing one surface is
  // what produced EIGHT consecutive rounds of the same defect, each fix opening the next
  // door (library-refresh-scan log, r17-r24). A complete success needs no
  // announcement, because the card's ✓ is the acknowledgement. A partial
  // multi-source success uses the same action-owned line as a warning.
  //
  // Like the scan's, it publishes regardless of which view is on screen — an action's
  // outcome is not view-scoped. It clears after eight seconds or immediately when the
  // next edit starts. `editAttempt` owns both publication and expiry: a newer edit
  // supersedes an older one's outcome, and the last-source teardown abandons one in
  // flight so it cannot land on Welcome about a library that no longer exists (the
  // r14/r15/r16-3 rule).
  const EDIT_STATUS_TTL_MS = 8000;
  let editStatus = $state<{ text: string; failed: boolean } | null>(null);
  let editAttempt = 0;
  let editStatusTimer: ReturnType<typeof setTimeout> | null = null;

  function clearEditStatus() {
    editStatus = null;
    if (editStatusTimer !== null) {
      clearTimeout(editStatusTimer);
      editStatusTimer = null;
    }
  }

  function publishEditStatus(attempt: number, text: string, failed: boolean) {
    if (attempt !== editAttempt) return;
    clearEditStatus();
    editStatus = { text, failed };
    editStatusTimer = setTimeout(() => {
      // Cancellation cannot retract a callback that is already queued. The
      // captured attempt is the final authority: an older expiry may never
      // clear a newer edit's failure.
      if (attempt === editAttempt) clearEditStatus();
    }, EDIT_STATUS_TTL_MS);
  }

  function publishEditFailure(attempt: number, text: string) {
    publishEditStatus(attempt, text, true);
  }

  onDestroy(() => {
    // Prevent an edit still awaiting the backend from publishing and arming a
    // fresh timer after the component has already torn down.
    editAttempt++;
    clearEditStatus();
  });

  // The mpv setup bar's own status (per-surface-status slice 3). The bar is global and
  // stays mounted until mpv is resolved — it does not belong to any view — so a view's
  // clear must not take its failure away. It did: a one-character search deleted the
  // reason an install had failed while the bar, and its Retry button, stayed on screen
  // (codex r24).
  let mpvStatus = $state<string | null>(null);

  // The open detail page's own status (per-surface-status slice 4). It layers OVER the
  // grid and survives things that replace what is underneath it — a search teardown, for
  // one — so a Play failure raised on it is not the view's to delete. It was: tearing down
  // a search root deleted the reason playback had failed while the detail, and the Play
  // button it was about, stayed on screen (codex r24). Cleared when the detail closes or
  // is replaced.
  let detailStatus = $state<string | null>(null);

  let scanStatus = $state<{ text: string; failed: boolean } | null>(null);
  let scanning = $state<Record<string, boolean>>({}); // menu-entry feedback only
  const scanGens: Record<string, number> = {};
  let scanAttempt = 0; // global publication ownership
  let scanStatusOwner: number | null = null; // owning attempt of the visible status
  let scanStatusTimer: ReturnType<typeof setTimeout> | null = null;
  onDestroy(() => {
    if (scanStatusTimer) clearTimeout(scanStatusTimer);
  });

  // Drop any scan status and stop an in-flight scan from publishing one. Used by
  // the next scan, and by the last-source teardown — where a scan still in flight
  // would otherwise land its outcome on the Welcome screen, about a library that
  // no longer exists (codex r15).
  function clearScanStatus() {
    scanStatus = null;
    scanStatusOwner = null;
    if (scanStatusTimer) {
      clearTimeout(scanStatusTimer);
      scanStatusTimer = null;
    }
  }

  async function scanSection(s: Section) {
    closeSectionMenu();
    const gen = (scanGens[s.key] = (scanGens[s.key] ?? 0) + 1);
    const attempt = ++scanAttempt;
    scanning[s.key] = true;
    // The action owns its own status and NOTHING else — the view's banner is not
    // a scan's to touch.
    clearScanStatus();
    try {
      // Hand back the provenance issued WITH this key: `s` is the section object
      // from the list this menu was opened on, which may no longer be the list
      // on screen (codex r11).
      await invoke("scan_section", { sectionKey: s.key, provenance: s.provenance ?? null });
      if (scanAttempt !== attempt) return; // superseded — stale outcome
      // No auto-refresh afterward: the scan runs asynchronously server-side
      // and completion is unknowable without polling (non-goal). The slice-1
      // refresh button is the companion action once the scan has landed.
      scanStatus = { text: `Scan started — ${s.title}`, failed: false };
      scanStatusOwner = attempt;
      scanStatusTimer = setTimeout(() => {
        // Only the owning attempt may clear — a timer armed by an earlier
        // success must not wipe a newer attempt's status.
        if (scanStatusOwner === attempt) clearScanStatus();
      }, 4000);
    } catch (e) {
      if (scanAttempt !== attempt) return;
      // Stays until the next scan: unlike the acknowledgement, a failure is not
      // something to tidy away on a timer.
      scanStatus = { text: String(e), failed: true };
      scanStatusOwner = attempt;
    } finally {
      if (scanGens[s.key] === gen) scanning[s.key] = false;
    }
  }

  async function setWatched(item: Item, played: boolean) {
    closeMenu();
    const browseOrigin = watchEditBrowseOrigin();
    // This attempt owns the edit line from here on; a newer edit supersedes it.
    const attempt = ++editAttempt;
    clearEditStatus(); // this attempt supersedes whatever the last one said
    try {
      const result = await invoke<WatchStateMutation>("set_watched", { item, played });
      // At least one source accepted the edit. Reflect that confirmed title
      // immediately (deep-reactive $state); the authoritative buffered repaint
      // below may still expose an offline backing's older state.
      item.played = played;
      item.viewOffsetMs = 0;
      if (result.failedSources > 0) {
        const total = result.succeededSources + result.failedSources;
        const names = result.failedSourceNames.join(", ");
        publishEditStatus(
          attempt,
          `Marked “${item.title}” ${played ? "watched" : "unwatched"} on ${result.succeededSources} of ${total} sources. Couldn't update: ${names}.`,
          false,
        );
      }
      // Curate Home and revalidate the originating browse root without tearing
      // down its loaded pages or scroll container.
      refreshAfterWatchEdit(browseOrigin);
    } catch (e) {
      // The backend curates BEFORE the server call and rolls back on failure. A Home
      // load inside that window may have rendered the transient state, so heal Home;
      // never reload a listing that the failed edit did not change.
      await repairFailedWatchEdit();
      // Report on the edit's OWN line, and report it wherever the user now is: they asked
      // for this change, it did not happen, and that is true no matter which view they are
      // looking at. No view-currency gate, banner ownership algebra, or retract — the
      // machinery all of that needed is exactly what having a second writer on the view's
      // banner cost.
      // A newer edit may supersede this one; otherwise its own expiry owns the clear.
      publishEditFailure(
        attempt,
        `Couldn't mark “${item.title}” ${played ? "watched" : "unwatched"} — ${String(e)}`,
      );
    }
  }

  // Explicit hero curation: tombstone + recents drop (backend), then the
  // standard re-fetch. No watched-state change.
  async function removeFromContinue(item: Item) {
    closeMenu();
    const attempt = ++editAttempt;
    clearEditStatus();
    try {
      await invoke("remove_from_continue", { ratingKey: item.ratingKey });
      refreshWatchState();
    } catch (e) {
      // A watch-state edit, so the same surface (see editStatus).
      publishEditFailure(
        attempt,
        `Couldn't remove “${item.title}” from Continue Watching — ${String(e)}`,
      );
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
    detailStatus = null; // its surface is gone; so is what it was saying
    // The grid comes back. If it still has pages but cannot SCROLL — a tall or
    // hi-dpi viewport where the cards already fit — then nothing will ever ask for
    // them: `onScroll` cannot fire on a grid that does not scroll, and the
    // auto-fill tail stops after a failed page (it must, or it storms the server).
    // So re-establish the fill-the-viewport invariant here, once, on the user's own
    // action. Without this, the very case r8-4 fixed comes back through another
    // door: a page silenced by a refresh, abandoned when the user opened a detail,
    // and a library left silently truncated on Back (codex r18).
    tick().then(() => {
      if (
        mode === "browse" &&
        hasMore &&
        !loadingMore &&
        gridEl &&
        gridEl.scrollHeight <= gridEl.clientHeight
      ) {
        loadMore();
      }
    });
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
    detailStatus = null; // a new detail supersedes what the last one was saying
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

<div class="app" tabindex="-1" bind:this={normalRoot}>
  {#if durableStatus === null}
    <main class="durable-block" aria-busy="true">
      <p class="eyebrow">Checking files</p>
      <h1>Opening Vela safely…</h1>
    </main>
  {:else if !durableReady(durableStatus)}
    {@const fault = durableFault(durableStatus)}
    <main
      class="durable-block"
      role="alert"
      aria-busy={durableBusy}
      aria-labelledby="durable-fault-heading"
    >
      <p class="eyebrow">{fault.file === "settings" ? "Settings problem" : "Connection problem"}</p>
      <h1 id="durable-fault-heading" tabindex="-1" bind:this={durableHeading}>
        Vela could not safely read your {fault.file === "settings"
          ? "settings"
          : "server connections"}.
      </h1>
      {#if fault.value.status === "recoverable_invalid"}
        {#if fault.file === "settings" && fault.value.layout === "post_split"}
          <p>
            The settings file may be damaged or may have been tampered with. Nothing from it was
            loaded.
          </p>
          <p>
            Choose a dated valid version below, rename the damaged file and create new settings,
            or exit Vela and repair it yourself. Your server connections are stored separately
            and will not be changed.
          </p>
        {:else if fault.file === "settings"}
          <p>
            This older settings file is damaged and may also contain your server connections.
            Vela loaded nothing from it and will not extract or guess any connection.
          </p>
          <p>
            Rename and create new settings preserves the whole old file under a private new name,
            creates fresh settings, and then requires you to reconnect your servers. Exit Vela if
            you want to repair the file yourself instead.
          </p>
        {:else}
          <p>
            The server-connections file may be damaged or may have been tampered with. No
            connection or token was loaded.
          </p>
          <p>
            Choose a dated valid version below to restore those connections. Rename damaged
            connections and reconnect instead preserves the whole file, creates an empty valid
            connections file, and opens the normal server-connection flow. Your settings, recents,
            and playlists will not be reset.
          </p>
        {/if}
      {:else if fault.value.status === "migration_blocked"}
        <p>
          Vela could not finish a protected settings or connection update safely. Nothing from
          the affected files was loaded.
        </p>
        <p>
          Try again to continue only from the exact recorded state. Vela will not guess, merge,
          or overwrite a file that changed.
        </p>
      {:else}
        <p>
          Vela could not safely inspect the file. Check its permissions and location; the
          original was left unchanged.
        </p>
      {/if}
      {#if durableRetryError}
        <p class="durable-error" role="alert">{durableRetryError}</p>
      {/if}
      {#if durableBackupFileName}
        <p role="status">
          The complete damaged file was preserved as <code>{durableBackupFileName}</code>.
        </p>
      {/if}
      <div class="durable-actions">
        {#if fault.value.status === "recoverable_invalid" && fault.value.canRecover}
          {#each fault.value.rollbackVersions as version (version.id)}
            <button
              data-rollback-version={version.id}
              disabled={durableBusy}
              onclick={() => rollbackInvalidFile(fault.file, version)}
            >
              {rollingBackVersionId === version.id
                ? "Restoring version…"
                : `Restore ${formatRollbackDate(version.createdAtUnixMs)}`}
            </button>
          {/each}
          <button
            class="primary"
            disabled={durableBusy}
            onclick={() => recoverInvalidFile(fault.file)}
          >
            {#if recoveringInvalidFile}
              Preserving file…
            {:else if fault.file === "settings"}
              Rename and create new settings
            {:else}
              Rename damaged connections and reconnect
            {/if}
          </button>
        {:else}
          <button class="primary" disabled={durableBusy} onclick={retryDurableState}>
            {retryingDurableState ? "Trying again…" : "Try again"}
          </button>
        {/if}
        <button disabled={durableBusy} onclick={exitVela}>Exit Vela</button>
      </div>
    </main>
  {:else}
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
    <button class="gear" title="Settings" aria-label="Settings" onclick={() => (showSettings = true)}>
      <Icon name="settings" />
    </button>
  </header>

  {#if durableRecoveryNotice}
    <div class="notice" role="status">{durableRecoveryNotice}</div>
  {/if}

  {#if error}
    <div class="error" role="alert">{friendlyError(error)}</div>
  {/if}

  {#if editStatus}
    <!-- The EDIT's own surface, never the view's error banner above (owner ruling
         2026-07-14). It can sit alongside an independently existing view failure rather
         than fighting it; a failed edit does not manufacture a listing request or view
         failure of its own. It stays readable for eight seconds, unless the next edit
         supersedes it first. -->
    {#if editStatus.failed}
      <div class="scanerror" role="alert">{friendlyError(editStatus.text)}</div>
    {:else}
      <div class="editwarning" role="status">{editStatus.text}</div>
    {/if}
  {/if}

  {#if scanStatus}
    <!-- The scan's OWN surface, never the view's error banner above (codex r15).
         Success is a transient acknowledgement — neutral, auto-clears; scan
         COMPLETION is unknowable without polling (non-goal). Failure is an alert
         that stays until the next scan, and it sits ALONGSIDE any listing failure
         rather than replacing it: both are true, and the listing's is the one that
         explains the empty grid. -->
    {#if scanStatus.failed}
      <div class="scanerror" role="alert">{friendlyError(scanStatus.text)}</div>
    {:else}
      <div class="notice" role="status">{scanStatus.text}</div>
    {/if}
  {/if}

  {#if showSettings}
    <Settings
      onClose={() => (showSettings = false)}
      onChanged={onSourcesChanged}
      onLinkPlex={beginLink}
      onMpvChanged={(m) => (mpvInfo = m)}
      onContinuePlayingChanged={changeContinuePlayingMode}
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
      {#if mpvStatus}
        <!-- The BAR's own surface (slice 3), never the view's error banner. -->
        <div class="mpverror" role="alert">{friendlyError(mpvStatus)}</div>
      {/if}
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
    {@const label = `${parts.join(" — ")}${pct !== null ? ` — ${pct}% watched` : item.played === true ? " — watched" : ""}`}
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
        <div class="noart" aria-hidden="true">{item.title}</div>
        {#if art}
          {@const src = posterSrc(art)}
          <img
            class="image-reveal image-cover"
            use:imageReveal={src}
            src={src}
            alt=""
            loading="lazy"
            decoding="async"
          />
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
              aria-label={d === 0 ? `${hasResume(it) ? "Resume" : "Play"} ${it.grandparentTitle ?? it.title}` : `Show ${it.grandparentTitle ?? it.title}`}
            >
              <div class="art">
                <div class="noart" aria-hidden="true">{it.grandparentTitle ?? it.title}</div>
                {#if art}
                  {@const src = posterSrc(art)}
                  <img
                    class="image-reveal image-cover"
                    use:imageReveal={src}
                    src={src}
                    alt=""
                    decoding="async"
                  />
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
    {#if !pin}
      <!-- Library navigation lives in a left sidebar (Infuse reference):
           Home, the Library entries for the current scope, and the source
           scopes — freeing the vertical space the top nav used to take. -->
      <aside class="sidebar">
        <nav class="sidenav" aria-label="Library">
          <button class="sideitem" class:active={mode === "home"} onclick={goHome}>Home</button>
          <button class="sideitem" class:active={mode === "playlists" && !selectedServerPlaylist} onclick={openPlaylists}>Playlists</button>
          {#if authenticated}
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
                disabled={refreshing || mode === "playlists"}
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
            {#if serverPlaylistGroups.length > 0}
              <div class="sidegroup">Server Playlists</div>
              {#each serverPlaylistGroups as group (group.sourceId)}
                <div class="serverplaylistgroup" data-source-id={group.sourceId} data-playlist-state={group.available ? "available" : "unavailable"}>
                  <div class="serversourcename">
                    <span>{group.sourceName}</span>
                    {#if group.sourceKind === "emby"}<small>Experimental</small>{/if}
                  </div>
                  {#if !group.available}
                    <span class="serverplayliststate">Unavailable</span>
                  {:else if group.playlists.length === 0}
                    <span class="serverplayliststate">No video playlists</span>
                  {:else}
                    {#each group.playlists as playlist (playlist.key)}
                      <button
                        class="sideitem serverplaylistitem"
                        class:active={selectedServerPlaylist?.key === playlist.key}
                        aria-label={`Open ${playlist.title} from ${group.sourceName}`}
                        onclick={() => openServerPlaylist(playlist)}
                      >
                        {playlist.title}
                      </button>
                    {/each}
                  {/if}
                </div>
              {/each}
            {/if}
            {#if sources.length > 1}
              <div class="sidegroup">Sources</div>
              <button class="sideitem" class:active={mode !== "playlists" && activeSource === null} onclick={() => selectSource(null)}>All</button>
              {#each sources as src (src.id)}
                <button class="sideitem" class:active={mode !== "playlists" && activeSource === src.id} onclick={() => selectSource(src.id)}>{src.name}</button>
              {/each}
            {/if}
          {/if}
        </nav>
      </aside>
    {/if}
    <div class="content">
  {#if pin}
    <div class="link">
      {#if plexServerChoices.length > 0}
        <h2>Choose a Plex server</h2>
        <p class="muted">This account has several reachable servers. Add one now; you can link the account again for another.</p>
        <div class="plex-server-choices">
          {#each plexServerChoices as server (server.machineIdentifier)}
            <button
              class="primary plex-server-choice"
              disabled={selectingPlexServer}
              onclick={() => selectPlexServer(server.machineIdentifier)}
            >
              {server.name}
            </button>
          {/each}
        </div>
      {:else}
        <h2>Link this device</h2>
        <p class="muted">Scan with your phone, or open Plex to authorize.</p>
        {#if pin.qrSvg}
          <button class="qr" onclick={() => openExternal(pin!.authUrl)} title="Open Plex to authorize">
            <img src={pin.qrSvg} alt="Plex device-link QR code" decoding="async" />
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
      {/if}
    </div>
  {:else if mode === "playlists" && selectedServerPlaylist}
    <ServerPlaylistView
      playlist={selectedServerPlaylist}
      refreshVersion={serverPlaylistVersion}
      {posterSrc}
      onBack={openPlaylists}
      onManualPlay={invalidateContinuationRun}
      onPlaybackResult={(result) => void handlePlayCommandResult(result)}
    />
  {:else if mode === "playlists"}
    <PlaylistsView
      sourceVersion={playlistVersion}
      {posterSrc}
      onManualPlay={invalidateContinuationRun}
      onPlaybackResult={(result) => void handlePlayCommandResult(result)}
    />
  {:else if !authenticated}
    <div class="empty-center">
      <EmptyState
        icon="film"
        heading="Welcome to Vela"
        hint="Connect Plex, Jellyfin, or Emby to start browsing your library in HDR."
      >
        <button class="primary" onclick={() => (showSettings = true)}>Add a source</button>
      </EmptyState>
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
    {#if detailStatus}
      <!-- The DETAIL's own surface (slice 4), never the view's error banner underneath. -->
      <div class="detailerror" role="alert">{friendlyError(detailStatus)}</div>
    {/if}
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
          onShow={(show) => open(show)}
          onSeason={(key, seed, sel) => {
            navEpoch++; // swapping the open detail surface is navigation
            detailStatus = null; // ...and it replaces the surface, so its status goes too
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
           connected source with an unfinished play must still show Continue
           Watching (2026-07-04 hero decision). -->
      {#if !error}
        <div class="empty-center">
          {#if sections.length === 0}
            <EmptyState
              icon="film"
              heading="No libraries found"
              hint="Check the connected server, then use Refresh libraries."
            />
          {:else}
            <EmptyState
              icon="film"
              heading="No titles on Home yet"
              hint="Choose a library from the sidebar to start browsing."
            />
          {/if}
        </div>
      {/if}
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
        <div class="sort-controls">
          <select
            class="sort-field"
            aria-label="Sort by"
            value={parsedSort.field.key}
            onchange={changeSortField}
          >
            <!-- Merged type view: DTO-sortable fields only. Section view:
                 hide the show-only field outside show sections. -->
            {#each activeType
              ? SORT_FIELDS.filter((field) => field.merged)
              : SORT_FIELDS.filter(
                  (field) => !field.showOnly || active?.sectionType === "show",
                ) as field (field.key)}
              <option value={field.key}>{field.label}</option>
            {/each}
          </select>
          <button
            type="button"
            class="sort-direction"
            aria-label={sortDirectionLabel}
            title={sortDirectionLabel}
            onclick={toggleSortDirection}
          >
            {parsedSort.direction === "asc" ? "↑" : "↓"}
          </button>
        </div>
      {/if}
    </div>
    {#if items.length === 0}
      {#if !error}
        <div class="empty-center">
          {#if searchTerm}
            <EmptyState
              icon="film"
              heading={`No matches for “${searchTerm}”`}
              hint="Check the spelling or try a broader search."
              announce
            />
          {:else if personView}
            <EmptyState
              icon="film"
              heading={`No titles found for ${personView.name}`}
              hint="Go back to keep browsing."
            />
          {:else}
            <EmptyState
              icon="film"
              heading="No titles in this view"
              hint="Go back, refresh libraries, or choose another library."
            />
          {/if}
        </div>
      {/if}
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
  {/if}
</div>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") {
      // Menus first (topmost surfaces), then the detail —
      // Escape with the scan menu open must not close a detail underneath it
      // (codex code review r1, finding 4).
      if (sourceChoiceRequest) {
        e.preventDefault();
        cancelSourceChoice();
      }
      else if (menu && qualityMenuFor) closeQualityMenu();
      else if (menu && versionMenuOpen) versionMenuOpen = false;
      else if (menu && addMenuOpen) closeAddMenu();
      else if (menu) closeMenu();
      else if (sectionMenu) closeSectionMenu();
      else if (detailView) closeDetail();
    }
  }}
/>

{#if sourceChoiceRequest}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="sourcechoicebackdrop"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) cancelSourceChoice();
    }}
  >
    <div
      class="sourcechoicedialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="source-choice-title"
      aria-describedby="source-choice-help"
      tabindex="-1"
      bind:this={sourceChoiceDialog}
      onkeydown={handleSourceChoiceDialogKeydown}
    >
      <p class="eyebrow">Ask Every Time</p>
      <h2 id="source-choice-title">Choose a source for “{sourceChoiceRequest.title}”</h2>
      <p id="source-choice-help" class="muted">
        Vela will use the best available version on the source you choose.
      </p>
      <div class="sourcechoices">
        {#each sourceChoiceRequest.choices as choice (choice.sourceId)}
          <button
            class="choice"
            disabled={sourceChoiceBusy}
            onclick={() => choosePlaybackSource(choice.sourceId)}
          >
            <span class="choicename">{choice.sourceName}</span>
            <span class="choicefacts">
              {localityLabel(choice.locality)} · {choice.qualityLabel}
            </span>
          </button>
        {/each}
      </div>
      <div class="sourcechoiceactions">
        <button disabled={sourceChoiceBusy} onclick={cancelSourceChoice}>Cancel</button>
      </div>
    </div>
  </div>
{/if}

{#if menu}
  {@const mi = menu.item}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="menubackdrop" role="presentation" onclick={closeMenu} oncontextmenu={(e) => { e.preventDefault(); closeMenu(); }}></div>
  {@const inProgress = (mi.viewOffsetMs ?? 0) > 0}
  {@const fullyWatched = mi.played === true && !inProgress}
  <div class="ctxmenu" style="left:{menu.x}px; top:{menu.y}px;" role="menu">
    {#if inProgress}
      <button role="menuitem" onclick={() => playFromCtx(mi, "resume")}>Resume</button>
      <button role="menuitem" onclick={() => playFromCtx(mi, "beginning")}>Play from Beginning</button>
    {:else}
      <button role="menuitem" onclick={() => playFromCtx(mi, "resume")}>Play</button>
    {/if}
    {#if mi.mediaType !== "show"}
      <!-- The info path for the Continue Watching flow, where click plays;
           shows get no entry — their info surface is the seasons drill. -->
      <button role="menuitem" onclick={() => openInfo(mi)}>Info</button>
    {/if}
    {#if mi.played != null && !fullyWatched}
      <button role="menuitem" onclick={() => setWatched(mi, true)}>Mark watched</button>
    {/if}
    {#if mi.played != null && (mi.played === true || inProgress)}
      <button role="menuitem" onclick={() => setWatched(mi, false)}>Mark unwatched</button>
    {/if}
    {#if menu.hero}
      <button role="menuitem" onclick={() => removeFromContinue(mi)}>Remove from Continue Watching</button>
    {/if}
    {#if mi.mediaType !== "show" && mi.mediaType !== "season"}
      <button role="menuitem" aria-expanded={addMenuOpen} onclick={toggleAddMenu}>Add to Playlist <Icon name="chevron" size={13} /></button>
      {#if addMenuOpen}
        <div class="addsubmenu" role="group" aria-label="Choose a playlist">
          {#if addMenuStatus}
            <div class:addfailure={addMenuStatus.failed} class="addstatus" role={addMenuStatus.failed ? "alert" : "status"}>
              {addMenuStatus.failed ? friendlyError(addMenuStatus.text) : addMenuStatus.text}
            </div>
          {/if}
          {#if addMenuLoading && addMenuPlaylists.length === 0}
            <div class="addempty" role="status">Loading playlists…</div>
          {:else if addMenuPlaylists.length === 0}
            <div class="addempty">No playlists yet — create one in Playlists.</div>
          {:else}
            {#each addMenuPlaylists as saved (saved.id)}
              <button role="menuitem" disabled={addMenuLoading} onclick={() => addToPlaylist(saved)}>
                {saved.name} <span>{saved.itemCount}</span>
              </button>
            {/each}
          {/if}
        </div>
      {/if}
    {/if}
    {#if (mi.backing?.length ?? 0) > 1 && mi.canonicalId}
      <button role="menuitem" aria-expanded={versionMenuOpen} onclick={toggleVersionMenu}>Play Version <Icon name="chevron" size={13} /></button>
      {#if versionMenuOpen}
        <!-- A deliberate source choice persists for this logical title in the
             three automatic modes. Ask mode changes this to one-shot in Slice 3.
             Quality nests one level deeper: version chooses WHICH COPY, quality
             chooses how that copy is delivered (2026-07-25 ruling). -->
        <div class="addsubmenu" role="group" aria-label="Play Version">
          {#each mi.backing! as b (b.sourceId + ":" + b.ratingKey)}
            {#if inProgress}
              <button role="menuitem" onclick={() => playFrom(mi, b, "resume")}>
                Resume on {sourceNameOf(b.sourceId)}
              </button>
              <button role="menuitem" onclick={() => playFrom(mi, b, "beginning")}>
                Start over on {sourceNameOf(b.sourceId)}
              </button>
            {:else}
              <button role="menuitem" onclick={() => playFrom(mi, b, "resume")}>
                {sourceNameOf(b.sourceId)}
              </button>
            {/if}
            <button
              class="qualityrow"
              role="menuitem"
              aria-expanded={qualityMenuFor === backingKey(b)}
              onclick={() => toggleQualityMenu(mi, b)}
            >
              Quality on {sourceNameOf(b.sourceId)} <Icon name="chevron" size={13} />
            </button>
            {#if qualityMenuFor === backingKey(b)}
              {@render qualitySubmenu(mi, b)}
            {/if}
          {/each}
        </div>
      {/if}
    {:else if mi.sourceId && mi.mediaType !== "show" && mi.mediaType !== "season"}
      <!-- One copy: quality is offered directly and the nesting label never
           appears alongside it. -->
      {@const only = { sourceId: mi.sourceId!, ratingKey: mi.ratingKey }}
      <button
        role="menuitem"
        aria-expanded={qualityMenuFor === backingKey(only)}
        onclick={() => toggleQualityMenu(mi, only)}
      >
        Play at Quality <Icon name="chevron" size={13} />
      </button>
      {#if qualityMenuFor === backingKey(only)}
        {@render qualitySubmenu(mi, only)}
      {/if}
    {/if}
  </div>
{/if}

{#snippet qualitySubmenu(item: Item, b: { sourceId: string; ratingKey: string })}
  <div class="addsubmenu" role="group" aria-label="Choose a quality">
    {#if qualityLoading}
      <div class="addempty" role="status">Asking the server…</div>
    {:else if qualityError}
      <div class="addstatus addfailure" role="alert">{friendlyError(qualityError)}</div>
    {:else if qualityOptions}
      {#if qualityOptions.canDirectPlay}
        <button role="menuitem" onclick={() => playAtQuality(item, b, "original")}>
          Original — play the file as it is
        </button>
      {/if}
      {#each qualityOptions.tiers as tier (tier.id)}
        <button role="menuitem" onclick={() => playAtQuality(item, b, tier.id)}>
          {qualityLabel(tier)}
        </button>
      {/each}
      {#if qualityOptions.tiers.length === 0}
        <!-- The server told us it will not convert this copy. Saying so beats a
             blank popup, and the entry itself cannot be withheld: the answer
             only exists once the submenu has been opened. -->
        <div class="addempty">This server won't convert this title.</div>
      {/if}
    {/if}
  </div>
{/snippet}

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

<style>
  .durable-block {
    min-height: 100vh;
    display: grid;
    align-content: center;
    justify-items: start;
    gap: 0.8rem;
    width: min(42rem, calc(100% - 3rem));
    margin: 0 auto;
  }
  .durable-block h1,
  .durable-block p {
    margin: 0;
  }
  .durable-block h1:focus {
    outline: none;
  }
  .durable-block p {
    max-width: 60ch;
    color: var(--text-muted);
    line-height: 1.55;
  }
  .durable-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.7rem;
    margin-top: 0.5rem;
  }
  .durable-error {
    color: var(--danger, #ff7474) !important;
  }
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
  }
  .app:focus {
    outline: none;
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
    overflow: hidden;
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
  .serverplaylistgroup {
    margin-bottom: 0.45rem;
  }
  .serversourcename {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.4rem;
    color: var(--text-dim);
    font-size: 0.76rem;
    padding: 0.3rem 0.65rem 0.15rem;
  }
  .serversourcename span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .serversourcename small {
    color: var(--accent);
    font-size: 0.58rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .serverplayliststate {
    display: block;
    color: var(--text-dim);
    font-size: 0.75rem;
    padding: 0.22rem 0.65rem 0.4rem 1.15rem;
  }
  .serverplaylistitem {
    padding-left: 1.15rem;
    font-size: 0.84rem;
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
    background: var(--accent-tint);
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
    box-shadow: 0 0 0 3px var(--accent-glow);
  }
  .crumbs {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.7rem 1.25rem 0;
    flex-wrap: wrap;
    animation: vela-slide-down 0.16s var(--ease);
  }
  .crumbs .sort-controls {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    flex: 0 0 auto;
  }
  .crumbs .sort-field,
  .crumbs .sort-direction {
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text-2);
    border-radius: 7px;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .crumbs .sort-field {
    padding: 0.3rem 0.5rem;
  }
  .crumbs .sort-direction {
    width: 2rem;
    height: 2rem;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-family: inherit;
    font-size: 1rem;
    line-height: 1;
  }
  .crumbs .sort-field:hover,
  .crumbs .sort-direction:hover {
    background: var(--surface-2);
    color: var(--text-bright);
  }
  .crumbs .sort-field:focus-visible,
  .crumbs .sort-direction:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-glow);
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
  .poster.landscape .art {
    aspect-ratio: 16 / 9;
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
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
  .flow::after {
    content: "";
    position: absolute;
    z-index: 0;
    left: 50%;
    bottom: 0.15rem;
    width: min(62%, 34rem);
    height: 11%;
    transform: translateX(-50%);
    background: radial-gradient(ellipse at center, var(--shadow-lg), transparent 72%);
    filter: blur(8px);
    pointer-events: none;
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
    will-change: transform;
    transition:
      transform 0.32s var(--ease),
      filter 0.32s var(--ease);
  }
  .flowcard .art {
    width: 100%;
    height: 100%;
    aspect-ratio: auto;
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
  .empty-center {
    margin: auto;
    width: min(100%, 32rem);
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
    box-shadow: 0 6px 24px var(--shadow-md);
    transition: transform 0.1s var(--ease);
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
  .plex-server-choices {
    width: min(100%, 24rem);
    display: grid;
    gap: 0.65rem;
    margin-top: 0.8rem;
  }
  .plex-server-choice {
    width: 100%;
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
    background: var(--surface-sunken);
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
  .mpvactions button:not(.primary) {
    background: var(--surface-sunken);
    color: var(--warn-text);
    border: 1px solid var(--warn-border);
    border-radius: 6px;
    padding: 0.4rem 0.9rem;
    cursor: pointer;
  }
  .center {
    margin: auto;
  }
  /* The VIEW's failure. `.scanerror` is a SCAN's, styled the same but a separate
     element: the two can be on screen together, and neither may clear the other
     (codex r15). Distinct class names keep that separation legible to tests too —
     a `div.error` selector must never accidentally match a scan's status. */
  .error,
  .scanerror {
    background: var(--danger-bg);
    color: var(--danger-text);
    padding: 0.6rem 1rem;
    font-size: 0.85rem;
    animation: vela-slide-down 0.2s var(--ease);
  }

  .editwarning {
    background: var(--warn-bg);
    color: var(--warn-text);
    border-bottom: 1px solid var(--warn-border);
    padding: 0.6rem 1rem;
    font-size: 0.85rem;
    animation: vela-slide-down 0.2s var(--ease);
  }

  /* The DETAIL page's own failure — never .error, which is the VIEW's. */
  .detailerror {
    margin: 0 18px 10px;
    padding: 8px 10px;
    border-radius: 8px;
    font-size: 13px;
    line-height: 1.45;
    color: var(--danger-text);
    background: var(--danger-bg);
    border: 1px solid var(--danger-border);
  }

  /* The mpv BAR's own failure — never .error, which is the VIEW's. */
  .mpverror {
    margin: 6px 0 0;
    font-size: 12px;
    line-height: 1.4;
    color: var(--danger-text);
  }

  /* Neutral transient status (scan started) — same slot as .error, calmer. */
  .notice {
    background: var(--surface-2);
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
    box-shadow: 0 1px 4px var(--shadow-md);
    animation: vela-pop 0.13s var(--ease);
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
    box-shadow: 0 8px 28px var(--shadow-lg);
    display: flex;
    flex-direction: column;
    max-height: calc(100vh - 16px);
    overflow-y: auto;
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
  .ctxmenu button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .addsubmenu {
    display: flex;
    flex-direction: column;
    margin: 0.15rem 0.2rem 0.3rem;
    padding: 0.25rem;
    max-height: 12rem;
    overflow-y: auto;
    border: 1px solid var(--border-subtle);
    border-radius: 0.4rem;
    background: var(--bg);
  }
  .addsubmenu button {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
  }
  .addsubmenu button span {
    color: var(--text-dim);
  }
  .addstatus,
  .addempty {
    color: var(--text-muted);
    font-size: 0.78rem;
    padding: 0.45rem 0.6rem;
  }
  .addstatus.addfailure {
    color: var(--danger-text);
  }

  .sourcechoicebackdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.6);
    animation: vela-fade 0.16s var(--ease);
  }
  .sourcechoicedialog {
    width: min(34rem, 100%);
    max-height: calc(100vh - 2rem);
    overflow-y: auto;
    padding: 1.25rem;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
    box-shadow: 0 20px 60px var(--shadow-lg);
    animation: vela-pop 0.18s var(--ease);
  }
  .sourcechoicedialog h2 {
    margin: 0.2rem 0 0.35rem;
    font-size: 1.2rem;
  }
  .sourcechoices {
    display: grid;
    gap: 0.55rem;
    margin-top: 1rem;
  }
  .sourcechoices .choice {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.2rem;
    width: 100%;
    padding: 0.8rem 0.9rem;
    color: var(--text);
    text-align: left;
    background: var(--surface-2);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    cursor: pointer;
  }
  .sourcechoices .choice:hover,
  .sourcechoices .choice:focus-visible {
    border-color: var(--accent);
  }
  .sourcechoices .choice:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .choicename {
    font-weight: 650;
  }
  .choicefacts {
    color: var(--text-muted);
    font-size: 0.82rem;
  }
  .sourcechoiceactions {
    display: flex;
    justify-content: flex-end;
    margin-top: 1rem;
  }

</style>
