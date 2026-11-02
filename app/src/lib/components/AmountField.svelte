<script lang="ts" module>
	const reading = new Set<Promise<void>>();

	/**
	 * Resolves once every amount typed so far has been read and reported. Reading takes a round
	 * trip to the core, so a form calls this before saving, or a value typed just before the click
	 * would be lost.
	 */
	export async function amountsRead() {
		while (reading.size) await Promise.all(reading);
	}
</script>

<script lang="ts">
	// The amount input (R-NF-8). Every keystroke is parsed by the core; the field previews the
	// result (`= 1.234,50 €`), in attention when the input was ambiguous, and turns minus with a
	// warning when it cannot be read. Nothing is committed until the input is valid. A sum
	// (`1,29+1,49`) stays as typed so it can be checked; only its total is ever saved.
	import { onDestroy, untrack } from 'svelte';
	import { api, errorMessage } from '../api';
	import type { Cents } from '../api/generated/Cents';
	import type { NumberFormat } from '../api/generated/NumberFormat';
	import type { ParsedSum } from '../api/generated/ParsedSum';
	import { formatAmount, formatPlain } from '../format';
	import Field from './Field.svelte';

	let {
		label,
		value,
		format,
		allowNegative = false,
		large = false,
		previewBelow = false,
		text: initial,
		autofocus = false,
		invalid = $bindable(false),
		oninput: onvalue,
		oncommit
	}: {
		label?: string;
		value: Cents | null;
		format: NumberFormat;
		allowNegative?: boolean;
		/** The bigger type of the entry panel's main amount. */
		large?: boolean;
		/** For narrow fields: the preview goes in the hint line under the field. */
		previewBelow?: boolean;
		/** Pre-fills the input, e.g. on the design page. */
		text?: string;
		autofocus?: boolean;
		/** Whether the input cannot be read. The field shows why; a form must not save. */
		invalid?: boolean;
		/** The parsed value after every keystroke (null while empty or invalid). */
		oninput?: (value: Cents | null) => void;
		oncommit?: (value: Cents | null) => void;
	} = $props();

	const id = $props.id();
	let text = $state('');
	let parsed = $state<ParsedSum | null>(null);
	let error = $state<string | null>(null);
	let seq = 0;
	// A read that returns after the field is gone must not touch the form that replaced it.
	let gone = false;
	onDestroy(() => (gone = true));

	function canonical(c: Cents | null) {
		return c === null ? '' : formatPlain(c, format);
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
		parsed = value === null ? null : { cents: value, ambiguous: false, terms: 1 };
		error = null;
		invalid = false;
		if (initial) untrack(parse);
	});

	function parse() {
		const run = read();
		reading.add(run);
		run.finally(() => reading.delete(run));
		return run;
	}

	async function read() {
		const mine = ++seq;
		if (!text.trim()) {
			parsed = null;
			error = null;
			invalid = false;
			onvalue?.(null);
			return;
		}
		try {
			const r = await api.parseSum(text, allowNegative);
			if (gone || mine !== seq) return;
			parsed = r;
			error = null;
			invalid = false;
			onvalue?.(r.cents);
		} catch (e) {
			if (gone || mine !== seq) return;
			parsed = null;
			error = errorMessage(e);
			invalid = true;
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
		if (gone || error) return;
		const next = parsed?.cents ?? null;
		if (!parsed || parsed.terms === 1) text = canonical(next);
		if (next !== value) oncommit?.(next);
	}
</script>

{#snippet below()}
	{#if !error && preview}<span class:ambiguous={parsed?.ambiguous}>= {preview}</span>{/if}
{/snippet}

<Field {label} {id} {error} hint={previewBelow ? below : undefined}>
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
		aria-invalid={!!error}
		aria-describedby={error ? `${id}-error` : undefined}
	/>
	{#if !error && preview && !previewBelow}
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

	.preview {
		flex: none;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
		white-space: nowrap;
	}

	.ambiguous {
		color: var(--color-attention);
	}
</style>
