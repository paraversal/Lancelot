<script lang="ts">
	// People (R-PER-3): advanced, held, settled and the net balance per person, with totals.
	// A person opens in the panel; "Settle" there opens the settle entry.
	import { goto } from '$app/navigation';
	import { api } from '#lib/api/index.ts';
	import BalanceCell from '#lib/components/BalanceCell.svelte';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import EntryPanel from '#lib/entry/EntryPanel.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import PersonPanel from './PersonPanel.svelte';

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const people = $derived([...snapshot.persons].sort((a, b) => a.name.localeCompare(b.name)));
	const totals = $derived(snapshot.personTotals);

	let selected = $state<number | null>(null);
	let editing = $state(false);
	let settling = $state(false);
	const person = $derived(people.find((p) => p.account === selected) ?? null);

	const cell = (c: number) => (c === 0 ? '–' : formatAmount(c, format));

	function open(id: number) {
		selected = id;
		editing = false;
		settling = false;
	}

	async function add() {
		const base = 'New person';
		let name = base;
		for (let n = 2; snapshot.persons.some((p) => p.name.toLowerCase() === name.toLowerCase()); n++)
			name = `${base} ${n}`;
		const added = await session.run(() => api.addPerson(name));
		open(added.id);
		editing = true;
	}

	function keydown(e: KeyboardEvent) {
		const target = e.target instanceof Element ? e.target : null;
		if (target?.closest('input, textarea, select') || e.metaKey || e.ctrlKey || !people.length)
			return;
		if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
		e.preventDefault();
		const at = people.findIndex((p) => p.account === selected);
		const next = e.key === 'ArrowDown' ? Math.min(people.length - 1, at + 1) : Math.max(0, at - 1);
		open(people[next].account);
	}
</script>

<svelte:window onkeydown={keydown} />

<div class="people">
	<section class="table">
		<header>
			<h1>People</h1>
			<div class="actions">
				{#if snapshot.payouts.payouts.length || snapshot.payouts.repayments.length}
					<Button
						icon="file"
						shortcut={formatAmount(snapshot.payouts.payoutTotal, format)}
						onclick={() => goto('/event/people/payouts')}
					>
						Payouts
					</Button>
				{/if}
				<Button kind="primary" icon="plus" onclick={add}>Person</Button>
			</div>
		</header>

		{#if people.length}
			<div class="rows" role="table">
				<div class="row head" role="row">
					<span class="name"></span>
					<span class="num">Advanced</span>
					<span class="num">Holds</span>
					<span class="num">Settled</span>
					<span class="balance">Balance</span>
				</div>
				{#each people as p (p.account)}
					<button
						type="button"
						class="row"
						class:selected={selected === p.account}
						role="row"
						onclick={() => open(p.account)}
					>
						<span class="name">
							{p.name}
							{#if p.summary.net > 0 && !p.iban}
								<span class="missing"><Icon name="warn" size={14} />IBAN</span>
							{/if}
						</span>
						<span class="num">{cell(p.summary.advanced)}</span>
						<span class="num">{cell(p.summary.held)}</span>
						<span class="num">{cell(p.summary.settled)}</span>
						<span class="balance"><BalanceCell net={p.summary.net} {format} /></span>
					</button>
				{/each}
				<div class="row totals" role="row">
					<span class="name muted"
						>{people.length === 1 ? '1 person' : `${people.length} people`}</span
					>
					<span class="num">{cell(totals.advanced)}</span>
					<span class="num">{cell(totals.held)}</span>
					<span class="num">{cell(totals.settled)}</span>
					<span class="balance"
						><span class="muted">net</span> {formatAmount(totals.net, format)}</span
					>
				</div>
			</div>
		{/if}
	</section>

	{#if person && settling}
		{#key person.account}
			<EntryPanel
				txn={null}
				settle={person.account}
				previewing={null}
				onpreview={() => {}}
				onclose={() => (settling = false)}
				onsaved={(id) => id !== null && (settling = false)}
			/>
		{/key}
	{:else if person}
		{#key person.account}
			<PersonPanel
				{person}
				bind:editing
				onclose={() => (selected = null)}
				onsettle={() => (settling = true)}
			/>
		{/key}
	{/if}
</div>

<style>
	.people {
		display: flex;
		height: 100%;
	}

	.table {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-6);
		flex: 1;
		min-width: 0;
		padding: var(--spacing-10) var(--spacing-8) var(--spacing-10) var(--spacing-10);
		overflow: auto;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		min-height: 40px;
	}

	.actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
	}

	h1 {
		margin: 0;
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.rows {
		display: flex;
		flex-direction: column;
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		height: 40px;
		padding: 0 8px;
		border: none;
		border-bottom: 1px solid var(--color-rule);
		background: none;
		text-align: left;
		cursor: pointer;
	}

	.row.head {
		height: auto;
		padding-bottom: 6px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
		cursor: default;
	}

	.row.selected {
		border-radius: var(--radius-sm);
		background: var(--color-marker);
		font-weight: 600;
	}

	.row.totals {
		height: auto;
		padding-top: 10px;
		padding-bottom: 10px;
		border-bottom: none;
		font-weight: 600;
		cursor: default;
	}

	.name {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		flex: 1;
		min-width: 0;
	}

	.missing {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-attention);
	}

	.num {
		width: 100px;
		flex: none;
		text-align: right;
	}

	.balance {
		display: flex;
		justify-content: flex-end;
		align-items: center;
		gap: 6px;
		width: 140px;
		flex: none;
	}

	.muted {
		font-size: 13px;
		font-weight: 400;
		color: var(--color-ink-muted);
	}
</style>
