<script lang="ts">
	// An IBAN, checked by the core as it is typed (R-PER-2): length for the country and the
	// mod-97 checksum. While the text differs from the grouped form, the hint previews it.
	import { untrack } from 'svelte';
	import { api, errorMessage } from '../api';
	import Check from '../motion/Check.svelte';
	import { level } from '../motion/mode.svelte';
	import Field from './Field.svelte';

	let {
		label = 'IBAN',
		value,
		oncommit
	}: {
		label?: string;
		/** Normalised: no spaces, upper case. */
		value: string | null;
		oncommit: (value: string | null) => void;
	} = $props();

	const id = $props.id();
	let text = $state('');
	let checked = $state<{ iban: string; formatted: string } | null>(null);
	let error = $state<string | null>(null);
	let focused = $state(false);
	let seq = 0;

	const group = (iban: string) => iban.replace(/(.{4})/g, '$1 ').trim();

	let synced: string | null | undefined;
	$effect.pre(() => {
		if (value === synced) return;
		synced = value;
		untrack(() => {
			text = value ? group(value) : '';
			checked = value ? { iban: value, formatted: group(value) } : null;
			error = null;
		});
	});

	async function check() {
		const mine = ++seq;
		if (!text.trim()) {
			checked = null;
			error = null;
			return;
		}
		try {
			const r = await api.checkIban(text);
			if (mine !== seq) return;
			checked = r;
			error = null;
		} catch (e) {
			if (mine !== seq) return;
			checked = null;
			error = errorMessage(e);
		}
	}

	const preview = $derived(
		checked && text.trim() && (focused || checked.formatted !== text.trim())
			? checked.formatted
			: null
	);

	async function commit() {
		focused = false;
		await check();
		if (error) return;
		const next = checked?.iban ?? null;
		if (checked) text = checked.formatted;
		if (next !== value) oncommit(next);
	}
</script>

<Field {label} {id} {error}>
	<input
		{id}
		type="text"
		autocapitalize="characters"
		spellcheck="false"
		bind:value={text}
		oninput={check}
		onfocus={() => (focused = true)}
		onblur={commit}
		onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
		aria-invalid={!!error}
		aria-describedby={error ? `${id}-error` : undefined}
	/>
	{#if checked && !error && text.trim()}
		<span class="ok" aria-label="Valid IBAN"><Check level={level('C3')} size={14} /></span>
	{/if}
	{#snippet hint()}
		{#if preview}= {preview}{/if}
	{/snippet}
</Field>

<style>
	.ok {
		display: flex;
		flex: none;
		color: var(--color-plus);
	}
</style>
