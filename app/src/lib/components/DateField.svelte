<script lang="ts">
	// A date as text in the user's format (R-NF-6). Both formats are accepted; while the typed
	// text differs from how the date is shown, the hint previews it (`= 06.12.2026`).
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
		oncommit
	}: {
		label: string;
		/** ISO `YYYY-MM-DD`. */
		value: string | null;
		format: NumberFormat;
		required?: boolean;
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
		synced = key;
		text = value ? formatDate(value, format) : '';
		parsed = value;
		error = null;
	});

	async function parse() {
		const mine = ++seq;
		if (!text.trim()) {
			parsed = null;
			error = null;
			return;
		}
		try {
			const r = await api.parseDate(text);
			if (mine !== seq) return;
			parsed = r.date;
			error = null;
		} catch (e) {
			if (mine !== seq) return;
			parsed = null;
			error = e instanceof ApiError ? e.message : String(e);
		}
	}

	const shown = $derived(parsed ? formatDate(parsed, format) : null);
	const preview = $derived(shown && shown !== text.trim() ? shown : null);

	async function commit() {
		await parse();
		if (error || (required && !parsed)) {
			text = value ? formatDate(value, format) : '';
			error = null;
			return;
		}
		text = shown ?? '';
		if (parsed !== value) oncommit(parsed);
	}
</script>

<Field {label} {id} invalid={!!error}>
	<input
		{id}
		type="text"
		inputmode="numeric"
		bind:value={text}
		oninput={parse}
		onblur={commit}
		onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
		class:error
		title={error ?? undefined}
		aria-invalid={!!error}
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
