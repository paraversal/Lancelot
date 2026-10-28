<script lang="ts">
	// A select whose last option adds a new entry on the spot: picking it turns the select into
	// a text field with an Add button (Enter adds, Esc goes back). Used for the payer (a new
	// person) and for split lines (a new category).
	import type { AccountId } from '../api/generated/AccountId';
	import Field from './Field.svelte';
	import Icon from './Icon.svelte';

	let {
		label,
		value,
		options,
		addLabel,
		invalid = false,
		error = null,
		hint,
		onchange,
		onadd
	}: {
		label?: string;
		value: AccountId | null;
		options: { value: AccountId; label: string; group?: string }[];
		/** The last option, e.g. `New person…`. */
		addLabel: string;
		invalid?: boolean;
		/** Why the choice was not accepted, e.g. from the core when saving. */
		error?: string | null;
		hint?: import('svelte').Snippet;
		onchange: (value: AccountId) => void;
		/** Creates the entry and returns its id, or null when it could not be added. */
		onadd: (name: string) => Promise<AccountId | null>;
	} = $props();

	const id = $props.id();
	const ADD = '__add__';
	let adding = $state(false);
	let name = $state('');
	let input = $state<HTMLInputElement | null>(null);
	let select = $state<HTMLSelectElement | null>(null);

	const groups = $derived(
		[...new Set(options.map((o) => o.group ?? ''))].map((g) => ({
			group: g,
			options: options.filter((o) => (o.group ?? '') === g)
		}))
	);

	function picked(raw: string) {
		if (raw === ADD) {
			adding = true;
			name = '';
			queueMicrotask(() => input?.focus());
			return;
		}
		onchange(Number(raw));
	}

	async function add() {
		if (!name.trim()) return;
		const created = await onadd(name.trim());
		if (created === null) return;
		adding = false;
		onchange(created);
		queueMicrotask(() => select?.focus());
	}

	function cancel() {
		adding = false;
		queueMicrotask(() => select?.focus());
	}
</script>

<Field {label} {id} {invalid} {error} {hint}>
	{#if adding}
		<span class="muted"><Icon name="plus" size={14} /></span>
		<input
			{id}
			bind:this={input}
			type="text"
			bind:value={name}
			onkeydown={(e) => {
				if (e.key === 'Enter') {
					e.preventDefault();
					add();
				} else if (e.key === 'Escape') {
					e.preventDefault();
					e.stopPropagation();
					cancel();
				}
			}}
		/>
		<button type="button" class="add" disabled={!name.trim()} onclick={add}>Add</button>
	{:else}
		<select
			{id}
			bind:this={select}
			value={value === null ? '' : String(value)}
			onchange={(e) => picked(e.currentTarget.value)}
		>
			{#if value === null}<option value="" disabled></option>{/if}
			{#each groups as g (g.group)}
				{#if g.group}
					<optgroup label={g.group}>
						{#each g.options as o (o.value)}<option value={String(o.value)}>{o.label}</option
							>{/each}
					</optgroup>
				{:else}
					{#each g.options as o (o.value)}<option value={String(o.value)}>{o.label}</option>{/each}
				{/if}
			{/each}
			<option value={ADD}>{addLabel}</option>
		</select>
		<span class="muted chevron"><Icon name="down" size={14} /></span>
	{/if}
</Field>

<style>
	select {
		appearance: none;
		cursor: pointer;
	}

	.muted {
		display: flex;
		color: var(--color-ink-muted);
	}

	.chevron {
		pointer-events: none;
	}

	.add {
		height: 28px;
		margin-right: -6px;
		padding: 0 10px;
		border: none;
		border-radius: var(--radius-field);
		background: var(--color-ink);
		color: var(--color-paper);
		font-size: 13px;
		font-weight: 600;
		cursor: pointer;
	}

	.add:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
