<script lang="ts">
	// The lock of a closed event, whose shackle snaps shut or springs open (E1, E2).
	import { lockShackle } from './effects';
	import type { Level } from './mode.svelte';

	let { closed, level, size = 16 }: { closed: boolean; level: Level; size?: number } = $props();
	let shackle: SVGGElement;
	let shown: boolean | undefined;

	$effect(() => {
		if (shown !== undefined && shown !== closed) lockShackle(shackle, closed, level);
		else if (shown === undefined && !closed)
			shackle.style.transform = 'translateY(-2px) rotate(-22deg)';
		shown = closed;
	});
</script>

<svg
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="1.75"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
>
	<path d="M5 11h14v10H5z" />
	<g bind:this={shackle} style="transform-origin: 16px 11px; transform-box: view-box">
		<path d="M8 11V7a4 4 0 0 1 8 0v4" />
	</g>
</svg>
