<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { closeFile } from '#lib/actions.ts';
	import { pageIn } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import { api } from '#lib/api/index.ts';
	import IntegrityBanner from '#lib/components/IntegrityBanner.svelte';
	import Sidebar from '#lib/components/Sidebar.svelte';
	import { dataSources } from '#lib/plugins/sources.svelte.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	onMount(async () => {
		dataSources.load();
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
		<Sidebar event={session.event} onclose={() => closeFile()} />
		<div class="content">
			<IntegrityBanner report={session.event.integrity} oncheck={check} />
			<main>
				<!-- Every page arrives (A1, F1 when the event opens). -->
				{#key page.url.pathname}
					<div class="page" in:pageIn|global={{ level: level('A1') }}>{@render children()}</div>
				{/key}
			</main>
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

	/* Pages bring their own padding, so the ledger's panel can run full height. */
	main {
		flex: 1;
		min-height: 0;
		overflow: auto;
	}

	.page {
		height: 100%;
	}

	@media print {
		.page {
			height: auto;
		}
	}
</style>
