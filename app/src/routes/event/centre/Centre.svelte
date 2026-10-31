<script lang="ts">
	// One cost centre (R-CC-1): its income, costs and result, and the lines tagged with it. Plugins
	// attached to it add their sections. Every figure of the core comes from the core.
	import { goto } from '$app/navigation';
	import type { Snippet } from 'svelte';
	import type { CostCentre } from '#lib/api/generated/CostCentre.ts';
	import { formatAmount, formatDate } from '#lib/format.ts';
	import { riseIn } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import MovingAmount from '#lib/motion/MovingAmount.svelte';
	import { pluginContext, pluginEnabled } from '#lib/plugins/host.ts';
	import PluginFrame from '#lib/plugins/PluginFrame.svelte';
	import { plugins } from '#lib/plugins/registry.ts';
	import { session } from '#lib/state/session.svelte.ts';

	let { centre }: { centre: CostCentre } = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const pnl = $derived(snapshot.budget.costCentres.find((c) => c.costCentre === centre.id));
	const result = $derived(pnl?.result.actual ?? 0);

	const nameOf = (id: number) => snapshot.accounts.find((a) => a.id === id);
	const breakdown = (kind: 'income' | 'expense') =>
		(pnl?.categories ?? []).flatMap((c) => {
			const account = nameOf(c.category);
			return account?.kind === kind && c.actual !== 0
				? [`${account.name} ${formatAmount(c.actual, format)}`]
				: [];
		});
	// The tagged lines, newest first, with what each does to the result.
	const tagged = $derived(
		[...snapshot.txns].reverse().flatMap((txn) =>
			txn.lines
				.filter((l) => l.costCentre === centre.id)
				.map((line) => ({
					txn,
					category: nameOf(line.account)?.name ?? '',
					effect: -line.amount
				}))
		)
	);

	const sections = plugins.flatMap((plugin) =>
		plugin.CentreSection
			? [{ plugin, ctx: pluginContext(plugin.id), Section: plugin.CentreSection }]
			: []
	);
	let panel = $state<Snippet | null>(null);
	function setPanel(next: Snippet | null) {
		panel = next;
	}
</script>

<div class="centre">
	<section class="main">
		<header>
			<h1>{centre.name}</h1>
		</header>

		<div class="hero" in:riseIn|global={{ level: level('D1'), i: 0 }}>
			<div class="figure">
				<span class="label">Result</span>
				<span class="result" class:plus={result > 0} class:minus={result < 0}
					><MovingAmount value={result} {format} sign="always" level={level('A5')} /></span
				>
			</div>
			<div class="figure">
				<span class="label">Cost</span>
				<span class="total">{formatAmount(pnl?.expense.actual ?? 0, format)}</span>
				<span class="note">{breakdown('expense').join(' · ')}&nbsp;</span>
			</div>
			<div class="figure">
				<span class="label">Income</span>
				<span class="total">{formatAmount(pnl?.income.actual ?? 0, format)}</span>
				<span class="note">{breakdown('income').join(' · ')}&nbsp;</span>
			</div>
		</div>

		{#each sections as { plugin, ctx, Section } (plugin.id)}
			{#if pluginEnabled(plugin.id) && plugin.attachedTo?.(ctx, centre.id)}
				<PluginFrame name={plugin.name}>
					<Section {ctx} {centre} {setPanel} />
				</PluginFrame>
			{/if}
		{/each}

		{#if tagged.length}
			<div class="records">
				<div class="caption"><span>Tagged lines</span></div>
				<div class="rows" role="table" aria-label="Tagged lines">
					{#each tagged as t, i (`${t.txn.id}-${i}`)}
						<button
							type="button"
							class="row"
							role="row"
							onclick={() => goto(`/event/ledger?txn=${t.txn.id}`)}
						>
							<span class="day muted">{formatDate(t.txn.date, format)}</span>
							<span class="name">{t.txn.description || t.txn.counterparty || ''}</span>
							<span class="category muted">{t.category}</span>
							<span class="cost" class:plus={t.effect > 0}
								>{formatAmount(t.effect, format, 'always')}</span
							>
						</button>
					{/each}
				</div>
			</div>
		{:else}
			<p class="note">Nothing is tagged with this cost centre yet.</p>
		{/if}
	</section>

	{#if panel}{@render panel()}{/if}
</div>

<style>
	.centre {
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
		flex-wrap: wrap;
		row-gap: 16px;
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
	.caption {
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
		container-type: inline-size;
	}

	/* Beside a panel in a small window, what the entry was matters more than its category. */
	@container (max-width: 420px) {
		.category {
			display: none;
		}
	}

	.caption {
		display: flex;
		justify-content: space-between;
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

	button.row {
		cursor: pointer;
	}

	button.row:hover .name {
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.day {
		width: 84px;
		flex: none;
	}

	.category {
		width: 140px;
		flex: none;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.cost {
		width: 88px;
		flex: none;
		text-align: right;
	}

	.muted {
		color: var(--color-ink-muted);
	}

	.plus {
		color: var(--color-plus);
	}

	.minus {
		color: var(--color-minus);
	}
</style>
