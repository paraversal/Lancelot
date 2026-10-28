<script lang="ts">
	// Overview (R-SET-1, R-SET-5): the result, where the money came from and went, where it is
	// now, and what is still open. Cash accounts are counted here; open items lead to their fix.
	import { goto } from '$app/navigation';
	import type { OpenItem } from '#lib/api/generated/OpenItem.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import StackedBar from '#lib/components/StackedBar.svelte';
	import { categoryColor } from '#lib/colors.ts';
	import { formatAmount } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import CountPanel from './CountPanel.svelte';

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const totals = $derived(snapshot.totals);
	const where = $derived(snapshot.whereTheMoneyIs);

	let counting = $state<number | null>(null);
	let width = $state(0);

	const name = (id: number) => snapshot.accounts.find((a) => a.id === id)?.name ?? '';
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

	function item(i: OpenItem): { text: string; amount: number | null; go: () => void } {
		switch (i.type) {
			case 'personBalance':
				return {
					text: `${name(i.person)} ${i.net > 0 ? 'is owed' : 'holds'}`,
					amount: Math.abs(i.net),
					go: () => goto(`/event/people?person=${i.person}`)
				};
			case 'missingReceipt': {
				const txn = snapshot.txns.find((t) => t.id === i.txn);
				return {
					text: `No receipt: ${txn?.description || 'entry'}`,
					amount: txn?.amount ?? null,
					go: () => goto(`/event/ledger?txn=${i.txn}`)
				};
			}
			case 'closingCountMissing':
				return {
					text: `${name(i.account)} not counted`,
					amount: null,
					go: () => (counting = i.account)
				};
			case 'cashDifference':
				return {
					text: `${name(i.account)} count differs`,
					amount: i.difference,
					go: () => (counting = i.account)
				};
		}
	}
	const open = $derived(snapshot.openItems.map(item));
</script>

<svelte:window
	onkeydown={(e) => {
		const target = e.target instanceof Element ? e.target : null;
		if (target?.closest('input, textarea, select') || e.metaKey || e.ctrlKey || counting !== null)
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
			<h1>Overview</h1>
			<Button kind="primary" icon="plus" shortcut="N" onclick={() => goto('/event/ledger?new')}
				>New entry</Button
			>
		</header>

		<div class="hero">
			<div class="figure">
				<span class="label">Result so far</span>
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
				<h2>Where the money is</h2>
				<ul>
					{#each assets as { account, cash } (account.id)}
						<li>
							<span class="name">
								{account.name}
								{#if cash?.counted != null}
									<span class:minus={!!cash.difference} class="note">
										counted {formatAmount(cash.counted, format)}
										{#if cash.difference}&nbsp;{formatAmount(
												cash.difference,
												format,
												'always'
											)}{/if}
									</span>
								{:else if account.assetType === 'cash'}
									<span class="note attention">not counted</span>
								{/if}
							</span>
							{#if account.assetType === 'cash' || cash}
								<Button size="sm" onclick={() => (counting = account.id)}>Count</Button>
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
			</section>
		</div>
	</div>

	{#if counting !== null}
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
