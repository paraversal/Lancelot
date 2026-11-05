<script lang="ts">
	// One tier as a row of places: sold, added in the scenario, still free. A click on a free place
	// sells up to it; the steppers do the same one ticket at a time, and change the price of the
	// tickets still to sell.
	import { formatAmount } from '../../format';
	import { isPlayful, type Level } from '../../motion/mode.svelte';
	import type { NumberFormat } from '../../api/generated/NumberFormat';
	import Figure from './Figure.svelte';
	import { netPrice, places, stepOf, type TierKnob } from './scenario';

	let {
		knob,
		color,
		more,
		price,
		format,
		parse,
		level,
		onmore,
		onprice
	}: {
		knob: TierKnob;
		color: string;
		more: number;
		price: number;
		format: NumberFormat;
		/** Reads an amount, as the app does. */
		parse: (text: string) => Promise<number>;
		level: Level;
		onmore: (more: number) => void;
		onprice: (price: number) => void;
	} = $props();

	const layout = $derived(places(knob, more));
	const boxes = $derived([...Array(layout.boxes).keys()]);
	const net = $derived(netPrice(price, knob.feeBp));
	const full = $derived(knob.maxMore === 0);
	const sold = $derived(
		knob.quota === null ? `${knob.places} sold` : `${knob.places} of ${knob.quota} sold`
	);

	// Places that were already added don't fill in again: the wave starts at the first new one.
	let from = $state(0);
	let last = 0;
	$effect.pre(() => {
		const added = layout.added;
		from = Math.min(last, added);
		last = added;
	});

	function pick(box: number) {
		if (box < layout.sold) return;
		const n = box - layout.sold + 1;
		// The last added place again takes it back.
		const count = n === layout.added ? n - 1 : n;
		onmore(Math.min(knob.maxMore, Math.max(0, (layout.sold + count) * layout.per - knob.places)));
	}

	const kind = (box: number) =>
		box < layout.sold
			? 'sold'
			: box < layout.sold + layout.added
				? 'added'
				: knob.quota === null
					? 'guess'
					: 'free';
	const delay = (box: number) => Math.min(360, Math.max(0, box - layout.sold - from) * 14);
</script>

<div class="tier" class:still={level === 'off'} class:springy={isPlayful(level)}>
	<div class="name">
		<span class="title"><span class="dot" style:background={color}></span>{knob.name}</span>
		<span class="label"
			>{sold}{#if layout.per > 1}&ensp;·&ensp;a box is {layout.per} tickets{/if}</span
		>
	</div>

	<div class="places" style:--c={color} aria-hidden="true">
		{#each boxes as box (box)}
			{@const k = kind(box)}
			<button
				type="button"
				tabindex="-1"
				class="box {k}"
				class:gap={box % 10 === 9 && box % 50 !== 49}
				style:--d="{delay(box)}ms"
				disabled={k === 'sold'}
				aria-label={k === 'sold' ? 'Sold' : 'Sell up to here'}
				onclick={() => pick(box)}
			></button>
		{/each}
	</div>

	{#if full}
		<span class="label full">Full</span>
	{:else}
		<div class="controls">
			<div class="steppers">
				<span class="stepper">
					<button
						type="button"
						disabled={more <= 0}
						aria-label="One ticket fewer of {knob.name}"
						onclick={() => onmore(more - 1)}>−</button
					>
					<Figure
						value={more}
						text={'+' + more}
						label="{knob.name} +{more}"
						max={knob.maxMore}
						moved={more > 0}
						testid="plugin-what-if-more"
						onchange={onmore}
					/>
					<button
						type="button"
						disabled={more >= knob.maxMore}
						aria-label="One more ticket of {knob.name}"
						onclick={() => onmore(more + 1)}>+</button
					>
				</span>
				<span class="stepper">
					<button
						type="button"
						disabled={price <= 0}
						aria-label="Lower the price of {knob.name}"
						onclick={() => onprice(Math.max(0, price - stepOf(price)))}>−</button
					>
					<Figure
						value={price}
						text={formatAmount(price, format)}
						label="{knob.name} {formatAmount(price, format)}"
						{format}
						{parse}
						max={knob.maxPrice}
						wide
						moved={price !== knob.price}
						onchange={onprice}
					/>
					<button
						type="button"
						disabled={price >= knob.maxPrice}
						aria-label="Raise the price of {knob.name}"
						onclick={() => onprice(Math.min(knob.maxPrice, price + stepOf(price)))}>+</button
					>
				</span>
			</div>
			<span class="label" class:plus={more > 0}>
				{#if more > 0}
					{more} × {formatAmount(net, format)} = {formatAmount(more * net, format, 'always')}
				{:else if price !== knob.price}
					for the tickets still to sell
				{:else if knob.quota === null}
					no quota
				{:else}
					{knob.maxMore} left
				{/if}
			</span>
		</div>
	{/if}
</div>

<style>
	.tier {
		display: grid;
		grid-template-columns: 140px auto minmax(0, 1fr);
		gap: var(--spacing-3) var(--spacing-6);
		align-items: start;
		padding: var(--spacing-3) 0;
		border-top: 1px solid var(--color-rule);
	}

	.name {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-1);
		min-width: 0;
	}

	.title {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
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

	.plus {
		color: var(--color-plus);
	}

	.places {
		display: flex;
		flex-wrap: wrap;
		gap: 3px 2px;
		width: 516px;
		max-width: 100%;
		padding-top: 2px;
	}

	.box {
		width: 8px;
		height: 14px;
		padding: 0;
		border: 1px solid var(--color-rule);
		border-radius: 2px;
		background: none;
		cursor: pointer;
	}

	.box.gap {
		margin-right: 4px;
	}

	.box.free:hover,
	.box.guess:hover {
		border-color: var(--color-ink);
	}

	.box.guess {
		border-style: dashed;
	}

	.box.sold {
		border-color: var(--c);
		background: var(--c);
		cursor: default;
	}

	.box.added {
		border-color: var(--c);
		background: color-mix(in srgb, var(--c) 30%, var(--color-field));
		animation: fill-in var(--motion-quick) var(--ease-out) var(--d) backwards;
	}

	.springy .box.added {
		animation-duration: 320ms;
		animation-timing-function: var(--spring-bouncy);
	}

	.still .box.added {
		animation: none;
	}

	@keyframes fill-in {
		from {
			transform: scaleY(0.3);
			opacity: 0;
		}
	}

	.full {
		padding-top: 2px;
	}

	.controls {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: var(--spacing-2);
		min-width: 0;
	}

	.steppers {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--spacing-2) var(--spacing-4);
	}

	.stepper {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-1);
	}

	.stepper button {
		width: 28px;
		height: 28px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-field);
		font-size: var(--type-md);
		line-height: 1;
		cursor: pointer;
		transition: transform var(--motion-instant) var(--ease-out);
	}

	.stepper button:hover:not(:disabled) {
		background: var(--color-paper-sunk);
	}

	.stepper button:active:not(:disabled) {
		transform: scale(0.92);
	}

	.stepper button:disabled {
		opacity: 0.4;
		cursor: default;
	}

	@container (width < 820px) {
		.tier {
			grid-template-columns: minmax(0, 1fr) auto;
		}

		.places {
			grid-column: 1 / -1;
			grid-row: 2;
		}
	}
</style>
