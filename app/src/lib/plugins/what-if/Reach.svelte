<script lang="ts">
	// Working back from a target: how far the scenario is from it, and the ways that would get
	// there. Applying one adds it to the scenario.
	import { flip } from 'svelte/animate';
	import { fade } from 'svelte/transition';
	import AmountField from '../../components/AmountField.svelte';
	import Button from '../../components/Button.svelte';
	import Chip from '../../components/Chip.svelte';
	import { formatAmount } from '../../format';
	import { flipOptions, riseIn } from '../../motion/effects';
	import { level } from '../../motion/mode.svelte';
	import MovingAmount from '../../motion/MovingAmount.svelte';
	import type { NumberFormat } from '../../api/generated/NumberFormat';
	import type { Suggestion } from './scenario';

	let {
		target = $bindable(),
		planned,
		gap,
		suggestions,
		format,
		colorOf,
		onapply
	}: {
		target: number | null;
		planned: number;
		/** From the scenario's result to the target. */
		gap: number;
		suggestions: Suggestion[];
		format: NumberFormat;
		colorOf: (s: Suggestion) => string;
		onapply: (s: Suggestion) => void;
	} = $props();

	const fmt = (cents: number, sign: 'plain' | 'always' = 'plain') =>
		formatAmount(cents, format, sign);
	const key = (s: Suggestion) => `${s.kind}:${s.kind === 'cut' ? s.category.id : s.tier.id}`;

	function title(s: Suggestion) {
		if (s.kind === 'tickets')
			return s.covers
				? `Sell ${s.count} more ${s.tier.name}`
				: `Sell the ${s.count} ${s.tier.name} left`;
		if (s.kind === 'price') return `Sell ${s.count} more ${s.tier.name} at ${fmt(s.price)}`;
		return `${s.category.name} down by ${fmt(s.by)}`;
	}

	function detail(s: Suggestion) {
		if (!s.covers) return `covers ${fmt(s.gain)} of ${fmt(gap)}`;
		if (s.kind === 'tickets') {
			const sum = `${s.count} × ${fmt(s.net)} = ${fmt(s.gain, 'always')}`;
			return s.tier.quota === null ? sum : `${sum} · ${s.leftAfter} left after`;
		}
		if (s.kind === 'price') return fmt(s.gain, 'always');
		return `to ${fmt(s.to)} · ${fmt(s.category.actual)} booked`;
	}
</script>

<div class="reach">
	<div class="target">
		<div class="field">
			<AmountField
				label="Target result"
				value={target}
				{format}
				allowNegative
				oninput={(v) => (target = v)}
			/>
		</div>
		<span class="presets">
			<Chip label="Break-even" active={target === 0} onclick={() => (target = 0)} />
			<Chip label="Planned" active={target === planned} onclick={() => (target = planned)} />
		</span>
	</div>

	{#if target !== null}
		<div class="gap" data-testid="plugin-what-if-gap">
			{#if gap > 0}
				<span class="attention"><MovingAmount value={gap} {format} level={level('A5')} /></span>
				<span class="label">to go</span>
			{:else}
				<span class="plus">Reached</span>
				<span class="label">{fmt(-gap)} to spare</span>
			{/if}
		</div>
	{/if}

	{#if suggestions.length}
		<ul>
			{#each suggestions as s, i (key(s))}
				<li
					animate:flip={flipOptions(level('B3'))}
					in:riseIn={{ level: level('D1'), i }}
					out:fade={{ duration: 90 }}
				>
					<span class="what">
						<span class="title"
							><span class="dot" style:background={colorOf(s)}></span>{title(s)}</span
						>
						<span class="label" class:attention={!s.covers}>{detail(s)}</span>
					</span>
					<Button size="sm" onclick={() => onapply(s)}>Apply</Button>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.reach {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-4);
	}

	.target {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		justify-content: space-between;
		gap: var(--spacing-3) var(--spacing-6);
	}

	.field {
		width: 240px;
	}

	.presets {
		display: flex;
		gap: var(--spacing-2);
		padding-bottom: 4px;
	}

	.gap {
		display: flex;
		align-items: baseline;
		gap: var(--spacing-2);
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.gap .label {
		font-weight: 400;
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	li {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: var(--spacing-4);
		align-items: center;
		padding: var(--spacing-3) 0;
		border-top: 1px solid var(--color-rule);
	}

	.what {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.title {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		font-weight: 600;
	}

	.what .label {
		padding-left: 16px;
	}

	.dot {
		flex: none;
		width: 8px;
		height: 8px;
		border-radius: 50%;
	}

	.label {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.attention,
	.label.attention {
		color: var(--color-attention);
	}

	.plus {
		color: var(--color-plus);
	}
</style>
