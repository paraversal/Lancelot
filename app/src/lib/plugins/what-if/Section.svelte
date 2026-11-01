<script lang="ts">
	// Sliders against the forecast. They live here until the page is left: nothing is saved, and
	// nothing they show reaches the ledger or the event file.
	import Button from '../../components/Button.svelte';
	import { formatAmount } from '../../format';
	import type { BudgetSectionProps } from '../api';
	import Knob from './Knob.svelte';
	import {
		categoryKnobs,
		categoryValue,
		moreOf,
		NOTHING,
		outcome,
		PRICE_STEP,
		tierKnobs,
		tierPrice,
		type Scenario
	} from './scenario';

	let { ctx }: BudgetSectionProps = $props();

	const format = $derived(ctx.format);
	const forecast = $derived(ctx.budget.result.forecast);
	const tiers = $derived(tierKnobs(ctx.tiers));
	const categories = $derived(categoryKnobs(ctx.budget, ctx.accounts));
	const groups = $derived([
		{ title: 'Income', knobs: categories.filter((c) => c.kind === 'income') },
		{ title: 'Expenses', knobs: categories.filter((c) => c.kind === 'expense') }
	]);

	let scenario = $state<Scenario>(NOTHING);
	let open = $state(false);
	const result = $derived(outcome(forecast, tiers, categories, scenario));
	const change = $derived(result - forecast);
	const moved = $derived(
		[scenario.more, scenario.price, scenario.category].some((steps) =>
			Object.values(steps).some((n) => n !== 0)
		)
	);

	function set(part: keyof Scenario, id: number, value: number) {
		scenario = { ...scenario, [part]: { ...scenario[part], [id]: value } };
	}
</script>

<div class="summary">
	<div class="figure">
		<span class="label">Result</span>
		<strong
			class="result"
			class:plus={result > 0}
			class:minus={result < 0}
			data-testid="plugin-what-if-result">{formatAmount(result, format, 'always')}</strong
		>
		{#if change}
			<span class="label" data-testid="plugin-what-if-change"
				>{formatAmount(change, format, 'always')} on the forecast</span
			>
		{/if}
	</div>
	<span class="actions">
		{#if moved}<Button size="sm" onclick={() => (scenario = NOTHING)}>Reset</Button>{/if}
		<Button size="sm" onclick={() => (open = !open)}>{open ? 'Hide' : 'Adjust'}</Button>
	</span>
</div>

{#if open}
	<div class="groups">
		{#if tiers.length}
			<section>
				<h3>Tickets still to sell</h3>
				{#each tiers as t (t.id)}
					<Knob
						label={t.name}
						text={`+${moreOf(t, scenario)}`}
						range={t.more}
						value={moreOf(t, scenario)}
						oninput={(v) => set('more', t.id, v)}
					/>
					<Knob
						label={`${t.name} price`}
						text={formatAmount(tierPrice(t, scenario), format)}
						range={t.priceSteps}
						value={(tierPrice(t, scenario) - t.price) / PRICE_STEP}
						oninput={(v) => set('price', t.id, v)}
					/>
				{/each}
			</section>
		{/if}
		{#each groups as g (g.title)}
			{#if g.knobs.length}
				<section>
					<h3>{g.title}</h3>
					{#each g.knobs as c (c.id)}
						<Knob
							label={c.name}
							text={formatAmount(categoryValue(c, scenario), format)}
							range={c.range}
							value={(categoryValue(c, scenario) - c.forecast) / c.step}
							oninput={(v) => set('category', c.id, v)}
						/>
					{/each}
				</section>
			{/if}
		{/each}
	</div>
{/if}

<style>
	.summary {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-2) var(--spacing-4);
	}

	.figure {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--spacing-1) var(--spacing-3);
	}

	.label {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.result {
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.actions {
		display: flex;
		gap: var(--spacing-2);
	}

	.groups {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
		gap: var(--spacing-6) var(--spacing-8);
		padding-top: var(--spacing-3);
	}

	section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-1);
		min-width: 0;
	}

	h3 {
		margin: 0 0 var(--spacing-1);
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	.plus {
		color: var(--color-plus);
	}

	.minus {
		color: var(--color-minus);
	}
</style>
