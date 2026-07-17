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
  for (const [theme, body] of blocks) {
    for (const token of requiredTokens) {
      assert.match(body, new RegExp(`${token.replaceAll("-", "\\-")}\\s*:`), `${theme} is missing ${token}`);
    }
  }
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
