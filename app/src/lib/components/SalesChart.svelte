<script lang="ts">
	// Sales per import (R-TKT-8), one stacked column per point of the core's sales over time, in
	// tier colours. Tickets shows places against the quota with the cancelled on top; revenue shows
	// net income. The figures of each column are in its tooltip.
	import type { NumberFormat } from '../api/generated/NumberFormat';
	import type { SalesPoint } from '../api/generated/SalesPoint';
	import type { TierFigures } from '../api/generated/TierFigures';
	import { tierColor } from '../colors';
	import { formatAmount } from '../format';
	import { growColumn } from '../motion/effects';
	import { level } from '../motion/mode.svelte';
	import { labelStep, salesBars, shortDay, totalQuota, type SalesView } from '../tickets';
	import Segmented from './Segmented.svelte';

	let {
		points,
		tiers,
		format
	}: { points: SalesPoint[]; tiers: TierFigures[]; format: NumberFormat } = $props();

	const HEIGHT = 200;
	const TOP = 22;
	const BOTTOM = 22;
	const BAR = 28;
	const GAP = 1;

	let view = $state<SalesView>('tickets');
	let width = $state(355);

	const bars = $derived(salesBars(points, view));
	const quota = $derived(view === 'tickets' ? totalQuota(tiers) : null);
	const scale = $derived(Math.max(1, quota ?? 0, ...bars.map((b) => b.size)));
	const slot = $derived(width / Math.max(1, bars.length));
	const barWidth = $derived(Math.max(3, Math.min(BAR, slot * 0.5)));
	const step = $derived(labelStep(bars.length, width));
	const hasCancelled = $derived(bars.some((b) => b.cancelled > 0));
	const floor = HEIGHT - BOTTOM;

	const px = (value: number) => (value / scale) * (floor - TOP);

	/** The column from the bottom up: each piece with its colour, at least a pixel high. */
	function pieces(bar: (typeof bars)[number]) {
		let y = floor;
		const stack: { y: number; h: number; color: string }[] = [];
		const add = (value: number, color: string, least: number) => {
			if (value <= 0) return;
			const h = Math.max(least, px(value));
			y -= h;
			stack.push({ y, h: Math.max(0, h - GAP), color });
		};
		for (const s of bar.segments) add(s.value, tierColor(s.tier), 1);
		add(bar.cancelled, 'var(--color-on-ink-muted)', 2);
		return { stack, top: y };
	}

	function tooltip({ point }: (typeof bars)[number]) {
		const { total } = point;
		return [
			shortDay(point.date),
			`${total.places} places · ${total.sold} sold · ${total.sold - total.places} cancelled`,
			`Gross ${formatAmount(total.gross, format)}`,
			`Fees ${formatAmount(-total.fees, format)}`,
			`Refunds ${formatAmount(-total.refunds, format)}`,
			`Net ${formatAmount(total.net, format)}`
		].join('\n');
	}

	const label = (bar: (typeof bars)[number]) =>
		view === 'tickets' ? String(bar.value) : formatAmount(bar.value, format);
</script>

<section class="sales" aria-label="Sales">
	<header>
		<h2>Sales</h2>
		<Segmented
			options={[
				{ value: 'tickets', label: 'Tickets' },
				{ value: 'revenue', label: 'Revenue' }
			]}
			bind:value={view}
			label="Sales by"
		/>
	</header>
	<div class="legend">
		{#each tiers as t, i (t.id)}
			<span><i style:background={tierColor(i)}></i>{t.name}</span>
		{/each}
		{#if view === 'tickets' && hasCancelled}
			<span><i style:background="var(--color-on-ink-muted)"></i>Cancelled</span>
		{/if}
	</div>
	<div class="plot" bind:clientWidth={width}>
		<svg {width} height={HEIGHT} role="img" aria-label="Sales per import">
			{#if quota !== null}
				<text x="0" y={TOP + 4} class="muted">{quota}</text>
				<line x1={String(quota).length * 7 + 6} x2={width} y1={TOP} y2={TOP} />
			{/if}
			{#key view}
				{#each bars as bar, i (`${bar.point.date}|${i}`)}
					{@const { stack, top } = pieces(bar)}
					{@const x = slot * i + slot / 2}
					{@const last = i === bars.length - 1}
					<g>
						<title>{tooltip(bar)}</title>
						<rect class="hit" x={slot * i} y="0" width={slot} height={HEIGHT} />
						<g
							class="column"
							{@attach (node: SVGGElement) => growColumn(node, level('D2'), i, bars.length)}
						>
							{#each stack as piece, p (p)}
								<rect
									x={x - barWidth / 2}
									y={piece.y}
									width={barWidth}
									height={piece.h}
									fill={piece.color}
								/>
							{/each}
						</g>
						<!-- What is wider than its column ends at the edge. -->
						{#if last || (view === 'tickets' && slot >= 28)}
							{@const atEdge = last && (view === 'revenue' || slot < 28)}
							<text
								x={atEdge ? width : x}
								y={top - 6}
								text-anchor={atEdge ? 'end' : 'middle'}
								class:muted={!last}
								class:strong={last}>{label(bar)}</text
							>
						{/if}
						{#if (bars.length - 1 - i) % step === 0}
							{@const atEdge = last && slot < 48}
							<text
								x={atEdge ? width : x}
								y={HEIGHT - 4}
								text-anchor={atEdge ? 'end' : 'middle'}
								class="muted">{shortDay(bar.point.date)}</text
							>
						{/if}
					</g>
				{/each}
			{/key}
		</svg>
	</div>
</section>

<style>
	.sales {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-3);
		min-width: 0;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-3);
	}

	h2 {
		margin: 0;
		font-size: var(--type-sm);
		font-weight: 600;
	}

	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: 4px var(--spacing-3);
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.legend span {
		display: flex;
		align-items: center;
		gap: var(--spacing-1);
	}

	.legend i {
		width: 8px;
		height: 8px;
		border-radius: 50%;
	}

	.plot {
		min-width: 0;
	}

	svg {
		display: block;
	}

	text {
		font-size: var(--type-xs);
		fill: var(--color-ink);
	}

	text.muted {
		fill: var(--color-ink-muted);
	}

	text.strong {
		font-weight: 600;
	}

	line {
		stroke: var(--color-rule);
	}

	.hit {
		fill: transparent;
	}

	.column {
		transform-box: fill-box;
		transform-origin: bottom;
	}
</style>
