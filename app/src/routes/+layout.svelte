<script lang="ts">
	import '#lib/styles/tokens.css';
	import '#lib/styles/base.css';
	import { onMount } from 'svelte';
	import { api } from '#lib/api/index.ts';
	import { installShortcuts } from '#lib/shortcuts.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	onMount(() => {
		api.settings().then((s) => {
			session.settings = s;
		});
		return installShortcuts();
	});
</script>

{#if session.settings}
	{@render children()}
{/if}
