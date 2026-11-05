<script lang="ts">
	// One person: contact details and how they are reimbursed, what the event owes them (or they owe), and their
	// running balance (R-PER-3). "Edit" turns the panel into the form; "Settle" opens the
	// settle entry, filled in from the balance.
	import { goto } from '$app/navigation';
	import { api, ApiError } from '#lib/api/index.ts';
	import type { Person } from '#lib/api/generated/Person.ts';
	import type { PersonView } from '#lib/api/generated/PersonView.ts';
	import Button from '#lib/components/Button.svelte';
	import IbanField from '#lib/components/IbanField.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { formatAmount, formatDate, formatPlain } from '#lib/format.ts';
	import { boxOpen, stampIn } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import MovingAmount from '#lib/motion/MovingAmount.svelte';
	import { session } from '#lib/state/session.svelte.ts';

	let {
		person,
		editing = $bindable(false),
		onclose,
		onsettle
	}: {
		person: PersonView;
		editing?: boolean;
		onclose: () => void;
		onsettle: () => void;
	} = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const net = $derived(person.summary.net);
	const first = $derived(person.name.split(' ')[0]);
	let error = $state<{ field: string | null; message: string } | null>(null);

	const iban = $derived(person.iban ? person.iban.replace(/(.{4})/g, '$1 ').trim() : null);

	function details(patch: Partial<Person>): Person {
		const { holder, iban, bic, otherMethod, contact, notes } = person;
		return { holder, iban, bic, otherMethod, contact, notes, ...patch };
	}

	// Choosing "Other" asks for the text; only the text makes it the person's method. Going back
	// to the bank removes the text and keeps the bank details, which an other method never touched.
	let choosingOther = $state(false);
	const method = $derived(person.otherMethod || choosingOther ? 'other' : 'bank');

	function chooseMethod(next: 'bank' | 'other') {
		choosingOther = next === 'other';
		if (next === 'bank' && person.otherMethod) save(person.name, { otherMethod: null });
	}

	async function save(name: string, patch: Partial<Person> = {}) {
		error = null;
		try {
			await session.change(() => api.updatePerson(person.account, name, details(patch)));
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = { field: e.field, message: e.message };
		}
	}

	// A mistyped person can go, as long as nothing refers to them.
	let deleting = $state(false);

	async function remove() {
		error = null;
		deleting = false;
		try {
			await session.change(() => api.deleteAccount(person.account));
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = { field: null, message: e.message };
		}
	}

	// The running balance, newest first, with what each transaction was.
	const entries = $derived(
		[...person.entries].reverse().map((e) => {
			const txn = snapshot.txns.find((t) => t.id === e.txn);
			let title = txn?.description || txn?.counterparty || '';
			if (txn?.entry?.type === 'settle') {
				const { account, direction } = txn.entry;
				const name = snapshot.accounts.find((a) => a.id === account)?.name ?? '';
				title = `${direction === 'reimburse' ? 'Reimbursed' : 'Paid back'} · ${name}`;
			}
			return { ...e, title, date: txn?.date ?? '', settle: txn?.kind === 'settle' };
		})
	);

	const shortDate = (iso: string) => {
		const [, m, d] = iso.split('-');
		return format === 'de' ? `${d}.${m}` : `${m}-${d}`;
	};
</script>

<Panel {onclose} onescape={editing ? () => (editing = false) : onclose}>
	{#snippet header()}
		<div class="head">
			<h2>{person.name}</h2>
			{#if !editing && person.contact}<span class="muted">{person.contact}</span>{/if}
		</div>
	{/snippet}

	{#if editing}
		<TextField label="Name" value={person.name} required oncommit={(v) => v && save(v)} />
		<div class="labelled">
			<span class="form-label">Reimburse by</span>
			<Segmented
				label="Reimburse by"
				value={method}
				onchange={chooseMethod}
				options={[
					{ value: 'bank', label: 'Bank transfer' },
					{ value: 'other', label: 'Other' }
				]}
			/>
		</div>
		{#if method === 'other'}
			<div class="method" transition:boxOpen={{ level: level('A7') }}>
				<TextField
					label="Method"
					value={person.otherMethod}
					oncommit={(v) => save(person.name, { otherMethod: v })}
				>
					{#snippet hint()}Such as a PayPal address, or cash.{/snippet}
				</TextField>
			</div>
		{:else}
			<div class="method" transition:boxOpen={{ level: level('A7') }}>
				<TextField
					label="Account holder"
					value={person.holder}
					oncommit={(v) => save(person.name, { holder: v })}
				/>
				<IbanField value={person.iban} oncommit={(v) => save(person.name, { iban: v })} />
				<TextField label="BIC" value={person.bic} oncommit={(v) => save(person.name, { bic: v })} />
			</div>
		{/if}
		<TextField
			label="Contact"
			value={person.contact}
			oncommit={(v) => save(person.name, { contact: v })}
		/>
		<TextField
			label="Notes"
			value={person.notes}
			multiline
			oncommit={(v) => save(person.name, { notes: v })}
		/>
		{#if error}<p class="form-error">{error.message}</p>{/if}
	{:else}
		{#if error}<p class="form-error">{error.message}</p>{/if}
		<section>
			<span class="form-label">Reimburse by</span>
			{#if person.otherMethod}
				<strong class="method">{person.otherMethod}</strong>
			{:else if iban}
				<strong class="iban">{iban}</strong>
				<span class="muted"
					>{[person.holder ?? person.name, person.bic].filter(Boolean).join(' · ')}</span
				>
			{:else}
				<span class="attention">No method</span>
			{/if}
		</section>

		<section>
			{#if net > 0}
				<span class="form-label">Owed to {first}</span>
				<span class="figure"
					><MovingAmount value={Math.abs(net)} {format} level={level('A5')} /></span
				>
			{:else if net < 0}
				<span class="form-label">{first} owes</span>
				<span class="figure attention"
					><MovingAmount value={Math.abs(net)} {format} level={level('A5')} /></span
				>
			{:else}
				<span class="form-label">Balance</span>
				<span class="figure settled" in:stampIn={{ level: level('C1') }}>Settled</span>
			{/if}
		</section>

		{#if entries.length}
			<div class="entries">
				<div class="entries-head"><span>balance</span></div>
				{#each entries as e (e.txn)}
					<button type="button" class="entry" onclick={() => goto(`/event/ledger?txn=${e.txn}`)}>
						<span class="date">{shortDate(e.date)}</span>
						<span class="title">{e.title}</span>
						<span class="effect" class:plus={e.settle}>{formatAmount(e.effect, format)}</span>
						<span class="running">{formatPlain(e.balance, format)}</span>
					</button>
				{/each}
			</div>
		{/if}
	{/if}

	{#snippet footer()}
		{#if session.closed}
			<span></span>
		{:else if editing}
			<Button kind="primary" shortcut="esc" onclick={() => (editing = false)}>Done</Button>
		{:else}
			{#if net !== 0}
				<Button kind="primary" onclick={onsettle}>
					{net > 0 ? 'Settle' : 'Repaid'}
					{formatAmount(net, format, 'magnitude')}
				</Button>
			{/if}
			<span class="grow"></span>
			{#if !person.entries.length}
				{#if deleting}
					<Button size="sm" onclick={() => (deleting = false)}>Keep</Button>
					<Button size="sm" kind="primary" onclick={remove}>Delete</Button>
				{:else}
					<Button size="sm" onclick={() => (deleting = true)}>Delete</Button>
				{/if}
			{/if}
			<Button size="sm" onclick={() => (editing = true)}>Edit</Button>
		{/if}
	{/snippet}
</Panel>

<style>
	/* The method's fields, spaced as the panel spaces its own; the box opens around them (A7). */
	.method {
		display: flex;
		flex-direction: column;
		gap: 18px;
	}

	.head {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
	}

	h2 {
		margin: 0;
		font-size: var(--type-lg);
		font-weight: 600;
	}

	section {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.muted {
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.iban {
		font-weight: 600;
		letter-spacing: 0.01em;
	}

	.method {
		font-weight: 600;
		overflow-wrap: anywhere;
	}

	.labelled {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.figure {
		font-size: var(--type-xl);
		font-weight: 600;
		line-height: 1.1;
	}

	.attention {
		color: var(--color-attention);
		font-weight: 600;
	}

	.figure.settled {
		color: var(--color-ink-muted);
		font-size: var(--type-lg);
	}

	.entries {
		display: flex;
		flex-direction: column;
	}

	.entries-head {
		display: flex;
		justify-content: flex-end;
		padding-bottom: 4px;
		border-bottom: 1px solid var(--color-rule);
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.entry {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		height: 36px;
		padding: 0;
		border: none;
		border-bottom: 1px solid var(--color-rule);
		background: none;
		text-align: left;
		cursor: pointer;
	}

	.date {
		width: 40px;
		flex: none;
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.title {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.effect {
		width: 84px;
		flex: none;
		text-align: right;
	}

	.effect.plus {
		color: var(--color-plus);
	}

	.running {
		width: 60px;
		flex: none;
		font-size: 13px;
		color: var(--color-ink-muted);
		text-align: right;
	}
</style>
