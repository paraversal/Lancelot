<script lang="ts">
	// Stock and sales in the settlement report: what each stocked cost centre took, gave back and
	// logged. Informational, like everything of this plugin; the report's figures are the core's.
	import { formatAmount } from '../../format';
	import type { ReportSectionProps } from '../api';
	import { computeFigures } from './figures';
	import { stockedCentres } from './records';

	let { ctx }: ReportSectionProps = $props();

	const format = $derived(ctx.format);
	const centres = $derived(
		stockedCentres(ctx.records, ctx.costCentres).map(({ centre, settings, products }) => ({
			centre,
			figures: computeFigures(centre.id, settings, products, ctx.txns),
			collected: ctx.figuresOf(centre.id)?.income.actual ?? 0
		}))
	);
	const count = (n: number | null) => (n === null ? '–' : String(n));
	const money = (c: number | null) => (c === null ? '–' : formatAmount(c, format));
</script>

{#each centres as { centre, figures, collected } (centre.id)}
	<table data-testid="plugin-drinks-report">
		<caption>{centre.name}</caption>
		<thead>
			<tr>
				<th></th>
				<th class="num">Billed</th>
				<th class="num">Left over</th>
				<th class="num">Logged</th>
				<th class="num">Gap</th>
				<th class="num">Cost</th>
				<th class="num">Expected</th>
			</tr>
		</thead>
		<tbody>
			{#each figures.rows as r (r.product.id)}
				<tr>
					<td>{r.product.name}</td>
					<td class="num">{r.billed}</td>
					<td class="num">{r.leftOver}</td>
					<td class="num">{count(r.logged)}</td>
					<td class="num">{count(r.gap)}</td>
					<td class="num">{money(r.cost)}</td>
					<td class="num">{money(r.expected)}</td>
				</tr>
			{/each}
		</tbody>
		<tfoot>
			<tr>
				<td>Total</td>
				<td class="num">{figures.billed}</td>
				<td class="num">{figures.leftOver}</td>
				<td class="num">{figures.logged}</td>
				<td class="num">{figures.gap}</td>
				<td class="num">{money(figures.cost)}</td>
				<td class="num">{money(figures.expected)}</td>
			</tr>
			{#if figures.expected !== null}
				<tr class="collected">
					<td colspan="6">Collected</td>
					<td class="num">{money(collected)}</td>
				</tr>
			{/if}
		</tfoot>
	</table>
{/each}

<style>
	table {
		width: 100%;
		border-collapse: collapse;
	}

	caption {
		padding-bottom: 4pt;
		font-weight: 600;
		text-align: left;
	}

	th {
		padding: 0 0 4pt 8pt;
		border-bottom: 0.75pt solid var(--color-rule);
		font-size: 8.5pt;
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	td {
		padding: 3pt 0 3pt 8pt;
		border-bottom: 0.75pt solid var(--color-rule);
	}

	th:first-child,
	td:first-child {
		padding-left: 0;
	}

	tr {
		break-inside: avoid;
	}

	tfoot td {
		border-bottom: none;
		font-weight: 600;
	}

	.collected td {
		padding-top: 0;
		font-weight: 400;
	}

	.num {
		text-align: right;
		white-space: nowrap;
	}
</style>
