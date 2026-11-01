<script lang="ts" module>
	// Remembered between entries, so a stack of receipts from one day and one payer goes fast.
	let lastDate: string | null = null;
	let lastPayer: number | null = null;
</script>

<script lang="ts">
	// The entry panel (R-TX-3): one form per kind of transaction. Amounts are parsed by the
	// core; the "left" amount of a split and a settle's preview are display-only sums of whole
	// cents, and the core checks the split against the total again when saving.
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { ApiError, api, errorMessage } from '#lib/api/index.ts';
	import type { AccountId } from '#lib/api/generated/AccountId.ts';
	import type { EntryProposal } from '../plugins/api';
	import type { BudgetLineId } from '#lib/api/generated/BudgetLineId.ts';
	import type { Cents } from '#lib/api/generated/Cents.ts';
	import type { CostCentreId } from '#lib/api/generated/CostCentreId.ts';
	import type { Entry } from '#lib/api/generated/Entry.ts';
	import type { TxnView } from '#lib/api/generated/TxnView.ts';
	import { categoryColor } from '#lib/colors.ts';
	import AmountField, { amountsRead } from '#lib/components/AmountField.svelte';
	import BalanceCell from '#lib/components/BalanceCell.svelte';
	import Button from '#lib/components/Button.svelte';
	import ChoiceField from '#lib/components/ChoiceField.svelte';
	import DateField, { datesRead } from '#lib/components/DateField.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Select from '#lib/components/Select.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import type { Attachment } from '#lib/api/generated/Attachment.ts';
	import Receipts from './Receipts.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { formIn, formOut } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import { session } from '#lib/state/session.svelte.ts';

	type Kind = 'expense' | 'income' | 'transfer' | 'settle';
	// The budget line link is kept while the category stays (R-BUD-2). A line can only link to a
	// budget line of its category, so picking one from another category changes the category too.
	// A category starts with its cost centre; a budget line brings its own (R-CC-1).
	type SplitLine = {
		account: AccountId | null;
		amount: Cents | null;
		budgetLine?: BudgetLineId | null;
		costCentre?: CostCentreId | null;
	};

	let {
		txn,
		settle = null,
		proposal = null,
		pending = $bindable([]),
		previewing,
		onpreview,
		onclose,
		onescape,
		onsaved
	}: {
		/** The transaction being edited, or null for a new one. */
		txn: TxnView | null;
		/** For a new entry: start as a settle with this person, filled in from their balance. */
		settle?: AccountId | null;
		/** For a new entry: start as what a plugin proposes (technical design §10). */
		proposal?: EntryProposal | null;
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
	// Ticket income comes from the Tickets page and cannot be changed here (R-TKT-6), and an
	// opening count from the Overview (R-ISO-3). A form can only save what it can show.
	const origin = $derived(txn?.origin ?? { type: 'manual' as const });
	const elsewhere = $derived(!!txn && (txn.kind === 'opening' || !txn.entry));
	const locked = $derived(session.closed || origin.type !== 'manual' || elsewhere);
	const byId = $derived(new Map(snapshot.accounts.map((a) => [a.id, a])));

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
	let notes = $state<string | null>(null);
	let error = $state<{ field: string | null; message: string } | null>(null);
	let confirming = $state(false);
	let generation = $state(0); // remounts the fields after "Save & next"
	let amountInvalid = $state(false);
	let dateInvalid = $state(false);
	// By position, like the split's fields; only the lines on show count.
	let lineInvalid = $state<boolean[]>([]);

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
			notes = null;
			return;
		}
		const e = t.entry;
		kind = !e || e.type === 'opening' ? 'transfer' : e.type;
		amount = t.amount;
		date = t.date;
		counterparty = t.counterparty;
		description = t.description || null;
		notes = t.notes;
		lines = [{ account: null, amount: null }];
		if (e?.type === 'expense' || e?.type === 'income') {
			party = e.type === 'expense' ? e.payer : e.destination;
			lines = e.lines.map((l) => ({
				account: l.account,
				amount: l.amount,
				budgetLine: l.budgetLine,
				costCentre: l.costCentre
			}));
		}
		if (e?.type === 'transfer') {
			party = e.from;
			other = e.to;
		}
		if (e?.type === 'settle') {
			party = e.person;
			other = e.account;
			direction = e.direction;
		}
		if (e?.type === 'opening') other = e.account;
	}

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
	// A transfer goes from any account to any other (not the opening account).
	const everywhere = $derived([
		...sources,
		...snapshot.accounts
			.filter((a) => a.kind === 'income')
			.map((a) => ({ value: a.id, label: a.name, group: 'Income' })),
		...snapshot.accounts
			.filter((a) => a.kind === 'expense')
			.map((a) => ({ value: a.id, label: a.name, group: 'Expenses' }))
	]);
	// The line's own category first, then the others under their names.
	function budgetOptions(category: AccountId | null) {
		const planned = snapshot.budgetLines.filter((b) =>
			categories.some((c) => c.value === b.category)
		);
		return [
			{ value: 0, label: 'Not budgeted' },
			...planned
				.filter((b) => b.category === category)
				.map((b) => ({ value: b.id, label: b.description })),
			...planned
				.filter((b) => b.category !== category)
				.map((b) => ({
					value: b.id,
					label: b.description,
					group: byId.get(b.category)?.name
				}))
		];
	}

	function pickBudgetLine(i: number, id: BudgetLineId) {
		const b = snapshot.budgetLines.find((p) => p.id === id);
		lines[i].budgetLine = b ? id : null;
		if (b) {
			lines[i].account = b.category;
			lines[i].costCentre = b.costCentre;
			clearError('lines');
		}
	}

	const personOptions = $derived(people.map((a) => ({ value: a.id, label: a.name })));
	const assetOptions = $derived(assets.map((a) => ({ value: a.id, label: a.name })));

	// --- derived figures --------------------------------------------------------------
	const summary = (id: AccountId | null) => snapshot.persons.find((p) => p.account === id)?.summary;
	const split = $derived(lines.length > 1);
	const left = $derived(
		split && amount !== null ? amount - lines.reduce((s, l) => s + (l.amount ?? 0), 0) : null
	);
	// A person's balance without the settle being edited, so changing one reads like entering it.
	function balanceOf(id: AccountId | null) {
		const n = summary(id)?.net ?? 0;
		const e = txn?.entry;
		if (e?.type !== 'settle' || e.person !== id) return n;
		return e.direction === 'reimburse' ? n + e.amount : n - e.amount;
	}
	const net = $derived(balanceOf(party));
	const after = $derived(
		kind !== 'settle' || party === null || amount === null
			? null
			: direction === 'reimburse'
				? net - amount
				: net + amount
	);
	const overshoots = $derived(
		after !== null && (direction === 'reimburse' ? after < 0 : after > 0)
	);

	function afterText(cents: Cents) {
		const name = byId.get(party!)?.name ?? '';
		const figure = formatAmount(cents, format, 'magnitude');
		if (overshoots) return `Too much: ${name} would ${cents < 0 ? 'owe' : 'be owed'} ${figure}`;
		return cents > 0 ? `${name} is still owed ${figure}` : `${name} still owes ${figure}`;
	}

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
		const n = balanceOf(id);
		direction = n >= 0 ? 'reimburse' : 'repay';
		if (n !== 0 && txn === null) amount = Math.abs(n);
		other ??= snapshot.event.defaultReimbursementAccount;
		clearError('party');
		generation++;
	}

	// A plugin's proposal, for the treasurer to check. Nothing is saved until they do.
	function fillProposal(p: EntryProposal) {
		kind = p.kind;
		amount = p.amount;
		date = p.date ?? date;
		description = p.description ?? null;
		notes = p.notes ?? null;
		if (p.kind === 'transfer') {
			party = p.from;
			other = p.to;
			return;
		}
		counterparty = p.counterparty;
		party =
			(p.kind === 'expense' ? p.payer : p.destination) ??
			snapshot.event.defaultReimbursementAccount;
		if (p.lines.length) lines = p.lines.map((l) => ({ ...l, amount: l.amount ?? null }));
	}

	// --- adding people and categories on the spot ------------------------------------
	async function addPerson(name: string) {
		try {
			return (await session.change(() => api.addPerson(name))).id;
		} catch (e) {
			error = { field: 'party', message: errorMessage(e) };
			return null;
		}
	}

	async function addCategory(name: string) {
		try {
			const added = await session.change(() =>
				api.addAccount(kind === 'income' ? 'income' : 'expense', name)
			);
			return added.id;
		} catch (e) {
			error = { field: 'lines', message: errorMessage(e) };
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
				budgetLine: l.budgetLine ?? null,
				costCentre: l.costCentre ?? null
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
		// A value typed just before the click is still being read. One that cannot be read shows
		// its own message, and must not be saved as the value it replaced.
		await Promise.all([amountsRead(), datesRead()]);
		if (amountInvalid || dateInvalid || (split && lineInvalid.slice(0, lines.length).some(Boolean)))
			return;
		const entry = build();
		if (!entry || !date) {
			if (!date) error = { field: 'date', message: 'enter a date' };
			return;
		}
		const total = kind === 'expense' || kind === 'income' ? amount : null;
		try {
			const files = [...pending];
			const saved = await session.change(() =>
				api.saveTxn(
					{
						date: date!,
						description: description ?? '',
						counterparty,
						notes,
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

	// A category chosen for a transfer means nothing to the other forms.
	function changeKind() {
		error = null;
		lines = [{ account: null, amount: null }];
		if (kind === 'transfer') return;
		if (!sources.some((o) => o.value === party)) party = null;
		if (!assetOptions.some((o) => o.value === other)) other = null;
	}

	async function remove() {
		if (!txn) return;
		await session.change(() => api.deleteTxn(txn.id));
		onsaved(null);
		onclose();
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !locked) {
			e.preventDefault();
			save(e.shiftKey);
		}
	}

	const fieldError = (name: string) => (error?.field === name ? error.message : null);

	// A field that was corrected stops showing why it was refused. Its message would otherwise

	// appear when the field loses focus and move Save away from under the click.

	const clearError = (name: string) => {
		if (error?.field === name) error = null;
	};
	const partyLabel = $derived(
		txn?.kind === 'refund'
			? 'Paid by'
			: { expense: 'Paid by', income: 'Received by', transfer: 'From', settle: 'Person' }[kind]
	);

	// Loaded once, after everything above is defined: the ledger remounts the panel for every
	// entry it opens.
	untrack(() => {
		load(txn);
		if (!txn && settle !== null) {
			kind = 'settle';
			pickPerson(settle);
		}
		if (!txn && proposal) fillProposal(proposal);
	});
</script>

<svelte:window onkeydown={keydown} />

<Panel {onclose} {onescape}>
	{#snippet header()}
		<fieldset class="plain" disabled={locked}>
			<Segmented
				label="Kind"
				bind:value={kind}
				onchange={changeKind}
				options={[
					{ value: 'expense', label: 'Expense' },
					{ value: 'income', label: 'Income' },
					{ value: 'transfer', label: 'Transfer' },
					{ value: 'settle', label: 'Settle' }
				]}
			/>
		</fieldset>
	{/snippet}

	<!-- B4: "Save & next" turns the form like a page; both forms share one grid cell. -->
	<div class="stage">
		{#key generation}
			<div class="gen" in:formIn={{ level: level('B4') }} out:formOut={{ level: level('B4') }}>
				<!-- A closed event shows its entries as they are (R-EVT-2). Its receipts stay outside,
				     so they can still be previewed. -->
				<fieldset class="plain" disabled={locked}>
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
							<span class="form-label">Direction</span>
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
							bind:invalid={amountInvalid}
							oninput={(v) => {
								amount = v;
								clearError('amount');
							}}
							oncommit={(v) => (amount = v)}
						/>
						{#if after === 0}
							<span class="after settled"
								><Icon name="check" size={14} />{byId.get(party!)?.name} is settled</span
							>
						{:else if after !== null}
							<span class="after" class:over={overshoots}>{afterText(after)}</span>
						{/if}
						{#if fieldError('amount')}<span class="form-error">{fieldError('amount')}</span>{/if}
					</div>

					{#if kind !== 'settle'}
						<ChoiceField
							label={partyLabel}
							value={party}
							options={kind === 'transfer' ? everywhere : sources}
							addLabel="New person…"
							error={fieldError('party')}
							onchange={(v) => {
								party = v;
								clearError('party');
							}}
							onadd={addPerson}
						>
							{#snippet hint()}{balanceHint(party) ?? ''}{/snippet}
						</ChoiceField>
					{/if}

					{#if kind === 'transfer'}
						<ChoiceField
							label="To"
							value={other}
							options={everywhere}
							addLabel="New person…"
							error={fieldError('other')}
							onchange={(v) => {
								other = v;
								clearError('other');
							}}
							onadd={addPerson}
						>
							{#snippet hint()}{balanceHint(other) ?? ''}{/snippet}
						</ChoiceField>
					{:else if kind === 'settle'}
						<ChoiceField
							label={direction === 'reimburse' ? 'From' : 'To'}
							value={other}
							options={assetOptions}
							addLabel="New account…"
							error={fieldError('other')}
							onchange={(v) => {
								other = v;
								clearError('other');
							}}
							onadd={async (name) => {
								try {
									return (await session.change(() => api.addAccount('asset', name))).id;
								} catch (e) {
									error = {
										field: 'other',
										message: errorMessage(e)
									};
									return null;
								}
							}}
						/>
					{/if}

					{#if kind === 'expense' || kind === 'income'}
						<div class="split">
							<div class="split-head">
								<span class="form-label">{split ? 'Split' : 'Category'}</span>
								{#if left !== null}
									<span class="left" class:done={left === 0}>{formatAmount(left, format)} left</span
									>
								{/if}
							</div>
							{#each lines as line, i (i)}
								{@const account = line.account === null ? undefined : byId.get(line.account)}
								{@const fixed = snapshot.budgetLines.find((b) => b.id === line.budgetLine)}
								<div class="split-line">
									<div class="line">
										<span
											class="dot"
											style:background={account ? categoryColor(account, snapshot.accounts) : null}
										></span>
										<div class="category">
											<ChoiceField
												value={line.account}
												options={categories}
												addLabel="New category…"
												invalid={!!fieldError('lines') && line.account === null}
												onchange={(v) => {
													if (v !== lines[i].account) {
														lines[i].budgetLine = null;
														lines[i].costCentre = byId.get(v)?.costCentre ?? null;
													}
													lines[i].account = v;
													clearError('lines');
												}}
												onadd={addCategory}
											/>
										</div>
										{#if split}
											<div class="line-amount">
												<AmountField
													value={line.amount}
													{format}
													bind:invalid={() => lineInvalid[i] ?? false, (v) => (lineInvalid[i] = v)}
													oninput={(v) => {
														lines[i].amount = v;
														clearError('lines');
													}}
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
									<div class="line-details">
										<Select
											label="Budget line"
											value={line.budgetLine ?? 0}
											options={budgetOptions(line.account)}
											onchange={(v) => pickBudgetLine(i, v)}
										/>
										<fieldset class="plain" disabled={!!fixed}>
											<Select
												label="Cost centre"
												value={line.costCentre ?? 0}
												options={[
													{ value: 0, label: 'None' },
													...snapshot.costCentres.map((c) => ({ value: c.id, label: c.name }))
												]}
												onchange={(v) => (lines[i].costCentre = v === 0 ? null : v)}
											/>
										</fieldset>
									</div>
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
							{#if fieldError('lines')}<span class="form-error">{fieldError('lines')}</span>{/if}
						</div>
					{/if}

					<div class="form-pair">
						<DateField
							label="Date"
							value={date}
							{format}
							required
							bind:invalid={dateInvalid}
							oninput={(v) => {
								if (v) date = v;
								clearError('date');
							}}
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
					<TextField
						label="Notes"
						value={notes}
						multiline
						oninput={(t) => (notes = t.trim() || null)}
						oncommit={(v) => (notes = v)}
					/>
				</fieldset>
				{#if origin.type === 'manual' && (!locked || attachments.length)}
					<Receipts
						{attachments}
						{pending}
						{previewing}
						readonly={locked}
						{onpreview}
						onremove={(a) => (removed = [...removed, a.id])}
						onremovePending={(i) => (pending = pending.filter((_, j) => j !== i))}
						onadd={(paths) =>
							(pending = [...pending, ...paths.filter((p) => !pending.includes(p))])}
					/>
				{/if}
				{#if error && !['party', 'other', 'lines', 'amount'].includes(error.field ?? '')}
					<p class="form-error">{error.message}</p>
				{/if}
			</div>
		{/key}
	</div>

	{#snippet footer()}
		{#if origin.type !== 'manual'}
			<span class="closed"><Icon name="lock" size={14} />From tickets</span>
			<span class="grow"></span>
			<Button
				size="sm"
				onclick={() =>
					goto(
						origin.type === 'ticketImport'
							? `/event/tickets?tier=${origin.tier}`
							: origin.type === 'ticketSale'
								? `/event/tickets?sale=${origin.sale}`
								: origin.type === 'ticketRefund'
									? `/event/tickets?cancellation=${origin.cancellation}`
									: '/event/tickets'
					)}>Tickets</Button
			>
		{:else if session.closed}
			<span class="closed"><Icon name="lock" size={14} />Closed</span>
		{:else if elsewhere}
			<span class="closed"><Icon name="lock" size={14} />Opening count</span>
			<span class="grow"></span>
			<Button size="sm" onclick={() => goto('/event/overview')}>Overview</Button>
		{:else if confirming}
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
	.stage {
		display: grid;
	}

	.gen {
		grid-area: 1 / 1;
		display: flex;
		flex-direction: column;
		gap: 18px;
		min-width: 0;
	}

	.closed {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		font-weight: 600;
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

	.split-line {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
	}

	/* A rule between lines, so each line's details read as its own. */
	.split-line + .split-line {
		margin-top: 6px;
		padding-top: 12px;
		border-top: 1px solid var(--color-rule);
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

	.line-details {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		gap: var(--spacing-2);
		padding-left: 14px;
	}

	.line-details fieldset {
		min-width: 0;
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

	.after {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.after.settled {
		color: var(--color-plus);
	}

	.after.over {
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
</style>
