<script lang="ts">
	// Drinks (R-DRK-1 to R-DRK-6): the result of the Drinks cost centre, the commission records with
	// the check of the supplier's invoice, and the price per Strich. Every figure comes from the
	// core. The frame's Strichliste joins with M11.
	import { goto } from '$app/navigation';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { riseIn } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import MovingAmount from '#lib/motion/MovingAmount.svelte';
	import { session } from '#lib/state/session.svelte.ts';
	import ProductPanel from './ProductPanel.svelte';
	import SettingsPanel from './SettingsPanel.svelte';

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const settings = $derived(snapshot.event.drinks);
	const drinks = $derived(snapshot.drinks);
	const centre = $derived(snapshot.costCentres.find((c) => c.role === 'drinks'));
	const pnl = $derived(snapshot.budget.costCentres.find((c) => c.costCentre === centre?.id));
	const result = $derived(pnl?.result.actual ?? 0);

	const costs = $derived(
		(pnl?.categories ?? []).flatMap((c) => {
			const account = snapshot.accounts.find((a) => a.id === c.category);
			return account?.kind === 'expense' && c.actual !== 0
				? [`${account.name} ${formatAmount(c.actual, format)}`]
				: [];
		})
	);
	const rows = $derived(
		snapshot.drinkProducts.map((product) => ({
			product,
			figures: drinks.products.find((f) => f.product === product.id)
		}))
	);

	let selected = $state<number | null>(null);
	let adding = $state(false);
	let editingSettings = $state(false);
	const product = $derived(snapshot.drinkProducts.find((p) => p.id === selected) ?? null);

	function open(id: number) {
		selected = id;
		adding = false;
		editingSettings = false;
	}

	function add() {
		selected = null;
		adding = true;
		editingSettings = false;
	}

	function openSettings() {
		selected = null;
		adding = false;
		editingSettings = true;
	}

	// The invoice names its supplier, so without one the settings come first.
	function invoice() {
		if (settings.supplier) goto('/event/ledger?invoice');
		else openSettings();
	}

	function keydown(e: KeyboardEvent) {
		const target = e.target instanceof Element ? e.target : null;
		if (target?.closest('input, textarea, select, aside') || e.metaKey || e.ctrlKey) return;
		if (e.key === 'n' && !session.closed) {
			e.preventDefault();
			add();
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<div class="drinks">
	<section class="main">
		<header>
			<h1>Drinks</h1>
			{#if !session.closed}
				<div class="actions">
					<Button icon="plus" onclick={invoice}>Supplier invoice</Button>
					<Button kind="primary" icon="plus" shortcut="N" onclick={add}>Product</Button>
				</div>
			{/if}
		</header>

		<div class="hero" in:riseIn|global={{ level: level('D1'), i: 0 }}>
			<div class="figure">
				<span class="label">Drinks result</span>
				<span class="result" class:plus={result > 0} class:minus={result < 0}
					><MovingAmount value={result} {format} sign="always" level={level('A5')} /></span
				>
			</div>
			<div class="figure">
				<span class="label">Cost</span>
				<span class="total">{formatAmount(pnl?.expense.actual ?? 0, format)}</span>
				<span class="note">{costs.join(' · ')}&nbsp;</span>
			</div>
			<div class="figure">
				<span class="label">Donations</span>
				<span class="total">{formatAmount(pnl?.income.actual ?? 0, format)}</span>
				<span class="note">&nbsp;</span>
			</div>
		</div>

		<div class="records">
			<div class="caption">
				<button type="button" class="link" onclick={openSettings}>
					{settings.supplier ? `${settings.supplier} · commission` : 'Commission'}
				</button>
				<span class="prices">
					{#if drinks.costPerDrink !== null}
						<span>{formatAmount(drinks.costPerDrink, format)} per drink</span>
					{/if}
					{#if settings.pricePerStrich !== null}
						<span>{formatAmount(settings.pricePerStrich, format)} per Strich</span>
					{/if}
				</span>
			</div>

			{#if rows.length}
				<div class="rows" role="table" aria-label="Commission records">
					<div class="row head" role="row">
						<span class="name"></span>
						<span class="count">Taken</span>
						<span class="count">Returned</span>
						<span class="count">Kept</span>
						<span class="unit">Unit</span>
						<span class="cost">Cost</span>
					</div>
					{#each rows as r (r.product.id)}
						<button
							type="button"
							class="row"
							class:selected={selected === r.product.id}
							role="row"
							onclick={() => open(r.product.id)}
						>
							<span class="name">{r.product.name}</span>
							<span class="count">{r.product.taken}</span>
							<span class="count muted">{r.product.returned}</span>
							<span class="count strong">{r.figures?.kept}</span>
							<span class="unit muted">{formatAmount(r.product.unitPrice, format)}</span>
							<span class="cost">{formatAmount(r.figures?.cost ?? 0, format)}</span>
						</button>
					{/each}
					<div class="row sum" role="row" data-testid="drinks-records">
						<span class="name"></span>
						<span class="count"></span>
						<span class="count"></span>
						<span class="count">{drinks.kept}</span>
						<span class="unit"></span>
						<span class="cost">{formatAmount(drinks.cost, format)}</span>
					</div>
				</div>
			{/if}

			{#if drinks.invoice}
				{@const check = drinks.invoice}
				<div class="invoice" data-testid="drinks-invoice">
					<span class="muted">
						{check.pfand ? `Invoice incl. Pfand ${formatAmount(check.pfand, format)}` : 'Invoice'}
					</span>
					<span class="grow"></span>
					{#if check.difference === 0}
						<span class="ok"><Icon name="check" size={14} /></span>
					{:else}
						<span class="attention">
							<Icon name="warn" size={14} />
							{formatAmount(check.difference, format, 'magnitude')}
							{check.difference > 0 ? 'more' : 'less'} than the records
						</span>
					{/if}
					<strong>{formatAmount(check.amount, format)}</strong>
				</div>
			{/if}
		</div>
	</section>

	{#if adding}
		<ProductPanel product={null} onclose={() => (adding = false)} />
	{:else if product}
		{#key product.id}
			<ProductPanel {product} onclose={() => (selected = null)} />
		{/key}
	{:else if editingSettings}
		<SettingsPanel onclose={() => (editingSettings = false)} />
	{/if}
</div>

<style>
	.drinks {
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
		min-height: 40px;
	}

	h1 {
		margin: 0;
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.actions {
		display: flex;
		gap: var(--spacing-2);
	}

	.hero {
		display: flex;
		padding-bottom: 28px;
		border-bottom: 1px solid var(--color-rule);
	}

	.figure {
		display: flex;
		flex-direction: column;
		justify-content: flex-end;
		gap: 10px;
		padding: 0 32px;
		border-left: 1px solid var(--color-rule);
	}

	.figure:first-child {
		padding-left: 0;
		border-left: none;
	}

	.label,
	.note,
	.caption,
	.head {
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
		line-height: 1.2;
	}

	.records {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
	}

	.caption {
		display: flex;
		justify-content: space-between;
		gap: var(--spacing-4);
	}

	.link {
		padding: 0;
		border: none;
		background: none;
		color: inherit;
		font: inherit;
		cursor: pointer;
	}

	.link:hover {
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.prices {
		display: flex;
		gap: var(--spacing-4);
	}

	.rows {
		display: flex;
		flex-direction: column;
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		height: 38px;
		padding: 0 8px;
		border: none;
		border-bottom: 1px solid var(--color-rule);
		background: none;
		text-align: left;
	}

	.row.head {
		height: auto;
		padding-bottom: 6px;
	}

	.row.sum {
		border-bottom: none;
		font-weight: 600;
	}

	button.row {
		cursor: pointer;
	}

	button.row:hover .name {
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.row.selected {
		border-radius: var(--radius-sm);
		background: var(--color-marker);
		font-weight: 600;
	}

	.name {
		flex: 1;
		min-width: 0;
	}

	.count {
		width: 64px;
		flex: none;
		text-align: right;
	}

	.unit {
		width: 72px;
		flex: none;
		text-align: right;
	}

	.cost {
		width: 88px;
		flex: none;
		text-align: right;
	}

	.muted {
		color: var(--color-ink-muted);
	}

	.strong {
		font-weight: 600;
	}

	.invoice {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		padding: 0 8px;
		font-size: 13px;
	}

	.ok {
		display: flex;
		color: var(--color-plus);
	}

	.attention {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--color-attention);
	}

	.plus {
		color: var(--color-plus);
	}

	.minus {
		color: var(--color-minus);
	}
</style>
