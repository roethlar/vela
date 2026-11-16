// Read by @sveltejs/vite-plugin-svelte and svelte-check. Vela has no SvelteKit;
// see vite.config.js for the app shell.
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import("@sveltejs/vite-plugin-svelte").SvelteConfig} */
export default {
  preprocess: vitePreprocess(),
};
