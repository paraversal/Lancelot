<script lang="ts">
	// What if: ideas tried against the forecast. Folded, it shows the result they lead to. Open, it
	// shows how the forecast becomes that result, with the tiers and categories to change (Try), or
	// works back from a target (Reach). Nothing is saved: the scenario lives here until the page is
	// left, and nothing it shows reaches the ledger or the event file.
	import { tick, untrack } from 'svelte';
	import { flip } from 'svelte/animate';
	import { fade, slide } from 'svelte/transition';
	import { categoryColor, tierColor } from '../../colors';
	import Button from '../../components/Button.svelte';
	import Icon from '../../components/Icon.svelte';
	import Segmented from '../../components/Segmented.svelte';
	import { formatAmount } from '../../format';
	import { flipOptions, popIn } from '../../motion/effects';
	import { level } from '../../motion/mode.svelte';
	import MovingAmount from '../../motion/MovingAmount.svelte';
	import type { BudgetSectionProps } from '../api';
	import Assumption from './Assumption.svelte';
	import Bridge, { type Part } from './Bridge.svelte';
	import CategoryBar from './CategoryBar.svelte';
	import Reach from './Reach.svelte';
	import {
		assumptionValue,
		bridge,
		categoryKnobs,
		categoryValue,
		changes,
		isPlanning,
		lineChange,
		moreOf,
		NOTHING,
		reach,
		scaledIn,
		scaledKnobs,
		startOf,
		tierKnobs,
		tierPrice,
		undo,
		unitPrice,
		type CategoryKnob,
		type LineKnob,
		type Change,
		type Scenario,
		type Suggestion
	} from './scenario';
	import Tier from './Tier.svelte';

	let { ctx }: BudgetSectionProps = $props();

	const id = $props.id();
	const format = $derived(ctx.format);
	// Before anything is booked, What if starts from the plan, with tickets as budgeted.
	const planning = $derived(isPlanning(ctx.budget, ctx.tiers));
	const forecast = $derived(startOf(ctx.budget, planning));
	const categories = $derived(categoryKnobs(ctx.budget, ctx.accounts, planning));
	const budgetedTickets = $derived(
		planning && categories.some((c) => ctx.accounts.find((a) => a.id === c.id)?.role != null)
	);
	const tiers = $derived(budgetedTickets ? [] : tierKnobs(ctx.tiers));
	const scaled = $derived(
		scaledKnobs(ctx.budget, ctx.budgetLines, ctx.assumptions, ctx.accounts, planning)
	);
	// A category with lines that scale moves with the assumptions; its own handle sits on top.
	const shifted = (c: CategoryKnob): CategoryKnob => {
		const d = scaledIn(c.id, scaled, scenario);
		return d ? { ...c, forecast: c.forecast + d, min: c.min + d, max: c.max + d } : c;
	};
	// The width of a category bar stands for this much, the same for every category.
	const scale = $derived(
		Math.max(1, ...categories.map((c) => c.max + Math.max(0, scaledIn(c.id, scaled, scenario))))
	);
	// Under 2 % of the expenses: behind a button, so the bars that matter stay in view.
	const small = $derived(Math.round(ctx.budget.expense.forecast * 0.02));

	let scenario = $state<Scenario>(NOTHING);
	let open = $state(false);
	let mode = $state<'try' | 'reach'>('try');
	let showSmall = $state(false);
	// Reach starts at the plan: what was meant to be left over.
	let target = $state<number | null>(untrack(() => ctx.budget.result.estimate));

	const figures = $derived(bridge(forecast, tiers, categories, scenario, scaled));
	const change = $derived(figures.result - forecast);
	const list = $derived(changes(tiers, categories, scenario, scaled));
	const gap = $derived(target === null ? 0 : target - figures.result);
	const suggestions = $derived(reach(gap, tiers, categories, scenario, scaled));
	const parts = $derived<Part[]>([
		...(tiers.length ? (['tickets'] as const) : []),
		...(scaled.assumptions.length ? (['assumptions'] as const) : []),
		'income',
		'expenses'
	]);

	const nameOf = (id: number) => ctx.assumptions.find((a) => a.id === id)?.name ?? '?';
	const names = (l: LineKnob) => l.per.map(nameOf).join(' × ');
	// Each line under the first assumption it scales with.
	const under = (assumption: number) =>
		scaled.lines
			.filter((l) => l.per[0] === assumption)
			.map((line) => ({
				line,
				price: unitPrice(line, scenario),
				figure: lineChange(line, scaled, scenario).value
			}));

	const fold = $derived(level('A2'));
	const bars = $derived(level('D2'));
	const amounts = $derived(level('A5'));

	const colorOfCategory = (cat: number) => {
		const account = ctx.accounts.find((a) => a.id === cat);
		return account ? categoryColor(account, ctx.accounts) : 'var(--color-ink-muted)';
	};
	const colorOfTier = (tier: number) => tierColor(tiers.findIndex((t) => t.id === tier));
	const groups = $derived(
		(['income', 'expense'] as const).map((kind) => {
			const all = categories.filter((c) => c.kind === kind);
			return {
				kind,
				title: kind === 'income' ? 'Income' : 'Expenses',
				part: (kind === 'income' ? 'income' : 'expenses') as Part,
				shown: all.filter((c) => showSmall || c.forecast >= small)
			};
		})
	);
	const hidden = $derived(categories.filter((c) => c.forecast < small).length);

	function set(part: keyof Scenario, key: number, value: number) {
		scenario = { ...scenario, [part]: { ...scenario[part], [key]: value } };
	}

	function chip(c: Change) {
		if (c.part === 'more') return `${c.name} +${c.value}`;
		if (c.part === 'price' || c.part === 'unit')
			return `${c.name} at ${formatAmount(c.value, format)}`;
		if (c.part === 'assumption') return `${c.name} ${c.value}`;
		return `${c.name} ${formatAmount(c.value, format, 'always')}`;
	}
	const lineCategory = (line: number) => scaled.lines.find((l) => l.id === line)?.category ?? 0;
	const chipColor = (c: Change) =>
		c.part === 'category'
			? colorOfCategory(c.id)
			: c.part === 'unit'
				? colorOfCategory(lineCategory(c.id))
				: c.part === 'assumption'
					? 'var(--color-ink)'
					: colorOfTier(c.id);

	// Jumping to a part: it comes into view and its heading lights up for a moment.
	const headings: Partial<Record<Part, HTMLElement>> = {};
	async function show(part: Part) {
		mode = 'try';
		await tick();
		const heading = headings[part];
		if (!heading) return;
		heading.scrollIntoView?.({ block: 'nearest', behavior: fold === 'off' ? 'auto' : 'smooth' });
		if (bars !== 'off')
			heading.animate(
				[{ backgroundColor: 'var(--color-marker)' }, { backgroundColor: 'transparent' }],
				{
					duration: 1100,
					easing: 'ease-out'
				}
			);
	}

	function apply(s: Suggestion) {
		scenario = s.scenario;
		void show(s.kind === 'cut' ? 'expenses' : s.kind === 'assumption' ? 'assumptions' : 'tickets');
	}

	const suggestionColor = (s: Suggestion) =>
		s.kind === 'cut'
			? colorOfCategory(s.category.id)
			: s.kind === 'assumption'
				? 'var(--color-ink)'
				: colorOfTier(s.tier.id);
</script>

<div class="summary">
	<div class="figure">
		<span class="label">Result</span>
		<strong
			class="result"
			class:plus={figures.result > 0}
			class:minus={figures.result < 0}
			data-testid="plugin-what-if-result"
			><MovingAmount value={figures.result} {format} sign="always" level={amounts} /></strong
		>
		{#if change}
			<span class="label" data-testid="plugin-what-if-change" transition:fade={{ duration: 120 }}
				>{formatAmount(change, format, 'always')} on the {planning ? 'plan' : 'forecast'}</span
			>
		{/if}
	</div>
	<span class="actions">
		{#if open}
			<span transition:fade={{ duration: 120 }}>
				<Segmented
					label="What if"
					options={[
						{ value: 'try', label: 'Try' },
						{ value: 'reach', label: 'Reach' }
					]}
					bind:value={mode}
				/>
			</span>
		{/if}
		<button
			type="button"
			class="toggle"
			class:open
			aria-expanded={open}
			aria-controls={id}
			onclick={() => (open = !open)}
		>
			{open ? 'Hide' : 'Adjust'}
			<span class="chevron"><Icon name="down" size={14} /></span>
		</button>
	</span>
</div>

{#if open}
	<div class="body" {id} transition:slide={{ duration: fold === 'off' ? 0 : 220 }}>
		<Bridge
			{figures}
			start={planning ? 'Plan' : 'Forecast'}
			{parts}
			{format}
			level={bars}
			{amounts}
			onpick={show}
		/>

		<div class="changes">
			<div class="chips">
				{#each list as c (`${c.part}:${c.id}`)}
					<button
						type="button"
						class="chip"
						aria-label="Undo {chip(c)}"
						onclick={() => (scenario = undo(scenario, c.part, c.id))}
						animate:flip={flipOptions(level('B3'))}
						in:popIn={{ level: level('D4') }}
						out:fade={{ duration: 90 }}
					>
						<span class="dot" style:background={chipColor(c)}></span>
						{chip(c)}
						<Icon name="x" size={12} />
					</button>
				{/each}
			</div>
			<Button size="sm" disabled={!list.length} onclick={() => (scenario = NOTHING)}>Reset</Button>
		</div>

		{#if mode === 'try'}
			<div class="try" in:fade={{ duration: fold === 'off' ? 0 : 140 }}>
				{#if scaled.assumptions.length}
					<section>
						<h3 bind:this={headings.assumptions}>Assumptions</h3>
						<div class="assumptions">
							{#each scaled.assumptions as a (a.id)}
								<Assumption
									knob={a}
									value={assumptionValue(a, scenario)}
									lines={under(a.id)}
									{names}
									{format}
									colorOf={colorOfCategory}
									onvalue={(v) => set('assumption', a.id, v)}
									onprice={(line, p) => set('unit', line, p)}
								/>
							{/each}
						</div>
					</section>
				{/if}

				{#if tiers.length}
					<section>
						<h3 bind:this={headings.tickets}>Tickets still to sell</h3>
						{#each tiers as t (t.id)}
							<Tier
								knob={t}
								color={colorOfTier(t.id)}
								more={moreOf(t, scenario)}
								price={tierPrice(t, scenario)}
								{format}
								level={bars}
								onmore={(n) => set('more', t.id, n)}
								onprice={(p) => set('price', t.id, p)}
							/>
						{/each}
					</section>
				{/if}

				{#if categories.length}
					<div class="categories">
						{#each groups as g (g.kind)}
							{#if g.shown.length}
								<section>
									<h3 bind:this={headings[g.part]}>{g.title}</h3>
									{#each g.shown as c (c.id)}
										{@const d = scaledIn(c.id, scaled, scenario)}
										<div transition:slide={{ duration: fold === 'off' ? 0 : 160 }}>
											<CategoryBar
												knob={shifted(c)}
												value={categoryValue(c, scenario) + d}
												{scale}
												color={colorOfCategory(c.id)}
												{format}
												onvalue={(v) => set('category', c.id, v - d)}
											/>
										</div>
									{/each}
								</section>
							{/if}
						{/each}
					</div>
					<div class="foot">
						{#if hidden}
							<Button size="sm" onclick={() => (showSmall = !showSmall)}
								>{showSmall ? 'Fewer categories' : `${hidden} smaller categories`}</Button
							>
						{/if}
						<span class="legend">
							<span><i class="booked"></i>booked</span>
							<span><i class="rest"></i>still to come</span>
							<span><i class="tick"></i>{planning ? 'plan' : 'forecast'}</span>
						</span>
					</div>
				{/if}
			</div>
		{:else}
			<div in:fade={{ duration: fold === 'off' ? 0 : 140 }}>
				<Reach
					bind:target
					planned={ctx.budget.result.estimate}
					{gap}
					{suggestions}
					{format}
					colorOf={suggestionColor}
					onapply={apply}
				/>
			</div>
		{/if}
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

	.plus {
		color: var(--color-plus);
	}

	.minus {
		color: var(--color-minus);
	}

	.actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
	}

	.toggle {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 28px;
		padding: 0 8px 0 10px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: none;
		font-size: 13px;
		font-weight: 600;
		cursor: pointer;
	}

	.toggle:hover {
		background: var(--color-paper-sunk);
	}

	.chevron {
		display: inline-flex;
		transition: transform var(--motion-normal) var(--spring-soft);
	}

	.open .chevron {
		transform: rotate(180deg);
	}

	.body {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-6);
		padding-top: var(--spacing-4);
		container-type: inline-size;
	}

	/* One line, always: a change coming or going never moves what is below. */
	.changes {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		height: 28px;
	}

	.chips {
		display: flex;
		flex: 1;
		gap: var(--spacing-2);
		min-width: 0;
		overflow-x: auto;
		scrollbar-width: none;
	}

	.chip {
		display: inline-flex;
		flex: none;
		align-items: center;
		gap: 6px;
		height: 26px;
		padding: 0 8px 0 10px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-pill);
		background: var(--color-field);
		font-size: var(--type-xs);
		color: var(--color-ink);
		cursor: pointer;
	}

	.chip:hover {
		border-color: var(--color-ink-muted);
	}

	.chip :global(svg) {
		color: var(--color-ink-muted);
	}

	.dot {
		flex: none;
		width: 8px;
		height: 8px;
		border-radius: 50%;
	}

	.try {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-8);
	}

	section {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	h3 {
		align-self: flex-start;
		margin: 0 0 var(--spacing-2) calc(-1 * var(--spacing-1));
		padding: 1px var(--spacing-1);
		border-radius: var(--radius-sm);
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
		scroll-margin: var(--spacing-8);
	}

	.assumptions {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-6);
	}

	.categories {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
		gap: var(--spacing-6) var(--spacing-10);
		margin-bottom: calc(-1 * var(--spacing-4));
	}

	.foot {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--spacing-3) var(--spacing-6);
	}

	.legend {
		display: inline-flex;
		flex-wrap: wrap;
		gap: var(--spacing-4);
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.legend span {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}

	.legend i {
		width: 14px;
		height: 10px;
		border-radius: 2px;
	}

	.legend .booked {
		background: var(--color-ink-muted);
	}

	.legend .rest {
		background: color-mix(in srgb, var(--color-ink-muted) 25%, var(--color-field));
	}

	.legend .tick {
		width: 2px;
		height: 12px;
		background: var(--color-ink);
	}
</style>
