<script lang="ts">
	// One budget line (R-BUD-1): its category, description and estimate, as an amount, as count ×
	// unit price, or as a price per assumption such as the guests expected (R-BUD-6), and whether
	// it is complete. Below, the receipt lines linked to it
	// (R-BUD-2), each opening its transaction in the ledger.
	import { tick, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { api, ApiError, errorMessage } from '#lib/api/index.ts';
	import type { AccountId } from '#lib/api/generated/AccountId.ts';
	import type { AssumptionId } from '#lib/api/generated/AssumptionId.ts';
	import type { BudgetLine } from '#lib/api/generated/BudgetLine.ts';
	import type { BudgetLineDraft } from '#lib/api/generated/BudgetLineDraft.ts';
	import type { Cents } from '#lib/api/generated/Cents.ts';
	import AmountField, { amountsRead } from '#lib/components/AmountField.svelte';
	import Button from '#lib/components/Button.svelte';
	import Chip from '#lib/components/Chip.svelte';
	import ChoiceField from '#lib/components/ChoiceField.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Select from '#lib/components/Select.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { isOver } from '#lib/budget.ts';
	import { formatAmount, formatDate } from '#lib/format.ts';
	import { boxGrow, boxOpen } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
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
	// An income and an expense category can share a name, and the closed field shows only the name.
	const categoryKind = $derived(
		{ income: 'Income', expense: 'Expense' }[
			snapshot.accounts.find((a) => a.id === category)?.kind as string
		] ?? ''
	);
	let description = $state('');
	let mode = $state<'amount' | 'quantity' | 'per'>('amount');
	let amount = $state<Cents | null>(null);
	let quantity = $state('');
	let unitPrice = $state<Cents | null>(null);
	/** The assumptions the quantity is multiplied by, in the order they were picked. */
	let per = $state<AssumptionId[]>([]);
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
			// Saved as 2 × Guests × price: a count as well, so it stays a count with its chips.
			mode = e.per.length && e.quantity === 1 ? 'per' : 'quantity';
			quantity = String(e.quantity);
			unitPrice = e.unitPrice;
			per = [...e.per];
		} else if (e) amount = e.amount;
	});
	let amountInvalid = $state(false);
	let unitPriceInvalid = $state(false);
	const values = () =>
		JSON.stringify([
			category,
			description,
			mode,
			amount,
			amountInvalid,
			quantity,
			unitPrice,
			per,
			unitPriceInvalid,
			complete,
			costCentre
		]);
	const clean = values();
	let confirming = $state(false);
	let error = $state<{ field: string | null; message: string } | null>(null);

	const count = $derived(
		mode === 'per' ? 1 : /^\d+$/.test(quantity.trim()) ? Number(quantity.trim()) : null
	);
	const picked = $derived(per.flatMap((id) => snapshot.assumptions.filter((a) => a.id === id)));
	// A preview: the core works out the estimate that counts.
	const total = $derived(
		mode === 'amount'
			? amount
			: count !== null && unitPrice !== null && (mode !== 'per' || picked.length)
				? picked.reduce((n, a) => n * a.value, count) * unitPrice
				: null
	);
	// What is multiplied: the count and the assumptions' values, without the price itself. Each
	// assumption is named, so several of them read as a product rather than a run of numbers.
	const factors = $derived([
		...(count !== null && mode === 'quantity' && (count !== 1 || !picked.length)
			? [String(count)]
			: []),
		...picked.map((a) => `${a.name} ${a.value}`)
	]);
	const formula = $derived(
		count !== null && unitPrice !== null && factors.length
			? [...factors, formatAmount(unitPrice, format)].join(' × ')
			: ''
	);

	const names = $derived(picked.map((a) => a.name).join(' × '));
	// The price field beside the assumptions is named by them, as the tab names the count.
	const priceName = $derived(names ? `Price per ${names}` : 'Price per assumption');

	// The mode before the latest switch: Segmented sets the new one before it calls back.
	let last = untrack(() => mode);
	// The tabs' fields: the box grows around whichever set the choice shows.
	let controls: HTMLDivElement;
	async function changeMode(to: 'amount' | 'quantity' | 'per') {
		error = null;
		// Measured before the new fields are in, so the box can ease into the room they take.
		const from = controls?.getBoundingClientRect().height ?? 0;
		// A count stops scaling, and a price per assumption has no count of its own.
		if (to === 'quantity' && last === 'per') per = [];
		last = to;
		if (to === 'per' && !per.length && snapshot.assumptions.length === 1)
			per = [snapshot.assumptions[0].id];
		await tick();
		if (controls) boxGrow(controls, from, level('A7'));
	}

	function toggle(id: AssumptionId) {
		per = per.includes(id) ? per.filter((p) => p !== id) : [...per, id];
		error = null;
	}
	const fieldError = (field: string) => (error?.field?.startsWith(field) ? error.message : null);

	const figures = $derived(line ? snapshot.budget.lines.find((f) => f.line === line.id) : null);
	const over = $derived(
		!!line &&
			!!figures &&
			isOver(
				snapshot.accounts.find((a) => a.id === line.category)?.kind === 'income'
					? 'income'
					: 'expense',
				figures,
				line.complete
			)
	);
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
		if (mode === 'per' && !per.length) return missing('estimate.per', 'choose an assumption');
		if (count === null) return missing('estimate.quantity', 'enter a whole number');
		if (unitPrice === null) return missing('estimate.unitPrice', 'enter a price');
		return { ...shared, estimate: { type: 'quantity', quantity: count, unitPrice, per } };
	}

	async function save() {
		error = null;
		await amountsRead();
		if (mode === 'amount' ? amountInvalid : unitPriceInvalid) return;
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

<Panel title={line ? line.description : 'New budget line'} {onclose} dirty={values() !== clean}>
	{#if figures}
		<div class="figures">
			<div><span class="form-label">Estimate</span>{formatAmount(figures.estimate, format)}</div>
			<div class:over>
				<span class="form-label">Actual</span>{formatAmount(figures.actual, format)}
			</div>
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
			>
				{#snippet hint()}{categoryKind}{/snippet}
			</ChoiceField>
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

		<div class="estimate">
			<span class="form-label">Estimate</span>
			<div class="method">
				<div class="method-tabs">
					<Segmented
						label="Estimate"
						variant="tabs"
						bind:value={mode}
						options={[
							{ value: 'amount', label: 'Amount' },
							{ value: 'quantity', label: 'Count × price' },
							...(snapshot.assumptions.length || mode === 'per'
								? [{ value: 'per' as const, label: 'Per assumption' }]
								: [])
						]}
						onchange={changeMode}
					/>
				</div>
				<div class="method-body">
					<div class="controls" bind:this={controls}>
						{#if mode === 'amount'}
							<AmountField
								value={amount}
								{format}
								bind:invalid={amountInvalid}
								oninput={(v) => (amount = v)}
							/>
						{:else if mode === 'quantity'}
							{#if per.length}
								<div class="scales" role="group" aria-label="Times">
									<span class="form-label">Times</span>
									{#each snapshot.assumptions as a (a.id)}
										<Chip
											label={a.name}
											count={a.value}
											active={per.includes(a.id)}
											onclick={() => toggle(a.id)}
										/>
									{/each}
								</div>
							{/if}
							<div class="terms count-terms">
								<div class="count">
									<TextField
										name="Count"
										placeholder="1"
										value={quantity}
										oninput={(v) => (quantity = v)}
										oncommit={(v) => (quantity = v ?? '')}
									/>
								</div>
								<span class="times">×</span>
								<div class="price">
									<AmountField
										name="Unit price"
										value={unitPrice}
										{format}
										bind:invalid={unitPriceInvalid}
										oninput={(v) => (unitPrice = v)}
									/>
								</div>
							</div>
						{:else}
							<div class="terms per-terms">
								<div class="scales" role="group" aria-label="Per assumption">
									{#each snapshot.assumptions as a (a.id)}
										<Chip
											label={a.name}
											count={a.value}
											active={per.includes(a.id)}
											onclick={() => toggle(a.id)}
										/>
									{/each}
								</div>
								<div class="price">
									<AmountField
										name={priceName}
										value={unitPrice}
										{format}
										bind:invalid={unitPriceInvalid}
										oninput={(v) => (unitPrice = v)}
									/>
								</div>
							</div>
						{/if}
					</div>
					<!-- Only as tall as its line: an empty estimate stays compact and the sum opens
					     when there is something to add up. -->
					{#if mode !== 'amount' && total !== null}
						<div class="sum" transition:boxOpen={{ level: level('A7') }}>
							<div class="sum-line" class:centred={mode === 'per'} data-testid="budget-line-total">
								<span class="formula">{formula}</span>
								<span class="equals">=</span>
								<strong>{formatAmount(total, format)}</strong>
							</div>
						</div>
					{/if}
				</div>
			</div>
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

	.figures .over {
		color: var(--color-attention);
	}

	.labelled {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	/* The estimate: one box, its tabs along the top, so what they change is visibly inside them.
	   The fields the tabs show grow into place (A7), so the fields below the box give way smoothly
	   while the tabs are tried. */
	.estimate {
		display: flex;
		flex-direction: column;
		gap: 6px;
		min-width: 0;
	}

	.method {
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-md);
		background: var(--color-paper);
		overflow: hidden;
	}

	.method-tabs {
		border-bottom: 1px solid var(--color-rule);
	}

	.method-body {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 12px;
	}

	.controls {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.terms {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
	}

	.count-terms .count {
		flex: 0 0 84px;
		/* The box's input would otherwise hold the row open to its default width. */
		min-width: 0;
	}

	.count-terms .price {
		flex: 1 1 auto;
		min-width: 0;
	}

	/* The assumptions centred over their price, the sum under both. */
	.per-terms {
		flex-direction: column;
		align-items: center;
		gap: 10px;
	}

	.per-terms .scales {
		flex: 0 1 auto;
		justify-content: center;
	}

	.per-terms .price {
		align-self: stretch;
		min-width: 0;
	}

	/* The assumptions wrap among themselves, so the row keeps its shape however many are picked. */
	.scales {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		flex: 1 1 auto;
		min-width: 0;
	}

	.scales .form-label {
		margin-right: 2px;
	}

	.times {
		flex: none;
		color: var(--color-ink-muted);
	}

	.sum-line {
		display: flex;
		align-items: baseline;
		gap: 6px;
		height: 20px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.sum-line.centred {
		justify-content: center;
	}

	.formula {
		min-width: 0;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		font-variant-numeric: tabular-nums;
	}

	.equals {
		flex: none;
	}

	.sum-line strong {
		flex: none;
		font-size: var(--type-sm);
		font-weight: 600;
		color: var(--color-ink);
		font-variant-numeric: tabular-nums;
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
