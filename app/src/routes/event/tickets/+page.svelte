<script lang="ts">
	// Tickets (requirements §5.6): the tiers with sold vs quota, gross and net, from the latest
	// import of the sheet and the manual sales. Every figure comes from the core. A tier, the
	// import and a manual sale each open in the panel on the right. Conflicts (M7c), break-even
	// and sales per import (M8) join the page later.
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { SHEET_EXTENSIONS, chooseTicketSheet } from '#lib/api/dialogs.ts';
	import type { ManualSaleId } from '#lib/api/generated/ManualSaleId.ts';
	import type { TierId } from '#lib/api/generated/TierId.ts';
	import { tierColor } from '#lib/colors.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import { onFileDrop } from '#lib/dragdrop.ts';
	import { formatAmount } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import { formatFee, shortDay } from '#lib/tickets.ts';
	import ImportPanel from './ImportPanel.svelte';
	import SalePanel from './SalePanel.svelte';
	import TierPanel from './TierPanel.svelte';

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const tickets = $derived(snapshot.tickets);
	const latest = $derived(tickets.imports.at(-1) ?? null);

	type Open =
		| { type: 'tier'; id: TierId | null; name?: string; then?: string | null }
		| { type: 'import'; path: string | null }
		| { type: 'sale'; id: ManualSaleId | null };
	let panel = $state<Open | null>(null);
	let panelKey = $state(0);
	let dragging = $state(false);

	function show(next: Open | null) {
		panel = next;
		panelKey++;
	}

	const isSheet = (path: string) =>
		SHEET_EXTENSIONS.includes(path.split('.').pop()?.toLowerCase() ?? '');

	// A sheet dropped onto the page opens the import (R-TKT-3).
	$effect(() =>
		onFileDrop({
			over: (names) =>
				(dragging = !session.closed && (names.some((n) => !n) || names.some(isSheet))),
			leave: () => (dragging = false),
			drop: (paths) => {
				dragging = false;
				// In a plain browser the names stand in for paths.
				const sheet = paths.find(isSheet);
				if (sheet && !session.closed) show({ type: 'import', path: sheet });
			}
		})
	);

	// Opened from elsewhere, e.g. generated income in the ledger: `?tier=2` or `?sale=3`.
	onMount(() => {
		const tier = Number(page.url.searchParams.get('tier'));
		const sale = Number(page.url.searchParams.get('sale'));
		if (tickets.tiers.some((t) => t.id === tier)) show({ type: 'tier', id: tier });
		else if (tickets.manualSales.some((m) => m.id === sale)) show({ type: 'sale', id: sale });
	});

	async function importSheet() {
		const path = await chooseTicketSheet();
		if (path) show({ type: 'import', path });
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
						onclick={() => show({ type: 'import', path: null })}
					>
						Import {tickets.imports.length} · {shortDay(latest.date)} · {latest.filename}
					</button>
				{/if}
				{#if !session.closed}
					<Button kind="primary" icon="upload" onclick={importSheet}>Import sheet</Button>
					<Button
						icon="plus"
						disabled={!tickets.tiers.length}
						onclick={() => show({ type: 'sale', id: null })}>Manual sale</Button
					>
				{/if}
			</div>
		</header>

		{#if dragging}
			<div class="drop">
				<Icon name="upload" size={20} />
				<strong>Import sheet</strong>
				<span>.ods · .xlsx · .csv</span>
			</div>
		{:else}
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
						</span>
						<span class="num price">{formatAmount(t.price, format)}</span>
						<span class="num fee">{formatFee(t.feeBp, format)}</span>
						<span class="sold">
							<span class="count">
								<strong>{t.sold}</strong>{#if t.quota !== null}<span class="quota">
										/ {t.quota}</span
									>{/if}
							</span>
							{#if t.quota !== null}
								<span class="bar" class:over={t.sold > t.quota}>
									<span
										style:width="{Math.min(100, (t.sold / t.quota) * 100)}%"
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
				{#if tickets.tiers.length}
					{#if tickets.total.fees}
						<div class="row total" role="row">
							<span class="name">Ticket fees</span>
							<span class="num">{formatAmount(-tickets.total.fees, format)}</span>
						</div>
					{/if}
					<div class="row total strong" role="row">
						<span class="name">Ticket income</span>
						<span class="num">{formatAmount(tickets.total.net, format)}</span>
					</div>
				{/if}
			</div>
		{/if}
	</section>

	{#key panelKey}
		{#if panel?.type === 'tier'}
			{@const then = panel.then}
			<TierPanel
				id={panel.id}
				name={panel.name}
				onclose={() => show(then !== undefined ? { type: 'import', path: then } : null)}
			/>
		{:else if panel?.type === 'import'}
			<ImportPanel
				path={panel.path}
				onclose={() => show(null)}
				onnewtier={(name, path) => show({ type: 'tier', id: null, name, then: path })}
			/>
		{:else if panel?.type === 'sale'}
			<SalePanel
				id={panel.id}
				onopen={(id) => show({ type: 'sale', id })}
				onclose={() => show(null)}
			/>
		{/if}
	{/key}
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

	.drop {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-2);
		flex: 1;
		border: 1.5px dashed var(--color-ink-muted);
		border-radius: var(--radius-md);
		background: color-mix(in srgb, var(--color-marker) 25%, transparent);
	}

	.drop strong {
		font-size: var(--type-md);
	}

	.drop span {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}
</style>
