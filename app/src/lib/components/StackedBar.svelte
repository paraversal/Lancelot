<script lang="ts">
	// A composition bar (in vs. out, ticket snapshots). Segments are drawn to scale against
	// `max`; labels appear only under segments wide enough to hold them.
	import type { Cents } from '../api/generated/Cents';
	import type { NumberFormat } from '../api/generated/NumberFormat';
	import { formatAmount } from '../format';
	import { growBar } from '../motion/effects';
	import { level } from '../motion/mode.svelte';

	let {
		segments,
		max,
		format,
		width = 760
	}: {
		segments: { label: string; value: Cents; color: string }[];
		max: Cents;
		format: NumberFormat;
		width?: number;
	} = $props();

	const LABEL_MIN = 56;
	const sized = $derived(
		segments.map((s) => ({
			...s,
			w: Math.max(2, Math.round((width * s.value) / Math.max(max, 1)))
		}))
	);
</script>

<div class="stack" style:width="{width}px">
	<div class="bar">
		{#each sized as s, i (s.label)}
			<span
				style:width="{s.w}px"
				style:background={s.color}
				title="{s.label} {formatAmount(s.value, format)}"
				{@attach (node) => growBar(node, level('D2'), i)}
			></span>
		{/each}
	</div>
	<div class="legend">
		{#each sized as s (s.label)}
			<span style:width="{s.w}px">
				{#if s.w >= LABEL_MIN}
					<span class="name">{s.label}</span><strong>{formatAmount(s.value, format)}</strong>
				{/if}
			</span>
		{/each}
	</div>
</div>

<style>
	.stack {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.bar,
	.legend {
		display: flex;
		gap: 2px;
	}

	.bar span {
		height: 14px;
		transform-origin: left center;
	}

	.legend > span {
		display: flex;
		flex-direction: column;
		gap: 2px;
		font-size: var(--type-xs);
		overflow: hidden;
		white-space: nowrap;
	}

	.name {
		color: var(--color-ink-muted);
	}

	strong {
		font-weight: 600;
	}
</style>
