<script lang="ts">
	// The assumptions the budget is planned on (R-BUD-6), such as the guests expected. A budget
	// line's quantity can scale with them, so changing one here changes every line that uses it.
	// Each opens in place to be renamed, given another value or deleted while nothing uses it.
	import { tick } from 'svelte';
	import { api, ApiError } from '#lib/api/index.ts';
	import type { Assumption } from '#lib/api/generated/Assumption.ts';
	import type { AssumptionId } from '#lib/api/generated/AssumptionId.ts';
	import type { BudgetLine } from '#lib/api/generated/BudgetLine.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import { usersOf } from '#lib/budget.ts';
	import { session } from '#lib/state/session.svelte.ts';

	let {
		assumptions,
		lines
	}: {
		assumptions: Assumption[];
		lines: BudgetLine[];
	} = $props();

	/** The one being edited: an id, or 'new'. */
	let editing = $state<AssumptionId | 'new' | null>(null);
	let name = $state('');
	let value = $state('');
	let error = $state<string | null>(null);
	let nameInput = $state<HTMLInputElement>();

	const MAX = 4294967295;
	const parsed = $derived(/^\d+$/.test(value.trim()) ? Number(value.trim()) : null);
	const used = (id: AssumptionId) => usersOf(lines, id).length;
	const lineCount = (n: number) => (n === 1 ? '1 line' : `${n} lines`);

	async function edit(a: Assumption | null) {
		editing = a?.id ?? 'new';
		name = a?.name ?? '';
		value = a ? String(a.value) : '';
		error = null;
		await tick();
		nameInput?.focus();
		if (a) nameInput?.select();
	}

	async function save() {
		if (parsed === null || parsed > MAX) {
			error = 'enter a whole number';
			return;
		}
		const id = editing;
		try {
			await session.change(() =>
				id === 'new' || id === null
					? api.addAssumption(name, parsed)
					: api.updateAssumption(id, name, parsed)
			);
			editing = null;
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = e.message;
		}
	}

	async function remove(id: AssumptionId) {
		try {
			await session.change(() => api.deleteAssumption(id));
			editing = null;
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = e.message;
		}
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			save();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			editing = null;
		}
	}
</script>

{#snippet editor(id: AssumptionId | null)}
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<form
		class="editor"
		aria-label={id === null ? 'New assumption' : `Edit ${name}`}
		onkeydown={keydown}
		onsubmit={(e) => {
			e.preventDefault();
			save();
		}}
	>
		<input
			class="name"
			aria-label="Name"
			placeholder="Guests"
			bind:this={nameInput}
			bind:value={name}
		/>
		<input
			class="value"
			aria-label="Value"
			inputmode="numeric"
			placeholder="250"
			bind:value
			class:invalid={!!value.trim() && parsed === null}
		/>
		<Button size="sm" kind="primary" type="submit">Save</Button>
		<Button size="sm" onclick={() => (editing = null)}>Cancel</Button>
		{#if id !== null}
			{@const n = used(id)}
			{#if n === 0}
				<Button size="sm" onclick={() => remove(id)}>Delete</Button>
			{:else}
				<span class="note">{lineCount(n)}</span>
			{/if}
		{/if}
	</form>
{/snippet}

<div class="assumptions" role="group" aria-label="Assumptions">
	<span class="form-label">Assumptions</span>
	<div class="items">
		{#each assumptions as a (a.id)}
			{#if editing === a.id}
				{@render editor(a.id)}
			{:else}
				<button
					type="button"
					class="item"
					disabled={session.closed}
					aria-label="{a.name} {a.value}, {lineCount(used(a.id))}"
					onclick={() => edit(a)}
				>
					<span>{a.name}</span>
					<strong>{a.value}</strong>
				</button>
			{/if}
		{/each}
		{#if editing === 'new'}
			{@render editor(null)}
		{:else if !session.closed}
			<button type="button" class="add" onclick={() => edit(null)}>
				<Icon name="plus" size={14} />Assumption
			</button>
		{/if}
	</div>
	{#if error}<p class="form-error">{error}</p>{/if}
</div>

<style>
	.assumptions {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
	}

	.items {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--spacing-2);
	}

	.item {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2);
		height: 32px;
		padding: 0 12px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-md);
		background: none;
		cursor: pointer;
	}

	.item:hover:not(:disabled) {
		background: var(--color-paper-sunk);
	}

	.item:disabled {
		cursor: default;
	}

	.item strong {
		font-weight: 600;
	}

	.add {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 32px;
		padding: 0 10px;
		border: 1px dashed var(--color-rule);
		border-radius: var(--radius-md);
		background: none;
		font-size: 13px;
		color: var(--color-ink-muted);
		cursor: pointer;
	}

	.add:hover {
		border-color: var(--color-ink-muted);
		color: var(--color-ink);
	}

	.editor {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2);
		padding: 2px;
	}

	.editor input {
		height: 32px;
		box-sizing: border-box;
		padding: 0 10px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-field);
		outline: none;
		user-select: text;
		-webkit-user-select: text;
	}

	.editor input:focus {
		border-color: var(--color-ink);
	}

	.editor input.invalid {
		border-color: var(--color-minus);
	}

	.name {
		width: 140px;
	}

	.value {
		width: 80px;
		text-align: right;
		font-variant-numeric: tabular-nums;
	}

	.note {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}
</style>
