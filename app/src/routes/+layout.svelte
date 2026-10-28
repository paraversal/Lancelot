<script lang="ts">
	import '#lib/styles/tokens.css';
	import '#lib/styles/base.css';
	import { onMount } from 'svelte';
	import { api } from '#lib/api/index.ts';
	import { effectiveMotion, motion, watchReducedMotion } from '#lib/motion/mode.svelte.ts';
	import { installShortcuts } from '#lib/shortcuts.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	onMount(() => {
		api.settings().then((s) => {
			session.settings = s;
		});
		const stopWatching = watchReducedMotion();
		const removeShortcuts = installShortcuts();
		return () => {
			stopWatching();
			removeShortcuts();
		};
	});

	// How much the app moves: the setting, calmed by macOS's "reduce motion". CSS reads it from
	// `data-motion` on the root element.
	$effect(() => {
		motion.setting = session.settings?.motion ?? 'full';
	});
	$effect(() => {
		document.documentElement.dataset.motion = effectiveMotion();
	});
</script>

{#if session.settings}
	{@render children()}
{/if}
