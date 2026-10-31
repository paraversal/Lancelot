<script lang="ts">
	// Overview (R-SET-1, R-SET-5): the result and its forecast, where the money came from and
	// went, where it is now, and what is still open. Cash accounts are counted here; open items lead to their fix.
	// Closing happens here too; a closed event shows its final balance and what was accepted.
	import { goto } from '$app/navigation';
	import { api, errorMessage } from '#lib/api/index.ts';
	import { chooseCsvPath } from '#lib/api/dialogs.ts';
	import type { OpenItem } from '#lib/api/generated/OpenItem.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import StackedBar from '#lib/components/StackedBar.svelte';
	import { categoryColor } from '#lib/colors.ts';
	import { differenceText, formatAmount, formatDate } from '#lib/format.ts';
	import { rowOut, riseIn, stampIn } from '#lib/motion/effects.ts';
	import { centreOf, confetti, sparkle } from '#lib/motion/confetti.ts';
	import { isPlayful, level } from '#lib/motion/mode.svelte.ts';
	import MovingAmount from '#lib/motion/MovingAmount.svelte';
	import Check from '#lib/motion/Check.svelte';
	import Lock from '#lib/motion/Lock.svelte';
	import { describe } from '#lib/openItems.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import { totalQuota } from '#lib/tickets.ts';
	import ClosePanel from './ClosePanel.svelte';
	import CountPanel from './CountPanel.svelte';
	import StatementPanel from './StatementPanel.svelte';

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const totals = $derived(snapshot.totals);
	const where = $derived(snapshot.whereTheMoneyIs);
	const budget = $derived(snapshot.budget);
	// The forecast (R-BUD-4) needs a budget, and means nothing once the event is closed.
	const forecast = $derived(snapshot.budgetLines.length > 0 && !session.closed);
	// What it takes to break even (R-TKT-9), once there is a gap and a tier to close it.
	const breakEven = $derived(
		snapshot.profitability?.gap && snapshot.profitability.tickets !== null
			? snapshot.profitability
			: null
	);
	const short = $derived(
		breakEven?.placesLeft != null && breakEven.tickets! > breakEven.placesLeft
	);
	const tickets = $derived(snapshot.tickets);
	const quota = $derived(totalQuota(tickets.tiers));
	const hasTickets = $derived(tickets.tiers.length > 0);
	const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

	let counting = $state<number | null>(null);
	let reconciling = $state<number | null>(null);
	const a5 = $derived(level('A5'));
	const countUp = isPlayful(level('D1'));
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
			error = errorMessage(e);
		}
	}
	let width = $state(0);

	// Refunds are negative income: the bar shows tickets net of them, so it adds up to the total.
	const refunds = $derived(
		snapshot.accounts.find((a) => a.role === 'ticket_refunds')?.balance ?? 0
	);
	const flows = (kind: 'income' | 'expense') =>
		snapshot.accounts
			.map((a) => ({ a, value: a.role === 'tickets' ? a.balance + refunds : a.balance }))
			.filter(({ a, value }) => a.kind === kind && value > 0)
			.map(({ a, value }) => ({
				label: a.name,
				value,
				color: categoryColor(a, snapshot.accounts)
			}));
	const max = $derived(Math.max(totals.income, totals.expense, 1));
	// The bars span the row between the "In"/"Out" label and the total, gaps included.
	const barWidth = $derived(Math.max(120, width - 56 - 140 - 2 * snapshot.accounts.length));

	const assets = $derived(
		snapshot.accounts
			.filter((a) => a.kind === 'asset')
			.map((a) => ({
				account: a,
				cash: snapshot.cash.find((c) => c.account === a.id) ?? null,
				// A mismatch stays open whatever came after, so it shows first.
				statement:
					snapshot.reconciliations.findLast((r) => r.account === a.id && r.difference) ??
					snapshot.reconciliations.findLast((r) => r.account === a.id) ??
					null
			}))
	);

	// One panel at a time.
	function panel(which: 'count' | 'statement' | 'close', account: number | null = null) {
		closing = which === 'close';
		counting = which === 'count' ? account : null;
		reconciling = which === 'statement' ? account : null;
	}

	function item(i: OpenItem) {
		const go =
			i.type === 'personBalance'
				? () => goto(`/event/people?person=${i.person}`)
				: i.type === 'missingReceipt'
					? () => goto(`/event/ledger?txn=${i.txn}`)
					: i.type === 'ticketConflicts'
						? () => goto('/event/tickets')
						: i.type === 'reconciliationMismatch'
							? () => panel('statement', i.account)
							: () => panel('count', i.account);
		return { ...describe(i, snapshot, format), go, key: JSON.stringify(i) };
	}
	const open = $derived(snapshot.openItems.map(item));

	// E3: the last open item resolved. Only while the event is open: closing with accepted items
	// is E1's moment.
	let openSection = $state<HTMLElement>();
	let hadOpen = false;
	$effect(() => {
		const n = open.length;
		if (hadOpen && n === 0 && !closed && openSection) {
			const l = level('E3');
			if (l === 'big') confetti(centreOf(openSection), 90, 0.9);
			else if (isPlayful(l)) sparkle(centreOf(openSection));
		}
		hadOpen = n > 0;
	});
</script>

<svelte:window
	onkeydown={(e) => {
		const target = e.target instanceof Element ? e.target : null;
		if (
			target?.closest('input, textarea, select') ||
			e.metaKey ||
			e.ctrlKey ||
			counting !== null ||
			reconciling !== null ||
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
				{#if closedOn}<span class="closed" in:stampIn={{ level: level('E1') }}
						><Lock closed level={level('E1')} size={14} />Closed {closedOn}</span
					>{/if}
			</div>
			<div class="actions">
				<Button icon="file" onclick={() => goto('/event/overview/report')}>Report</Button>
				<Button onclick={exportCsv}>CSV</Button>
				{#if !closed}
					<Button icon="lock" onclick={() => panel('close')}>Close event</Button>
					<Button kind="primary" icon="plus" shortcut="N" onclick={() => goto('/event/ledger?new')}
						>New entry</Button
					>
				{/if}
			</div>
		</header>
		{#if error}<p class="form-error" role="alert">{error}</p>{/if}

		<div class="hero" in:riseIn|global={{ level: level('D1'), i: 0 }}>
			<div class="figure">
				<span class="label">{closed ? 'Result' : 'Result so far'}</span>
				<span
					class="result"
					class:plus={totals.result > 0}
					class:minus={totals.result < 0}
					data-testid="result"
					><MovingAmount value={totals.result} {format} sign="always" level={a5} {countUp} /></span
				>
			</div>
			{#if forecast}
				<button
					type="button"
					class="figure forecast"
					data-testid="forecast"
					onclick={() => goto('/event/budget')}
				>
					<span class="label">Forecast<Icon name="chev" size={12} /></span>
					<span
						class="total"
						class:plus={budget.result.forecast > 0}
						class:minus={budget.result.forecast < 0}
						><MovingAmount value={budget.result.forecast} {format} sign="always" level={a5} /></span
					>
					{#if breakEven}
						<span class="note">
							~{count(breakEven.tickets!, 'ticket', 'tickets')} to break even
							{#if short}<span class="attention"
									>· {count(breakEven.placesLeft ?? 0, 'place', 'places')} left</span
								>{/if}
						</span>
					{:else}
						<span class="note"
							>planned {formatAmount(budget.result.estimate, format, 'always')}</span
						>
					{/if}
				</button>
			{/if}
			{#if hasTickets}
				<button
					type="button"
					class="figure tickets"
					data-testid="tickets"
					onclick={() => goto('/event/tickets')}
				>
					<span class="label">Tickets<Icon name="chev" size={12} /></span>
					<span class="total"
						>{tickets.total.places}{#if quota !== null}{' '}<span class="quota">/ {quota}</span
							>{/if}</span
					>
					{#if quota !== null}
						<span class="bar" class:over={tickets.total.places > quota}>
							<span style:width="{quota ? Math.min(100, (tickets.total.places / quota) * 100) : 0}%"
							></span>
						</span>
					{/if}
				</button>
			{/if}
			<!-- Beside both the forecast and the tickets, the In and Out rows carry these totals. -->
			{#if !(forecast && hasTickets)}
				<div class="figure">
					<span class="label">Income</span>
					<span class="total"><MovingAmount value={totals.income} {format} level={a5} /></span>
				</div>
				<div class="figure">
					<span class="label">Expenses</span>
					<span class="total"><MovingAmount value={totals.expense} {format} level={a5} /></span>
				</div>
			{/if}
		</div>

		{#if totals.income || totals.expense}
			<div class="flows" bind:clientWidth={width} in:riseIn|global={{ level: level('D1'), i: 1 }}>
				{#each [{ label: 'In', kind: 'income', total: totals.income }, { label: 'Out', kind: 'expense', total: totals.expense }] as const as f (f.kind)}
					<div class="flow">
						<strong class="side">{f.label}</strong>
						<StackedBar segments={flows(f.kind)} {max} {format} width={barWidth} />
						<strong class="sum"><MovingAmount value={f.total} {format} level={a5} /></strong>
					</div>
				{/each}
			</div>
		{/if}

		<div class="columns" in:riseIn|global={{ level: level('D1'), i: 2 }}>
			<section>
				<h2>{closed ? 'Final balance' : 'Where the money is'}</h2>
				<ul>
					{#each assets as { account, cash, statement } (account.id)}
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
								{#if statement}
									<span class:minus={!!statement.difference} class="note">
										statement {formatDate(statement.date, format)}
										{#if statement.difference}· {differenceText(
												statement.difference,
												format
											)}{:else}✓{/if}
									</span>
								{/if}
							</span>
							{#if !closed}
								<Button size="sm" onclick={() => panel('count', account.id)}>Count</Button>
							{/if}
							{#if !closed}
								<Button size="sm" onclick={() => panel('statement', account.id)}>Statement</Button>
							{/if}
							<span class="amount"
								><MovingAmount value={account.balance} {format} level={a5} /></span
							>
						</li>
					{/each}
					{#if where.heldByPeople}
						<li>
							<span class="name">Held by people</span>
							<span class="amount"
								><MovingAmount value={where.heldByPeople} {format} level={a5} /></span
							>
						</li>
					{/if}
					{#if where.owedToPeople}
						<li>
							<span class="name">Owed to people</span>
							<span class="amount"
								><MovingAmount value={-where.owedToPeople} {format} level={a5} /></span
							>
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
							><MovingAmount value={where.total} {format} sign="always" level={a5} /></span
						>
					</li>
				</ul>
			</section>

			<section bind:this={openSection}>
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
							{#each open as o (o.key)}
								<li out:rowOut={{ level: level('D3') }}>
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
						<p class="done"><Check level={level('E3')} size={16} delay={120} />Nothing open</p>
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
	{:else if reconciling !== null && !closed}
		{#key reconciling}
			<StatementPanel account={reconciling} onclose={() => (reconciling = null)} />
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
		display: flex;
		align-items: center;
		gap: 2px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.forecast,
	.tickets {
		margin: 0;
		border-top: none;
		border-right: none;
		border-bottom: none;
		background: none;
		font: inherit;
		color: inherit;
		text-align: left;
		cursor: pointer;
	}

	.forecast:hover .label,
	.tickets:hover .label {
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	/* The bar gives way when a panel leaves the hero too little room. */
	.tickets {
		min-width: 170px;
	}

	.quota {
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	.bar {
		display: block;
		width: 200px;
		max-width: 100%;
		height: 6px;
		overflow: hidden;
		border-radius: 3px;
		background: var(--color-rule);
	}

	.bar span {
		display: block;
		height: 100%;
		border-radius: 3px;
		background: var(--color-ink);
	}

	.bar.over {
		outline: 1px solid var(--color-attention);
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
		flex-wrap: wrap;
		align-items: baseline;
		gap: 2px var(--spacing-2);
		flex: 1;
		min-width: 0;
		padding: 6px 0;
	}

	.note {
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
