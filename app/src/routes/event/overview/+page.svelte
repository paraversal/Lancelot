<script lang="ts">
	// Overview (R-SET-1, R-SET-5): the result, where the money came from and went, where it is
	// now, and what is still open. Cash accounts are counted here; open items lead to their fix.
	// Closing happens here too; a closed event shows its final balance and what was accepted.
	import { goto } from '$app/navigation';
	import { api, ApiError } from '#lib/api/index.ts';
	import { chooseCsvPath } from '#lib/api/dialogs.ts';
	import type { OpenItem } from '#lib/api/generated/OpenItem.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import StackedBar from '#lib/components/StackedBar.svelte';
	import { categoryColor } from '#lib/colors.ts';
	import { differenceText, formatAmount, formatDate } from '#lib/format.ts';
	import { describe } from '#lib/openItems.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import ClosePanel from './ClosePanel.svelte';
	import CountPanel from './CountPanel.svelte';

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const totals = $derived(snapshot.totals);
	const where = $derived(snapshot.whereTheMoneyIs);

	let counting = $state<number | null>(null);
	let closing = $state(false);
	let error = $state<string | null>(null);
	const closed = $derived(session.closed);
	const closedOn = $derived(
		snapshot.event.closedAt ? formatDate(snapshot.event.closedAt.slice(0, 10), format) : null
	);
	const accepted = $derived(
		snapshot.event.closingOverrides.map((o) => ({
			...describe(o.item, snapshot, format),
			note: o.note
		}))
	);

	async function exportCsv() {
		error = null;
		try {
			const target = await chooseCsvPath(`${snapshot.event.name} entries.csv`);
			if (target) await api.exportTransactionsCsv(target);
		} catch (e) {
			error = e instanceof ApiError ? e.message : String(e);
		}
	}
	let width = $state(0);

	const flows = (kind: 'income' | 'expense') =>
		snapshot.accounts
			.filter((a) => a.kind === kind && a.balance > 0)
			.map((a) => ({
				label: a.name,
				value: a.balance,
				color: categoryColor(a, snapshot.accounts)
			}));
	const max = $derived(Math.max(totals.income, totals.expense, 1));
	// The bars span the row between the "In"/"Out" label and the total, gaps included.
	const barWidth = $derived(Math.max(120, width - 56 - 140 - 2 * snapshot.accounts.length));

	const assets = $derived(
		snapshot.accounts
			.filter((a) => a.kind === 'asset')
			.map((a) => ({ account: a, cash: snapshot.cash.find((c) => c.account === a.id) ?? null }))
	);

	function item(i: OpenItem) {
		const go =
			i.type === 'personBalance'
				? () => goto(`/event/people?person=${i.person}`)
				: i.type === 'missingReceipt'
					? () => goto(`/event/ledger?txn=${i.txn}`)
					: () => (counting = i.account);
		return { ...describe(i, snapshot, format), go };
	}
	const open = $derived(snapshot.openItems.map(item));
</script>

<svelte:window
	onkeydown={(e) => {
		const target = e.target instanceof Element ? e.target : null;
		if (
			target?.closest('input, textarea, select') ||
			e.metaKey ||
			e.ctrlKey ||
			counting !== null ||
			closing ||
			closed
		)
			return;
		if (e.key === 'n' || e.key === 'N') {
			e.preventDefault();
			goto('/event/ledger?new');
		}
	}}
/>

<div class="overview">
	<div class="main">
		<header>
			<div class="title">
				<h1>Overview</h1>
				{#if closedOn}<span class="closed"><Icon name="lock" size={14} />Closed {closedOn}</span
					>{/if}
			</div>
			<div class="actions">
				<Button icon="file" onclick={() => goto('/event/overview/report')}>Report</Button>
				<Button onclick={exportCsv}>CSV</Button>
				{#if !closed}
					<Button icon="lock" onclick={() => ((closing = true), (counting = null))}
						>Close event</Button
					>
					<Button kind="primary" icon="plus" shortcut="N" onclick={() => goto('/event/ledger?new')}
						>New entry</Button
					>
				{/if}
			</div>
		</header>
		{#if error}<p class="error" role="alert">{error}</p>{/if}

		<div class="hero">
			<div class="figure">
				<span class="label">{closed ? 'Result' : 'Result so far'}</span>
				<span
					class="result"
					class:plus={totals.result > 0}
					class:minus={totals.result < 0}
					data-testid="result">{formatAmount(totals.result, format, 'always')}</span
				>
			</div>
			<div class="figure">
				<span class="label">Income</span>
				<span class="total">{formatAmount(totals.income, format)}</span>
			</div>
			<div class="figure">
				<span class="label">Expenses</span>
				<span class="total">{formatAmount(totals.expense, format)}</span>
			</div>
		</div>

		{#if totals.income || totals.expense}
			<div class="flows" bind:clientWidth={width}>
				{#each [{ label: 'In', kind: 'income', total: totals.income }, { label: 'Out', kind: 'expense', total: totals.expense }] as const as f (f.kind)}
					<div class="flow">
						<strong class="side">{f.label}</strong>
						<StackedBar segments={flows(f.kind)} {max} {format} width={barWidth} />
						<strong class="sum">{formatAmount(f.total, format)}</strong>
					</div>
				{/each}
			</div>
		{/if}

		<div class="columns">
			<section>
				<h2>{closed ? 'Final balance' : 'Where the money is'}</h2>
				<ul>
					{#each assets as { account, cash } (account.id)}
						<li>
							<span class="name">
								{account.name}
								{#if cash?.counted != null}
									<span class:minus={!!cash.difference} class="note">
										counted {formatAmount(cash.counted, format)}
										{#if cash.difference}· {differenceText(cash.difference, format)}{/if}
									</span>
								{:else if account.assetType === 'cash'}
									<span class="note attention">not counted</span>
								{/if}
							</span>
							{#if !closed && (account.assetType === 'cash' || cash)}
								<Button size="sm" onclick={() => ((counting = account.id), (closing = false))}
									>Count</Button
								>
							{/if}
							<span class="amount">{formatAmount(account.balance, format)}</span>
						</li>
					{/each}
					{#if where.heldByPeople}
						<li>
							<span class="name">Held by people</span>
							<span class="amount">{formatAmount(where.heldByPeople, format)}</span>
						</li>
					{/if}
					{#if where.owedToPeople}
						<li>
							<span class="name">Owed to people</span>
							<span class="amount">{formatAmount(-where.owedToPeople, format)}</span>
						</li>
					{/if}
					{#if where.opening}
						<li>
							<span class="name">Opening counts</span>
							<span class="amount">{formatAmount(-where.opening, format)}</span>
						</li>
					{/if}
					<li class="sumrow">
						<span class="name">Result</span>
						<span class="amount" class:plus={where.total > 0} class:minus={where.total < 0}
							>{formatAmount(where.total, format, 'always')}</span
						>
					</li>
				</ul>
			</section>

			<section>
				{#if closed}
					<h2>Accepted when closing</h2>
					<ul>
						{#each accepted as a, i (i)}
							<li class="accepted">
								<span class="name">{a.text}<small>{a.note}</small></span>
								{#if a.amount !== null}<strong>{formatAmount(a.amount, format)}</strong>{/if}
							</li>
						{:else}
							<p class="done"><Icon name="check" size={16} />Everything was settled</p>
						{/each}
					</ul>
				{:else}
					<h2>Open</h2>
					{#if open.length}
						<ul>
							{#each open as o, i (i)}
								<li>
									<button type="button" class="item" onclick={o.go}>
										<span class="dot"></span>
										<span class="name">{o.text}</span>
										{#if o.amount !== null}
											<strong>{formatAmount(o.amount, format)}</strong>
										{/if}
										<span class="chevron"><Icon name="chev" size={16} /></span>
									</button>
								</li>
							{/each}
						</ul>
					{:else}
						<p class="done"><Icon name="check" size={16} />Nothing open</p>
					{/if}
				{/if}
			</section>
		</div>
	</div>

	{#if closing && !closed}
		<ClosePanel onclose={() => (closing = false)} />
	{:else if counting !== null && !closed}
		{#key counting}
			<CountPanel account={counting} onclose={() => (counting = null)} />
		{/key}
	{/if}
</div>

<style>
	.overview {
		display: flex;
		height: 100%;
	}

	.main {
		display: flex;
		flex-direction: column;
		gap: 28px;
		flex: 1;
		min-width: 0;
		padding: var(--spacing-10);
		overflow: auto;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		min-height: 40px;
	}

	.title,
	.actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
	}

	.actions {
		gap: var(--spacing-2);
	}

	.closed {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 28px;
		padding: 0 10px;
		border-radius: var(--radius-sm);
		background: var(--color-paper-sunk);
		font-size: 13px;
		font-weight: 600;
		color: var(--color-ink-muted);
	}

	li.accepted {
		align-items: flex-start;
		padding: 10px 0;
	}

	li.accepted .name {
		flex-direction: column;
		align-items: flex-start;
		gap: 2px;
	}

	li.accepted small {
		display: block;
		margin-top: 2px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.error {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-minus);
	}

	h1 {
		margin: 0;
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.hero {
		display: flex;
		padding-bottom: 28px;
		border-bottom: 1px solid var(--color-rule);
	}

	.figure {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 0 32px;
		border-left: 1px solid var(--color-rule);
	}

	.figure:first-child {
		padding-left: 0;
		border-left: none;
	}

	.label {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.result {
		font-size: 40px;
		font-weight: 600;
		line-height: 1.1;
	}

	.total {
		font-size: 22px;
		font-weight: 600;
		line-height: 1.6;
	}

	.flows {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-6);
		padding-bottom: 28px;
		border-bottom: 1px solid var(--color-rule);
	}

	.flow {
		display: flex;
		align-items: flex-start;
	}

	.side {
		width: 56px;
		flex: none;
		font-weight: 600;
		line-height: 14px;
	}

	.sum {
		width: 140px;
		flex: none;
		margin-left: auto;
		font-weight: 600;
		line-height: 14px;
		text-align: right;
	}

	.columns {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
		gap: 28px 48px;
	}

	h2 {
		margin: 0 0 4px;
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	li {
		display: flex;
		align-items: center;
		gap: 10px;
		min-height: 44px;
		border-bottom: 1px solid var(--color-rule);
	}

	li.sumrow {
		border-bottom: none;
		font-weight: 600;
	}

	.name {
		display: flex;
		align-items: baseline;
		gap: var(--spacing-2);
		flex: 1;
		min-width: 0;
	}

	.note {
		white-space: nowrap;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.amount {
		width: 120px;
		flex: none;
		text-align: right;
	}

	.plus {
		color: var(--color-plus);
	}

	.minus {
		color: var(--color-minus);
	}

	.attention {
		color: var(--color-attention);
	}

	.item {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		min-height: 44px;
		padding: 0;
		border: none;
		background: none;
		text-align: left;
		cursor: pointer;
	}

	.item:hover .name {
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.dot {
		width: 6px;
		height: 6px;
		flex: none;
		border-radius: 50%;
		background: var(--color-attention);
	}

	strong {
		font-weight: 600;
	}

	.chevron {
		display: flex;
		color: var(--color-ink-muted);
	}

	.done {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		margin: 12px 0 0;
		color: var(--color-plus);
	}
</style>
