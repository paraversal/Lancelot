<script lang="ts">
	// One category as a bar on the scale all categories share: what is booked, what is still to
	// come, and a tick at the forecast. The bar's end is a handle; it stops at what is booked.
	import { formatAmount } from '../../format';
	import type { NumberFormat } from '../../api/generated/NumberFormat';
	import { snapCategory, type CategoryKnob } from './scenario';

	let {
		knob,
		value,
		scale,
		color,
		format,
		onvalue
	}: {
		knob: CategoryKnob;
		value: number;
		/** The figure the whole width stands for. */
		scale: number;
		color: string;
		format: NumberFormat;
		onvalue: (value: number) => void;
	} = $props();

	const moved = $derived(value !== knob.forecast);
	const share = (cents: number) => Math.max(0, Math.min(1, cents / scale));

	function input(e: Event & { currentTarget: HTMLInputElement }) {
		const snapped = snapCategory(knob, e.currentTarget.valueAsNumber);
		// The handle sits where the figure is, also when the figure didn't change.
		e.currentTarget.value = String(snapped);
		if (snapped !== value) onvalue(snapped);
	}
</script>

<div class="row" style:--c={color}>
	<span class="name">{knob.name}</span>
	<span class="track">
		<span class="bar rest" style:transform="scaleX({share(value)})"></span>
		<span class="bar booked" style:transform="scaleX({share(Math.min(knob.actual, value))})"></span>
		<span class="tick" style:left="{share(knob.forecast) * 100}%"></span>
		<input
			type="range"
			min="0"
			max={scale}
			step={knob.step}
			{value}
			aria-label={knob.name}
			aria-valuetext={formatAmount(value, format)}
			oninput={input}
		/>
	</span>
	<span class="figure">
		<span class:moved>{formatAmount(value, format)}</span>
		<span class="label">
			{#if moved}
				{formatAmount(value - knob.forecast, format, 'always')}
			{:else if knob.actual >= knob.forecast}
				all booked
			{:else if knob.actual <= 0}
				nothing booked
			{:else}
				{formatAmount(knob.actual, format)} booked
			{/if}
		</span>
	</span>
</div>

<style>
	.row {
		display: grid;
		grid-template-columns: 110px minmax(0, 1fr) 96px;
		gap: var(--spacing-3);
		align-items: center;
		min-height: 44px;
		border-top: 1px solid var(--color-rule);
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.track {
		position: relative;
		height: 24px;
	}

	.bar {
		position: absolute;
		inset: 4px 0;
		border-radius: 2px;
		transform-origin: left center;
		transition: transform var(--motion-quick) var(--ease-out);
	}

	.rest {
		background: color-mix(in srgb, var(--c) 25%, var(--color-field));
	}

	.booked {
		background: var(--c);
	}

	.tick {
		position: absolute;
		top: 0;
		width: 2px;
		height: 24px;
		margin-left: -1px;
		border-radius: 1px;
		background: var(--color-ink);
	}

	input {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 24px;
		margin: 0;
		background: transparent;
		appearance: none;
		cursor: ew-resize;
	}

	input::-webkit-slider-runnable-track {
		height: 24px;
		background: transparent;
	}

	input::-webkit-slider-thumb {
		width: 8px;
		height: 24px;
		border: 2px solid var(--color-field);
		border-radius: 4px;
		background: var(--color-ink);
		appearance: none;
		transition: transform var(--motion-instant) var(--ease-out);
	}

	input:active::-webkit-slider-thumb {
		transform: scaleY(1.15);
	}

	input::-moz-range-track {
		background: transparent;
	}

	input::-moz-range-thumb {
		width: 4px;
		height: 20px;
		border: 2px solid var(--color-field);
		border-radius: 4px;
		background: var(--color-ink);
	}

	input:focus-visible {
		outline: 2px solid var(--color-ink);
		outline-offset: 2px;
		border-radius: var(--radius-sm);
	}

	.figure {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		line-height: 1.3;
		white-space: nowrap;
	}

	.moved {
		font-weight: 600;
	}

	.label {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}
</style>
