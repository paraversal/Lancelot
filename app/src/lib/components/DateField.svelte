<script lang="ts" module>
	const reading = new Set<Promise<void>>();

	/** As `amountsRead`: a form calls this before saving, or a date just typed would be lost. */
	export async function datesRead() {
		while (reading.size) await Promise.all(reading);
	}
</script>

<script lang="ts">
	// A date as text in the user's format (R-NF-6). Both formats are accepted; while the typed
	// text differs from how the date is shown, the hint previews it (`= 06.12.2026`).
	import { onDestroy, untrack } from 'svelte';
	import { api, errorMessage } from '../api';
	import type { NumberFormat } from '../api/generated/NumberFormat';
	import { formatDate } from '../format';
	import Field from './Field.svelte';

	let {
		label,
		value,
		format,
		required = false,
		invalid = $bindable(false),
		oninput: onvalue,
		oncommit
	}: {
		label: string;
		/** ISO `YYYY-MM-DD`. */
		value: string | null;
		format: NumberFormat;
		required?: boolean;
		/** Whether the input cannot be read. The field shows why; a form must not save. */
		invalid?: boolean;
		/** The parsed date after every keystroke (null while empty or invalid). */
		oninput?: (value: string | null) => void;
		oncommit: (value: string | null) => void;
	} = $props();

	const id = $props.id();
	let text = $state('');
	let parsed = $state<string | null>(null);
	let error = $state<string | null>(null);
	let seq = 0;
	// A read that returns after the field is gone must not touch the form that replaced it.
	let gone = false;
	onDestroy(() => (gone = true));

	// Reset only when the value or format really changed (see TextField).
	let synced: string | undefined;
	$effect.pre(() => {
		const key = `${value}|${format}`;
		if (key === synced) return;
		const echo = synced?.split('|')[1] === format && value === untrack(() => parsed);
		synced = key;
		// The parent passing back what this field just reported (`oninput`) is not a change.
		if (echo) return;
		text = value ? formatDate(value, format) : '';
		parsed = value;
		error = null;
		invalid = false;
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
			const r = await api.parseDate(text);
			if (gone || mine !== seq) return;
			parsed = r.date;
			error = null;
			invalid = false;
			onvalue?.(r.date);
		} catch (e) {
			if (gone || mine !== seq) return;
			parsed = null;
			error = errorMessage(e);
			invalid = true;
			onvalue?.(null);
		}
	}

	const shown = $derived(parsed ? formatDate(parsed, format) : null);
	// As in AmountField: shown while typing, otherwise only when the text differs.
	let focused = $state(false);
	const preview = $derived(
		shown && text.trim() && (focused || shown !== text.trim()) ? shown : null
	);

	async function commit() {
		await parse();
		if (gone) return;
		// Invalid input stays, with its message, until it is fixed; nothing is committed.
		if (error) return;
		if (required && !parsed) {
			text = value ? formatDate(value, format) : '';
			return;
		}
		text = shown ?? '';
		if (parsed !== value) oncommit(parsed);
	}
</script>

<Field {label} {id} {error}>
	<input
		{id}
		type="text"
		inputmode="numeric"
		bind:value={text}
		oninput={parse}
		onfocus={() => (focused = true)}
		onblur={() => ((focused = false), commit())}
		onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
		class:error
		aria-invalid={!!error}
		aria-describedby={error ? `${id}-error` : undefined}
	/>
	{#snippet hint()}
		{#if preview}= {preview}{/if}
	{/snippet}
</Field>

<style>
	.error {
		color: var(--color-minus);
	}
</style>
