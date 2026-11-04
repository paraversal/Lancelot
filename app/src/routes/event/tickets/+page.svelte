<script lang="ts">
	// Tickets (requirements §5.6): the tiers with sold vs quota, gross and net, from the latest
	// import and the manual sales. Every figure comes from the core. Imports come from plugins, each
	// in its frame under the header, and are reviewed here. Tickets that left the latest import are
	// conflicts on top, each a card to resolve (R-TKT-13). A tier, the import, a manual sale and a
	// cancellation each open in the panel on the right. Beside the tiers, or under them when a panel
	// leaves no room: break-even and the sales per import (R-TKT-8, R-TKT-9).
	import { onMount, type Snippet } from 'svelte';
	import { page } from '$app/state';
	import type { CancellationId } from '#lib/api/generated/CancellationId.ts';
	import type { CancellationView } from '#lib/api/generated/CancellationView.ts';
	import type { ManualSaleId } from '#lib/api/generated/ManualSaleId.ts';
	import type { TicketBatch } from '#lib/api/generated/TicketBatch.ts';
	import type { TierId } from '#lib/api/generated/TierId.ts';
	import { tierColor } from '#lib/colors.ts';
	import BreakEven from '#lib/components/BreakEven.svelte';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import SalesChart from '#lib/components/SalesChart.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { pluginContext, pluginEnabled } from '#lib/plugins/host.ts';
	import PluginFrame from '#lib/plugins/PluginFrame.svelte';
	import { plugins } from '#lib/plugins/registry.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import {
		conflictCount,
		formatFee,
		orderName,
		otherTicketIncome,
		shortDay
	} from '#lib/tickets.ts';
	import CancellationPanel from './CancellationPanel.svelte';
	import ConflictCard from './ConflictCard.svelte';
	import ImportPanel from './ImportPanel.svelte';
	import SalePanel from './SalePanel.svelte';
	import TierPanel from './TierPanel.svelte';

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const tickets = $derived(snapshot.tickets);
	const latest = $derived(tickets.imports.at(-1) ?? null);
	const conflicts = $derived(conflictCount(tickets));
	const otherIncome = $derived(otherTicketIncome(snapshot));
	const ticketCategory = $derived(snapshot.accounts.find((a) => a.role === 'tickets')?.id);

	type Open =
		| { type: 'tier'; id: TierId | null; name?: string; then?: TicketBatch | null }
		| { type: 'import'; batch: TicketBatch | null }
		| { type: 'sale'; id: ManualSaleId | null }
		| { type: 'cancellation'; id: CancellationId | null };
	let panel = $state<Open | null>(null);
	let panelKey = $state(0);
	/** A plugin's own panel. It takes the place of the page's. */
	let pluginPanel = $state<Snippet | null>(null);

	function show(next: Open | null) {
		panel = next;
		pluginPanel = null;
		panelKey++;
	}

	function setPluginPanel(next: Snippet | null) {
		if (next) panel = null;
		pluginPanel = next;
	}

	const sources = plugins.flatMap((plugin) =>
		plugin.TicketSection
			? [{ plugin, ctx: pluginContext(plugin.id), Section: plugin.TicketSection }]
			: []
	);

	// Opened from elsewhere, e.g. generated income in the ledger: `?tier=2`, `?sale=3` or
	// `?cancellation=4`.
	onMount(() => {
		const tier = Number(page.url.searchParams.get('tier'));
		const sale = Number(page.url.searchParams.get('sale'));
		const cancellation = Number(page.url.searchParams.get('cancellation'));
		if (tickets.tiers.some((t) => t.id === tier)) show({ type: 'tier', id: tier });
		else if (tickets.manualSales.some((m) => m.id === sale)) show({ type: 'sale', id: sale });
		else if (tickets.cancellations.some((c) => c.id === cancellation))
			show({ type: 'cancellation', id: cancellation });
	});

	const tierName = (id: TierId) => tickets.tiers.find((t) => t.id === id)?.name ?? '';
	function cancellationText(c: CancellationView) {
		const what = `${c.count} × ${tierName(c.tier)}`;
		const from = c.order !== null ? orderName(c.order) : 'Manual sale';
		const how = c.terms ? `${formatFee(c.terms.refundBp, format)} refund` : 'Correction';
		return `${from} · ${what} · ${how}`;
	}

	const selectedTier = $derived(panel?.type === 'tier' ? panel.id : null);
	const order = $derived(tickets.tiers.map((t) => t.id));

	function keydown(e: KeyboardEvent) {
		const target = e.target instanceof Element ? e.target : null;
		if (target?.closest('input, textarea, select, aside') || e.metaKey || e.ctrlKey) return;
		if (e.key === 'n' && !session.closed) {
			e.preventDefault();
			show({ type: 'tier', id: null });
			return;
		}
		if ((e.key !== 'ArrowDown' && e.key !== 'ArrowUp') || !order.length) return;
		e.preventDefault();
		const at = order.indexOf(selectedTier ?? -1);
		const next = e.key === 'ArrowDown' ? Math.min(order.length - 1, at + 1) : Math.max(0, at - 1);
		show({ type: 'tier', id: order[next] });
	}
</script>

<svelte:window onkeydown={keydown} />

<div class="tickets">
	<section class="main">
		<header>
			<h1>Tickets</h1>
			<div class="actions">
				{#if latest}
					<button
						type="button"
						class="meta"
						title="Imports"
						onclick={() => show({ type: 'import', batch: null })}
					>
						Import {tickets.imports.length} · {shortDay(latest.date)} · {latest.filename}
					</button>
				{/if}
				{#if !session.closed}
					<Button
						icon="plus"
						disabled={!tickets.tiers.length}
						onclick={() => show({ type: 'sale', id: null })}>Manual sale</Button
					>
				{/if}
			</div>
		</header>

		{#if !session.closed}
			{#each sources as { plugin, ctx, Section } (plugin.id)}
				{#if pluginEnabled(plugin.id) && (!plugin.dataSource || ctx.dataSource)}
					<PluginFrame name={plugin.name}>
						<Section
							{ctx}
							proposeImport={(batch) => show({ type: 'import', batch })}
							setPanel={setPluginPanel}
						/>
					</PluginFrame>
				{/if}
			{/each}
		{/if}

		{#if tickets.conflicts.length}
			<section class="conflicts" aria-label="Conflicts">
				<h2>Conflicts <span class="badge">{conflicts}</span></h2>
				{#each tickets.conflicts as conflict (conflict.order)}
					<article class="card">
						<strong>{orderName(conflict.order)}</strong>
						{#each conflict.removed as removed (removed.tier)}
							{#key `${removed.count}|${removed.matches.length}`}
								<ConflictCard {conflict} {removed} />
							{/key}
						{/each}
					</article>
				{/each}
			</section>
		{/if}

		<div class="body">
			<div class="ledger">
				<div class="rows" role="table" aria-label="Tiers">
					<div class="row head" role="row">
						<span class="name"></span>
						<span class="num price">Price</span>
						<span class="num fee">Fee</span>
						<span class="sold">Sold</span>
						<span class="num gross">Gross</span>
					</div>
					{#each tickets.tiers as t, i (t.id)}
						<button
							type="button"
							class="row tier"
							class:selected={selectedTier === t.id}
							role="row"
							onclick={() => show({ type: 'tier', id: t.id })}
						>
							<span class="name">
								<span class="dot" style:background={tierColor(i)}></span>
								<span class="label">{t.name}</span>
								{#if t.manual}<span class="manual">+{t.manual} manual</span>{/if}
								{#if t.unresolved}<span class="unresolved">−{t.unresolved} open</span>{/if}
							</span>
							<span class="num price">{formatAmount(t.price, format)}</span>
							<span class="num fee">{formatFee(t.feeBp, format)}</span>
							<span class="sold">
								<span class="count">
									<strong>{t.places}</strong>{#if t.quota !== null}<span class="quota">
											/ {t.quota}</span
										>{/if}
								</span>
								{#if t.quota !== null}
									<span class="bar" class:over={t.places > t.quota}>
										<span
											style:width="{Math.min(100, (t.places / t.quota) * 100)}%"
											style:background={tierColor(i)}
										></span>
									</span>
								{/if}
							</span>
							<span class="num gross">{formatAmount(t.gross, format)}</span>
						</button>
					{/each}
					{#if !session.closed}
						<button
							type="button"
							class="row add"
							class:selected={panel?.type === 'tier' && panel.id === null}
							onclick={() => show({ type: 'tier', id: null })}
						>
							<Icon name="plus" size={14} />Tier
						</button>
					{/if}
					{#if tickets.tiers.length || otherIncome}
						{#if tickets.total.fees}
							<div class="row total" role="row">
								<span class="name">Ticket fees</span>
								<span class="num">{formatAmount(-tickets.total.fees, format)}</span>
							</div>
						{/if}
						{#if tickets.total.refunds}
							<div class="row total" role="row">
								<span class="name">Refunds</span>
								<span class="num">{formatAmount(-tickets.total.refunds, format)}</span>
							</div>
						{/if}
						{#if otherIncome}
							<div class="row total" role="row">
								<a class="name other" href="/event/ledger?category={ticketCategory}"
									>Other income in the ledger</a
								>
								<span class="num">{formatAmount(otherIncome, format)}</span>
							</div>
						{/if}
						<div class="row total strong" role="row">
							<span class="name">Ticket income</span>
							<span class="num">{formatAmount(tickets.total.net + otherIncome, format)}</span>
						</div>
						{#if conflicts}
							<p class="warning" role="note">
								{tickets.total.unresolved} removed {tickets.total.unresolved === 1
									? 'ticket is'
									: 'tickets are'} not counted until the conflicts are resolved
							</p>
						{/if}
					{/if}
				</div>

				{#if tickets.cancellations.length || (latest && !session.closed) || (tickets.manualSales.length && !session.closed)}
					<div class="rows cancellations" aria-label="Cancellations">
						<div class="row head"><span class="name">Cancellations</span></div>
						{#each tickets.cancellations as c (c.id)}
							<button
								type="button"
								class="row cancellation"
								class:selected={panel?.type === 'cancellation' && panel.id === c.id}
								onclick={() => show({ type: 'cancellation', id: c.id })}
							>
								<span class="name"><span class="label">{cancellationText(c)}</span></span>
								<span class="num">{c.refund ? formatAmount(-c.refund, format) : '–'}</span>
							</button>
						{/each}
						{#if !session.closed}
							<button
								type="button"
								class="row add"
								class:selected={panel?.type === 'cancellation' && panel.id === null}
								onclick={() => show({ type: 'cancellation', id: null })}
							>
								<Icon name="plus" size={14} />Cancellation
							</button>
						{/if}
					</div>
				{/if}
			</div>
			{#if snapshot.profitability || tickets.sales.length}
				<div class="statistics" role="group" aria-label="Statistics">
					{#if snapshot.profitability}
						<BreakEven
							profitability={snapshot.profitability}
							tiers={tickets.tiers}
							{format}
							onaddtier={() => show({ type: 'tier', id: null })}
						/>
					{/if}
					{#if snapshot.profitability && tickets.sales.length}
						<hr />
					{/if}
					{#if tickets.sales.length}
						<SalesChart points={tickets.sales} tiers={tickets.tiers} {format} />
					{/if}
				</div>
			{/if}
		</div>
	</section>

	{#key panelKey}
		{#if panel?.type === 'tier'}
			{@const then = panel.then}
			<TierPanel
				id={panel.id}
				name={panel.name}
				onclose={() => show(then !== undefined ? { type: 'import', batch: then } : null)}
			/>
		{:else if panel?.type === 'import'}
			<ImportPanel
				batch={panel.batch}
				onclose={() => show(null)}
				onnewtier={(name, batch) => show({ type: 'tier', id: null, name, then: batch })}
			/>
		{:else if panel?.type === 'sale'}
			<SalePanel
				id={panel.id}
				onopen={(id) => show({ type: 'sale', id })}
				onclose={() => show(null)}
			/>
		{:else if panel?.type === 'cancellation'}
			<CancellationPanel id={panel.id} onclose={() => show(null)} />
		{/if}
	{/key}
	{@render pluginPanel?.()}
</div>

<style>
	.tickets {
		display: flex;
		height: 100%;
	}

	.main {
		display: flex;
		flex-direction: column;
		gap: 28px;
		flex: 1;
		min-width: 0;
		padding: var(--spacing-10) var(--spacing-8) var(--spacing-10) var(--spacing-10);
		overflow: auto;
		container-type: inline-size;
	}

	.body {
		display: flex;
		flex-direction: column;
		gap: 28px;
	}

	.ledger {
		display: flex;
		flex-direction: column;
		gap: 28px;
		min-width: 0;
	}

	.statistics {
		display: flex;
		flex-direction: column;
		gap: 20px;
		min-width: 0;
		max-width: 560px;
	}

	.statistics hr {
		width: 100%;
		margin: 0;
		border: none;
		border-top: 1px solid var(--color-rule);
	}

	/* Beside the tiers when no panel takes the room, as in UI.fig. */
	@container (min-width: 920px) {
		.body {
			flex-direction: row;
			align-items: stretch;
			gap: var(--spacing-8);
		}

		.ledger {
			flex: 0 0 560px;
		}

		.statistics {
			flex: 1;
			padding-left: var(--spacing-8);
			border-left: 1px solid var(--color-rule);
		}
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-3);
		min-height: 40px;
	}

	h1 {
		margin: 0;
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		min-width: 0;
	}

	.meta {
		overflow: hidden;
		margin-right: var(--spacing-1);
		padding: 0;
		border: none;
		background: none;
		font-size: 13px;
		color: var(--color-ink-muted);
		text-overflow: ellipsis;
		white-space: nowrap;
		cursor: pointer;
	}

	.meta:hover {
		color: var(--color-ink);
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.rows {
		display: flex;
		flex-direction: column;
		max-width: 560px;
	}

	.conflicts {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-3);
		max-width: 560px;
	}

	h2 {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		margin: 0;
		font-size: var(--type-sm);
		font-weight: 600;
	}

	.badge {
		padding: 0 6px;
		border-radius: 9px;
		background: var(--color-attention);
		font-size: var(--type-xs);
		color: var(--color-paper);
	}

	.card {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-3);
		padding: var(--spacing-4);
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-md);
	}

	.unresolved {
		flex: none;
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-attention);
	}

	.warning {
		margin: 8px 0 0;
		font-size: 13px;
		color: var(--color-attention);
	}

	.cancellation {
		min-height: 40px;
		font-size: 13px;
	}

	button.cancellation:hover .label {
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		min-height: 53px;
		padding: 0 8px 0 0;
		border: none;
		border-bottom: 1px solid var(--color-rule);
		background: none;
		text-align: left;
	}

	.row.head {
		min-height: 0;
		padding-bottom: 6px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	button.row {
		cursor: pointer;
	}

	button.tier:hover .label {
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.row.selected {
		padding-left: 8px;
		border-radius: var(--radius-sm);
		background: var(--color-marker);
		font-weight: 600;
	}

	.row.add {
		gap: 6px;
		min-height: 40px;
		border-bottom: none;
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.row.add.selected,
	.row.add:hover {
		color: var(--color-ink);
	}

	.row.total {
		justify-content: space-between;
		min-height: 0;
		padding-top: 10px;
		border-bottom: none;
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.row.total + .row.total {
		padding-top: 6px;
	}

	.other {
		color: inherit;
		text-decoration: none;
	}

	.other:hover {
		color: var(--color-ink);
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.row.total.strong {
		font-size: var(--type-sm);
		font-weight: 600;
		color: var(--color-ink);
	}

	.name {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		flex: 1;
		min-width: 0;
	}

	.dot {
		width: 8px;
		height: 8px;
		flex: none;
		border-radius: 50%;
	}

	.label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.manual {
		flex: none;
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	.num {
		flex: none;
		text-align: right;
		white-space: nowrap;
	}

	.price {
		width: 76px;
	}

	.fee {
		width: 52px;
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.sold {
		display: flex;
		flex-direction: column;
		gap: 4px;
		width: 120px;
		flex: none;
		margin-left: var(--spacing-1);
	}

	.row.head .sold {
		gap: 0;
	}

	.count strong {
		font-weight: 600;
	}

	.quota {
		margin-left: 3px;
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	.bar {
		display: block;
		height: 6px;
		overflow: hidden;
		border-radius: 3px;
		background: var(--color-rule);
	}

	.bar span {
		display: block;
		height: 100%;
		border-radius: 3px;
	}

	.bar.over {
		outline: 1px solid var(--color-attention);
	}

	.gross {
		width: 110px;
		font-weight: 600;
	}
</style>
