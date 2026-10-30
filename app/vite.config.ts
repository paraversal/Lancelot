import { loadEnv } from 'vite';
import { defineConfig } from 'vitest/config';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { svelteTesting } from '@testing-library/svelte/vite';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// SPA inside the Tauri webview: no SSR, everything falls back to index.html.
			adapter: adapter({ fallback: 'index.html' })
		})
	],
	// Tauri expects a fixed port and its own console output.
	clearScreen: false,
	server: {
		port: 5173,
		strictPort: true,
		// A plain browser talks to the dev bridge (crates/lancelot-bridge); see lib/api/dev.ts.
		proxy: {
			'/__bridge': {
				target: `http://127.0.0.1:${loadEnv('', '.', 'LANCELOT_').LANCELOT_BRIDGE_PORT ?? 5174}`,
				rewrite: (path) => path.replace(/^\/__bridge/, '')
			}
		}
	},
	test: {
		expect: { requireAssertions: true },
		passWithNoTests: true,
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'server',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			},
			{
				// Component tests: `*.svelte.test.ts`, rendered in jsdom.
				extends: './vite.config.ts',
				plugins: [svelteTesting()],
				test: {
					name: 'client',
					environment: 'jsdom',
					include: ['src/**/*.svelte.{test,spec}.{js,ts}'],
					setupFiles: ['src/test-setup.ts']
				}
			}
		]
	}
});
