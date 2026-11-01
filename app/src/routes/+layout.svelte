<script lang="ts">
	import '#lib/styles/tokens.css';
	import '#lib/styles/theme.css';
	import '#lib/styles/base.css';
	import '#lib/styles/forms.css';
	import { onMount } from 'svelte';
	import { api } from '#lib/api/index.ts';
	import { applyAppearance } from '#lib/appearance.ts';
	import { motion, reduced, watchReducedMotion } from '#lib/motion/mode.svelte.ts';
	import { installShortcuts } from '#lib/shortcuts.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	// The app cannot get here without its backend; a plain browser can, without the dev bridge.
	let unavailable = $state<string | null>(null);

	onMount(() => {
		api
			.settings()
			.then((s) => {
				session.settings = s;
			})
			.catch((e: Error) => {
				unavailable = e.message;
			});
		const stopWatching = watchReducedMotion();
		const removeShortcuts = installShortcuts();
		return () => {
			stopWatching();
			removeShortcuts();
		};
	});

	// How much the app moves: macOS's "reduce motion", unless Settings overrides it. CSS reads
	// it from `data-motion` on the root element.
	$effect(() => {
		motion.setting = session.settings?.reducedMotion ?? null;
	});
	$effect(() => {
		document.documentElement.dataset.motion = reduced() ? 'reduced' : 'full';
	});
	// Light or dark: the system's, unless Settings overrides it. Also sets the window's title bar.
	$effect(() => {
		applyAppearance(session.settings?.appearance ?? 'system');
	});
</script>

{#if session.settings}
	{@render children()}
{:else if unavailable}
	<p class="unavailable">{unavailable}</p>
{/if}

<style>
	.unavailable {
		margin: var(--spacing-10);
	}
</style>
