<script lang="ts">
	// One budget line (R-BUD-1): its category, description and estimate, as an amount or as
	// quantity × unit price, and whether it is complete. Below, the receipt lines linked to it
	// (R-BUD-2), each opening its transaction in the ledger.
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { api, ApiError, errorMessage } from '#lib/api/index.ts';
	import type { AccountId } from '#lib/api/generated/AccountId.ts';
	import type { BudgetLine } from '#lib/api/generated/BudgetLine.ts';
	import type { BudgetLineDraft } from '#lib/api/generated/BudgetLineDraft.ts';
	import type { Cents } from '#lib/api/generated/Cents.ts';
	import AmountField from '#lib/components/AmountField.svelte';
	import Button from '#lib/components/Button.svelte';
	import ChoiceField from '#lib/components/ChoiceField.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Select from '#lib/components/Select.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { formatAmount, formatDate } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';

	let {
		line,
		category: initialCategory,
		onclose
	}: {
		/** The line being edited; null for a new one. */
		line: BudgetLine | null;
		/** The category a new line starts in. */
		category: AccountId | null;
		onclose: () => void;
	} = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const categories = $derived(
		snapshot.accounts
			.filter((a) => a.kind === 'income' || a.kind === 'expense')
			.map((a) => ({
				value: a.id,
				label: a.name,
				group: a.kind === 'income' ? 'Income' : 'Expenses'
			}))
	);

	// --- form state, filled once from the line being edited: the page remounts the panel --
	let category = $state<AccountId | null>(null);
	let description = $state('');
	let mode = $state<'amount' | 'quantity'>('amount');
	let amount = $state<Cents | null>(null);
	let quantity = $state('');
	let unitPrice = $state<Cents | null>(null);
	let complete = $state(false);
	let costCentre = $state(0); // 0: none
	const startsIn = (id: AccountId | null) =>
		snapshot.accounts.find((a) => a.id === id)?.costCentre ?? 0;
	untrack(() => {
		category = line?.category ?? initialCategory;
		costCentre = line ? (line.costCentre ?? 0) : startsIn(category);
		description = line?.description ?? '';
		complete = line?.complete ?? false;
		const e = line?.estimate;
		if (e?.type === 'quantity') {
			mode = 'quantity';
			quantity = String(e.quantity);
			unitPrice = e.unitPrice;
		} else if (e) amount = e.amount;
	});
	let confirming = $state(false);
	let error = $state<{ field: string | null; message: string } | null>(null);

	const count = $derived(/^\d+$/.test(quantity.trim()) ? Number(quantity.trim()) : null);
	const total = $derived(
		mode === 'amount' ? amount : count !== null && unitPrice !== null ? count * unitPrice : null
	);
	const fieldError = (field: string) => (error?.field?.startsWith(field) ? error.message : null);

	const figures = $derived(line ? snapshot.budget.lines.find((f) => f.line === line.id) : null);
	const linked = $derived(
		line
			? snapshot.txns.flatMap((t) =>
					t.lines
						.filter((l) => l.budgetLine === line.id)
						.map((l) => ({ txn: t, amount: Math.abs(l.amount) }))
				)
			: []
	);

	function draft(): BudgetLineDraft | null {
		const missing = (field: string, message: string) => ((error = { field, message }), null);
		if (category === null) return missing('category', 'choose a category');
		const shared = { category, description, complete, costCentre: costCentre || null };
		if (mode === 'amount') {
			if (amount === null) return missing('estimate', 'enter an amount');
			return { ...shared, estimate: { type: 'amount', amount } };
		}
		if (count === null) return missing('estimate.quantity', 'enter a whole number');
		if (unitPrice === null) return missing('estimate.unitPrice', 'enter a price');
		return { ...shared, estimate: { type: 'quantity', quantity: count, unitPrice } };
	}

	async function save() {
		error = null;
		const d = draft();
		if (!d) return;
		try {
			await session.change(() => (line ? api.updateBudgetLine(line.id, d) : api.addBudgetLine(d)));
			onclose();
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = { field: e.field, message: e.message };
		}
	}

	async function addCategory(name: string) {
		try {
			return (await session.change(() => api.addAccount('expense', name))).id;
		} catch (e) {
			error = { field: 'category', message: errorMessage(e) };
			return null;
		}
	}

	async function remove() {
		if (!line) return;
		await session.change(() => api.deleteBudgetLine(line.id));
		onclose();
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !session.closed) {
			e.preventDefault();
			save();
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<Panel title={line ? line.description : 'New budget line'} {onclose}>
	{#if figures}
		<div class="figures">
			<div><span class="form-label">Estimate</span>{formatAmount(figures.estimate, format)}</div>
			<div><span class="form-label">Actual</span>{formatAmount(figures.actual, format)}</div>
			<div>
				<span class="form-label">Forecast</span><strong
					>{formatAmount(figures.forecast, format)}</strong
				>
			</div>
		</div>
	{/if}

	<fieldset disabled={session.closed}>
		<div class="labelled">
			<ChoiceField
				label="Category"
				value={category}
				options={categories}
				addLabel="New expense category…"
				error={fieldError('category')}
				onchange={(v) => {
					if (v !== category) costCentre = startsIn(v);
					category = v;
					error = null;
				}}
				onadd={addCategory}
			/>
		</div>

		<Select
			label="Cost centre"
			value={costCentre}
			options={[
				{ value: 0, label: 'None' },
				...snapshot.costCentres.map((c) => ({ value: c.id, label: c.name }))
			]}
			onchange={(v) => (costCentre = v)}
		/>

		<div class="labelled">
			<TextField
				label="Description"
				value={description}
				autofocus={!line}
				oninput={(v) => (description = v)}
				oncommit={(v) => (description = v ?? '')}
			/>
			{#if fieldError('description')}<span class="form-error">{fieldError('description')}</span
				>{/if}
		</div>

		<div class="labelled">
			<span class="form-label">Estimate</span>
			<Segmented
				label="Estimate"
				bind:value={mode}
				options={[
					{ value: 'amount', label: 'Amount' },
					{ value: 'quantity', label: 'Quantity × price' }
				]}
				onchange={() => (error = null)}
			/>
			{#if mode === 'amount'}
				<AmountField value={amount} {format} oninput={(v) => (amount = v)} />
			{:else}
				<div class="times">
					<TextField
						label="Quantity"
						value={quantity}
						oninput={(v) => (quantity = v)}
						oncommit={(v) => (quantity = v ?? '')}
					/>
					<span class="x">×</span>
					<AmountField
						label="Unit price"
						value={unitPrice}
						{format}
						oninput={(v) => (unitPrice = v)}
					/>
				</div>
				{#if total !== null}<span class="hint">= {formatAmount(total, format)}</span>{/if}
			{/if}
			{#if fieldError('estimate')}<span class="form-error">{fieldError('estimate')}</span>{/if}
		</div>

		<label class="check">
			<input type="checkbox" bind:checked={complete} />
			Complete
		</label>
	</fieldset>

	{#if error && !['category', 'description', 'estimate'].some((f) => error?.field?.startsWith(f))}
		<p class="form-error">{error.message}</p>
	{/if}

	{#if linked.length}
		<div class="linked">
			<span class="form-label">Linked</span>
			{#each linked as l, i (i)}
				<button type="button" class="entry" onclick={() => goto(`/event/ledger?txn=${l.txn.id}`)}>
					<span class="date">{formatDate(l.txn.date, format)}</span>
					<span class="title">{l.txn.description || l.txn.counterparty}</span>
					<span class="num">{formatAmount(l.amount, format)}</span>
				</button>
			{/each}
		</div>
	{/if}

	{#snippet footer()}
		{#if session.closed}
			<span class="closed"><Icon name="lock" size={14} />Closed</span>
		{:else if confirming}
			<div class="form-confirm">
				<strong>Delete this budget line?</strong>
				<span class="grow"></span>
				<Button size="sm" kind="primary" onclick={remove}>Delete</Button>
				<Button size="sm" onclick={() => (confirming = false)}>Keep</Button>
			</div>
		{:else}
			<Button kind="primary" shortcut="⌘↵" onclick={save}>Save</Button>
			{#if line}
				<Button size="sm" onclick={() => (confirming = true)}>Delete</Button>
			{/if}
		{/if}
	{/snippet}
</Panel>

<style>
	.figures {
		display: flex;
		gap: var(--spacing-6);
		padding-bottom: 18px;
		border-bottom: 1px solid var(--color-rule);
	}

	.figures div {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.labelled {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.times {
		display: flex;
		align-items: flex-end;
		gap: var(--spacing-2);
	}

	.times > :global(*:not(.x)) {
		flex: 1;
		min-width: 0;
	}

	.x {
		padding-bottom: 10px;
		color: var(--color-ink-muted);
	}

	.hint {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.check {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		cursor: pointer;
	}

	.linked {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.entry {
		display: flex;
		align-items: baseline;
		gap: var(--spacing-3);
		min-height: 32px;
		padding: 0;
		border: none;
		border-bottom: 1px solid var(--color-rule);
		background: none;
		text-align: left;
		cursor: pointer;
	}

	.entry:hover .title {
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.date {
		width: 84px;
		flex: none;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.title {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.num {
		flex: none;
		text-align: right;
	}

	.closed {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		font-weight: 600;
		color: var(--color-ink-muted);
	}
</style>
