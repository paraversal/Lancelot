<script lang="ts">
	// One assumption as a slider, with the lines that scale with it below: each with its unit price
	// to change and what it comes to. A line that scales with several shows under the first.
	import { formatAmount } from '../../format';
	import type { NumberFormat } from '../../api/generated/NumberFormat';
	import type { AssumptionKnob, LineKnob } from './scenario';

	let {
		knob,
		value,
		lines,
		names,
		format,
		colorOf,
		onvalue,
		onprice
	}: {
		knob: AssumptionKnob;
		value: number;
		/** The lines under this assumption, with their unit price and figure in the scenario. */
		lines: { line: LineKnob; price: number; figure: number }[];
		/** The names a line scales with, `Guests × Days`. */
		names: (line: LineKnob) => string;
		format: NumberFormat;
		colorOf: (category: number) => string;
		onvalue: (value: number) => void;
		onprice: (line: number, price: number) => void;
	} = $props();

	const moved = $derived(value !== knob.value);
</script>

<div class="assumption">
	<div class="head">
		<label class="name" for="assumption-{knob.id}">{knob.name}</label>
		<input
			id="assumption-{knob.id}"
			type="range"
			min="0"
			max={knob.max}
			step="1"
			{value}
			oninput={(e) => onvalue(e.currentTarget.valueAsNumber)}
		/>
		<span class="stepper">
			<button
				type="button"
				disabled={value <= 0}
				aria-label="One fewer {knob.name}"
				onclick={() => onvalue(value - 1)}>−</button
			>
			<span class="value" class:moved data-testid="plugin-what-if-assumption">{value}</span>
			<button
				type="button"
				disabled={value >= knob.max}
				aria-label="One more {knob.name}"
				onclick={() => onvalue(value + 1)}>+</button
			>
		</span>
	</div>
	<span class="label">{moved ? `planned on ${knob.value}` : 'as planned'}</span>

	{#each lines as { line, price, figure } (line.id)}
		<div class="line">
			<span class="title"
				><span class="dot" style:background={colorOf(line.category)}></span>{line.name}</span
			>
			<span class="per">
				{#if line.quantity !== 1}{line.quantity} ×{/if}
				{names(line)} ×
			</span>
			<span class="stepper">
				<button
					type="button"
					disabled={line.fixed || price <= 0}
					aria-label="Lower the unit price of {line.name}"
					onclick={() => onprice(line.id, Math.max(0, price - line.step))}>−</button
				>
				<span class="value price" class:moved={price !== line.unitPrice}
					>{formatAmount(price, format)}</span
				>
				<button
					type="button"
					disabled={line.fixed || price >= line.maxPrice}
					aria-label="Raise the unit price of {line.name}"
					onclick={() => onprice(line.id, Math.min(line.maxPrice, price + line.step))}>+</button
				>
			</span>
			<span
				class="figure"
				class:fixed={line.fixed}
				title={line.fixed ? 'Follows what is booked' : undefined}
				>{line.kind === 'income' ? '+' : '−'}{formatAmount(figure, format)}</span
			>
		</div>
	{/each}
</div>

<style>
	.assumption {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
	}

	.head {
		display: grid;
		grid-template-columns: 120px minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--spacing-4);
	}

	.name {
		font-weight: 600;
	}

	input[type='range'] {
		width: 100%;
		accent-color: var(--color-ink);
	}

	.label {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.line {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto auto 110px;
		align-items: center;
		gap: var(--spacing-3);
		min-height: 36px;
		border-top: 1px solid var(--color-rule);
	}

	.title {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2);
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.dot {
		flex: none;
		width: 8px;
		height: 8px;
		border-radius: 50%;
	}

	.per {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.figure {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}

	.fixed {
		color: var(--color-ink-muted);
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

	.value {
		min-width: 44px;
		text-align: center;
		font-variant-numeric: tabular-nums;
	}

	.price {
		min-width: 72px;
	}

	.moved {
		font-weight: 600;
	}

	@container (width < 560px) {
		.head {
			grid-template-columns: minmax(0, 1fr) auto;
		}

		.head input {
			grid-column: 1 / -1;
			grid-row: 2;
		}

		.line {
			grid-template-columns: minmax(0, 1fr) auto;
		}
	}
</style>
