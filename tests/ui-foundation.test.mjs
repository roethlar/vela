import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const srcRoot = path.join(repoRoot, "src");
const appCss = fs.readFileSync(path.join(srcRoot, "app.css"), "utf8");
const appHtml = fs.readFileSync(path.join(srcRoot, "app.html"), "utf8");

function filesBelow(dir, suffix) {
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) => {
      const full = path.join(dir, entry.name);
      return entry.isDirectory() ? filesBelow(full, suffix) : full.endsWith(suffix) ? [full] : [];
    })
    .sort();
}

const svelteFiles = filesBelow(srcRoot, ".svelte");
const sources = new Map(
  svelteFiles.map((file) => [path.relative(repoRoot, file), fs.readFileSync(file, "utf8")]),
);

function componentStyle(source) {
  return [...source.matchAll(/<style(?:\s[^>]*)?>([\s\S]*?)<\/style>/g)]
    .map((match) => match[1])
    .join("\n");
}

function cssRules(css) {
  const withoutComments = css.replaceAll(/\/\*[\s\S]*?\*\//g, "");
  return [...withoutComments.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((match) => ({
    selectors: match[1]
      .split(",")
      .map((selector) => selector.trim())
      .filter((selector) => selector && !selector.startsWith("@")),
    declarations: match[2],
  }));
}

function ruleFor(rules, selector) {
  return rules.find((rule) => rule.selectors.includes(selector));
}

function properties(declarations) {
  return new Set(
    [...declarations.matchAll(/(?:^|;)\s*([a-z-]+)\s*:/g)].map((match) => match[1]),
  );
}

test("all themes define the Slice 1 accent and destructive-action tokens", () => {
  const expectedThemes = [
    "dark",
    "oled",
    "dracula",
    "nord",
    "solarized-dark",
    "gruvbox-dark",
    "solarized-light",
    "gruvbox-light",
    "catppuccin-latte",
    "rose-pine-dawn",
    "one-light",
  ];
  const requiredTokens = [
    "--accent-tint",
    "--accent-glow",
    "--danger-solid",
    "--danger-solid-hover",
    "--on-danger",
  ];
  const blocks = new Map();
  for (const match of appCss.matchAll(
    /((?:^|\n)\s*:root(?:\s*,\s*:root\[data-theme="dark"\]|\[data-theme="[^"]+"\]))\s*\{([^}]*)\}/g,
  )) {
    const theme = match[1].match(/data-theme="([^"]+)"/)?.[1];
    if (theme) blocks.set(theme, match[2]);
  }

  assert.deepEqual([...blocks.keys()], expectedThemes, "the test must cover the complete theme catalog");
  const settings = sources.get("src/lib/Settings.svelte");
  const settingsThemeIds = [
    ...settings.matchAll(/\{ id: "([^"]+)", label: "[^"]+", mode: "(?:dark|light)", swatch:/g),
  ].map((match) => match[1]);
  assert.deepEqual(settingsThemeIds, expectedThemes, "Settings must expose the complete theme catalog");
  const prepaintIds = JSON.parse(appHtml.match(/var ids = (\[[^;]+\]);/)?.[1] ?? "[]");
  assert.deepEqual(prepaintIds, expectedThemes, "first paint must accept the complete theme catalog");
  for (const [theme, body] of blocks) {
    for (const token of requiredTokens) {
      assert.match(body, new RegExp(`${token.replaceAll("-", "\\-")}\\s*:`), `${theme} is missing ${token}`);
    }
  }
});

test("OLED Black keeps a literal-black canvas and dims only chrome", () => {
  const oled = appCss.match(/:root\[data-theme="oled"\]\s*\{([^}]*)\}/)?.[1];
  assert.ok(oled, "app.css must define OLED Black");
  const palette = new Map(
    [...oled.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)].map((match) => [match[1], match[2].trim()]),
  );
  assert.equal(palette.get("--bg"), "#000000", "the OLED canvas must be literal black");
  assert.equal(palette.get("--surface"), "#070707", "OLED controls stay near black");
  assert.equal(palette.get("--surface-2"), "#0e0e0e", "raised OLED controls stay near black");
  assert.equal(palette.get("--text"), "#c7c7c7", "OLED primary text is intentionally dimmed");
  assert.equal(palette.get("--text-muted"), "#777777", "OLED muted text remains legible on black");
  assert.equal(palette.get("--text-bright"), "#e6e6e6", "OLED bright UI never reaches media white");
  assert.equal(palette.get("--accent"), "#c58a0b", "OLED keeps a dimmed Vela amber");

  const overrides = [...appCss.matchAll(/:root\[data-theme="oled"\]\s+([^{}]+)\s*\{([^}]*)\}/g)].map(
    (match) => ({ selector: match[1].trim(), declarations: match[2] }),
  );
  assert.deepEqual(
    overrides.map(({ selector }) => selector),
    ["body", ".grain"],
    "OLED may suppress ambient chrome but must not dim media selectors",
  );
  assert.match(overrides[0].declarations, /background-image\s*:\s*none\s*;/);
  assert.match(overrides[1].declarations, /display\s*:\s*none\s*;/);
});

test("component styles contain no hardcoded semantic accent or danger colors", () => {
  const forbidden = [
    ["Vela Dark accent glow", /rgba\(\s*229\s*,\s*160\s*,\s*13\s*,\s*(?:0?\.15|15%)\s*\)/i],
    ["legacy danger copy", /#(?:ffb4ad|ffb4a9|ff9e96)\b/i],
    ["legacy danger border", /#(?:e25d52|c94a44)\b/i],
    ["legacy solid danger", /#a62e29\b/i],
    ["legacy danger tint", /rgba\(\s*120\s*,\s*24\s*,\s*20\s*,\s*(?:0?\.24|24%)\s*\)/i],
    ["legacy error foreground", /rgba\(\s*255\s*,\s*80\s*,\s*60\s*,/i],
  ];

  for (const [file, source] of sources) {
    const style = componentStyle(source);
    for (const [label, pattern] of forbidden) {
      assert.doesNotMatch(style, pattern, `${file} still owns a hardcoded ${label}`);
    }
  }
});

test("app.css exclusively owns the six shared visual primitives", () => {
  const globalRules = cssRules(appCss);
  const requiredBaseSelectors = [
    ".playoverlay",
    ".playbtn",
    ".progress",
    ".progress .bar",
    ".noart",
    ".chip",
    ".chip.watched",
    ".personlink",
    ".personlink:hover",
    ".personlink:focus-visible",
  ];
  for (const selector of requiredBaseSelectors) {
    assert.ok(ruleFor(globalRules, selector), `app.css must own ${selector}`);
  }
  const primaryStates = [
    ["button.primary", ".primary"],
    ["button.primary:hover:not(:disabled)", "button.primary:hover", ".primary:hover:not(:disabled)", ".primary:hover"],
    [
      "button.primary:active:not(:disabled)",
      "button.primary:active",
      ".primary:active:not(:disabled)",
      ".primary:active",
      "button:active:not(:disabled)",
    ],
    ["button.primary:disabled", ".primary:disabled"],
  ];
  for (const alternatives of primaryStates) {
    assert.ok(
      alternatives.some((selector) => ruleFor(globalRules, selector)),
      `app.css must own ${alternatives[0]}`,
    );
  }

  const progress = ruleFor(globalRules, ".progress");
  assert.match(progress.declarations, /\bheight\s*:\s*4px\s*;/, "the one progress primitive is 4px");
  assert.match(
    ruleFor(globalRules, ".progress .bar").declarations,
    /background(?:-image)?\s*:\s*linear-gradient\(/,
    "the progress fill keeps its shared accent gradient",
  );

  const exactComponentOwned = new Set([
    ".playoverlay",
    ".playbtn",
    ".progress",
    ".progress .bar",
    ".noart",
    ".chip",
    ".chip.watched",
    ".personlink",
    ".personlink:hover",
    ".personlink:focus-visible",
  ]);
  const primaryVisualProperties = new Set([
    "background",
    "background-color",
    "color",
    "border",
    "border-color",
    "border-radius",
    "padding",
    "font",
    "font-weight",
    "cursor",
    "opacity",
    "transform",
    "transition",
  ]);
  const noArtVisualProperties = new Set([
    "background",
    "background-color",
    "background-image",
    "color",
    "font-size",
    "font-weight",
    "line-height",
    "text-align",
  ]);

  for (const [file, source] of sources) {
    for (const rule of cssRules(componentStyle(source))) {
      for (const selector of rule.selectors) {
        assert.ok(!exactComponentOwned.has(selector), `${file} duplicates global ${selector}`);
        const ownedProperties = properties(rule.declarations);
        if (selector.includes(".progress")) {
          assert.fail(`${file} overrides the one global progress primitive in ${selector}`);
        }
        const positiveSelector = selector.replaceAll(/:not\(\.primary\)/g, "");
        if (positiveSelector.includes(".primary")) {
          const visual = [...ownedProperties].filter((property) => primaryVisualProperties.has(property));
          assert.deepEqual(visual, [], `${file} visually overrides global primary in ${selector}`);
        }
        if (/^button(?::(?:hover|active|disabled|focus-visible))?$/.test(selector)) {
          const visual = [...ownedProperties].filter((property) => primaryVisualProperties.has(property));
          assert.deepEqual(
            visual,
            [],
            `${file} has a generic ${selector} rule that visually overrides global primary`,
          );
        }
        const positiveNoArtSelector = selector.replaceAll(/:not\(\.noart\)/g, "");
        if (positiveNoArtSelector.includes(".noart")) {
          const visual = [...ownedProperties].filter((property) => noArtVisualProperties.has(property));
          assert.deepEqual(visual, [], `${file} visually overrides global no-art in ${selector}`);
        }
      }
    }
  }
});

test("Player Settings exposes the four duplicate-source policies and exact priority help", () => {
  const settings = sources.get("src/lib/Settings.svelte");
  assert.ok(settings, "Settings source is available");
  const policies = [...settings.matchAll(/value: "(best|compatible|fastest|ask)",\s+label: "([^"]+)"/g)]
    .map((match) => [match[1], match[2]]);
  assert.deepEqual(policies, [
    ["best", "Prefer Best"],
    ["compatible", "Prefer Compatible"],
    ["fastest", "Prefer Fastest Source"],
    ["ask", "Ask Every Time"],
  ]);
  assert.match(settings, /resolution → HDR within that resolution → bitrate/);
  assert.match(settings, /this machine → local network → internet/);
  assert.match(settings, /Play Version/);
  assert.match(settings, /only for that playback session/);
  assert.match(settings, /Advanced display override/);
  assert.match(settings, /Resolution and HDR can be overridden independently/);
  assert.match(settings, /invoke<PlaybackPreferences>\("get_playback_preferences"\)/);
  assert.match(settings, /invoke\("set_playback_preferences"/);

  const page = sources.get("src/routes/+page.svelte");
  assert.ok(page, "main page source is available");
  assert.match(page, />Play Version <Icon name="chevron"/);
  assert.match(page, /aria-label="Play Version"/);
  assert.match(page, /explicitSourceId/);
  assert.match(page, /resolve_playback_source_choice/);
  assert.match(page, /role="dialog"/);
  assert.match(page, /aria-modal="true"/);
  assert.match(page, /handleSourceChoiceDialogKeydown/);
  assert.match(page, /sourceChoicePreviousFocus = document\.activeElement/);
  assert.match(page, /previous\?\.isConnected[\s\S]{0,80}previous\.focus\(\)/);
  assert.match(page, /querySelector<HTMLButtonElement>\("button\.choice"\)\?\.focus\(\)/);
  assert.match(page, /listen<\{ requestId: string \}>\("source-choice-required"/);
  assert.match(page, /get_playback_source_choice/);
  assert.match(page, /if \(sourceChoiceRequest\)[\s\S]{0,180}cancelSourceChoice\(\)/);
  assert.match(page, /async function play\([\s\S]{0,500}invalidateContinuationRun\(\)/);
  assert.equal(
    [...page.matchAll(/onManualPlay=\{invalidateContinuationRun\}/g)].length,
    2,
    "both Vela and server playlist manual starts invalidate delayed continuation work",
  );
  for (const component of ["src/lib/PlaylistsView.svelte", "src/lib/ServerPlaylistView.svelte"]) {
    assert.match(sources.get(component), /onManualPlay\?\.\(\);\s*try \{/);
  }
  assert.doesNotMatch(
    page,
    /playFrom[\s\S]{0,700}set_merged_override/,
    "Ask-mode Play Version choices must go through the one-shot backend path",
  );
});

test("merged hierarchy coordinates travel through browse and season pagination", () => {
  const page = sources.get("src/routes/+page.svelte");
  const season = sources.get("src/lib/SeasonDetail.svelte");
  assert.ok(page && season, "hierarchy surfaces are available");
  assert.match(page, /backing: here\.backing/);
  assert.match(page, /canonicalId: here\.canonicalId/);
  assert.match(page, /mediaType: here\.mediaType/);
  assert.match(page, /backing: request\.backing/);
  assert.match(season, /backing: seedItem\.backing/);
  assert.match(season, /canonicalId: seedItem\.canonicalId/);
  assert.match(season, /mediaType: seedItem\.mediaType/);
});

test("multi-server Plex linking pauses for an explicit credential-free server choice", () => {
  const page = sources.get("src/routes/+page.svelte");
  const choiceType = page.match(/type PlexServerChoice = \{([^}]+)\}/)?.[1] ?? "";

  assert.match(choiceType, /machineIdentifier:\s*string/);
  assert.match(choiceType, /name:\s*string/);
  assert.doesNotMatch(choiceType, /token|credential|clientIdentifier/i);
  assert.match(
    page,
    /result\.status === "chooseServer"[\s\S]*plexServerChoices = result\.servers;[\s\S]*return;/,
    "polling must stop and publish the backend's server choices",
  );
  assert.match(
    page,
    /invoke<Source>\("link_select_server",\s*\{[\s\S]*machineIdentifier,/,
    "the selected stable machine identifier must return to the backend",
  );
  assert.match(
    page,
    /\{#each plexServerChoices as server \(server\.machineIdentifier\)\}/,
    "every reachable physical server must be rendered once",
  );
});

test("Settings removes every connected server by its exact source id", () => {
  const settings = sources.get("src/lib/Settings.svelte");

  assert.doesNotMatch(settings, /unlink_plex|unlinkPlex|>Disconnect</);
  assert.match(
    settings,
    /\{#each sources as s \(s\.id\)\}[\s\S]*onclick=\{\(\) => removeSource\(s\.id\)\}>Remove<\/button>[\s\S]*\{\/each\}/,
    "Plex, Jellyfin, and Emby rows must all use the normal exact-id removal path",
  );
  assert.match(
    settings,
    /onLinkPlex\(\); onClose\(\);[\s\S]*>Link Plex…<\/button>/,
    "repeatable Plex linking must stay available after a source is connected",
  );
});

test("Continue Watching has no duplicate playback action row", () => {
  const page = sources.get("src/routes/+page.svelte");
  assert.ok(page, "the application page must exist");
  assert.equal(
    /\bflowactions\b|Playback choices for/.test(page),
    false,
    "the centered carousel card already owns Play/Resume; do not add another action row",
  );
  assert.match(
    page,
    /onclick=\{\(\) => \(d === 0 \? play\(it\) : \(heroPos = i\)\)\}/,
    "the centered carousel card must remain the playback control",
  );
  assert.match(
    page,
    /aria-label=\{d === 0 \? `\$\{hasResume\(it\) \? "Resume" : "Play"\}/,
    "the centered carousel card must expose its dynamic Play/Resume action",
  );
});

test("Icon names are typed, defined, used, and free of migrated raw UI glyphs", () => {
  const iconFile = "src/lib/Icon.svelte";
  const iconSource = sources.get(iconFile);
  assert.ok(iconSource, `${iconFile} must exist`);

  const union = iconSource.match(/type\s+IconName\s*=\s*([\s\S]*?);/)?.[1];
  assert.ok(union, "Icon.svelte must define an IconName literal union");
  const typedNames = [...union.matchAll(/["']([a-z][a-z0-9-]*)["']/g)].map((match) => match[1]);
  const definedNames = [...iconSource.matchAll(/name\s*===\s*["']([a-z][a-z0-9-]*)["']/g)].map(
    (match) => match[1],
  );
  assert.match(iconSource, /\bname\s*:\s*IconName\b/, "the name prop must use IconName");
  assert.deepEqual(new Set(definedNames), new Set(typedNames), "the literal union and SVG branches must agree");

  const uses = [];
  let iconTagCount = 0;
  for (const [file, source] of sources) {
    if (file === iconFile) continue;
    iconTagCount += [...source.matchAll(/<Icon\b/g)].length;
    for (const match of source.matchAll(/<Icon\b[^>]*\bname=["']([^"']+)["']/g)) {
      uses.push({ file, name: match[1] });
    }
  }
  assert.equal(uses.length, iconTagCount, "every Icon use must name a literal checked by this contract");
  for (const use of uses) {
    assert.ok(typedNames.includes(use.name), `${use.file} uses undefined icon '${use.name}'`);
  }
  for (const name of typedNames) {
    assert.ok(uses.some((use) => use.name === name), `Icon '${name}' is defined but dead`);
  }
  for (const name of ["star", "heart", "alert"]) assert.ok(typedNames.includes(name), `missing ${name} icon`);
  assert.ok(!typedNames.includes("search"), "the sole dead search icon must be deleted");

  const rawGlyphContracts = [
    ["src/lib/ItemDetail.svelte", /[★♥]/u],
    ["src/lib/SeasonDetail.svelte", /★/u],
    ["src/lib/Settings.svelte", /[✓✗⚠]/u],
  ];
  for (const [file, glyphs] of rawGlyphContracts) {
    assert.doesNotMatch(sources.get(file), glyphs, `${file} still renders a migrated raw glyph`);
  }
  const page = sources.get("src/routes/+page.svelte");
  assert.doesNotMatch(page, /Add to Playlist\s*→/u, "the playlist submenu still uses a raw arrow");
  assert.match(
    page,
    /Add to Playlist[\s\S]{0,160}<Icon\s+name=["']chevron["']/,
    "the playlist submenu must use the shared chevron icon",
  );
});
