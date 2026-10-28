<script lang="ts">
	// One person: contact and bank details, what the event owes them (or they owe), and their
	// running balance (R-PER-3). "Edit" turns the panel into the form; "Settle" opens the
	// settle entry, filled in from the balance.
	import { goto } from '$app/navigation';
	import { api, ApiError } from '#lib/api/index.ts';
	import type { Person } from '#lib/api/generated/Person.ts';
	import type { PersonView } from '#lib/api/generated/PersonView.ts';
	import Button from '#lib/components/Button.svelte';
	import IbanField from '#lib/components/IbanField.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { formatAmount, formatDate } from '#lib/format.ts';
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
	const fallback = $derived(
		snapshot.accounts.find((a) => a.id === snapshot.event.defaultReimbursementAccount)?.name
	);

	function details(patch: Partial<Person>): Person {
		const { holder, iban, bic, contact, notes } = person;
		return { holder, iban, bic, contact, notes, ...patch };
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

	// The running balance, newest first, with what each transaction was.
	const entries = $derived(
		[...person.entries].reverse().map((e) => {
			const txn = snapshot.txns.find((t) => t.id === e.txn);
			let title = txn?.description || txn?.counterparty || '';
			if (txn?.kind === 'settle') {
				const account = txn.lines
					.map((l) => snapshot.accounts.find((a) => a.id === l.account))
					.find((a) => a?.kind === 'asset');
				title = `${e.effect < 0 ? 'Reimbursed' : 'Paid back'} · ${account?.name ?? ''}`;
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
		<TextField
			label="Account holder"
			value={person.holder}
			oncommit={(v) => save(person.name, { holder: v })}
		/>
		<IbanField value={person.iban} oncommit={(v) => save(person.name, { iban: v })} />
		<TextField label="BIC" value={person.bic} oncommit={(v) => save(person.name, { bic: v })} />
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
		{#if error}<p class="error">{error.message}</p>{/if}
	{:else}
		<section>
			<span class="label">Bank</span>
			{#if iban}
				<strong class="iban">{iban}</strong>
				<span class="muted"
					>{[person.holder ?? person.name, person.bic].filter(Boolean).join(' · ')}</span
				>
			{:else}
				<span class="attention">No IBAN</span>
			{/if}
		</section>

		<section>
			{#if net > 0}
				<span class="label">Owed to {first}</span>
				<span class="figure">{formatAmount(net, format, 'magnitude')}</span>
			{:else if net < 0}
				<span class="label">{first} owes</span>
				<span class="figure attention">{formatAmount(net, format, 'magnitude')}</span>
			{:else}
				<span class="label">Balance</span>
				<span class="figure settled">Settled</span>
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
						<span class="running"
							>{formatAmount(e.balance, format, 'plain').replace(/\s?€|€/, '')}</span
						>
					</button>
				{/each}
			</div>
		{/if}
	{/if}

	{#snippet footer()}
		{#if editing}
			<Button kind="primary" shortcut="esc" onclick={() => (editing = false)}>Done</Button>
		{:else}
			{#if net !== 0}
				<Button kind="primary" onclick={onsettle}>
					{net > 0 ? 'Settle' : 'Repaid'}
					{formatAmount(net, format, 'magnitude')}
				</Button>
				{#if net > 0 && fallback}<span class="muted">from {fallback}</span>{/if}
			{/if}
			<span class="grow"></span>
			<Button size="sm" onclick={() => (editing = true)}>Edit</Button>
		{/if}
	{/snippet}
</Panel>

<style>
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

	.label {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.muted {
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.iban {
		font-weight: 600;
		letter-spacing: 0.01em;
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

	.grow {
		flex: 1;
	}

	.error {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-minus);
	}
</style>
