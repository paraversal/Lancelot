<script lang="ts">
	// An amount that moves to its new value instead of jumping (A5). Regular slides the new
	// figure in from the direction it moved; playful rolls the digits that changed, like an
	// odometer. With `countUp` it counts up from zero once when it first appears (D1).
	import { onMount, untrack } from 'svelte';
	import type { Cents } from '../api/generated/Cents';
	import type { NumberFormat } from '../api/generated/NumberFormat';
	import type { SignStyle } from '../api/generated/SignStyle';
	import { formatAmount } from '../format';
	import { easing } from './easing';
	import { isPlayful, type Level } from './mode.svelte';

	let {
		value,
		format,
		sign = 'plain',
		level,
		countUp = false
	}: {
		value: Cents;
		format: NumberFormat;
		sign?: SignStyle;
		level: Level;
		countUp?: boolean;
	} = $props();

	// Read once: whether this amount starts at zero and counts up.
	let shown = $state(untrack(() => (countUp && isPlayful(level) ? 0 : value)));
	let counting = false;
	const text = $derived(formatAmount(shown, format, sign));
	let root: HTMLSpanElement;
	let spans: HTMLSpanElement[] = $state([]);
	let last = { text: '', value: 0 };

	onMount(() => {
		if (shown === value) return;
		counting = true;
		const start = performance.now();
		const to = value;
		const step = (now: number) => {
			const t = Math.min(1, (now - start) / 700);
			shown = Math.round(to * (1 - Math.pow(1 - t, 3)));
			if (t < 1) requestAnimationFrame(step);
			else counting = false;
		};
		requestAnimationFrame(step);
	});

	// Follow the value after the first appearance.
	$effect(() => {
		if (!countUp || last.text) shown = value;
	});

	$effect(() => {
		const now = text;
		if (!last.text) return void (last = { text: now, value: shown });
		if (now === last.text) return;
		if (counting) return void (last = { text: now, value: shown });
		const up = shown > last.value;
		const before = [...last.text];
		last = { text: now, value: shown };
		if (level === 'off') return;
		if (level === 'calm') {
			root.animate([{ opacity: 0.2 }, { opacity: 1 }], { duration: 120 });
			return;
		}
		if (!isPlayful(level)) {
			root.animate(
				[{ transform: `translateY(${up ? 0.45 : -0.45}em)`, opacity: 0 }, { transform: 'none' }],
				{ duration: 180, easing: easing('ease-out') }
			);
			return;
		}
		const chars = [...now];
		const shift = chars.length - before.length;
		chars.forEach((c, i) => {
			if (before[i - shift] === c) return;
			spans[i]?.animate(
				[{ transform: `translateY(${up ? 0.9 : -0.9}em)`, opacity: 0 }, { transform: 'none' }],
				{
					duration: 320,
					delay: (chars.length - 1 - i) * 22,
					easing: easing('spring-soft'),
					fill: 'backwards'
				}
			);
		});
	});
</script>

<span class="amount" bind:this={root} aria-label={formatAmount(value, format, sign)}
	>{#each text as c, i (i)}<span bind:this={spans[i]} aria-hidden="true">{c}</span>{/each}</span
>

<style>
	.amount {
		display: inline-flex;
		white-space: pre;
		clip-path: inset(-0.1em -0.2em);
	}

	.amount > span {
		display: inline-block;
	}
</style>
