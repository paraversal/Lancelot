<script lang="ts">
	// What a data source says was logged, before it counts (R-DRK-6). It replaces only the imported
	// part: what was typed in stays, and a product the source doesn't name goes back to none. A name
	// that matches no product blocks it until it is one, and is remembered for the next pull.
	import Button from '../../components/Button.svelte';
	import Icon from '../../components/Icon.svelte';
	import Panel from '../../components/Panel.svelte';
	import type { PluginChange, PluginContext, Pulled } from '../api';
	import { matchPull } from './pull';
	import { allProducts, countsOf, nextProductId, setCounts, setProduct } from './records';

	let {
		ctx,
		centre,
		pulled,
		onclose
	}: { ctx: PluginContext; centre: number; pulled: Pulled; onclose: () => void } = $props();

	const products = $derived(allProducts(ctx.records).filter((p) => p.centre === centre));
	const match = $derived(matchPull(products, pulled.rows));
	const lines = $derived(
		products.map((product) => {
			const counts = countsOf(ctx.records, product.id);
			return { product, counts, after: match.totals.get(product.id) ?? null };
		})
	);
	let error = $state<string | null>(null);

	async function run(action: () => Promise<void>) {
		error = null;
		try {
			await action();
		} catch (e) {
			if (!(e instanceof Error)) throw e;
			error = e.message;
		}
	}

	const mapTo = (name: string, id: number) =>
		run(async () => {
			const product = products.find((p) => p.id === id);
			if (!product) return;
			await ctx.apply([setProduct({ ...product, aliases: [...(product.aliases ?? []), name] })]);
		});

	// A Strich from a paper list has no stock, so a new product needs no cost yet.
	const addProduct = (name: string) =>
		run(() =>
			ctx.apply([
				setProduct({ id: nextProductId(ctx.records), centre, name, cost: 0, donation: null })
			])
		);

	const apply = () =>
		run(async () => {
			const changes: PluginChange[] = lines
				.filter((l) => l.counts.imported !== l.after)
				.map((l) => setCounts(l.product.id, { ...l.counts, imported: l.after }));
			if (changes.length) await ctx.apply(changes);
			onclose();
		});
</script>

<Panel title="Pull logged" {onclose}>
	<div class="file">
		<strong>{pulled.label}</strong>
		<span class="meta">{pulled.rows.length} {pulled.rows.length === 1 ? 'row' : 'rows'}</span>
	</div>

	{#each match.unknown as u (u.name)}
		<div class="unknown">
			<div class="top">
				<span class="name"><Icon name="warn" size={14} />„{u.name}“</span>
				<span class="meta">{u.quantity}</span>
			</div>
			<div class="map">
				<select
					aria-label="Same as"
					disabled={!products.length}
					onchange={(e) => mapTo(u.name, Number(e.currentTarget.value))}
				>
					<option value="" selected disabled>Same as…</option>
					{#each products as p (p.id)}
						<option value={p.id}>{p.name}</option>
					{/each}
				</select>
				<Button size="sm" icon="plus" onclick={() => addProduct(u.name)}>New product</Button>
			</div>
		</div>
	{/each}

	<div class="changes" role="table" aria-label="Logged">
		<div class="row head" role="row"><span>Imported</span><span>Now</span></div>
		{#each lines as l (l.product.id)}
			<div class="row" role="row">
				<span class="product">{l.product.name}</span>
				<span class="count">
					{l.counts.imported ?? '–'} → <strong>{l.after ?? '–'}</strong>
				</span>
			</div>
		{/each}
	</div>
	<p class="note">What was typed in by hand stays.</p>

	{#if match.unreadable.length || pulled.problems.length}
		<div class="problems" data-testid="plugin-drinks-pull-problems">
			<strong>{match.unreadable.length + pulled.problems.length} not read</strong>
			{#each match.unreadable as r, n (n)}
				<span>{r.name}: „{r.quantity}“ is not a whole number</span>
			{/each}
			{#each pulled.problems as p, n (n)}
				<span>Item {p.row}: no {p.field}</span>
			{/each}
		</div>
	{/if}

	{#if error}<p class="form-error">{error}</p>{/if}

	{#snippet footer()}
		<Button kind="primary" disabled={!!match.unknown.length} onclick={apply}>Apply</Button>
		{#if match.unknown.length}
			<span class="todo">
				{match.unknown.length}
				{match.unknown.length === 1 ? 'name' : 'names'} to map
			</span>
		{/if}
	{/snippet}
</Panel>

<style>
	.file {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.file strong {
		font-weight: 600;
	}

	.meta,
	.note {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.unknown {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 12px;
		border: 1px solid var(--color-attention);
		border-radius: var(--radius-md);
	}

	.top {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--spacing-3);
	}

	.name {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-weight: 600;
		color: var(--color-attention);
	}

	.map {
		display: flex;
		gap: var(--spacing-2);
	}

	select {
		flex: 1;
		min-width: 0;
		height: 32px;
		padding: 0 8px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-field);
		font: inherit;
		font-size: 13px;
		color: var(--color-ink);
	}

	.changes {
		display: flex;
		flex-direction: column;
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-3);
		min-height: 32px;
		border-bottom: 1px solid var(--color-rule);
	}

	.row.head {
		min-height: 0;
		padding-bottom: 6px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.product {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.count {
		flex: none;
	}

	.count strong {
		font-weight: 600;
	}

	.problems {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.problems strong {
		font-weight: 600;
		color: var(--color-attention);
	}

	.todo {
		font-size: var(--type-xs);
		color: var(--color-attention);
	}
</style>
