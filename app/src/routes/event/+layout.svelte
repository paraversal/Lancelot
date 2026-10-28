<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { api } from '#lib/api/index.ts';
	import IntegrityBanner from '#lib/components/IntegrityBanner.svelte';
	import Sidebar from '#lib/components/Sidebar.svelte';
	import { session } from '#lib/state/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	onMount(async () => {
		if (session.event) return;
		// After a reload the backend may still have the event open.
		const current = await api.currentEvent();
		if (current) session.update(current);
		else goto('/', { replaceState: true });
	});

	async function check() {
		await session.change(() => api.checkIntegrity(), true);
	}
</script>

{#if session.event}
	<div class="window">
		<Sidebar event={session.event} />
		<div class="content">
			<IntegrityBanner report={session.event.integrity} oncheck={check} />
			<main>{@render children()}</main>
		</div>
	</div>
{/if}

<style>
	.window {
		display: flex;
		height: 100vh;
	}

	.content {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-width: 0;
	}

	main {
		flex: 1;
		min-height: 0;
		overflow: auto;
		padding: var(--spacing-10);
	}
</style>
