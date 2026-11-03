<script lang="ts">
	// Every component in its states, to compare with the System frame in UI.fig.
	import AmountField from '#lib/components/AmountField.svelte';
	import BalanceCell from '#lib/components/BalanceCell.svelte';
	import Button from '#lib/components/Button.svelte';
	import Chip from '#lib/components/Chip.svelte';
	import DateField from '#lib/components/DateField.svelte';
	import GroupedTable from '#lib/components/GroupedTable.svelte';
	import ReceiptIndicator from '#lib/components/ReceiptIndicator.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Select from '#lib/components/Select.svelte';
	import StackedBar from '#lib/components/StackedBar.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
	import type { NumberFormat } from '#lib/api/generated/NumberFormat.ts';
	import { formatAmount, formatDate } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const snapshot = $derived(data.snapshot);
	let format = $state<NumberFormat>('de');
	let type = $state<'expense' | 'income' | 'transfer' | 'settle'>('expense');
	let chip = $state('All');

	const colors = [
		['Paper', 'paper'],
		['Sunk', 'paper-sunk'],
		['Ink', 'ink'],
		['Muted', 'ink-muted'],
		['Rule', 'rule'],
		['Marker', 'marker'],
		['Plus', 'plus'],
		['Minus', 'minus'],
		['Attention', 'attention'],
		['Series 1', 'tier-1'],
		['Series 2', 'tier-2'],
		['Series 3', 'tier-3'],
		['Series 4', 'tier-4'],
		['Series 5', 'tier-5']
	];

	const name = (id: number) => snapshot.accounts.find((a) => a.id === id)?.name ?? '?';
	const byDay = $derived(
		Object.entries(
			Object.groupBy(snapshot.txns.slice(-8).toReversed(), (t) => t.date) as Record<
				string,
				EventSnapshot['txns']
			>
		).map(([key, rows]) => ({ key, rows }))
	);
	const expenses = $derived(snapshot.accounts.filter((a) => a.kind === 'expense' && a.balance > 0));
	const series = [
		'--color-tier-1',
		'--color-tier-4',
		'--color-tier-3',
		'--color-tier-3',
		'--color-tier-5',
		'--color-ink-muted'
	];
</script>

<main>
	<h1>Lancelot</h1>

	<section>
		<h3>Colour</h3>
		<div class="swatches">
			{#each colors as [label, token] (token)}
				<div class="swatch">
					<span style:background="var(--color-{token})"></span>
					<strong>{label}</strong>
					<small>{token}</small>
				</div>
			{/each}
		</div>
	</section>

	<section>
		<h3>Familjen Grotesk · 400 / 600</h3>
		<p class="t40">+2.027,50 €</p>
		<p class="t22">Summer Festival 2026</p>
		<p class="t16">Anna Krüger</p>
		<p>Metro weekly shop · 312,40 €</p>
		<p class="t12">since import 4</p>
	</section>

	<section>
		<h3>Controls</h3>
		<div class="row">
			<Button kind="primary" shortcut="⌘↵">Save</Button>
			<Button kind="primary" icon="upload">Import sheet</Button>
			<Button>CSV</Button>
			<Button icon="plus">Manual sale</Button>
			<Button size="sm" icon="refresh">Check again</Button>
			{#each ['All', 'Expenses', 'Income'] as c (c)}
				<Chip
					label={c}
					count={c === 'All' ? 38 : undefined}
					active={chip === c}
					onclick={() => (chip = c)}
				/>
			{/each}
			<Segmented
				bind:value={type}
				options={[
					{ value: 'expense', label: 'Expense' },
					{ value: 'income', label: 'Income' },
					{ value: 'transfer', label: 'Transfer' },
					{ value: 'settle', label: 'Settle' }
				]}
			/>
		</div>
		<div class="row">
			<Segmented
				bind:value={format}
				options={[
					{ value: 'de', label: '1.234,56 €  ·  31.12.2026' },
					{ value: 'en', label: '€1,234.56  ·  2026-12-31' }
				]}
			/>
		</div>
	</section>

	<section>
		<h3>Amount input</h3>
		<div class="fields">
			<AmountField label="idle" value={null} {format} />
			<AmountField
				label="typing"
				value={null}
				{format}
				text={format === 'de' ? '1.234,5' : '1,234.5'}
			/>
			<AmountField
				label="ambiguous"
				value={null}
				{format}
				text={format === 'de' ? '1.234' : '1,234'}
			/>
			<AmountField label="invalid" value={null} {format} text="1.23.4" />
		</div>
		<div class="fields">
			<TextField label="Name" value="Summer Festival 2026" oncommit={() => {}} />
			<DateField label="Date" value="2026-06-13" {format} oncommit={() => {}} />
			<Select
				label="Paid by"
				value={1}
				options={snapshot.accounts
					.filter((a) => a.kind === 'asset' || a.kind === 'person')
					.map((a) => ({ value: a.id, label: a.name }))}
				onchange={() => {}}
			/>
		</div>
	</section>

	<section>
		<h3>Values</h3>
		<div class="row">
			<span class="plus">{formatAmount(81250, format, 'always')}</span>
			<span>{formatAmount(-4680, format)}</span>
			<span class="muted">{formatAmount(20000, format)}</span>
			<BalanceCell net={21240} {format} />
			<BalanceCell net={-18000} {format} />
			<BalanceCell net={0} {format} />
			<ReceiptIndicator state="attached" />
			<ReceiptIndicator state="missing" />
		</div>
		<StackedBar
			{format}
			max={snapshot.totals.income}
			segments={expenses.map((a, i) => ({
				label: a.name,
				value: a.balance,
				color: `var(${series[i % series.length]})`
			}))}
		/>
	</section>

	<section>
		<h3>Table</h3>
		<GroupedTable groups={byDay}>
			{#snippet head(g)}{formatDate(g.key, format)}{/snippet}
			{#snippet row(t)}
				<span class="grow">{t.description}</span>
				<span class="muted">{name(t.lines[0].account)}</span>
				<span class="amount"
					>{formatAmount(
						t.lines.filter((l) => l.amount > 0).reduce((s, l) => s + l.amount, 0),
						format
					)}</span
				>
				<ReceiptIndicator
					state={t.attachments.length ? 'attached' : t.kind === 'expense' ? 'missing' : 'none'}
				/>
			{/snippet}
		</GroupedTable>
	</section>
</main>

<style>
	main {
		display: flex;
		flex-direction: column;
		gap: 36px;
		max-width: 1280px;
		padding: 48px;
		height: 100vh;
		overflow: auto;
	}

	h1 {
		margin: 0;
		font-size: var(--type-xl);
	}

	h3 {
		margin: 0 0 12px;
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	p {
		margin: 0 0 10px;
	}

	.t40 {
		font-size: var(--type-xl);
		font-weight: 600;
	}

	.t22 {
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.t16 {
		font-size: var(--type-md);
		font-weight: 600;
	}

	.t12 {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.swatches,
	.row,
	.fields {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 12px;
		margin-bottom: 16px;
	}

	.fields {
		align-items: end;
	}

	.fields > :global(*) {
		width: 240px;
	}

	.swatch {
		display: flex;
		flex-direction: column;
		gap: 4px;
		width: 120px;
	}

	.swatch span {
		width: 96px;
		height: 56px;
		border-radius: var(--radius-field);
		border: 1px solid var(--color-rule);
	}

	.swatch small {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.plus {
		color: var(--color-plus);
	}

	.muted {
		color: var(--color-ink-muted);
	}

	.grow {
		flex: 1;
	}

	.amount {
		width: 100px;
		text-align: right;
	}
</style>
