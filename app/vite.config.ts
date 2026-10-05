import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

export default defineConfig({
  // Where the site is served from: `/` locally, `/<repo>/` on GitHub Pages (scripts/publish.mjs).
  base: process.env.WS_BASE ?? '/',
  plugins: [svelte()],
  worker: { format: 'es' },
  // Two pages: the map (the DM's at a table) and the players' window.
  build: { target: 'es2022', rollupOptions: { input: { main: fileURLToPath(new URL('./index.html', import.meta.url)), player: fileURLToPath(new URL('./player.html', import.meta.url)) } } },
  server: { port: 5173 },
});
