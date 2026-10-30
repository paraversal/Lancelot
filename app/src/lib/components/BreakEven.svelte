<script lang="ts">
	// Break-even (R-TKT-9): the exact gap, about how many tickets close it at the average so far,
	// and what each tier would take alone. Every figure comes from the core.
	import type { NumberFormat } from '../api/generated/NumberFormat';
	import type { Profitability } from '../api/generated/Profitability';
	import type { TierFigures } from '../api/generated/TierFigures';
	import { tierColor } from '../colors';
	import { formatAmount } from '../format';
	import Icon from './Icon.svelte';

	let {
		profitability,
		tiers,
		format,
		onaddtier
	}: {
		profitability: Profitability;
		tiers: TierFigures[];
		format: NumberFormat;
		onaddtier: () => void;
	} = $props();

	const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
	const reached = $derived(profitability.gap === 0);
	const short = $derived(
		profitability.placesLeft !== null &&
			profitability.tickets !== null &&
			profitability.tickets > profitability.placesLeft
	);
	const rows = $derived(
		profitability.tiers.map((row) => ({
			...row,
			index: tiers.findIndex((t) => t.id === row.tier),
			name: tiers.find((t) => t.id === row.tier)?.name ?? ''
		}))
	);
</script>

<section class="break-even" aria-label="Break-even">
	<div class="gap">
		<span class="label">Break-even</span>
		{#if reached}
			<p class="headline">
				<strong class:plus={profitability.projected > 0}
					>{formatAmount(profitability.projected, format, 'always')}</strong
				>
				<span class="label-md">projected</span>
			</p>
			<strong class="line">Reached</strong>
		{:else}
			<p class="headline">
				<strong>{formatAmount(profitability.gap, format)}</strong>
				<span class="label-md">to break even</span>
			</p>
			{#if profitability.tickets !== null}
				<p class="line">
					<strong>~{count(profitability.tickets, 'ticket', 'tickets')}</strong>
					{#if short}
						<span class="attention"
							>· {count(profitability.placesLeft ?? 0, 'place', 'places')} left</span
						>
					{/if}
				</p>
			{:else if !tiers.length}
				<button type="button" class="add" onclick={onaddtier}>
					<Icon name="plus" size={14} />Tier
				</button>
			{/if}
		{/if}
	</div>
	{#if rows.length}
		<div class="tiers">
			{#each rows as row (row.tier)}
				<div class="row">
					<span class="dot" style:background={tierColor(row.index)}></span>
					<span class="name">{row.name}</span>
					<strong>{count(row.tickets, 'ticket', 'tickets')}</strong>
					{#if row.left !== null}
						<span class="left" class:attention={row.left < row.tickets}>{row.left} left</span>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</section>

<style>
	.break-even {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.gap {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--spacing-1);
	}

	p {
		margin: 0;
	}

	.label,
	.name,
	.left {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.label-md {
		font-size: var(--type-sm);
		color: var(--color-ink-muted);
	}

	.headline {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 6px;
	}

	.headline strong {
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.headline .plus {
		color: var(--color-plus);
	}

	.line {
		display: flex;
		gap: 6px;
		font-size: var(--type-sm);
	}

	strong {
		font-weight: 600;
	}

	.attention {
		color: var(--color-attention);
	}

	.add {
		display: flex;
		align-items: center;
		gap: 6px;
		min-height: 32px;
		padding: 0;
		border: none;
		background: none;
		font-size: 13px;
		color: var(--color-ink-muted);
		cursor: pointer;
	}

	.add:hover {
		color: var(--color-ink);
	}

	.tiers {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-1);
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		font-size: var(--type-xs);
	}

	.dot {
		width: 8px;
		height: 8px;
		flex: none;
		border-radius: 50%;
	}

	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.left {
		min-width: 44px;
		text-align: right;
	}

	.left.attention {
		color: var(--color-attention);
	}
</style>
