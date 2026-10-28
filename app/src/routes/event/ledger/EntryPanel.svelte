<script lang="ts" module>
	// Remembered between entries, so a stack of receipts from one day and one payer goes fast.
	let lastDate: string | null = null;
	let lastPayer: number | null = null;
</script>

<script lang="ts">
	// The entry panel (R-TX-3): one form per kind of transaction. Amounts are parsed by the
	// core; the "left" amount of a split is a display-only subtraction of whole cents, and the
	// core checks the split against the total again when saving.
	import { untrack } from 'svelte';
	import { ApiError, api } from '#lib/api/index.ts';
	import type { AccountId } from '#lib/api/generated/AccountId.ts';
	import type { Cents } from '#lib/api/generated/Cents.ts';
	import type { Entry } from '#lib/api/generated/Entry.ts';
	import type { TxnView } from '#lib/api/generated/TxnView.ts';
	import { categoryColor } from '#lib/colors.ts';
	import AmountField from '#lib/components/AmountField.svelte';
	import BalanceCell from '#lib/components/BalanceCell.svelte';
	import Button from '#lib/components/Button.svelte';
	import ChoiceField from '#lib/components/ChoiceField.svelte';
	import DateField from '#lib/components/DateField.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import type { Attachment } from '#lib/api/generated/Attachment.ts';
	import Receipts from './Receipts.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';

	type Kind = 'expense' | 'income' | 'transfer' | 'settle';
	type SplitLine = { account: AccountId | null; amount: Cents | null };

	let {
		txn,
		pending = $bindable([]),
		previewing,
		onpreview,
		onclose,
		onescape,
		onsaved
	}: {
		/** The transaction being edited, or null for a new one. */
		txn: TxnView | null;
		/** Files to attach on save (paths), e.g. dropped onto the window. */
		pending?: string[];
		previewing: number | null;
		onpreview: (a: Attachment) => void;
		onclose: () => void;
		/** Esc: the ledger closes an open preview first. */
		onescape?: () => void;
		onsaved: (id: number | null) => void;
	} = $props();

	let removed = $state<number[]>([]);
	const attachments = $derived((txn?.attachments ?? []).filter((a) => !removed.includes(a.id)));

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const byId = $derived(new Map(snapshot.accounts.map((a) => [a.id, a])));
	const kindOf = (id: AccountId | null) => (id === null ? undefined : byId.get(id)?.kind);

	// --- state, filled from the transaction being edited --------------------------------
	let kind = $state<Kind>('expense');
	let amount = $state<Cents | null>(null);
	let lines = $state<SplitLine[]>([{ account: null, amount: null }]);
	let party = $state<AccountId | null>(null); // payer · destination · from · person
	let other = $state<AccountId | null>(null); // to · account
	let direction = $state<'reimburse' | 'repay'>('reimburse');
	let date = $state<string | null>(null);
	let counterparty = $state<string | null>(null);
	let description = $state<string | null>(null);
	let error = $state<{ field: string | null; message: string } | null>(null);
	let confirming = $state(false);
	let generation = $state(0); // remounts the fields after "Save & next"

	function load(t: TxnView | null) {
		error = null;
		confirming = false;
		if (!t) {
			kind = 'expense';
			amount = null;
			lines = [{ account: null, amount: null }];
			party = lastPayer;
			other = null;
			date = lastDate ?? snapshot.event.startsOn ?? new Date().toISOString().slice(0, 10);
			counterparty = null;
			description = null;
			return;
		}
		const category = (id: AccountId) => kindOf(id) === 'income' || kindOf(id) === 'expense';
		kind = t.kind === 'opening' ? 'transfer' : t.kind;
		amount = t.amount;
		date = t.date;
		counterparty = t.counterparty;
		description = t.description || null;
		const cats = t.lines.filter((l) => category(l.account));
		lines = cats.map((l) => ({ account: l.account, amount: Math.abs(l.amount) }));
		if (!lines.length) lines = [{ account: null, amount: null }];
		const parties = t.lines.filter((l) => !category(l.account));
		if (t.kind === 'expense') party = parties.find((l) => l.amount < 0)?.account ?? null;
		if (t.kind === 'income') party = parties.find((l) => l.amount > 0)?.account ?? null;
		if (t.kind === 'transfer' || t.kind === 'opening') {
			party = parties.find((l) => l.amount < 0)?.account ?? null;
			other = parties.find((l) => l.amount > 0)?.account ?? null;
		}
		if (t.kind === 'settle') {
			const person = parties.find((l) => kindOf(l.account) === 'person');
			party = person?.account ?? null;
			other = parties.find((l) => kindOf(l.account) === 'asset')?.account ?? null;
			direction = (person?.amount ?? 0) > 0 ? 'reimburse' : 'repay';
		}
	}
	// Loaded once: the ledger remounts the panel for every entry it opens.
	untrack(() => load(txn));

	// --- options ----------------------------------------------------------------------
	const assets = $derived(snapshot.accounts.filter((a) => a.kind === 'asset'));
	const people = $derived(snapshot.accounts.filter((a) => a.kind === 'person'));
	const sources = $derived([
		...assets.map((a) => ({ value: a.id, label: a.name })),
		...people.map((a) => ({ value: a.id, label: a.name, group: 'People' }))
	]);
	const categories = $derived(
		snapshot.accounts
			.filter((a) => a.kind === (kind === 'income' ? 'income' : 'expense'))
			.map((a) => ({ value: a.id, label: a.name }))
	);
	const personOptions = $derived(people.map((a) => ({ value: a.id, label: a.name })));
	const assetOptions = $derived(assets.map((a) => ({ value: a.id, label: a.name })));

	// --- derived figures --------------------------------------------------------------
	const summary = (id: AccountId | null) => snapshot.persons.find((p) => p.account === id)?.summary;
	const split = $derived(lines.length > 1);
	const left = $derived(
		split && amount !== null ? amount - lines.reduce((s, l) => s + (l.amount ?? 0), 0) : null
	);
	const net = $derived(summary(party)?.net ?? 0);
	const overSettles = $derived(kind === 'settle' && amount !== null && amount > Math.abs(net));

	function balanceHint(id: AccountId | null) {
		const s = summary(id);
		if (!s || s.net === 0) return null;
		const name = byId.get(id!)?.name ?? '';
		return s.net > 0
			? `${name} is owed ${formatAmount(s.net, format, 'magnitude')}`
			: `${name} owes ${formatAmount(s.net, format, 'magnitude')}`;
	}

	function pickPerson(id: AccountId) {
		party = id;
		const n = summary(id)?.net ?? 0;
		direction = n >= 0 ? 'reimburse' : 'repay';
		if (n !== 0 && txn === null) amount = Math.abs(n);
		other ??= snapshot.event.defaultReimbursementAccount;
		generation++;
	}

	// --- adding people and categories on the spot ------------------------------------
	async function addPerson(name: string) {
		try {
			return (await session.run(() => api.addPerson(name))).id;
		} catch (e) {
			error = { field: 'party', message: e instanceof ApiError ? e.message : String(e) };
			return null;
		}
	}

	async function addCategory(name: string) {
		try {
			const added = await session.run(() =>
				api.addAccount(kind === 'income' ? 'income' : 'expense', name)
			);
			return added.id;
		} catch (e) {
			error = { field: 'lines', message: e instanceof ApiError ? e.message : String(e) };
			return null;
		}
	}

	// --- saving -----------------------------------------------------------------------
	function build(): Entry | null {
		const missing = (field: string, message: string) => ((error = { field, message }), null);
		if (party === null) return missing('party', 'choose who');
		if (amount === null) return missing('amount', 'enter an amount');
		if (kind === 'expense' || kind === 'income') {
			if (lines.some((l) => l.account === null)) return missing('lines', 'choose a category');
			const splits = lines.map((l) => ({
				account: l.account!,
				amount: split ? (l.amount ?? 0) : amount!,
				memo: null
			}));
			return kind === 'expense'
				? { type: 'expense', payer: party, lines: splits }
				: { type: 'income', destination: party, lines: splits };
		}
		if (other === null) return missing('other', 'choose an account');
		if (kind === 'transfer') return { type: 'transfer', from: party, to: other, amount };
		return { type: 'settle', person: party, account: other, amount, direction };
	}

	async function save(next: boolean) {
		error = null;
		const entry = build();
		if (!entry || !date) {
			if (!date) error = { field: 'date', message: 'enter a date' };
			return;
		}
		const total = kind === 'expense' || kind === 'income' ? amount : null;
		try {
			const files = [...pending];
			const saved = await session.run(() =>
				api.saveTxn(
					{
						date: date!,
						description: description ?? '',
						counterparty,
						notes: txn?.notes ?? null,
						entry,
						total
					},
					txn?.id ?? null,
					files,
					removed
				)
			);
			pending = [];
			removed = [];
			lastDate = date;
			if (kind === 'expense') lastPayer = party;
			if (next) {
				load(null);
				kind = entry.type as Kind;
				generation++;
				onsaved(null);
			} else onsaved(saved.id);
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			const field = e.field?.startsWith('lines')
				? 'lines'
				: e.field && ['payer', 'destination', 'from', 'person'].includes(e.field)
					? 'party'
					: e.field && ['to', 'account'].includes(e.field)
						? 'other'
						: e.field;
			error = { field, message: e.message };
		}
	}

	async function remove() {
		if (!txn) return;
		await session.change(() => api.deleteTxn(txn.id));
		onsaved(null);
		onclose();
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			save(e.shiftKey);
		}
	}

	const fieldError = (name: string) => (error?.field === name ? error.message : null);
	const partyLabel = $derived(
		{ expense: 'Paid by', income: 'Received by', transfer: 'From', settle: 'Person' }[kind]
	);
</script>

<svelte:window onkeydown={keydown} />

<Panel {onclose} {onescape}>
	{#snippet header()}
		<Segmented
			label="Kind"
			bind:value={kind}
			onchange={() => ((error = null), (lines = [{ account: null, amount: null }]))}
			options={[
				{ value: 'expense', label: 'Expense' },
				{ value: 'income', label: 'Income' },
				{ value: 'transfer', label: 'Transfer' },
				{ value: 'settle', label: 'Settle' }
			]}
		/>
	{/snippet}

	{#key generation}
		{#if kind === 'settle'}
			<ChoiceField
				label="Person"
				value={party}
				options={personOptions}
				addLabel="New person…"
				error={fieldError('party')}
				onchange={pickPerson}
				onadd={addPerson}
			>
				{#snippet hint()}
					{#if party !== null}<BalanceCell {net} {format} />{/if}
				{/snippet}
			</ChoiceField>
			<div class="labelled">
				<span class="label">Direction</span>
				<Segmented
					label="Direction"
					bind:value={direction}
					options={[
						{ value: 'reimburse', label: 'Reimburse' },
						{ value: 'repay', label: 'Repay' }
					]}
				/>
			</div>
		{/if}

		<div class="amount">
			<AmountField
				label="Amount"
				value={amount}
				{format}
				large
				autofocus={kind !== 'settle'}
				oninput={(v) => (amount = v)}
				oncommit={(v) => (amount = v)}
			/>
			{#if overSettles}
				<span class="attention">more than {formatAmount(net, format, 'magnitude')}</span>
			{/if}
			{#if fieldError('amount')}<span class="error">{fieldError('amount')}</span>{/if}
		</div>

		{#if kind !== 'settle'}
			<ChoiceField
				label={partyLabel}
				value={party}
				options={sources}
				addLabel="New person…"
				error={fieldError('party')}
				onchange={(v) => (party = v)}
				onadd={addPerson}
			>
				{#snippet hint()}{balanceHint(party) ?? ''}{/snippet}
			</ChoiceField>
		{/if}

		{#if kind === 'transfer' || kind === 'settle'}
			<ChoiceField
				label={kind === 'transfer' ? 'To' : direction === 'reimburse' ? 'From' : 'To'}
				value={other}
				options={assetOptions}
				addLabel="New account…"
				error={fieldError('other')}
				onchange={(v) => (other = v)}
				onadd={async (name) => {
					try {
						return (await session.run(() => api.addAccount('asset', name))).id;
					} catch (e) {
						error = { field: 'other', message: e instanceof ApiError ? e.message : String(e) };
						return null;
					}
				}}
			/>
		{/if}

		{#if kind === 'expense' || kind === 'income'}
			<div class="split">
				<div class="split-head">
					<span class="label">Split</span>
					{#if left !== null}
						<span class="left" class:done={left === 0}>{formatAmount(left, format)} left</span>
					{/if}
				</div>
				{#each lines as line, i (i)}
					{@const account = line.account === null ? undefined : byId.get(line.account)}
					<div class="line">
						{#if account}<span
								class="dot"
								style:background={categoryColor(account, snapshot.accounts)}
							></span>{/if}
						<div class="category">
							<ChoiceField
								value={line.account}
								options={categories}
								addLabel="New category…"
								invalid={!!fieldError('lines') && line.account === null}
								onchange={(v) => (lines[i].account = v)}
								onadd={addCategory}
							/>
						</div>
						{#if split}
							<div class="line-amount">
								<AmountField
									value={line.amount}
									{format}
									oninput={(v) => (lines[i].amount = v)}
									oncommit={(v) => (lines[i].amount = v)}
								/>
							</div>
							<button
								type="button"
								class="icon"
								aria-label="Remove line"
								onclick={() => (lines = lines.filter((_, j) => j !== i))}
							>
								<Icon name="x" size={14} />
							</button>
						{/if}
					</div>
				{/each}
				<button
					type="button"
					class="add-line"
					onclick={() =>
						(lines = [
							...lines.map((l, j) => (j === 0 && !split ? { ...l, amount } : l)),
							{ account: null, amount: null }
						])}
				>
					<Icon name="plus" size={14} />Line
				</button>
				{#if fieldError('lines')}<span class="error">{fieldError('lines')}</span>{/if}
			</div>
		{/if}

		<div class="pair">
			<DateField
				label="Date"
				value={date}
				{format}
				required
				oninput={(v) => v && (date = v)}
				oncommit={(v) => (date = v)}
			/>
			{#if kind === 'expense' || kind === 'income'}
				<TextField
					label="Counterparty"
					value={counterparty}
					oninput={(t) => (counterparty = t.trim() || null)}
					oncommit={(v) => (counterparty = v)}
				/>
			{/if}
		</div>
		<TextField
			label="Description"
			value={description}
			oninput={(t) => (description = t.trim() || null)}
			oncommit={(v) => (description = v)}
		/>
		<Receipts
			{attachments}
			{pending}
			{previewing}
			{onpreview}
			onremove={(a) => (removed = [...removed, a.id])}
			onremovePending={(i) => (pending = pending.filter((_, j) => j !== i))}
			onadd={(paths) => (pending = [...pending, ...paths.filter((p) => !pending.includes(p))])}
		/>
		{#if error && !['party', 'other', 'lines', 'amount'].includes(error.field ?? '')}
			<p class="error">{error.message}</p>
		{/if}
	{/key}

	{#snippet footer()}
		{#if confirming}
			<div class="confirm">
				<strong>Delete this entry?</strong>
				<span class="grow"></span>
				<Button size="sm" kind="primary" onclick={remove}>Delete</Button>
				<Button size="sm" onclick={() => (confirming = false)}>Keep</Button>
			</div>
		{:else}
			<Button kind="primary" shortcut="⌘↵" onclick={() => save(false)}>Save</Button>
			{#if txn}
				<Button size="sm" onclick={() => (confirming = true)}>Delete</Button>
			{:else}
				<Button shortcut="⇧⌘↵" onclick={() => save(true)}>Save & next</Button>
			{/if}
		{/if}
	{/snippet}
</Panel>

<style>
	.label {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.labelled,
	.amount,
	.split {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.split-head {
		display: flex;
		justify-content: space-between;
	}

	.left {
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-attention);
	}

	.left.done {
		color: var(--color-plus);
	}

	.line {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
	}

	.dot {
		flex: none;
		width: 6px;
		height: 6px;
		border-radius: 50%;
	}

	.category {
		flex: 1;
		min-width: 0;
	}

	.line-amount {
		width: 120px;
	}

	.icon,
	.add-line {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0;
		border: none;
		background: none;
		color: var(--color-ink-muted);
		font-size: 13px;
		cursor: pointer;
	}

	.add-line {
		align-self: flex-start;
	}

	.pair {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--spacing-3);
	}

	.attention {
		font-size: var(--type-xs);
		color: var(--color-attention);
	}

	.error {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-minus);
	}

	.confirm {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		width: 100%;
		padding-top: 14px;
		border-top: 1px solid var(--color-rule);
		font-size: 13px;
	}

	.grow {
		flex: 1;
	}
</style>
