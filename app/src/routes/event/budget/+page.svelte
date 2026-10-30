<script lang="ts">
	// Budget (R-BUD-3, R-BUD-4): estimate, actual and forecast per budget line, per category and
	// in total, for income and expenses. Every figure comes from the core; a line opens in the
	// panel. Not designed in UI.fig yet: this follows the People page's table and panel.
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import type { AccountId } from '#lib/api/generated/AccountId.ts';
	import type { BudgetLine } from '#lib/api/generated/BudgetLine.ts';
	import type { Figures } from '#lib/api/generated/Figures.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { riseIn } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import MovingAmount from '#lib/motion/MovingAmount.svelte';
	import { session } from '#lib/state/session.svelte.ts';
	import BudgetLinePanel from './BudgetLinePanel.svelte';

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const budget = $derived(snapshot.budget);
	const a5 = $derived(level('A5'));

	type Row = { line: BudgetLine; figures: Figures; over: boolean };
	type Group = {
		id: AccountId;
		name: string;
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
					// An expense over its estimate, or income short of it once complete.
					const over =
						kind === 'expense'
							? figures.actual > figures.estimate
							: line.complete && figures.actual < figures.estimate;
					return { line, figures, over };
				});
			if (!rows.length && c.actual === 0) return [];
			return [{ id: c.category, name: account.name, figures: c, unbudgeted: c.unbudgeted, rows }];
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

	let selected = $state<number | null>(null);
	let adding = $state<AccountId | null | false>(false);
	const line = $derived(snapshot.budgetLines.find((b) => b.id === selected) ?? null);

	const cell = (c: number) => (c === 0 ? '–' : formatAmount(c, format));

	// Opened from elsewhere: `/event/budget?line=3`.
	onMount(() => {
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
				<Button kind="primary" icon="plus" shortcut="N" onclick={() => add()}>Budget line</Button>
			{/if}
		</header>

		<div class="hero" in:riseIn|global={{ level: level('D1'), i: 0 }}>
			<div class="figure">
				<span class="form-label">Forecast</span>
				<span
					class="result"
					class:plus={budget.result.forecast > 0}
					class:minus={budget.result.forecast < 0}
					><MovingAmount value={budget.result.forecast} {format} sign="always" level={a5} /></span
				>
			</div>
			<div class="figure">
				<span class="form-label">Planned</span>
				<span class="total">{formatAmount(budget.result.estimate, format, 'always')}</span>
			</div>
			<div class="figure">
				<span class="form-label">{session.closed ? 'Result' : 'Result so far'}</span>
				<span class="total">{formatAmount(budget.result.actual, format, 'always')}</span>
			</div>
		</div>

		{#each sections as section (section.title)}
			<div class="rows" role="table" aria-label={section.title}>
				<div class="row head" role="row">
					<span class="name">{section.title}</span>
					<span class="num">Estimate</span>
					<span class="num">Actual</span>
					<span class="num">Forecast</span>
				</div>
				{#each section.groups as g (g.id)}
					<div class="row category" role="row">
						<span class="name">{g.name}</span>
						<span class="num">{cell(g.figures.estimate)}</span>
						<span class="num">{cell(g.figures.actual)}</span>
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
								{r.line.description}
								{#if r.line.complete}<span class="done" title="Complete"
										><Icon name="check" size={14} /></span
									>{/if}
							</span>
							<span class="num">{cell(r.figures.estimate)}</span>
							<span class="num" class:over={r.over}>{cell(r.figures.actual)}</span>
							<span class="num">{cell(r.figures.forecast)}</span>
						</button>
					{/each}
					{#if g.unbudgeted !== 0}
						<div class="row line unbudgeted" role="row">
							<span class="name">Not budgeted</span>
							<span class="num"></span>
							<span class="num">{cell(g.unbudgeted)}</span>
							<span class="num">{cell(g.unbudgeted)}</span>
						</div>
					{/if}
				{/each}
				<div class="row totals" role="row">
					<span class="name"></span>
					<span class="num">{cell(section.total.estimate)}</span>
					<span class="num">{cell(section.total.actual)}</span>
					<span class="num">{cell(section.total.forecast)}</span>
				</div>
			</div>
		{/each}

		{#if centres.length}
			<div class="rows" role="table" aria-label="Cost centres">
				<div class="row head" role="row">
					<span class="name">Cost centres</span>
					<span class="num">Estimate</span>
					<span class="num">Actual</span>
					<span class="num">Forecast</span>
				</div>
				{#each centres as c (c.centre.id)}
					<div class="row" role="row">
						<span class="name">
							{#if c.centre.role === 'drinks'}
								<a href="/event/drinks">{c.centre.name}</a>
							{:else}
								{c.centre.name}
							{/if}
						</span>
						<span class="num">{formatAmount(c.figures.estimate, format, 'always')}</span>
						<span class="num">{formatAmount(c.figures.actual, format, 'always')}</span>
						<span class="num">{formatAmount(c.figures.forecast, format, 'always')}</span>
					</div>
				{/each}
			</div>
		{/if}
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

	.rows {
		display: flex;
		flex-direction: column;
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		height: 40px;
		padding: 0 8px;
		border: none;
		border-bottom: 1px solid var(--color-rule);
		background: none;
		text-align: left;
	}

	.row.head {
		height: auto;
		padding-bottom: 6px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
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

	button.row:hover .name {
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.row.selected {
		border-radius: var(--radius-sm);
		background: var(--color-marker);
		font-weight: 600;
	}

	.row.unbudgeted {
		color: var(--color-ink-muted);
	}

	.row.totals {
		height: auto;
		padding-top: 10px;
		padding-bottom: 10px;
		border-bottom: none;
		font-weight: 600;
	}

	.name {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		flex: 1;
		min-width: 0;
	}

	.done {
		display: inline-flex;
		color: var(--color-ink-muted);
	}

	.num {
		width: 110px;
		flex: none;
		text-align: right;
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
</style>
