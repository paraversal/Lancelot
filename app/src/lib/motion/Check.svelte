<script lang="ts">
	// A check mark that draws itself when it appears (C3, D3, D4, F2).
	import { onMount } from 'svelte';
	import { drawCheck } from './effects';
	import type { Level } from './mode.svelte';

	let { level, size = 16, delay = 0 }: { level: Level; size?: number; delay?: number } = $props();
	let path: SVGPathElement;
	onMount(() => drawCheck(path, level, delay));
</script>

<svg
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="2.25"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
>
	<path bind:this={path} d="M20 6L9 17l-5-5" pathLength="1" stroke-dasharray="1" />
</svg>
