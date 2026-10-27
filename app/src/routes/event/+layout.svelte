<script lang="ts">
	import Sidebar from '#lib/components/Sidebar.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<div class="window">
	<Sidebar />
	<main>
		{@render children()}
	</main>
</div>

<style>
	.window {
		display: flex;
		height: 100vh;
	}

	main {
		flex: 1;
		min-width: 0;
		overflow: auto;
		padding: var(--spacing-10);
	}
</style>
