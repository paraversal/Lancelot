<script lang="ts" module>
	export type Part = 'tickets' | 'income' | 'expenses';
</script>

<script lang="ts">
	// How the forecast becomes the scenario's result: the forecast, each kind of change as a bar
	// that starts where the one before it ended, and the result, against a line at zero.
	import { formatAmount } from '../../format';
	import { growBar } from '../../motion/effects';
	import { isPlayful, type Level } from '../../motion/mode.svelte';
	import MovingAmount from '../../motion/MovingAmount.svelte';
	import type { NumberFormat } from '../../api/generated/NumberFormat';
	import type { Bridge } from './scenario';

	let {
		figures,
		format,
		level,
		amounts,
		onpick
	}: {
		figures: Bridge;
		format: NumberFormat;
		/** How the bars move (D2). */
		level: Level;
		/** How the totals move (A5). */
		amounts: Level;
		onpick: (part: Part) => void;
	} = $props();

	type Row = {
		label: string;
		from: number;
		to: number;
		part?: Part;
		total?: 'forecast' | 'result';
	};

	const rows = $derived.by(() => {
		const f = figures;
		const out: Row[] = [{ label: 'Forecast', from: 0, to: f.forecast, total: 'forecast' }];
		let at = f.forecast;
		const steps: [string, number, Part][] = [
			['Tickets', f.tickets, 'tickets'],
			['Prices', f.prices, 'tickets'],
			['Income', f.income, 'income'],
			['Expenses', f.expenses, 'expenses']
		];
		for (const [label, change, part] of steps) {
			out.push({ label, from: at, to: at + change, part });
			at += change;
		}
		out.push({ label: 'What if', from: 0, to: f.result, total: 'result' });
		return out;
	});

	const span = $derived.by(() => {
		const ends = [0, ...rows.flatMap((r) => [r.from, r.to])];
		let lo = Math.min(...ends);
		let hi = Math.max(...ends);
		const pad = (hi - lo || 1) * 0.03;
		lo -= lo < 0 ? pad : 0;
		hi += pad;
		return { lo, width: hi - lo || 1 };
	});
	const at = (cents: number) => (cents - span.lo) / span.width;

	const fill = (r: Row) => {
		const change = r.to - r.from;
		if (r.total === 'forecast') return 'var(--color-rule)';
		if (r.total === 'result') return r.to < 0 ? 'var(--color-minus)' : 'var(--color-plus)';
		if (change > 0) return 'var(--color-plus)';
		return change < 0 ? 'var(--color-minus)' : 'var(--color-ink-muted)';
	};
	const tone = (cents: number) => (cents > 0 ? 'plus' : cents < 0 ? 'minus' : 'none');
</script>

{#snippet bar(r: Row)}
	{@const a = Math.min(r.from, r.to)}
	{@const w = Math.max(at(Math.max(r.from, r.to)) - at(a), 0.002)}
	<span class="track" aria-hidden="true">
		<span class="zero" style:transform="translateX({at(0) * 100}cqw)"></span>
		<span class="bar" style:transform="translateX({at(a) * 100}%) scaleX({w})">
			<span class="fill" style:background={fill(r)} use:growBar={level}></span>
		</span>
	</span>
{/snippet}

<div class="bridge" class:springy={isPlayful(level)} class:still={level === 'off'}>
	{#each rows as r (r.label)}
		{#if r.total}
			<div class="row" class:result={r.total === 'result'}>
				<span>{r.label}</span>
				{@render bar(r)}
				<span
					class="amount {r.total === 'result' ? tone(r.to) : 'muted'}"
					data-testid={r.total === 'result' ? 'plugin-what-if-bridge-result' : undefined}
					><MovingAmount value={r.to} {format} sign="always" level={amounts} /></span
				>
			</div>
		{:else}
			{@const change = r.to - r.from}
			<button type="button" class="row link" onclick={() => r.part && onpick(r.part)}>
				<span>{r.label}</span>
				{@render bar(r)}
				<span class="amount {tone(change)}"
					>{change ? formatAmount(change, format, 'always') : '–'}</span
				>
			</button>
		{/if}
	{/each}
</div>

<style>
	.bridge {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.row {
		display: grid;
		grid-template-columns: 88px minmax(0, 1fr) 120px;
		gap: var(--spacing-3);
		align-items: center;
		height: 30px;
		margin: 0 calc(-1 * var(--spacing-2));
		padding: 0 var(--spacing-2);
		border: none;
		border-radius: var(--radius-field);
		background: none;
		text-align: left;
	}

	.link {
		cursor: pointer;
	}

	.link:hover {
		background: var(--color-paper);
	}

	.result {
		height: 38px;
		margin-top: var(--spacing-1);
		border-top: 1px solid var(--color-rule);
		border-radius: 0;
		font-weight: 600;
	}

	.track {
		position: relative;
		height: 16px;
		container-type: inline-size;
	}

	.zero {
		position: absolute;
		top: -8px;
		bottom: -8px;
		left: 0;
		width: 1px;
		background: var(--color-ink);
		transition: transform var(--motion-normal) var(--ease-out);
	}

	.bar {
		position: absolute;
		inset: 0;
		transform-origin: left center;
		transition: transform var(--motion-normal) var(--ease-out);
	}

	.springy .bar {
		transition-duration: var(--motion-slow);
		transition-timing-function: var(--spring-soft);
	}

	.still .bar,
	.still .zero {
		transition: none;
	}

	.fill {
		display: block;
		height: 100%;
		border-radius: 2px;
		transform-origin: left center;
	}

	.amount {
		text-align: right;
		white-space: nowrap;
	}

	.result .amount {
		font-size: var(--type-md);
	}

	.plus {
		color: var(--color-plus);
	}

	.minus {
		color: var(--color-minus);
	}

	.muted,
	.none {
		color: var(--color-ink-muted);
	}
</style>
