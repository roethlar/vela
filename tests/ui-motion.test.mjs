import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const srcRoot = path.join(repoRoot, "src");
const appCss = fs.readFileSync(path.join(srcRoot, "app.css"), "utf8");

function filesBelow(dir, suffix) {
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) => {
      const full = path.join(dir, entry.name);
      return entry.isDirectory() ? filesBelow(full, suffix) : full.endsWith(suffix) ? [full] : [];
    })
    .sort();
}

function repoPath(file) {
  return path.relative(repoRoot, file).split(path.sep).join("/");
}

const sources = new Map(
  filesBelow(srcRoot, ".svelte").map((file) => [
    repoPath(file),
    fs.readFileSync(file, "utf8"),
  ]),
);

function componentStyle(source) {
  return [...source.matchAll(/<style(?:\s[^>]*)?>([\s\S]*?)<\/style>/g)]
    .map((match) => match[1])
    .join("\n");
}

function componentMarkup(source) {
  return source.replaceAll(/<style(?:\s[^>]*)?>[\s\S]*?<\/style>/g, "");
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function balancedBlock(source, openBrace) {
  assert.equal(source[openBrace], "{", "balancedBlock must start on an opening brace");
  let depth = 0;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace + 1, index);
    }
  }
  assert.fail("unterminated CSS block");
}

function ruleBody(css, selector, { optional = false } = {}) {
  const match = new RegExp(`(?:^|\\n)\\s*${escapeRegex(selector)}\\s*\\{`, "m").exec(css);
  if (!match) {
    if (optional) return null;
    assert.fail(`missing CSS rule ${selector}`);
  }
  return balancedBlock(css, css.indexOf("{", match.index));
}

function declaration(body, property, { optional = false } = {}) {
  const match = new RegExp(`(?:^|;)\\s*${escapeRegex(property)}\\s*:\\s*([^;]+)`, "i").exec(body);
  if (!match) {
    if (optional) return null;
    assert.fail(`missing ${property} declaration in ${body.trim()}`);
  }
  return match[1].trim();
}

function timeToMs(value) {
  const match = /(-?\d*\.?\d+)\s*(ms|s)\b/i.exec(value);
  assert.ok(match, `expected a CSS time, got ${value}`);
  const amount = Number.parseFloat(match[1]);
  return match[2].toLowerCase() === "s" ? amount * 1000 : amount;
}

function animationContract(body, name, durationMs, label, { backwards = false } = {}) {
  const value = declaration(body, "animation");
  assert.match(value, new RegExp(`(?:^|\\s)${escapeRegex(name)}(?:\\s|$)`), `${label} animation name`);
  assert.equal(timeToMs(value), durationMs, `${label} duration`);
  assert.match(value, /var\(--ease\)/, `${label} must use --ease`);
  if (backwards) assert.match(value, /\bbackwards\b/, `${label} must retain backwards fill`);
  return value;
}

function animationWithin(body, name, minimumMs, maximumMs, label) {
  const value = declaration(body, "animation");
  assert.match(value, new RegExp(`(?:^|\\s)${escapeRegex(name)}(?:\\s|$)`), `${label} animation name`);
  const duration = timeToMs(value);
  assert.ok(
    duration >= minimumMs && duration <= maximumMs,
    `${label} duration must be ${minimumMs}–${maximumMs}ms, got ${duration}ms`,
  );
  assert.match(value, /var\(--ease\)/, `${label} must use --ease`);
  return value;
}

function simpleRules(css) {
  const withoutComments = css.replaceAll(/\/\*[\s\S]*?\*\//g, "");
  return [...withoutComments.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((match) => ({
    selectors: match[1]
      .split(",")
      .map((selector) => selector.trim())
      .filter(Boolean),
    declarations: match[2],
  }));
}

function mediaBody(css, query) {
  const marker = `@media (${query})`;
  const start = css.indexOf(marker);
  assert.notEqual(start, -1, `missing ${marker}`);
  return balancedBlock(css, css.indexOf("{", start));
}

function normalized(value) {
  return value.replaceAll(/\s+/g, " ").trim();
}

test("Slice 3 surface motion is CSS-only and reduced motion kills duration, delay, and translate", () => {
  const itemSource = sources.get("src/lib/ItemDetail.svelte");
  const seasonSource = sources.get("src/lib/SeasonDetail.svelte");
  const settingsSource = sources.get("src/lib/Settings.svelte");
  const pageSource = sources.get("src/routes/+page.svelte");
  assert.ok(itemSource && seasonSource && settingsSource && pageSource);

  for (const [file, source] of [
    ["ItemDetail", itemSource],
    ["SeasonDetail", seasonSource],
    ["Settings", settingsSource],
    ["page", pageSource],
  ]) {
    const markup = componentMarkup(source);
    assert.doesNotMatch(source, /from\s+["']svelte\/(?:transition|animate)["']/, `${file} imports JS motion`);
    assert.doesNotMatch(
      markup,
      /\b(?:transition|in|out|animate):[a-zA-Z]/,
      `${file} uses a Svelte motion directive instead of the CSS language`,
    );
  }

  animationContract(ruleBody(componentStyle(itemSource), ".detail"), "vela-rise", 200, "item detail");
  animationContract(ruleBody(componentStyle(seasonSource), ".season"), "vela-rise", 200, "season detail");
  animationContract(ruleBody(componentStyle(pageSource), ".crumbs"), "vela-slide-down", 160, "crumb bar");
  animationContract(ruleBody(componentStyle(settingsSource), ".overlay"), "vela-fade", 160, "Settings scrim");
  animationContract(ruleBody(componentStyle(settingsSource), ".panel"), "vela-pop", 180, "Settings panel");

  assert.equal(
    [...componentMarkup(pageSource).matchAll(/<div\s+class=["']crumbs["']/g)].length,
    2,
    "both mount-time page crumb branches must use the shared animated class",
  );
  assert.doesNotMatch(
    componentMarkup(pageSource),
    /{#key[^}]*crumb/i,
    "in-place crumb updates must not replay the mount animation",
  );

  const reduced = mediaBody(appCss, "prefers-reduced-motion: reduce");
  const blanket = simpleRules(reduced).find((rule) =>
    ["*", "*::before", "*::after"].every((selector) => rule.selectors.includes(selector)),
  );
  assert.ok(blanket, "reduced motion must cover elements and both pseudo-elements");
  for (const property of [
    "animation-duration",
    "animation-delay",
    "transition-duration",
    "transition-delay",
  ]) {
    const value = declaration(blanket.declarations, property);
    assert.ok(timeToMs(value) <= 0.01, `${property} is not effectively zero: ${value}`);
    assert.match(value, /!important/, `${property} must beat component motion`);
  }
  assert.match(declaration(blanket.declarations, "animation-iteration-count"), /^1\s*!important$/);
  assert.match(
    declaration(blanket.declarations, "translate"),
    /^(?:none|0(?:px)?(?:\s+0(?:px)?)?)\s*!important$/,
    "reduced motion must suppress the individual press/hover translate",
  );
});

test("grid and cast entrances share a bounded rise while cover-flow depth and easing stay coherent", () => {
  const pageSource = sources.get("src/routes/+page.svelte");
  const itemSource = sources.get("src/lib/ItemDetail.svelte");
  assert.ok(pageSource && itemSource);
  const pageMarkup = componentMarkup(pageSource);
  const itemMarkup = componentMarkup(itemSource);
  const pageStyle = componentStyle(pageSource);
  const itemStyle = componentStyle(itemSource);

  assert.match(
    pageMarkup,
    /style=["'][^"']*animation-delay:\s*\{Math\.min\(i,\s*14\)\s*\*\s*22\}ms;?[^"']*["']/,
    "the shared poster snippet must cap its 22ms stagger at index 14",
  );
  animationContract(ruleBody(pageStyle, ".poster"), "vela-rise", 400, "poster", { backwards: true });
  const hubLoop =
    /{#each\s+hub\.items\s+as\s+item,\s*i[^}]*}([\s\S]*?){\/each}/.exec(pageMarkup)?.[1];
  assert.ok(hubLoop, "Home hub rendering must expose an index for the shared stagger");
  assert.match(
    hubLoop,
    /{@render\s+poster\(item,\s*i,/,
    "Home hub cards must continue to use the same animated poster snippet as the grid",
  );

  const castLoop = /{#each\s+detail\??\.cast\s+as\s+c,\s*i[^}]*}([\s\S]*?){\/each}/.exec(itemMarkup)?.[1];
  assert.ok(castLoop, "cast rendering must expose an index for its bounded stagger");
  assert.match(castLoop, /Math\.min\(i,\s*14\)\s*\*\s*22/, "cast delay must use the grid cap and step");
  const castTags = [...castLoop.matchAll(/<(?:button|div)\b[^>]*class=["'][^"']*\bcastcard\b[^"']*["'][^>]*>/g)].map(
    (match) => match[0],
  );
  assert.ok(castTags.length >= 2, "both clickable and static cast cards must be guarded");
  for (const tag of castTags) {
    assert.match(tag, /animation-delay/, `cast card lacks its bounded delay: ${tag}`);
  }
  const castAnimation = animationWithin(ruleBody(itemStyle, ".castcard"), "vela-rise", 100, 300, "cast card");
  assert.match(castAnimation, /\bbackwards\b/, "cast card must stay hidden only for its bounded entrance delay");

  assert.match(pageMarkup, /Math\.abs\(d\)\s*<=\s*4/, "cover-flow must remain bounded to at most nine cards");
  assert.match(declaration(ruleBody(pageStyle, ".flowcard"), "will-change"), /\btransform\b/);
  const ground = ruleBody(pageStyle, ".flow::after");
  assert.match(declaration(ground, "content"), /^(?:""|'')$/);
  assert.equal(declaration(ground, "pointer-events"), "none");
  assert.match(declaration(ground, "position"), /absolute/);
  assert.match(declaration(ground, "background"), /radial-gradient\(/);
  assert.match(declaration(ground, "background"), /var\(--shadow-(?:md|lg)\)/);
  const groundZ = Number.parseInt(declaration(ground, "z-index"), 10);
  assert.ok(Number.isInteger(groundZ) && groundZ <= 1, `ground shadow must remain below cards: ${groundZ}`);

  const motionSources = [
    ["src/app.css", appCss],
    ...[...sources].map(([file, source]) => [file, componentStyle(source)]),
  ];
  const linearNames = new Set(["vela-shimmer", "refresh-spin"]);
  for (const [file, css] of motionSources) {
    for (const match of css.matchAll(/\b(animation|transition)\s*:\s*([^;]+);/g)) {
      const [kind, value] = [match[1], normalized(match[2])];
      if (kind === "animation" && /\blinear\b/.test(value)) {
        assert.ok(
          [...linearNames].some((name) => new RegExp(`(?:^|\\s)${escapeRegex(name)}(?:\\s|$)`).test(value)),
          `${file} has an unclassified linear animation: ${value}`,
        );
        continue;
      }
      assert.match(value, /var\(--ease\)/, `${file} ${kind} must use --ease: ${value}`);
      const withoutToken = value.replaceAll(/var\(--ease\)/g, "");
      assert.doesNotMatch(
        withoutToken,
        /\b(?:linear|ease|ease-in|ease-out|ease-in-out|step-start|step-end)\b|cubic-bezier\(|steps\(/,
        `${file} has a second easing language: ${value}`,
      );
    }
    for (const match of css.matchAll(/\b(?:animation|transition)-timing-function\s*:\s*([^;]+);/g)) {
      assert.match(match[1], /var\(--ease\)/, `${file} has a bare timing function: ${match[1].trim()}`);
    }
  }
});

test("button press, episode hover, and watched-badge pop are composable and narrowly owned", () => {
  const seasonSource = sources.get("src/lib/SeasonDetail.svelte");
  const pageSource = sources.get("src/routes/+page.svelte");
  assert.ok(seasonSource && pageSource);
  const seasonStyle = componentStyle(seasonSource);
  const pageStyle = componentStyle(pageSource);

  const press = ruleBody(appCss, "button:active:not(:disabled)");
  assert.match(normalized(declaration(press, "translate")), /^(?:0|0px)\s+1px$/);
  assert.equal(
    declaration(press, "transform", { optional: true }),
    null,
    "the universal press must compose through translate, not replace transform geometry",
  );
  const oldPrimary = ruleBody(appCss, "button.primary:active:not(:disabled)", { optional: true });
  if (oldPrimary) {
    assert.equal(declaration(oldPrimary, "transform", { optional: true }), null, "remove duplicate primary press transform");
    assert.equal(declaration(oldPrimary, "translate", { optional: true }), null, "global press owns translate");
  }
  assert.doesNotMatch(appCss, /transform\s*:\s*translateY\(\s*1px\s*\)/, "legacy primary-only press remains");

  const episode = ruleBody(seasonStyle, ".eprow");
  const transition = declaration(episode, "transition");
  for (const property of ["background", "border-color", "translate"]) {
    const segment = transition
      .split(",")
      .map(normalized)
      .find((part) => part.startsWith(property === "background" ? "background" : property));
    assert.ok(segment, `episode row does not transition ${property}: ${transition}`);
    assert.ok(timeToMs(segment) <= 200, `episode ${property} transition is not short: ${segment}`);
    assert.match(segment, /var\(--ease\)/);
  }
  const episodeHover = ruleBody(seasonStyle, ".eprow:hover");
  assert.match(normalized(declaration(episodeHover, "translate")), /^2px(?:\s+(?:0|0px))?$/);
  assert.match(declaration(episodeHover, "background"), /var\(--bg-blur\)/);

  animationWithin(ruleBody(pageStyle, ".watchedbadge"), "vela-pop", 100, 300, "grid watched badge");
  for (const [label, body] of [
    ["static watched chip", ruleBody(appCss, ".chip.watched")],
    ["episode watched mark", ruleBody(seasonStyle, ".watchedmark")],
  ]) {
    assert.equal(declaration(body, "animation", { optional: true }), null, `${label} must not pop on static render`);
  }
});

function emptyStateTags(source) {
  return [...source.matchAll(/<EmptyState\b[\s\S]*?>/g)].map((match) => ({
    index: match.index,
    tag: normalized(match[0]),
  }));
}

function callFor(file, headingAttribute, hintAttribute, icon) {
  const source = sources.get(file);
  assert.ok(source, `${file} must exist`);
  const calls = emptyStateTags(source);
  const call = calls.find(
    ({ tag }) => tag.includes(headingAttribute) && tag.includes(hintAttribute),
  );
  assert.ok(
    call,
    `${file} lacks exact EmptyState call: ${headingAttribute} / ${hintAttribute}`,
  );
  assert.match(
    call.tag,
    new RegExp(`\\bicon\\s*=\\s*["']${icon}["']`),
    `${headingAttribute} icon taxonomy`,
  );
  return { ...call, file, source };
}

function nearestBranchCondition(source, index) {
  const prefix = source.slice(0, index);
  return [...prefix.matchAll(/{(?:#if|:else if)\s+([^}]*)}/g)].at(-1)?.[1] ?? "";
}

test("the shared EmptyState owns exact settled-empty structure and excludes loaders and failures", () => {
  const emptyFile = "src/lib/EmptyState.svelte";
  const emptySource = sources.get(emptyFile);
  assert.ok(emptySource, `${emptyFile} must provide the shared primitive`);
  const emptyMarkup = componentMarkup(emptySource);
  const emptyStyle = componentStyle(emptySource);

  const iconUses = [...emptyMarkup.matchAll(/<Icon\b[^>]*\bname=["'](film|playlist)["'][^>]*>/g)];
  assert.deepEqual(
    new Set(iconUses.map((match) => match[1])),
    new Set(["film", "playlist"]),
    "the typed icon choice must render one of the two empty-state icons",
  );
  assert.equal(iconUses.length, 2, "the primitive must not carry any third icon branch");
  assert.match(
    emptyMarkup,
    /{#if\s+icon\s*===\s*["']film["']}[\s\S]*?<Icon\b[^>]*name=["']film["'][^>]*>[\s\S]*?{:else}[\s\S]*?<Icon\b[^>]*name=["']playlist["'][^>]*>[\s\S]*?{\/if}/,
    "the two typed branches must be exclusive so exactly one icon renders",
  );
  assert.equal((emptyMarkup.match(/class=["']empty-state-icon["']/g) ?? []).length, 1, "EmptyState has one decorative icon slot");
  assert.match(emptyMarkup, /class=["']empty-state-icon["'][^>]*aria-hidden=["']true["']/);
  assert.equal((emptyMarkup.match(/<h2\b/g) ?? []).length, 1, "EmptyState has exactly one heading");
  assert.equal((emptyMarkup.match(/<p\b/g) ?? []).length, 1, "EmptyState has exactly one hint");
  assert.match(emptyMarkup, /\bicon\s*:\s*(?:["']film["']\s*\|\s*["']playlist["']|["']playlist["']\s*\|\s*["']film["']|IconName\b|ComponentProps\b)/, "icon prop must be typed");
  const statusInput = /\b(?:announce|polite)\s*(?:=|:)/.exec(emptyMarkup)?.[0];
  assert.ok(statusInput, "the primitive needs an optional polite-status input");
  assert.match(emptyMarkup, /role\s*=\s*\{[^}]*(?:announce|polite)[^}]*["']status["']/, "polite mode must expose status semantics");
  assert.match(emptyStyle, /var\(--text-muted\)/, "the hint must use the muted semantic token");
  assert.doesNotMatch(emptyStyle, /margin\s*:\s*auto|position\s*:\s*(?:absolute|fixed)|flex\s*:\s*1\b/, "parents own centering and in-flow placement");

  const categories = [
    callFor(
      "src/routes/+page.svelte",
      'heading="Welcome to Vela"',
      'hint="Connect Plex, Jellyfin, or Emby to start browsing your library in HDR."',
      "film",
    ),
    callFor(
      "src/routes/+page.svelte",
      'heading="No titles on Home yet"',
      'hint="Choose a library from the sidebar to start browsing."',
      "film",
    ),
    callFor(
      "src/routes/+page.svelte",
      'heading="No libraries found"',
      'hint="Check the connected server, then use Refresh libraries."',
      "film",
    ),
    callFor(
      "src/routes/+page.svelte",
      'heading="No titles in this view"',
      'hint="Go back, refresh libraries, or choose another library."',
      "film",
    ),
    callFor(
      "src/routes/+page.svelte",
      'heading={`No matches for “${searchTerm}”`}',
      'hint="Check the spelling or try a broader search."',
      "film",
    ),
    callFor(
      "src/routes/+page.svelte",
      'heading={`No titles found for ${personView.name}`}',
      'hint="Go back to keep browsing."',
      "film",
    ),
    callFor(
      "src/lib/PlaylistsView.svelte",
      'heading="No playlists yet"',
      'hint="Name one above, then add titles from their context menus."',
      "playlist",
    ),
    callFor(
      "src/lib/PlaylistsView.svelte",
      'heading="This playlist is empty"',
      'hint="Use a title\'s context menu to add it here."',
      "playlist",
    ),
    callFor(
      "src/lib/ServerPlaylistView.svelte",
      'heading="This server playlist is empty"',
      'hint={`Add videos on ${playlist.sourceName}, then reopen it here.`}',
      "playlist",
    ),
    callFor(
      "src/lib/SeasonDetail.svelte",
      'heading="No episodes in this season"',
      'hint="Go back and choose another season."',
      "film",
    ),
    callFor(
      "src/lib/SeasonDetail.svelte",
      'heading="Choose an episode"',
      'hint="Select one from the list to see details and playback options."',
      "film",
    ),
  ];

  const searchCall = categories[4];
  assert.match(searchCall.tag, /\b(?:announce|polite)(?:\s|=|\/>|>)/, "only no-match search is a polite status");
  for (const category of categories.filter((entry) => entry !== searchCall)) {
    assert.doesNotMatch(category.tag, /\b(?:announce|polite)(?:\s|=|\/>|>)/, "non-search empty states are not live regions");
  }

  const recognized = new Set(categories.map(({ file, index }) => `${file}:${index}`));
  const expectedFiles = new Set([
    "src/routes/+page.svelte",
    "src/lib/PlaylistsView.svelte",
    "src/lib/ServerPlaylistView.svelte",
    "src/lib/SeasonDetail.svelte",
  ]);
  for (const [file, source] of sources) {
    const calls = emptyStateTags(source);
    if (calls.length > 0) assert.ok(expectedFiles.has(file), `${file} adds an unclassified full-surface empty state`);
    for (const { index, tag } of calls) {
      assert.ok(recognized.has(`${file}:${index}`), `${file} has an unclassified EmptyState call: ${tag}`);
      assert.doesNotMatch(tag, /\baria-busy\b|Loading|Couldn't|failed/i, `${file} puts a loader/failure in EmptyState`);
    }
  }

  const page = sources.get("src/routes/+page.svelte");
  const playlists = sources.get("src/lib/PlaylistsView.svelte");
  const serverPlaylist = sources.get("src/lib/ServerPlaylistView.svelte");
  const season = sources.get("src/lib/SeasonDetail.svelte");
  assert.ok(page && playlists && serverPlaylist && season);

  assert.match(page, /<button\s+class=["']primary["'][\s\S]{0,180}>Add a source<\/button>/, "Welcome retains Add a source");
  for (const retained of [
    /aria-label=["']Search your libraries["']/,
    /class=["']sideitem["']/,
    /class=["']crumbs["']/,
    /aria-label=["']Sort by["']/,
    /class=["']sort-direction["']/,
    /aria-label=["']Settings["']/,
  ]) assert.match(page, retained, `page control disappeared while migrating empties: ${retained}`);
  for (const retained of [/id=["']playlist-create["']/, /id=["']playlist-rename["']/, /Delete playlist/]) {
    assert.match(playlists, retained, `playlist control disappeared while migrating empties: ${retained}`);
  }

  for (const [file, source, loader] of [
    ["PlaylistsView", playlists, "Loading playlists…"],
    ["PlaylistsView", playlists, "Loading playlist…"],
    ["ServerPlaylistView", serverPlaylist, "Loading server playlist…"],
  ]) {
    assert.ok(source.includes(loader), `${file} lost loader '${loader}'`);
    const containingCall = emptyStateTags(source).find(({ tag }) => tag.includes(loader));
    assert.equal(containingCall, undefined, `${file} loader entered the settled-empty primitive`);
  }
  const episodeLoader =
    /{#if\s+loadingList\s*&&\s*episodes\.length\s*===\s*0}([\s\S]*?){:else}/.exec(
      season,
    )?.[1];
  assert.ok(episodeLoader, "episode loading must retain a dedicated skeleton branch");
  assert.match(
    episodeLoader,
    /class=["']eprow skel["'][^>]*aria-hidden=["']true["']/,
    "episode skeleton remains decorative and outside EmptyState",
  );
  assert.doesNotMatch(episodeLoader, /<EmptyState\b/, "episode loader cannot use EmptyState");
  assert.match(page, /class=["']addempty["'][^>]*>No playlists yet[^<]*<\/div>/, "compact add-menu absence stays compact");
  assert.match(page, /class=["']serverplayliststate["'][^>]*>No video playlists<\/span>/, "sidebar playlist metadata stays inline");
  assert.match(sources.get("src/lib/Settings.svelte"), /No servers yet\. Add one under Servers\./, "Settings no-server copy stays inline");

  const playlistIndex = categories[6];
  const playlistCondition = nearestBranchCondition(playlistIndex.source, playlistIndex.index);
  assert.match(playlistCondition, /playlists\.length\s*===\s*0/);
  assert.match(playlistCondition, /!\s*listStatus\?\.failed/, "failed playlist load cannot claim an empty index");

  const serverEmpty = categories[8];
  const serverCondition = nearestBranchCondition(serverEmpty.source, serverEmpty.index);
  assert.match(serverCondition, /items\.length\s*===\s*0/);
  assert.match(serverCondition, /!\s*status\?\.failed/, "failed server-playlist load cannot claim an empty result");

  const zeroEpisodes = categories[9];
  const zeroCondition = nearestBranchCondition(zeroEpisodes.source, zeroEpisodes.index);
  assert.match(zeroCondition, /episodes\.length\s*===\s*0/);
  assert.match(
    season,
    /{#if\s+selected}[\s\S]*?{:else if\s+loadingList}[\s\S]*?{:else if\s+listError}[\s\S]*?{:else if\s+episodes\.length\s*===\s*0}[\s\S]*?No episodes in this season[\s\S]*?{:else}[\s\S]*?Choose an episode[\s\S]*?{\/if}/,
    "selection, loading, failure, zero episodes, and choose-an-episode must be mutually exclusive",
  );

  assert.match(
    page,
    /hubs\.length\s*===\s*0\s*&&\s*heroItems\.length\s*===\s*0[\s\S]*?{#if\s+!error}[\s\S]*?No libraries found[\s\S]*?No titles on Home yet[\s\S]*?{\/if}/,
    "Home failure and authoritative empty states must not render together",
  );
  assert.match(
    page,
    /{#if\s+items\.length\s*===\s*0}[\s\S]*?{#if\s+!error}[\s\S]*?No matches for[\s\S]*?No titles found for[\s\S]*?No titles in this view[\s\S]*?{\/if}/,
    "browse/search/person failure and authoritative empty states must not render together",
  );
});
