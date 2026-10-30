<script lang="ts">
	// Checking an asset account against its statement (R-ACC-4). For the Verein account that is the
	// event's net movement, since its real balance is never entered (R-ISO-2). A mismatch stays open
	// until the books or the checkpoint change.
	import { api, ApiError } from '#lib/api/index.ts';
	import type { OpenEvent } from '#lib/api/generated/OpenEvent.ts';
	import type { ReconciliationStatus } from '#lib/api/generated/ReconciliationStatus.ts';
	import AmountField from '#lib/components/AmountField.svelte';
	import Button from '#lib/components/Button.svelte';
	import DateField from '#lib/components/DateField.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import { differenceText, formatAmount, formatDate } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';

	let { account, onclose }: { account: number; onclose: () => void } = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const details = $derived(snapshot.accounts.find((a) => a.id === account));
	const movement = $derived(details?.role === 'verein');
	const checkpoints = $derived(snapshot.reconciliations.filter((r) => r.account === account));

	const initialDate = () =>
		session.event!.snapshot.event.endsOn ?? new Date().toISOString().slice(0, 10);
	let editing = $state<number | null>(null);
	let deleting = $state(false);
	let date = $state<string | null>(initialDate());
	let amount = $state<number | null>(null);
	// The fields are rebuilt when another checkpoint is picked.
	let form = $state(0);
	let error = $state<string | null>(null);

	function reset() {
		editing = null;
		deleting = false;
		date = initialDate();
		amount = null;
		error = null;
		form += 1;
	}

	function edit(r: ReconciliationStatus) {
		editing = r.id;
		deleting = false;
		date = r.date;
		amount = r.amount;
		error = null;
		form += 1;
	}

	async function attempt(f: () => Promise<OpenEvent>) {
		try {
			error = null;
			await session.change(f);
			reset();
		} catch (e) {
			error = e instanceof ApiError ? e.message : String(e);
		}
	}

	function save() {
		const on = date;
		const value = amount;
		if (!on || value === null) return;
		const id = editing;
		return attempt(() =>
			id === null
				? api.addReconciliation(account, on, value)
				: api.updateReconciliation(id, on, value)
		);
	}

	function remove() {
		const id = editing;
		if (id !== null) return attempt(() => api.deleteReconciliation(id));
	}
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			save();
		}
	}}
/>

<Panel title={details?.name ?? ''} {onclose}>
	{#key form}
		<div class="pair">
			<DateField
				label="Date"
				value={date}
				{format}
				oninput={(v) => v && (date = v)}
				oncommit={(v) => (date = v)}
			/>
			<AmountField
				label={movement ? 'Net movement' : 'Statement shows'}
				value={amount}
				{format}
				allowNegative
				autofocus
				oninput={(v) => (amount = v)}
				oncommit={(v) => (amount = v)}
			/>
		</div>
	{/key}
	{#if error}<p class="error">{error}</p>{/if}

	{#if checkpoints.length}
		<ul>
			{#each checkpoints as r (r.id)}
				<li class:selected={editing === r.id}>
					<button type="button" onclick={() => edit(r)}>
						<span class="line">
							<strong>{formatDate(r.date, format)}</strong>
							<strong>{formatAmount(r.amount, format)}</strong>
						</span>
						<span class="line note">
							<span>In the books</span>
							<span>{formatAmount(r.computed, format)}</span>
						</span>
						{#if r.fromSheet}
							<span class="line note">
								<span>Tickets in the sheet by then</span>
								<span>{formatAmount(r.fromSheet, format)}</span>
							</span>
						{/if}
						{#if r.difference}
							<span class="note minus">{differenceText(r.difference, format)}</span>
						{:else}
							<span class="note plus">✓ Matches the books</span>
						{/if}
					</button>
				</li>
			{/each}
		</ul>
	{/if}

	{#snippet footer()}
		<Button kind="primary" shortcut="⌘↵" disabled={!date || amount === null} onclick={save}
			>{editing === null ? 'Add' : 'Save'}</Button
		>
		{#if editing === null}
			<Button shortcut="esc" onclick={onclose}>Close</Button>
		{:else if deleting}
			<Button onclick={remove}>Delete</Button>
			<Button onclick={() => (deleting = false)}>Keep</Button>
		{:else}
			<Button onclick={reset}>Cancel</Button>
			<Button onclick={() => (deleting = true)}>Delete</Button>
		{/if}
	{/snippet}
</Panel>

<style>
	.pair {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--spacing-3);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	li {
		border-bottom: 1px solid var(--color-rule);
	}

	li:first-child {
		border-top: 1px solid var(--color-rule);
	}

	li button {
		display: flex;
		flex-direction: column;
		gap: 2px;
		width: 100%;
		padding: 10px 8px;
		border: none;
		background: none;
		font: inherit;
		color: inherit;
		text-align: left;
		cursor: pointer;
	}

	li button:hover {
		background: var(--color-paper-sunk);
	}

	li.selected button {
		background: var(--color-marker);
	}

	.line {
		display: flex;
		justify-content: space-between;
		gap: var(--spacing-3);
	}

	strong {
		font-weight: 600;
	}

	.note {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.note.minus {
		color: var(--color-minus);
	}

	.note.plus {
		color: var(--color-plus);
	}

	.error {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-minus);
	}
</style>
