<script lang="ts">
	// One slider with what it is set to. The value is in steps; the text says what that means.
	import type { Range } from './scenario';

	let {
		label,
		text,
		range,
		value,
		oninput
	}: {
		label: string;
		text: string;
		range: Range;
		value: number;
		oninput: (value: number) => void;
	} = $props();
</script>

<label class="knob" class:moved={value !== 0}>
	<span class="label">{label}</span>
	<input
		type="range"
		min={range.min}
		max={range.max}
		step="1"
		{value}
		disabled={range.min === range.max}
		aria-valuetext={text}
		oninput={(e) => oninput(e.currentTarget.valueAsNumber)}
	/>
	<span class="text">{text}</span>
</label>

<style>
	.knob {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr) 88px;
		align-items: center;
		gap: var(--spacing-3);
		min-height: 32px;
	}

	.label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	input {
		width: 100%;
		margin: 0;
		accent-color: var(--color-ink);
		cursor: pointer;
	}

	input:disabled {
		cursor: default;
		opacity: 0.4;
	}

	.text {
		text-align: right;
		white-space: nowrap;
		color: var(--color-ink-muted);
	}

	.moved .text {
		font-weight: 600;
		color: var(--color-ink);
	}
</style>
