<script lang="ts">
	// A date as text in the user's format (R-NF-6). Both formats are accepted; while the typed
	// text differs from how the date is shown, the hint previews it (`= 06.12.2026`).
	import { untrack } from 'svelte';
	import { api, ApiError } from '../api';
	import type { NumberFormat } from '../api/generated/NumberFormat';
	import { formatDate } from '../format';
	import Field from './Field.svelte';
	import Icon from './Icon.svelte';

	let {
		label,
		value,
		format,
		required = false,
		oninput: onvalue,
		oncommit
	}: {
		label: string;
		/** ISO `YYYY-MM-DD`. */
		value: string | null;
		format: NumberFormat;
		required?: boolean;
		/** The parsed date after every keystroke (null while empty or invalid). */
		oninput?: (value: string | null) => void;
		oncommit: (value: string | null) => void;
	} = $props();

	const id = $props.id();
	let text = $state('');
	let parsed = $state<string | null>(null);
	let error = $state<string | null>(null);
	let seq = 0;

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
			const r = await api.parseDate(text);
			if (mine !== seq) return;
			parsed = r.date;
			error = null;
			onvalue?.(r.date);
		} catch (e) {
			if (mine !== seq) return;
			parsed = null;
			error = e instanceof ApiError ? e.message : String(e);
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
	{#if error}<span class="warn"><Icon name="warn" size={14} /></span>{/if}
	{#snippet hint()}
		{#if preview}= {preview}{/if}
	{/snippet}
</Field>

<style>
	.error {
		color: var(--color-minus);
	}

	.warn {
		display: flex;
		color: var(--color-minus);
	}
</style>
