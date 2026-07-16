import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { imageReveal } from "../src/lib/imageReveal.ts";

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

const sources = new Map(
  filesBelow(srcRoot, ".svelte").map((file) => [
    path.relative(repoRoot, file),
    fs.readFileSync(file, "utf8"),
  ]),
);

class FakeClassList {
  #classes = new Set(["image-reveal"]);

  add(name) {
    this.#classes.add(name);
  }

  remove(name) {
    this.#classes.delete(name);
  }

  contains(name) {
    return this.#classes.has(name);
  }
}

class FakeImage {
  #attributes = new Map();
  #listeners = new Map();

  classList = new FakeClassList();
  complete = false;
  naturalWidth = 0;

  constructor(source) {
    this.setAttribute("src", source);
  }

  getAttribute(name) {
    return this.#attributes.get(name) ?? null;
  }

  setAttribute(name, value) {
    this.#attributes.set(name, String(value));
  }

  addEventListener(type, listener) {
    const listeners = this.#listeners.get(type) ?? new Set();
    listeners.add(listener);
    this.#listeners.set(type, listeners);
  }

  removeEventListener(type, listener) {
    this.#listeners.get(type)?.delete(listener);
  }

  dispatch(type) {
    for (const listener of this.#listeners.get(type) ?? []) listener({ currentTarget: this });
  }

  listenerCount(type) {
    return this.#listeners.get(type)?.size ?? 0;
  }
}

const loaded = (node) => node.classList.contains("image-loaded");
const queued = () => new Promise((resolve) => queueMicrotask(resolve));

test("imageReveal reveals only a successful nonzero-width load and hides an error", async () => {
  const node = new FakeImage("/ordinary.jpg");
  const action = imageReveal(node, "/ordinary.jpg");

  await queued();
  assert.equal(loaded(node), false, "an incomplete image must remain transparent");

  node.complete = true;
  node.naturalWidth = 0;
  node.dispatch("load");
  assert.equal(loaded(node), false, "a zero-width load is not a successful image");

  node.naturalWidth = 640;
  node.dispatch("load");
  assert.equal(loaded(node), true, "a successful load must reveal its image");

  node.dispatch("error");
  assert.equal(loaded(node), false, "an image error must restore the underlay");
  action.destroy();
});

test("imageReveal resets changed sources and ignores stale source work", async () => {
  const node = new FakeImage("/first.jpg");
  node.complete = true;
  node.naturalWidth = 320;
  const action = imageReveal(node, "/first.jpg");
  await queued();
  assert.equal(loaded(node), true);

  action.update("/second.jpg");
  assert.equal(loaded(node), false, "a source update must hide synchronously");
  node.dispatch("load");
  assert.equal(loaded(node), false, "the previous source cannot reveal a replacement URL");

  node.setAttribute("src", "/second.jpg");
  node.complete = false;
  node.naturalWidth = 0;
  await queued();
  assert.equal(loaded(node), false);

  node.complete = true;
  node.naturalWidth = 480;
  node.dispatch("load");
  assert.equal(loaded(node), true);
  action.destroy();
});

test("imageReveal catches cached images in both source-update orderings", async () => {
  const node = new FakeImage("/cached-a.jpg");
  node.complete = true;
  node.naturalWidth = 300;
  const action = imageReveal(node, "/cached-a.jpg");
  await queued();
  assert.equal(loaded(node), true, "an image cached before action attachment must reveal");

  node.setAttribute("src", "/cached-b.jpg");
  node.naturalWidth = 301;
  action.update("/cached-b.jpg");
  assert.equal(loaded(node), false);
  await queued();
  assert.equal(loaded(node), true, "attribute-before-action updates must reveal cached images");

  action.update("/cached-c.jpg");
  assert.equal(loaded(node), false);
  node.setAttribute("src", "/cached-c.jpg");
  node.naturalWidth = 302;
  await queued();
  assert.equal(loaded(node), true, "action-before-attribute updates must reveal cached images");
  action.destroy();
});

test("imageReveal invalidates queued checks and detaches listeners on destroy", async () => {
  const node = new FakeImage("/cached.jpg");
  node.complete = true;
  node.naturalWidth = 300;
  const action = imageReveal(node, "/cached.jpg");

  assert.equal(node.listenerCount("load"), 1);
  assert.equal(node.listenerCount("error"), 1);
  action.destroy();
  assert.equal(node.listenerCount("load"), 0);
  assert.equal(node.listenerCount("error"), 0);

  await queued();
  assert.equal(loaded(node), false, "destroy must invalidate an already-queued cached check");
  node.dispatch("load");
  assert.equal(loaded(node), false, "detached listeners cannot mutate the destroyed node");
});

test("the global reveal primitive changes only opacity and owns cover geometry", () => {
  const reveal = appCss.match(/\.image-reveal\s*\{([^}]*)\}/)?.[1];
  const loadedRule = appCss.match(/\.image-reveal\.image-loaded\s*\{([^}]*)\}/)?.[1];
  const cover = appCss.match(/\.image-cover\s*\{([^}]*)\}/)?.[1];

  assert.ok(reveal, "app.css must own .image-reveal");
  assert.match(reveal, /opacity\s*:\s*0\s*;/);
  assert.match(reveal, /transition\s*:\s*opacity\s+180ms\s+var\(--ease\)\s*;/);
  assert.equal((reveal.match(/\btransition\b/g) ?? []).length, 1);
  assert.ok(loadedRule, "app.css must own the loaded state");
  assert.match(loadedRule, /opacity\s*:\s*1\s*;/);
  assert.ok(cover, "app.css must own .image-cover geometry");
  for (const declaration of [
    /position\s*:\s*absolute\s*;/,
    /inset\s*:\s*0\s*;/,
    /width\s*:\s*100%\s*;/,
    /height\s*:\s*100%\s*;/,
    /object-fit\s*:\s*cover\s*;/,
  ]) {
    assert.match(cover, declaration);
  }
});

test("every runtime image has async decoding and media art uses the shared reveal contract", () => {
  const images = [];
  for (const [file, source] of sources) {
    for (const match of source.matchAll(/<img\b[\s\S]*?>/g)) images.push({ file, tag: match[0] });
  }

  assert.equal(images.length, 10, "the image inventory changed; classify every new runtime image explicitly");
  for (const image of images) {
    assert.match(image.tag, /\bdecoding=["']async["']/, `${image.file} image lacks async decoding`);
  }

  const qrImages = images.filter(({ file, tag }) =>
    file === "src/routes/+page.svelte" && /alt=["']Plex device-link QR code["']/.test(tag),
  );
  assert.equal(qrImages.length, 1, "the Plex authorization QR must remain the sole functional image");
  const qr = qrImages[0];
  assert.doesNotMatch(qr.tag, /\buse:imageReveal\b|\bimage-reveal\b|\bimage-cover\b/);

  const media = images.filter((image) => image !== qr);
  assert.equal(media.length, 9);
  for (const image of media) {
    assert.match(image.tag, /\buse:imageReveal=\{[^}]+\}/, `${image.file} media lacks the reveal action`);
    assert.match(image.tag, /\bclass=["'][^"']*\bimage-reveal\b[^"']*["']/, `${image.file} media lacks image-reveal`);
    assert.match(image.tag, /\bclass=["'][^"']*\bimage-cover\b[^"']*["']/, `${image.file} media lacks image-cover`);
  }
});

test("media frames keep their fixed underlays and obsolete failure hiding is absent", () => {
  const page = sources.get("src/routes/+page.svelte");
  const item = sources.get("src/lib/ItemDetail.svelte");
  const season = sources.get("src/lib/SeasonDetail.svelte");
  const playlists = sources.get("src/lib/PlaylistsView.svelte");
  const serverPlaylist = sources.get("src/lib/ServerPlaylistView.svelte");

  assert.match(page, /class=["']noart["'][^>]*aria-hidden=["']true["'][^>]*>\{item\.title\}<\/div>[\s\S]{0,220}<img\b/);
  assert.match(page, /class=["']noart["'][^>]*aria-hidden=["']true["'][^>]*>\{it\.grandparentTitle \?\? it\.title\}<\/div>[\s\S]{0,220}<img\b/);
  assert.match(item, /class=["']headshot placeholder["'][^>]*aria-hidden=["']true["'][\s\S]{0,160}<Icon\s+name=["']film["']/);
  assert.match(item, /class=["']backdrop-underlay["'][^>]*aria-hidden=["']true["']/);
  assert.match(item, /class=["']noart["'][^>]*aria-hidden=["']true["'][^>]*>\{title\}<\/div>[\s\S]{0,220}<img\b/);
  assert.match(season, /class=["']epthumb["'][^>]*>[\s\S]{0,160}class=["']noart["'][^>]*aria-hidden=["']true["'][^>]*>\{e\.title\}<\/div>[\s\S]{0,220}<img\b/);
  assert.match(season, /class=["']stillwrap["'][^>]*>[\s\S]{0,160}class=["']still noart["'][^>]*aria-hidden=["']true["'][^>]*>\{selected\.title\}<\/div>[\s\S]{0,220}<img\b/);
  for (const [file, source] of [
    ["src/lib/PlaylistsView.svelte", playlists],
    ["src/lib/ServerPlaylistView.svelte", serverPlaylist],
  ]) {
    assert.match(source, /class=["']thumb["'][^>]*aria-hidden=["']true["'][^>]*>[\s\S]{0,120}<Icon\s+name=["']film["'][\s\S]{0,220}<img\b/, `${file} needs its film-icon underlay`);
  }

  const allSvelte = [...sources.values()].join("\n");
  for (const obsolete of [
    /\bfailedPosters\b/,
    /\bposterFailed\b/,
    /\bstillFailed\b/,
    /onerror\s*=\s*\{[^}]*\.style\.(?:display|visibility)/,
  ]) {
    assert.doesNotMatch(allSvelte, obsolete, `obsolete image-failure behavior remains: ${obsolete}`);
  }
});
