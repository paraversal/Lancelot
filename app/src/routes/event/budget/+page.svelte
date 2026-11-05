<script lang="ts">
	// Budget (R-BUD-3, R-BUD-4): estimate, actual and forecast per budget line, per category and
	// in total, for income and expenses. Every figure comes from the core; a line opens in the
	// panel. Follows the Budget frames on the Update page of UI.fig.
	import { goto } from '$app/navigation';
	import { centreLink } from '#lib/nav.ts';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import type { AccountId } from '#lib/api/generated/AccountId.ts';
	import type { BudgetLine } from '#lib/api/generated/BudgetLine.ts';
	import type { Figures } from '#lib/api/generated/Figures.ts';
	import { formula, furthestFromPlan, isOver, isWaiting } from '#lib/budget.ts';
	import { categoryColor } from '#lib/colors.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { boxOpen, riseIn } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import MovingAmount from '#lib/motion/MovingAmount.svelte';
	import { pluginContext, pluginEnabled } from '#lib/plugins/host.ts';
	import PluginFrame from '#lib/plugins/PluginFrame.svelte';
	import { plugins } from '#lib/plugins/registry.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import Assumptions from './Assumptions.svelte';
	import BudgetLinePanel from './BudgetLinePanel.svelte';

	const snapshot = $derived(session.event!.snapshot);
	// A plugin's box that first appears once the event has its first budget line grows in rather
	// than popping; the first render of the page itself stays still.
	let mounted = $state(false);
	const format = $derived(session.format);
	const budget = $derived(snapshot.budget);
	const a5 = $derived(level('A5'));

	type Row = { line: BudgetLine; figures: Figures; over: boolean; waiting: boolean; used: number };
	type Group = {
		id: AccountId;
		name: string;
		color: string;
		figures: Figures;
		unbudgeted: number;
		rows: Row[];
	};

	// Categories with a budget line or something booked to them, in account order.
	function groups(kind: 'income' | 'expense'): Group[] {
		return budget.categories.flatMap((c) => {
			const account = snapshot.accounts.find((a) => a.id === c.category);
			if (account?.kind !== kind) return [];
			const rows = snapshot.budgetLines
				.filter((b) => b.category === c.category)
				.map((line) => {
					const figures = budget.lines.find((f) => f.line === line.id)!;
					return {
						line,
						figures,
						over: isOver(kind, figures, line.complete),
						waiting: isWaiting(figures, line.complete),
						used:
							figures.estimate > 0
								? Math.min(1, figures.actual / figures.estimate)
								: Number(figures.actual > 0)
					};
				});
			if (!rows.length && c.actual === 0) return [];
			return [
				{
					id: c.category,
					name: account.name,
					color: categoryColor(account, snapshot.accounts),
					figures: c,
					unbudgeted: c.unbudgeted,
					rows
				}
			];
		});
	}
	const sections = $derived([
		{ title: 'Income', total: budget.income, groups: groups('income') },
		{ title: 'Expenses', total: budget.expense, groups: groups('expense') }
	]);
	// Cost centres with a plan or something tagged (R-BUD-3).
	const centres = $derived(
		budget.costCentres.flatMap((f) => {
			const centre = snapshot.costCentres.find((c) => c.id === f.costCentre);
			return centre && f.categories.length ? [{ centre, figures: f.result }] : [];
		})
	);
	const order = $derived(
		sections.flatMap((s) => s.groups.flatMap((g) => g.rows.map((r) => r.line.id)))
	);
	const empty = $derived(sections.every((s) => !s.groups.length));
	// Before anything is booked, the forecast would only repeat the plan.
	const planning = $derived(
		!session.closed && budget.income.actual === 0 && budget.expense.actual === 0
	);
	const furthest = $derived(
		furthestFromPlan(budget.lines).map((f) => ({
			...f,
			description: snapshot.budgetLines.find((b) => b.id === f.line)?.description ?? ''
		}))
	);

	let selected = $state<number | null>(null);
	let adding = $state<AccountId | null | false>(false);
	const line = $derived(snapshot.budgetLines.find((b) => b.id === selected) ?? null);

	const cell = (c: number) => (c === 0 ? '–' : formatAmount(c, format));
	const signed = (c: number) => (c === 0 ? '–' : formatAmount(c, format, 'always'));

	const pluginSections = plugins.flatMap((plugin) =>
		plugin.BudgetSection
			? [{ plugin, ctx: pluginContext(plugin.id), Section: plugin.BudgetSection }]
			: []
	);

	// Opened from elsewhere: `/event/budget?line=3`.
	onMount(() => {
		mounted = true;
		const id = Number(page.url.searchParams.get('line'));
		if (snapshot.budgetLines.some((b) => b.id === id)) open(id);
	});

	function open(id: number) {
		selected = id;
		adding = false;
	}

	function add(category: AccountId | null = line?.category ?? null) {
		selected = null;
		adding = category;
	}

	function keydown(e: KeyboardEvent) {
		const target = e.target instanceof Element ? e.target : null;
		if (target?.closest('input, textarea, select, aside') || e.metaKey || e.ctrlKey) return;
		if (e.key === 'n' && !session.closed) {
			e.preventDefault();
			add();
			return;
		}
		if ((e.key !== 'ArrowDown' && e.key !== 'ArrowUp') || !order.length) return;
		e.preventDefault();
		const at = order.indexOf(selected ?? -1);
		const next = e.key === 'ArrowDown' ? Math.min(order.length - 1, at + 1) : Math.max(0, at - 1);
		open(order[next]);
	}
</script>

<svelte:window onkeydown={keydown} />

<div class="budget">
	<section class="main">
		<header>
			<h1>Budget</h1>
			{#if !session.closed}
				<div class="actions">
					{#if empty}
						<Button
							icon="folder"
							label="Copy from another event"
							hint="Copy from another event"
							onclick={() => goto('/event/settings#copy')}>Copy from another event</Button
						>
					{/if}
					<Button
						kind="primary"
						icon="plus"
						shortcut="N"
						label="Budget line"
						hint="Budget line"
						onclick={() => add()}>Budget line</Button
					>
				</div>
			{/if}
		</header>

		{#if !empty}
			<div class="hero" in:riseIn|global={{ level: level('D1'), i: 0 }}>
				{#if planning}
					<div class="figure">
						<span class="form-label">Planned</span>
						<span
							class="result"
							class:plus={budget.result.estimate > 0}
							class:minus={budget.result.estimate < 0}
							>{formatAmount(budget.result.estimate, format, 'always')}</span
						>
					</div>
					<div class="figure">
						<span class="form-label">Income</span>
						<span class="total">{formatAmount(budget.income.estimate, format)}</span>
					</div>
					<div class="figure">
						<span class="form-label">Expenses</span>
						<span class="total">{formatAmount(budget.expense.estimate, format)}</span>
					</div>
				{:else if session.closed}
					<div class="figure">
						<span class="form-label">Result</span>
						<span
							class="result"
							class:plus={budget.result.actual > 0}
							class:minus={budget.result.actual < 0}
							>{formatAmount(budget.result.actual, format, 'always')}</span
						>
					</div>
					<div class="figure">
						<span class="form-label">Planned</span>
						<span class="total">{formatAmount(budget.result.estimate, format, 'always')}</span>
					</div>
				{:else}
					<div class="figure">
						<span class="form-label">Forecast</span>
						<span
							class="result"
							class:plus={budget.result.forecast > 0}
							class:minus={budget.result.forecast < 0}
							><MovingAmount
								value={budget.result.forecast}
								{format}
								sign="always"
								level={a5}
							/></span
						>
					</div>
					<div class="figure">
						<span class="form-label">Planned</span>
						<span class="total">{formatAmount(budget.result.estimate, format, 'always')}</span>
						{#if budget.fromPlan !== 0}
							<span class="note wide">
								{formatAmount(Math.abs(budget.fromPlan), format)}
								{budget.fromPlan < 0 ? 'above' : 'below'} the forecast
							</span>
						{/if}
					</div>
					<div class="figure">
						<span class="form-label">Result so far</span>
						<span class="total">{formatAmount(budget.result.actual, format, 'always')}</span>
					</div>
					{#if furthest.length}
						<div class="figure furthest wide">
							<span class="form-label">Furthest from plan</span>
							{#each furthest as f (f.line)}
								<button type="button" onclick={() => open(f.line)}>
									<span class="title">{f.description}</span>
									<span>{formatAmount(f.fromPlan, format, 'always')}</span>
								</button>
							{/each}
						</div>
					{/if}
				{/if}
			</div>
		{/if}

		{#if !session.closed}
			{#each pluginSections as { plugin, ctx, Section } (plugin.id)}
				{#if pluginEnabled(plugin.id) && (plugin.onBudget?.(ctx) ?? true)}
					<div
						class="plugin-slot"
						transition:boxOpen|global={{ level: mounted ? level('A7') : 'off' }}
					>
						<PluginFrame name={plugin.name}>
							<Section {ctx} />
						</PluginFrame>
					</div>
				{/if}
			{/each}
		{/if}

		{#if !session.closed || snapshot.assumptions.length}
			<Assumptions assumptions={snapshot.assumptions} lines={snapshot.budgetLines} />
		{/if}

		<div class="table" role="table" aria-label="Budget">
			<div class="row columns" role="row">
				<span class="name"></span>
				<span class="bar-space"></span>
				<span class="num">Estimate</span>
				<span class="num actual">Actual</span>
				<span class="num">Forecast</span>
			</div>

			{#if !empty}
				{#each sections as section (section.title)}
					{#if section.groups.length}
						<div class="section" role="rowgroup" aria-label={section.title}>
							<div class="row heading" role="row">
								<span class="name">{section.title}</span>
								<span class="bar-space"></span>
								<span class="num">{cell(section.total.estimate)}</span>
								<span class="num actual">{cell(section.total.actual)}</span>
								<span class="num">{cell(section.total.forecast)}</span>
							</div>
							{#each section.groups as g (g.id)}
								<div class="row category" role="row">
									<span class="name">
										<span class="dot" style:background={g.color}></span>
										<span class="title">{g.name}</span>
										{#if !session.closed}
											<button
												type="button"
												class="add"
												aria-label="New budget line in {g.name}"
												onclick={() => add(g.id)}><Icon name="plus" size={14} /></button
											>
										{/if}
									</span>
									<span class="bar-space"></span>
									<span class="num">{cell(g.figures.estimate)}</span>
									<span class="num actual" class:muted={g.figures.actual === 0}
										>{cell(g.figures.actual)}</span
									>
									<span class="num">{cell(g.figures.forecast)}</span>
								</div>
								{#each g.rows as r (r.line.id)}
									<button
										type="button"
										class="row line"
										class:selected={selected === r.line.id}
										role="row"
										onclick={() => open(r.line.id)}
									>
										<span class="name">
											<span class="title">{r.line.description}</span>
											{#if formula(r.line.estimate, snapshot.assumptions, format)}
												<span class="formula"
													>{formula(r.line.estimate, snapshot.assumptions, format)}</span
												>
											{/if}
											{#if r.line.complete}<span class="done" title="Complete"
													><Icon name="check" size={14} /></span
												>{/if}
										</span>
										<span class="bar-space bar" aria-hidden="true">
											<span
												style:width="{r.used * 100}%"
												style:background={r.over ? 'var(--color-attention)' : g.color}
											></span>
										</span>
										<span class="num">{cell(r.figures.estimate)}</span>
										<span
											class="num actual"
											class:over={r.over}
											class:muted={r.figures.actual === 0}>{cell(r.figures.actual)}</span
										>
										<span class="num" class:muted={r.waiting}>{cell(r.figures.forecast)}</span>
									</button>
								{/each}
								{#if g.unbudgeted !== 0}
									<div class="row line unbudgeted" role="row">
										<span class="name"><span class="title">Not budgeted</span></span>
										<span class="bar-space"></span>
										<span class="num"></span>
										<span class="num actual">{cell(g.unbudgeted)}</span>
										<span class="num">{cell(g.unbudgeted)}</span>
									</div>
								{/if}
							{/each}
						</div>
					{/if}
				{/each}

				<div class="section" role="rowgroup" aria-label="Result">
					<div class="row heading" role="row">
						<span class="name">Result</span>
						<span class="bar-space"></span>
						<span class="num">{signed(budget.result.estimate)}</span>
						<span class="num actual">{signed(budget.result.actual)}</span>
						<span class="num">{signed(budget.result.forecast)}</span>
					</div>
				</div>

				{#if centres.length}
					<div class="section" role="rowgroup" aria-label="Cost centres">
						<div class="row heading" role="row">
							<span class="name">Cost centres</span>
						</div>
						{#each centres as c (c.centre.id)}
							<div class="row" role="row">
								<span class="name">
									<a href={centreLink(c.centre.id)}>{c.centre.name}</a>
								</span>
								<span class="bar-space"></span>
								<span class="num">{signed(c.figures.estimate)}</span>
								<span class="num actual">{signed(c.figures.actual)}</span>
								<span class="num">{signed(c.figures.forecast)}</span>
							</div>
						{/each}
					</div>
				{/if}
			{/if}
		</div>
	</section>

	{#if adding !== false}
		<BudgetLinePanel line={null} category={adding} onclose={() => (adding = false)} />
	{:else if line}
		{#key line.id}
			<BudgetLinePanel {line} category={null} onclose={() => (selected = null)} />
		{/key}
	{/if}
</div>

<style>
	.budget {
		display: flex;
		height: 100%;
	}

	.main {
		container-type: inline-size;
		display: flex;
		flex-direction: column;
		gap: var(--spacing-8);
		flex: 1;
		min-width: 0;
		padding: var(--spacing-10) var(--spacing-8) var(--spacing-10) var(--spacing-10);
		overflow: auto;
	}

	header {
		display: flex;
		flex: none;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-2) var(--spacing-3);
		min-height: 40px;
	}

	.actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--spacing-2);
		margin-left: auto;
	}

	/* A narrow window, or a panel beside it, keeps the actions on one line by showing their icons
	alone; each keeps its label for screen readers and as a tooltip. */
	@container (max-width: 560px) {
		.actions :global(.label),
		.actions :global(kbd) {
			display: none;
		}

		.actions :global(button[data-press]) {
			padding: 0 10px;
		}
	}

	h1 {
		margin: 0;
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.hero {
		display: flex;
		flex: none;
	}

	.figure {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
		padding: 0 var(--spacing-8);
		border-left: 1px solid var(--color-rule);
	}

	.figure:first-child {
		padding-left: 0;
		border-left: none;
	}

	.result {
		font-size: var(--type-xl);
		font-weight: 600;
		line-height: 1.2;
		white-space: nowrap;
	}

	.total {
		font-size: var(--type-lg);
		font-weight: 600;
		line-height: 1.4;
		white-space: nowrap;
	}

	.note {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
		white-space: nowrap;
	}

	.furthest {
		flex: 1;
		min-width: 0;
		gap: 6px;
		padding-right: 0;
	}

	.furthest button {
		display: flex;
		justify-content: space-between;
		gap: var(--spacing-3);
		padding: 0;
		border: none;
		background: none;
		text-align: left;
		white-space: nowrap;
		cursor: pointer;
	}

	.furthest button:hover .title {
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.table {
		display: flex;
		flex-direction: column;
		flex: none;
	}

	.section {
		display: flex;
		flex-direction: column;
	}

	.section + .section {
		margin-top: var(--spacing-6);
		border-top: 1px solid var(--color-ink);
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		flex: none;
		height: 36px;
		padding: 0 8px;
		border: none;
		border-bottom: 1px solid var(--color-rule);
		background: none;
		text-align: left;
	}

	/* Stays at the top of the page while it scrolls, above the padding the page scrolls in. */
	.row.columns {
		position: sticky;
		top: calc(-1 * var(--spacing-10));
		z-index: 1;
		height: auto;
		padding-top: 4px;
		padding-bottom: 6px;
		border-bottom-color: var(--color-ink);
		background: var(--color-paper);
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.row.heading {
		height: 44px;
		font-weight: 600;
	}

	.row.heading .name {
		font-size: var(--type-md);
	}

	.row.category {
		font-weight: 600;
	}

	.row.line {
		padding-left: 24px;
	}

	button.row {
		cursor: pointer;
	}

	button.row:hover .title {
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.row.selected {
		border-radius: var(--radius-sm);
		background: var(--color-marker);
		font-weight: 600;
	}

	.row.selected .num {
		font-weight: 400;
		color: var(--color-ink);
	}

	.row.selected .num.over {
		font-weight: 600;
	}

	.row.unbudgeted {
		color: var(--color-ink-muted);
	}

	.name {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		flex: 1;
		min-width: 0;
	}

	.title {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.dot {
		width: 8px;
		height: 8px;
		flex: none;
		border-radius: 50%;
	}

	/* Shown while its row is hovered or it has the focus. */
	.add {
		display: inline-flex;
		padding: 0;
		border: none;
		background: none;
		color: var(--color-ink-muted);
		opacity: 0;
		cursor: pointer;
	}

	.row:hover .add,
	.add:focus-visible {
		opacity: 1;
	}

	/* Gives way before the line's title does. */
	.formula {
		flex: 0 10 auto;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	.done {
		display: inline-flex;
		flex: none;
		color: var(--color-ink-muted);
	}

	.row.selected .done {
		color: var(--color-ink);
	}

	.bar-space {
		width: 96px;
		flex: none;
	}

	.bar {
		height: 4px;
		overflow: hidden;
		border-radius: 2px;
		background: var(--color-rule);
	}

	.bar span {
		display: block;
		height: 100%;
		border-radius: 2px;
	}

	.num {
		width: 96px;
		flex: none;
		text-align: right;
		white-space: nowrap;
	}

	.muted {
		color: var(--color-ink-muted);
	}

	.over {
		color: var(--color-attention);
	}

	.plus {
		color: var(--color-plus);
	}

	.minus {
		color: var(--color-minus);
	}

	/* Beside a panel: only the table's figures and the hero's main ones. */
	@container (max-width: 800px) {
		.wide,
		.bar-space {
			display: none;
		}
	}

	/* The hero's figures move closer and get smaller, and at the narrowest the main one takes its
	own line, so every figure stays on screen. */
	@container (max-width: 800px) {
		.figure {
			padding: 0 var(--spacing-6);
		}

		.result {
			font-size: 34px;
		}

		.total {
			font-size: 20px;
		}
	}

	@container (max-width: 600px) {
		.hero {
			display: grid;
			grid-template-columns: repeat(auto-fit, minmax(100px, 1fr));
			gap: var(--spacing-4) var(--spacing-6);
		}

		.figure,
		.figure:first-child {
			gap: 4px;
			min-width: 0;
			padding: 0;
			border-left: none;
		}

		.figure:first-child {
			grid-column: 1 / -1;
		}

		.total {
			font-size: 18px;
		}
	}

	/* Narrower still, the figures move closer together and give the names more room. */
	@container (max-width: 560px) {
		.row {
			gap: var(--spacing-2);
		}

		.row.line {
			padding-left: 16px;
		}

		.num {
			width: 84px;
		}
	}

	/* At the narrowest, the plan and the forecast; the ledger has the actual figures. */
	@container (max-width: 440px) {
		.actual {
			display: none;
		}
	}
</style>
