<script lang="ts">
	// The settlement report (R-SET-1, R-SET-2, R-SET-4, R-SET-5, R-SET-6): what the screen shows
	// is what prints. Income and expenses by category, plan vs actual when there is a budget, the
	// final balance with every count, what is open or was accepted when closing, every entry, and
	// the receipts, one per page. Saved as PDF from the print dialog.
	import { goto } from '$app/navigation';
	import { api, attachmentUrl } from '#lib/api/index.ts';
	import type { Attachment } from '#lib/api/generated/Attachment.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import { differenceText, formatAmount, formatDate, formatDateRange } from '#lib/format.ts';
	import { rows, type Row } from '#lib/ledger.ts';
	import { describe } from '#lib/openItems.ts';
	import { nudge } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import { renderPdf } from '#lib/report/pdf.ts';
	import { session } from '#lib/state/session.svelte.ts';

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const event = $derived(snapshot.event);
	const totals = $derived(snapshot.totals);
	const where = $derived(snapshot.whereTheMoneyIs);
	const dates = $derived(formatDateRange(event.startsOn, event.endsOn, format));
	const today = new Date().toISOString().slice(0, 10);

	let withReceipts = $state(true);

	const categories = (kind: 'income' | 'expense') =>
		snapshot.accounts.filter((a) => a.kind === kind && a.balance !== 0);
	const assets = $derived(snapshot.accounts.filter((a) => a.kind === 'asset'));
	const budget = $derived(snapshot.budget);
	// Plan vs actual per category (R-SET-2), for categories with an estimate or an actual.
	const planned = (kind: 'income' | 'expense') =>
		budget.categories.flatMap((c) => {
			const account = snapshot.accounts.find((a) => a.id === c.category);
			if (account?.kind !== kind || (c.estimate === 0 && c.actual === 0)) return [];
			return [{ ...c, name: account.name }];
		});
	// The P&L per cost centre (R-SET-3), for those with something tagged.
	const centres = $derived(
		budget.costCentres.flatMap((f) => {
			const centre = snapshot.costCentres.find((c) => c.id === f.costCentre);
			const booked = f.income.actual !== 0 || f.expense.actual !== 0;
			return centre && booked ? [{ ...f, name: centre.name }] : [];
		})
	);
	const cell = (c: number) => (c === 0 ? '–' : formatAmount(c, format));
	const entries = $derived(rows(snapshot));
	const open = $derived(snapshot.openItems.map((i) => describe(i, snapshot, format)));
	const accepted = $derived(
		event.closingOverrides.map((o) => ({ ...describe(o.item, snapshot, format), note: o.note }))
	);

	function countNote(account: number) {
		const c = snapshot.cash.find((x) => x.account === account);
		if (!c) return '';
		const parts = [];
		if (c.opening) parts.push(`opened ${formatAmount(c.opening.amount, format)}`);
		if (c.counted !== null) parts.push(`counted ${formatAmount(c.counted, format)}`);
		else if (c.assetType === 'cash') parts.push('not counted');
		if (c.difference) parts.push(differenceText(c.difference, format));
		return parts.join(' · ');
	}

	function amount(row: Row) {
		const a = row.txn.amount;
		if (row.flow === 'out') return formatAmount(-a, format);
		if (row.flow === 'in') return formatAmount(a, format, 'always');
		return formatAmount(a, format);
	}

	const detail = (row: Row) =>
		[row.who, row.categories.map((c) => c.name).join(', ')].filter(Boolean).join(' · ');

	// Receipts, in the order of the entries. Images print as they are; PDFs are drawn page by
	// page. Printing waits until every one is ready.
	type Receipt = { row: Row; attachment: Attachment; pages: string[] | null; failed: boolean };
	let receipts = $state<Receipt[]>([]);
	const ready = $derived(receipts.filter((r) => r.pages || r.failed).length);

	$effect(() => {
		if (!withReceipts) return;
		const list = entries.flatMap((row) =>
			row.txn.attachments.map((attachment) => ({ row, attachment, pages: null, failed: false }))
		);
		receipts = list;
		let cancelled = false;
		list.forEach(async (r, i) => {
			try {
				const url = await attachmentUrl(r.attachment);
				const pages = r.attachment.mime === 'application/pdf' ? await renderPdf(url) : [url];
				if (!cancelled) receipts[i].pages = pages;
			} catch {
				if (!cancelled) receipts[i].failed = true;
			}
		});
		return () => (cancelled = true);
	});

	const printable = $derived(!withReceipts || ready === receipts.length);

	// F3: Print gives a little nudge when the last receipt is ready.
	let printButton = $state<HTMLElement>();
	let wasPrintable = true;
	$effect(() => {
		const now = printable;
		if (now && !wasPrintable && printButton) nudge(printButton, level('F3'));
		wasPrintable = now;
	});
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Escape' && !e.defaultPrevented) goto('/event/overview');
		if (e.key === 'p' && (e.metaKey || e.ctrlKey) && printable) {
			e.preventDefault();
			api.print();
		}
	}}
/>

<div class="report">
	<header class="toolbar no-print">
		<Button icon="back" onclick={() => goto('/event/overview')}>Overview</Button>
		<span class="grow"></span>
		<label class="toggle">
			<input type="checkbox" bind:checked={withReceipts} />
			Receipts
			{#if withReceipts && receipts.length}
				<span class="muted">{ready < receipts.length ? `${ready}/` : ''}{receipts.length}</span>
			{/if}
		</label>
		<span bind:this={printButton}
			><Button
				kind="primary"
				icon="file"
				shortcut="⌘P"
				disabled={!printable}
				onclick={() => api.print()}>Print</Button
			></span
		>
	</header>

	<article class="sheet">
		<header class="doc-head">
			<div class="doc-title">
				<span class="kicker">Settlement</span>
				<h1>{event.name}</h1>
				{#if dates || event.location}
					<span class="meta">{[dates, event.location].filter(Boolean).join(' · ')}</span>
				{/if}
			</div>
			<span class="meta">Printed {formatDate(today, format)}</span>
		</header>

		<dl class="facts">
			<div>
				<dt>Result</dt>
				<dd
					class="big"
					class:plus={totals.result > 0}
					class:minus={totals.result < 0}
					data-testid="report-result"
				>
					{formatAmount(totals.result, format, 'always')}
				</dd>
			</div>
			<div>
				<dt>Income</dt>
				<dd>{formatAmount(totals.income, format)}</dd>
			</div>
			<div>
				<dt>Expenses</dt>
				<dd>{formatAmount(totals.expense, format)}</dd>
			</div>
			<div>
				<dt>Status</dt>
				<dd>
					{event.closedAt ? `Closed ${formatDate(event.closedAt.slice(0, 10), format)}` : 'Open'}
				</dd>
			</div>
		</dl>

		<div class="pair">
			{#each [{ title: 'Income', kind: 'income', total: totals.income }, { title: 'Expenses', kind: 'expense', total: totals.expense }] as const as t (t.kind)}
				<table>
					<caption>{t.title}</caption>
					<tbody>
						{#each categories(t.kind) as a (a.id)}
							<tr><td>{a.name}</td><td class="num">{formatAmount(a.balance, format)}</td></tr>
						{/each}
					</tbody>
					<tfoot>
						<tr><td>Total</td><td class="num">{formatAmount(t.total, format)}</td></tr>
					</tfoot>
				</table>
			{/each}
		</div>

		{#if snapshot.budgetLines.length}
			<table class="budget">
				<caption>Plan vs actual</caption>
				<thead>
					<tr>
						<th></th><th class="num">Estimate</th><th class="num">Actual</th><th class="num"
							>Forecast</th
						>
					</tr>
				</thead>
				{#each [{ title: 'Income', kind: 'income', total: budget.income }, { title: 'Expenses', kind: 'expense', total: budget.expense }] as const as t (t.kind)}
					<tbody>
						{#each planned(t.kind) as c (c.category)}
							<tr>
								<td
									>{c.name}{#if c.unbudgeted && c.estimate}<small
											>{cell(c.unbudgeted)} not budgeted</small
										>{/if}</td
								>
								<td class="num">{cell(c.estimate)}</td>
								<td class="num">{cell(c.actual)}</td>
								<td class="num">{cell(c.forecast)}</td>
							</tr>
						{/each}
						<tr class="subtotal">
							<td>{t.title}</td>
							<td class="num">{cell(t.total.estimate)}</td>
							<td class="num">{cell(t.total.actual)}</td>
							<td class="num">{cell(t.total.forecast)}</td>
						</tr>
					</tbody>
				{/each}
				<tfoot>
					<tr data-testid="report-plan">
						<td>Result</td>
						{#each [budget.result.estimate, budget.result.actual, budget.result.forecast] as r, i (i)}
							<td class="num" class:plus={r > 0} class:minus={r < 0}
								>{formatAmount(r, format, 'always')}</td
							>
						{/each}
					</tr>
				</tfoot>
			</table>
		{/if}

		{#if centres.length}
			<table class="budget">
				<caption>Cost centres</caption>
				<thead>
					<tr>
						<th></th><th class="num">Income</th><th class="num">Expenses</th><th class="num"
							>Result</th
						>
					</tr>
				</thead>
				<tbody>
					{#each centres as c (c.costCentre)}
						<tr data-testid="report-cost-centre">
							<td>{c.name}</td>
							<td class="num">{cell(c.income.actual)}</td>
							<td class="num">{cell(c.expense.actual)}</td>
							<td class="num" class:plus={c.result.actual > 0} class:minus={c.result.actual < 0}
								>{formatAmount(c.result.actual, format, 'always')}</td
							>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}

		<table>
			<caption>Final balance</caption>
			<tbody>
				{#each assets as a (a.id)}
					<tr>
						<td>{a.name} <small class="inline">{countNote(a.id)}</small></td>
						<td class="num">{formatAmount(a.balance, format)}</td>
					</tr>
				{/each}
				{#if where.heldByPeople}
					<tr
						><td>Held by people</td><td class="num">{formatAmount(where.heldByPeople, format)}</td
						></tr
					>
				{/if}
				{#if where.owedToPeople}
					<tr
						><td>Owed to people</td><td class="num">{formatAmount(-where.owedToPeople, format)}</td
						></tr
					>
				{/if}
				{#if where.opening}
					<tr><td>Opening counts</td><td class="num">{formatAmount(-where.opening, format)}</td></tr
					>
				{/if}
			</tbody>
			<tfoot>
				<tr>
					<td>Result</td>
					<td class="num" class:plus={where.total > 0} class:minus={where.total < 0}
						>{formatAmount(where.total, format, 'always')}</td
					>
				</tr>
			</tfoot>
		</table>

		{#if accepted.length}
			<table>
				<caption>Accepted when closing</caption>
				<tbody>
					{#each accepted as a, i (i)}
						<tr>
							<td>{a.text}<small>{a.note}</small></td>
							<td class="num">{a.amount === null ? '' : formatAmount(a.amount, format)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{:else if open.length}
			<table>
				<caption>Open</caption>
				<tbody>
					{#each open as o, i (i)}
						<tr>
							<td class="attention">{o.text}</td>
							<td class="num">{o.amount === null ? '' : formatAmount(o.amount, format)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}

		{#if entries.length}
			<table class="entries">
				<caption>Entries</caption>
				<thead>
					<tr>
						<th>#</th><th>Date</th><th>Description</th><th class="num">Amount</th><th class="num"
							>Receipt</th
						>
					</tr>
				</thead>
				<tbody>
					{#each entries as row (row.txn.id)}
						<tr>
							<td class="muted">#{row.txn.id}</td>
							<td class="date">{formatDate(row.txn.date, format)}</td>
							<td>{row.title}<small>{detail(row)}</small></td>
							<td class="num">{amount(row)}</td>
							<td class="num">
								{#if row.receipt === 'attached'}<Icon name="check" size={12} />
								{:else if row.receipt === 'missing'}<span class="attention">none</span>{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}

		{#if withReceipts}
			{#each receipts as r (r.attachment.id)}
				{#each r.pages ?? [null] as src, n (n)}
					<section class="receipt">
						<header>
							<div>
								<strong>#{r.row.txn.id} · {r.row.title}</strong>
								<small
									>{[formatDate(r.row.txn.date, format), detail(r.row)]
										.filter(Boolean)
										.join(' · ')}</small
								>
							</div>
							<strong>{amount(r.row)}</strong>
						</header>
						{#if src}
							<img {src} alt={r.attachment.filename} />
						{:else}
							<p class="placeholder" class:attention={r.failed}>
								{r.failed ? `${r.attachment.filename} could not be read` : '…'}
							</p>
						{/if}
						<small
							>{r.attachment.filename}{(r.pages?.length ?? 0) > 1
								? ` · page ${n + 1} of ${r.pages!.length}`
								: ''}</small
						>
					</section>
				{/each}
			{/each}
		{/if}
	</article>
</div>

<style>
	.report {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: auto;
		background: var(--color-paper-sunk);
	}

	.toolbar {
		position: sticky;
		top: 0;
		z-index: 1;
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		padding: var(--spacing-4) var(--spacing-10);
		border-bottom: 1px solid var(--color-rule);
		background: var(--color-paper);
	}

	.toggle {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2);
		font-size: 13px;
		cursor: pointer;
	}

	.toggle input {
		accent-color: var(--color-ink);
	}

	.muted {
		color: var(--color-ink-muted);
	}

	/* The paper: on screen a page on the desk, in print just the content within the @page
	   margins set in base.css. */
	.sheet {
		box-sizing: border-box;
		width: 210mm;
		margin: var(--spacing-8) auto;
		padding: 20mm 18mm;
		border: 1px solid var(--color-rule);
		background: white;
		color: var(--color-ink);
		font-size: 10pt;
		line-height: 1.4;
		font-variant-numeric: tabular-nums;
	}

	.doc-head {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		gap: 24pt;
		padding-bottom: 14pt;
		border-bottom: 1.5pt solid var(--color-ink);
	}

	.doc-title {
		display: flex;
		flex-direction: column;
		gap: 3pt;
	}

	.kicker {
		font-size: 9pt;
		font-weight: 600;
		color: var(--color-ink-muted);
	}

	h1 {
		margin: 0;
		font-size: 20pt;
		font-weight: 600;
		line-height: 1.15;
	}

	.meta {
		font-size: 9pt;
		color: var(--color-ink-muted);
	}

	.facts {
		display: flex;
		gap: 28pt;
		margin: 14pt 0 0;
		padding-bottom: 14pt;
		border-bottom: 0.75pt solid var(--color-rule);
	}

	.facts div {
		display: flex;
		flex-direction: column;
		gap: 2pt;
	}

	.facts dt {
		font-size: 8.5pt;
		color: var(--color-ink-muted);
	}

	.facts dd {
		margin: 0;
		font-size: 12pt;
		font-weight: 600;
	}

	.facts dd.big {
		font-size: 17pt;
	}

	.pair {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 22pt;
		align-items: start;
	}

	table {
		width: 100%;
		margin-top: 18pt;
		border-collapse: collapse;
	}

	caption {
		padding-bottom: 4pt;
		font-size: 11pt;
		font-weight: 600;
		text-align: left;
	}

	th {
		padding: 0 8pt 4pt 0;
		border-bottom: 0.75pt solid var(--color-rule);
		font-size: 8.5pt;
		font-weight: 400;
		color: var(--color-ink-muted);
		text-align: left;
	}

	td {
		padding: 4pt 8pt 4pt 0;
		border-bottom: 0.75pt solid var(--color-rule);
		vertical-align: top;
	}

	tr {
		break-inside: avoid;
	}

	tfoot td {
		border-bottom: none;
		border-top: 1.5pt solid var(--color-ink);
		font-weight: 600;
	}

	.budget td.num,
	.budget th.num {
		width: 72pt;
		padding-left: 8pt;
	}

	.budget .subtotal td {
		font-weight: 600;
	}

	.num {
		padding-right: 0;
		text-align: right;
		white-space: nowrap;
	}

	small {
		display: block;
		font-size: 8.5pt;
		color: var(--color-ink-muted);
	}

	small.inline {
		display: inline;
		margin-left: 6pt;
	}

	.date {
		width: 58pt;
		white-space: nowrap;
	}

	.entries td:first-child {
		width: 26pt;
	}

	.plus {
		color: var(--color-plus);
	}

	.minus {
		color: var(--color-minus);
	}

	.attention {
		color: var(--color-attention);
		font-weight: 600;
	}

	/* One receipt per page: the entry it belongs to, then the image as large as fits. */
	.receipt {
		display: flex;
		flex-direction: column;
		gap: 10pt;
		margin-top: 28pt;
		padding-top: 18pt;
		border-top: 1.5pt solid var(--color-ink);
		break-before: page;
		break-inside: avoid;
	}

	.receipt header {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		gap: 16pt;
		font-size: 11pt;
	}

	/* F3: a receipt fades in when it has loaded. */
	.receipt img {
		animation: arrive var(--motion-normal) var(--ease-out) backwards;
		display: block;
		max-width: 100%;
		max-height: 215mm;
		margin: 0 auto;
		object-fit: contain;
	}

	@keyframes arrive {
		from {
			opacity: 0;
		}
	}

	.placeholder {
		margin: 40pt 0;
		text-align: center;
		color: var(--color-ink-muted);
	}

	@media print {
		.report {
			display: block;
			height: auto;
			overflow: visible;
			background: none;
		}

		.sheet {
			width: auto;
			margin: 0;
			padding: 0;
			border: none;
		}

		.receipt {
			margin-top: 0;
		}
	}
</style>
