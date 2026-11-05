<script lang="ts">
	// A cost centre's stock and sales: what the supplier bills, what is left, what people logged and
	// what they should have given. Nothing here moves money; the invoice is an entry like any other.
	import { onDestroy } from 'svelte';
	import Button from '../../components/Button.svelte';
	import Icon from '../../components/Icon.svelte';
	import { formatAmount } from '../../format';
	import type { CentreSectionProps, Pulled } from '../api';
	import { computeFigures, EXPLAINERS } from './figures';
	import ProductPanel from './ProductPanel.svelte';
	import { PULL_FIELDS } from './pull';
	import PullPanel from './PullPanel.svelte';
	import { allProducts, centreSettings, countsOf, NEW_CENTRE, sourceKey } from './records';
	import SupplierPanel from './SupplierPanel.svelte';

	let { ctx, centre, setPanel }: CentreSectionProps = $props();

	const format = $derived(ctx.format);
	const settings = $derived(centreSettings(ctx.records, centre.id) ?? NEW_CENTRE);
	const products = $derived(
		allProducts(ctx.records)
			.filter((p) => p.centre === centre.id)
			.map((product) => ({ product, counts: countsOf(ctx.records, product.id) }))
	);
	const figures = $derived(computeFigures(centre.id, settings, products, ctx.txns));
	const collected = $derived(ctx.figuresOf(centre.id)?.income.actual ?? 0);
	const shortfall = $derived(figures.expected === null ? null : figures.expected - collected);
	/** This cost centre's own source: another one is set up for each other cost centre. */
	const source = $derived(ctx.dataSource(sourceKey(centre.name)));

	let selected = $state<number | null>(null);
	let adding = $state(false);
	/** The supplier panel, and whether saving it goes on to the invoice. */
	let supplierPanel = $state<'settings' | 'invoice' | null>(null);
	/** What a pull read, waiting to be looked over. */
	let pulled = $state<Pulled | null>(null);
	let pulling = $state(false);
	let pullError = $state<string | null>(null);
	const open = $derived(products.find((p) => p.product.id === selected) ?? null);

	function show(next: {
		product?: number;
		add?: boolean;
		supplier?: 'settings' | 'invoice';
		pull?: Pulled;
	}) {
		selected = next.product ?? null;
		adding = next.add ?? false;
		supplierPanel = next.supplier ?? null;
		pulled = next.pull ?? null;
	}

	async function pull() {
		pulling = true;
		pullError = null;
		try {
			show({ pull: await ctx.pull(PULL_FIELDS, sourceKey(centre.name)) });
		} catch (e) {
			if (!(e instanceof Error)) throw e;
			pullError = e.message;
		} finally {
			pulling = false;
		}
	}

	function bookInvoice(supplier: string) {
		const category = ctx.accounts.find(
			(a) => a.kind === 'expense' && a.costCentre === centre.id && a.id !== settings.deposit
		);
		ctx.proposeEntry({
			kind: 'expense',
			counterparty: supplier,
			amount: figures.cost || null,
			lines: category ? [{ account: category.id, costCentre: centre.id }] : []
		});
	}

	// The invoice names its supplier, so without one that comes first and then goes straight on.
	function invoice() {
		if (settings.supplier) bookInvoice(settings.supplier);
		else show({ supplier: 'invoice' });
	}

	function keydown(e: KeyboardEvent) {
		const target = e.target instanceof Element ? e.target : null;
		if (target?.closest('input, textarea, select, aside') || e.metaKey || e.ctrlKey) return;
		if (e.key === 'n' && !ctx.closed) {
			e.preventDefault();
			show({ add: true });
		}
	}

	// Only whether there is a panel: it stays mounted while the snapshot changes behind it.
	const hasPanel = $derived(
		adding || selected !== null || supplierPanel !== null || pulled !== null
	);
	$effect(() => {
		setPanel(hasPanel ? panel : null);
	});
	onDestroy(() => setPanel(null));
</script>

<svelte:window onkeydown={keydown} />

{#snippet panel()}
	{#if pulled}
		<PullPanel {ctx} centre={centre.id} {pulled} onclose={() => show({})} />
	{:else if adding}
		<ProductPanel
			{ctx}
			centre={centre.id}
			commission={settings.commission}
			product={null}
			onclose={() => show({})}
		/>
	{:else if open}
		{#key open.product.id}
			<ProductPanel
				{ctx}
				centre={centre.id}
				commission={settings.commission}
				product={open.product}
				counts={open.counts}
				onclose={() => show({})}
			/>
		{/key}
	{:else if supplierPanel}
		<SupplierPanel
			{ctx}
			centre={centre.id}
			{settings}
			forInvoice={supplierPanel === 'invoice'}
			onclose={() => show({})}
			onsaved={(supplier) => {
				const goOn = supplierPanel === 'invoice' && supplier;
				show({});
				if (goOn) bookInvoice(supplier);
			}}
		/>
	{/if}
{/snippet}

<div class="records">
	<div class="caption">
		<button type="button" class="link" onclick={() => show({ supplier: 'settings' })}>
			{settings.supplier ?? 'Supplier'}{settings.commission ? ' · on commission' : ''}
		</button>
		<span class="side">
			{#if figures.costPerUnit !== null}
				<span>{formatAmount(figures.costPerUnit, format)} per unit</span>
			{/if}
			{#if !ctx.closed}
				{#if source}
					<Button size="sm" disabled={pulling} onclick={pull}>Pull logged</Button>
				{/if}
				<Button size="sm" onclick={invoice}>Book invoice</Button>
				<Button size="sm" icon="plus" shortcut="N" onclick={() => show({ add: true })}>
					Product
				</Button>
			{/if}
		</span>
	</div>

	{#if pullError}<p class="form-error">{pullError}</p>{/if}

	{#if figures.rows.length}
		<div class="rows" role="table" aria-label="Stock and sales">
			<div class="row head" role="row">
				<span class="name"></span>
				<span class="count wide">Billed</span>
				<span class="count wide" title={EXPLAINERS.leftOver}>Left over</span>
				<span class="count" title={EXPLAINERS.logged}>Logged</span>
				<span class="count" title="Unaccounted: billed, minus left over, minus logged">Gap</span>
				<span class="cost wide">Cost</span>
				<span class="cost wide">Expected</span>
			</div>
			{#each figures.rows as r (r.product.id)}
				<button
					type="button"
					class="row"
					class:selected={selected === r.product.id}
					role="row"
					onclick={() => show({ product: r.product.id })}
				>
					<span class="name">{r.product.name}</span>
					<span class="count strong wide">{r.billed}</span>
					<span class="count muted wide">{r.leftOver}</span>
					<span class="count">{r.logged ?? '–'}</span>
					<span class="count" class:attention={!!r.gap}>{r.gap ?? '–'}</span>
					<span class="cost wide">{formatAmount(r.cost, format)}</span>
					<span class="cost wide"
						>{r.expected === null ? '–' : formatAmount(r.expected, format)}</span
					>
				</button>
			{/each}
			<div class="row sum" role="row" data-testid="plugin-drinks-total">
				<span class="name"></span>
				<span class="count wide">{figures.billed}</span>
				<span class="count wide">{figures.leftOver}</span>
				<span class="count">{figures.logged}</span>
				<span class="count">{figures.gap}</span>
				<span class="cost wide">{formatAmount(figures.cost, format)}</span>
				<span class="cost wide">
					{figures.expected === null ? '–' : formatAmount(figures.expected, format)}
				</span>
			</div>
		</div>
	{/if}

	{#if shortfall !== null}
		<div class="note-row" data-testid="plugin-drinks-collected">
			<span class="muted">
				Expected {formatAmount(figures.expected ?? 0, format)} · collected
				{formatAmount(collected, format)}
			</span>
			<span class="grow"></span>
			{#if shortfall === 0}
				<span class="ok"><Icon name="check" size={14} /></span>
			{:else}
				<span class="attention">
					<Icon name="warn" size={14} />
					{formatAmount(shortfall, format, 'magnitude')}
					{shortfall > 0 ? 'less than logged' : 'more than logged'}
				</span>
			{/if}
		</div>
	{/if}

	{#if figures.invoice}
		{@const check = figures.invoice}
		<div class="note-row" data-testid="plugin-drinks-invoice">
			<span class="muted">
				{check.deposit ? `Invoice incl. deposit ${formatAmount(check.deposit, format)}` : 'Invoice'}
			</span>
			<span class="grow"></span>
			{#if check.difference === 0}
				<span class="ok"><Icon name="check" size={14} /></span>
			{:else if check.difference !== null}
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

<style>
	.records {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
		container-type: inline-size;
	}

	/* Beside a panel. The product's panel shows what is left out, the money first. */
	@container (max-width: 720px) {
		.cost.wide {
			display: none;
		}
	}

	@container (max-width: 560px) {
		.wide {
			display: none;
		}

		.count {
			width: 44px;
		}

		.row {
			gap: var(--spacing-2);
			padding: 0 4px;
		}
	}

	.caption,
	.head {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.caption {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-2) var(--spacing-4);
	}

	.side {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		white-space: nowrap;
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
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.count {
		width: 76px;
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

	.note-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--spacing-2);
		padding: 0 8px;
		font-size: 13px;
	}

	.grow {
		flex: 1;
	}

	.ok {
		display: flex;
		color: var(--color-plus);
	}

	.attention {
		display: inline-flex;
		align-items: center;
		justify-content: flex-end;
		gap: 4px;
		color: var(--color-attention);
	}
</style>
