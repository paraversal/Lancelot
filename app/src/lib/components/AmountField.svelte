<script lang="ts">
	// The amount input (R-NF-8). Every keystroke is parsed by the core; the field previews the
	// result (`= 1.234,50 €`), in attention when the input was ambiguous, and turns minus with a
	// warning when it cannot be read. Nothing is committed until the input is valid.
	import { untrack } from 'svelte';
	import { api, ApiError } from '../api';
	import type { Cents } from '../api/generated/Cents';
	import type { NumberFormat } from '../api/generated/NumberFormat';
	import { formatAmount } from '../format';
	import Field from './Field.svelte';
	import Icon from './Icon.svelte';

	let {
		label,
		value,
		format,
		allowNegative = false,
		large = false,
		text: initial,
		autofocus = false,
		oninput: onvalue,
		oncommit
	}: {
		label?: string;
		value: Cents | null;
		format: NumberFormat;
		allowNegative?: boolean;
		/** The bigger type of the entry panel's main amount. */
		large?: boolean;
		/** Pre-fills the input, e.g. on the design page. */
		text?: string;
		autofocus?: boolean;
		/** The parsed value after every keystroke (null while empty or invalid). */
		oninput?: (value: Cents | null) => void;
		oncommit?: (value: Cents | null) => void;
	} = $props();

	const id = $props.id();
	let text = $state('');
	let parsed = $state<{ cents: Cents; ambiguous: boolean } | null>(null);
	let error = $state<string | null>(null);
	let seq = 0;

	function canonical(c: Cents | null) {
		return c === null ? '' : formatAmount(c, format, 'plain').replace(/\s?€/, '').replace('€', '');
	}

	// Reset only when the value or format really changed (see TextField).
	let synced: string | undefined;
	$effect.pre(() => {
		const key = `${value}|${format}|${initial}`;
		if (key === synced) return;
		const echo = synced?.split('|')[1] === format && value === untrack(() => parsed?.cents ?? null);
		synced = key;
		// The parent passing back what this field just reported (`oninput`) is not a change.
		if (echo) return;
		text = initial ?? canonical(value);
		parsed = value === null ? null : { cents: value, ambiguous: false };
		error = null;
		if (initial) untrack(parse);
	});

	async function parse() {
		const mine = ++seq;
		if (!text.trim()) {
			parsed = null;
			error = null;
			onvalue?.(null);
			return;
		}
		try {
			const r = await api.parseAmount(text, allowNegative);
			if (mine !== seq) return;
			parsed = r;
			error = null;
			onvalue?.(r.cents);
		} catch (e) {
			if (mine !== seq) return;
			parsed = null;
			error = e instanceof ApiError ? e.message : String(e);
			onvalue?.(null);
		}
	}

	// The preview confirms what was read while typing, even when the text already looks like
	// the result (`1.20` in En); outside the field it shows only when the text differs.
	let focused = $state(false);
	const preview = $derived(
		parsed && text.trim() && (focused || text.trim() !== canonical(parsed.cents))
			? formatAmount(parsed.cents, format)
			: null
	);

	async function commit() {
		await parse();
		if (error) return;
		const next = parsed?.cents ?? null;
		text = canonical(next);
		if (next !== value) oncommit?.(next);
	}
</script>

<Field {label} {id} invalid={!!error}>
	<!-- svelte-ignore a11y_autofocus -->
	<input
		{id}
		type="text"
		inputmode="decimal"
		placeholder={canonical(0)}
		bind:value={text}
		oninput={parse}
		onfocus={() => (focused = true)}
		onblur={() => ((focused = false), commit())}
		onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
		class:large
		{autofocus}
		class:error
		title={error ?? undefined}
		aria-invalid={!!error}
	/>
	{#if error}
		<span class="warn"><Icon name="warn" size={14} /></span>
	{:else if preview}
		<span class="preview" class:ambiguous={parsed?.ambiguous}>= {preview}</span>
	{/if}
</Field>

<style>
	input {
		font-variant-numeric: tabular-nums;
	}

	input:not(:placeholder-shown) {
		font-size: var(--type-md);
		font-weight: 600;
	}

	.large:not(:placeholder-shown) {
		font-size: var(--type-lg);
	}

	input::placeholder {
		color: var(--color-ink-muted);
	}

	.error {
		color: var(--color-minus);
	}

	.warn {
		display: flex;
		color: var(--color-minus);
	}

	.preview {
		flex: none;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
		white-space: nowrap;
	}

	.preview.ambiguous {
		color: var(--color-attention);
	}
</style>
