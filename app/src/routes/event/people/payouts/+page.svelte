<script lang="ts">
	// Payouts: the payout list for the association's treasurer (R-PER-4) and the repayment list
	// (R-PER-7). PDF prints a clean layout (saved from the print dialog), CSV follows the
	// number format, "Mark as paid" records the reimbursements, "Repaid" one repayment.
	import { api, errorMessage } from '#lib/api/index.ts';
	import { chooseCsvPath } from '#lib/api/dialogs.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import EntryPanel from '#lib/entry/EntryPanel.svelte';
	import { formatAmount, formatDate, formatDateRange } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import MarkPaidPanel from './MarkPaidPanel.svelte';

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const list = $derived(snapshot.payouts);

	let panel = $state<'mark' | { repay: number } | null>(null);
	let itemised = $state(true);
	let error = $state<string | null>(null);

	const people = (n: number) => (n === 1 ? '1 person' : `${n} people`);
	const group = (iban: string) => iban.replace(/(.{4})/g, '$1 ').trim();
	const accountName = (id: number) => snapshot.accounts.find((a) => a.id === id)?.name ?? '';
	const today = new Date().toISOString().slice(0, 10);
	const dates = $derived(formatDateRange(snapshot.event.startsOn, snapshot.event.endsOn, format));

	/**
	 * Everything behind a person's balance for the itemised appendix: advances, money they
	 * held and settles so far, so the lines add up to the open balance.
	 */
	function advances(person: number) {
		const p = snapshot.persons.find((x) => x.account === person);
		return (p?.entries ?? []).map((e) => {
			const txn = snapshot.txns.find((t) => t.id === e.txn);
			const categories = (txn?.lines ?? [])
				.map((l) => snapshot.accounts.find((a) => a.id === l.account))
				.filter((a) => a?.kind === 'expense' || a?.kind === 'income')
				.map((a) => a!.name)
				.join(', ');
			const entry = txn?.entry;
			const title =
				entry?.type === 'settle'
					? `${entry.direction === 'reimburse' ? 'Reimbursed' : 'Paid back'} · ${accountName(entry.account)}`
					: txn?.description ||
						txn?.counterparty ||
						categories ||
						(entry?.type === 'transfer' ? `Into ${accountName(entry.to)}` : '');
			return { ...e, date: txn?.date ?? '', title };
		});
	}

	async function exportCsv() {
		error = null;
		try {
			const target = await chooseCsvPath(`${snapshot.event.name} payouts.csv`);
			if (target) await api.exportPayoutsCsv(target);
		} catch (e) {
			error = errorMessage(e);
		}
	}
</script>

<div class="payouts">
	<section class="page no-print">
		<header>
			<h1>Payouts</h1>
			<div class="actions">
				<Button onclick={() => api.print()}>PDF</Button>
				<Button onclick={exportCsv}>CSV</Button>
				{#if !session.closed}
					<Button kind="primary" disabled={!list.payableTotal} onclick={() => (panel = 'mark')}
						>Mark as paid</Button
					>
				{/if}
			</div>
		</header>

		<section>
			<div class="title">
				<h2>Payout list</h2>
				<span class="muted"
					>{people(list.payouts.length)} · {formatAmount(list.payoutTotal, format)}</span
				>
			</div>
			{#if list.payouts.length}
				<div class="row head">
					<span class="person">Person</span>
					<span class="iban">Reimburse by</span>
					{#if !panel}<span class="reference">Reference</span>{:else}<span class="grow"></span>{/if}
					<span class="amount">Amount</span>
				</div>
				{#each list.payouts as p (p.person)}
					<div class="row">
						<span class="person"><strong>{p.name}</strong><small>{p.holder}</small></span>
						{#if p.otherMethod}
							<span class="iban other">{p.otherMethod}</span>
						{:else if p.iban}
							<span class="iban">{group(p.iban)}</span>
						{:else}
							<span class="iban attention"><Icon name="warn" size={14} />No method</span>
						{/if}
						{#if !panel}<span class="reference muted">{p.otherMethod ? '' : p.reference}</span
							>{:else}<span class="grow"></span>{/if}
						<strong class="amount">{formatAmount(p.amount, format)}</strong>
					</div>
				{/each}
				<div class="row total">
					<span class="grow"></span>
					<strong class="amount">{formatAmount(list.payoutTotal, format)}</strong>
				</div>
			{/if}
		</section>

		{#if list.repayments.length}
			<section>
				<div class="title">
					<h2>Repayments</h2>
					<span class="muted"
						>{people(list.repayments.length)} · {formatAmount(list.repaymentTotal, format)}</span
					>
				</div>
				{#each list.repayments as r (r.person)}
					<div class="row">
						<span class="person"><strong>{r.name}</strong></span>
						<span class="grow muted">to {accountName(r.account)}</span>
						<strong class="amount attention">{formatAmount(r.amount, format)}</strong>
						{#if !session.closed}
							<Button size="sm" onclick={() => (panel = { repay: r.person })}>Repaid</Button>
						{/if}
					</div>
				{/each}
			</section>
		{/if}

		{#if list.payouts.length}
			<label class="toggle">
				<input type="checkbox" bind:checked={itemised} />
				With advances and payments
			</label>
		{/if}
		{#if error}<p class="form-error">{error}</p>{/if}
	</section>

	{#if panel === 'mark'}
		<div class="no-print"><MarkPaidPanel onclose={() => (panel = null)} /></div>
	{:else if panel}
		<div class="no-print">
			{#key panel.repay}
				<EntryPanel
					txn={null}
					settle={panel.repay}
					previewing={null}
					onpreview={() => {}}
					onclose={() => (panel = null)}
					onsaved={(id) => id !== null && (panel = null)}
				/>
			{/key}
		</div>
	{/if}

	<!-- What PDF prints: the payout list for the treasurer, with the advances behind it. -->
	<article class="print-only sheet">
		<header class="doc-head">
			<div class="doc-title">
				<span class="kicker">Payout list</span>
				<h1>{snapshot.event.name}</h1>
				{#if dates || snapshot.event.location}
					<span class="meta">{[dates, snapshot.event.location].filter(Boolean).join(' · ')}</span>
				{/if}
			</div>
			<dl class="facts">
				<div>
					<dt>To pay</dt>
					<dd class="big">{formatAmount(list.payoutTotal, format)}</dd>
				</div>
				<div>
					<dt>People</dt>
					<dd>{list.payouts.length}</dd>
				</div>
				<div>
					<dt>As of</dt>
					<dd>{formatDate(today, format)}</dd>
				</div>
			</dl>
		</header>

		<table class="list">
			<thead>
				<tr><th>Person</th><th>Reimburse by</th><th>Reference</th><th class="num">Amount</th></tr>
			</thead>
			<tbody>
				{#each list.payouts as p (p.person)}
					<tr>
						<td>
							<strong>{p.name}</strong>
							{#if p.holder !== p.name}<small>{p.holder}</small>{/if}
						</td>
						<td>
							{#if p.otherMethod}
								{p.otherMethod}
							{:else if p.iban}
								<span class="iban">{group(p.iban)}</span>
								{#if p.bic}<small>{p.bic}</small>{/if}
							{:else}
								<span class="attention">No method</span>
							{/if}
						</td>
						<td class="reference">{p.otherMethod ? '' : p.reference}</td>
						<td class="num"><strong>{formatAmount(p.amount, format)}</strong></td>
					</tr>
				{/each}
			</tbody>
			<tfoot>
				<tr
					><td colspan="3">Total</td><td class="num">{formatAmount(list.payoutTotal, format)}</td
					></tr
				>
			</tfoot>
		</table>

		{#if itemised && list.payouts.length}
			<h2 class="section">Advances and payments</h2>
			{#each list.payouts as p (p.person)}
				<section class="advances">
					<div class="advances-head">
						<strong>{p.name}</strong>
						<span class="muted">open balance</span>
						<strong class="num">{formatAmount(p.amount, format)}</strong>
					</div>
					<table class="items">
						<tbody>
							{#each advances(p.person) as e (e.txn)}
								<tr>
									<td class="date">{formatDate(e.date, format)}</td>
									<td>{e.title}</td>
									<td class="num">{formatAmount(e.effect, format)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</section>
			{/each}
		{/if}

		<footer class="doc-foot">{session.event!.fileName} · Lancelot</footer>
	</article>
</div>

<style>
	.payouts {
		display: flex;
		height: 100%;
	}

	.page {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-8);
		flex: 1;
		min-width: 0;
		padding: var(--spacing-10);
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

	.actions {
		display: flex;
		gap: var(--spacing-2);
	}

	.title {
		display: flex;
		align-items: baseline;
		gap: 10px;
		margin-bottom: var(--spacing-2);
	}

	h2 {
		margin: 0;
		font-size: var(--type-md);
		font-weight: 600;
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--spacing-4);
		min-height: 52px;
		padding: 0 8px;
		border-bottom: 1px solid var(--color-rule);
	}

	.row.head {
		min-height: 0;
		padding-bottom: 6px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.row.total {
		min-height: 44px;
		border-bottom: none;
	}

	.person {
		display: flex;
		flex-direction: column;
		gap: 2px;
		width: 150px;
		flex: none;
	}

	small {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.iban {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 230px;
		flex: none;
		font-size: 13px;
	}

	.iban.other {
		overflow-wrap: anywhere;
	}

	.reference {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		font-size: 13px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.amount {
		width: 100px;
		flex: none;
		text-align: right;
	}

	strong {
		font-weight: 600;
	}

	.muted {
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.attention {
		color: var(--color-attention);
		font-weight: 600;
	}

	.toggle {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2);
		align-self: flex-start;
		font-size: 13px;
		cursor: pointer;
	}

	.toggle input {
		accent-color: var(--color-ink);
	}

	/* The printed page: ink on white in the app's type, rules instead of boxes. The page
	   margins are set with @page in base.css. */
	.sheet {
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

	.sheet h1 {
		margin: 0;
		font-size: 20pt;
		font-weight: 600;
		line-height: 1.15;
	}

	.meta {
		font-size: 10pt;
		color: var(--color-ink-muted);
	}

	.facts {
		display: flex;
		gap: 22pt;
		margin: 0;
	}

	.facts div {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 2pt;
	}

	.facts dt {
		font-size: 8.5pt;
		color: var(--color-ink-muted);
	}

	.facts dd {
		margin: 0;
		font-size: 11pt;
		font-weight: 600;
	}

	.facts dd.big {
		font-size: 16pt;
	}

	.sheet table {
		width: 100%;
		border-collapse: collapse;
	}

	.list {
		margin-top: 14pt;
	}

	.list th {
		padding: 0 8pt 5pt 0;
		border-bottom: 0.75pt solid var(--color-rule);
		font-size: 8.5pt;
		font-weight: 400;
		color: var(--color-ink-muted);
		text-align: left;
	}

	.list td {
		padding: 7pt 8pt 7pt 0;
		border-bottom: 0.75pt solid var(--color-rule);
		vertical-align: top;
	}

	.list tr {
		break-inside: avoid;
	}

	.list small,
	.sheet small {
		display: block;
		margin-top: 1pt;
		font-size: 8.5pt;
		color: var(--color-ink-muted);
	}

	.iban {
		white-space: nowrap;
		letter-spacing: 0.02em;
	}

	.reference {
		color: var(--color-ink-muted);
	}

	.sheet .num {
		padding-right: 0;
		text-align: right;
		white-space: nowrap;
	}

	.list tfoot td {
		padding-top: 8pt;
		border-bottom: none;
		border-top: 1.5pt solid var(--color-ink);
		font-weight: 600;
	}

	.sheet .attention {
		color: var(--color-attention);
		font-weight: 600;
	}

	.section {
		margin: 26pt 0 4pt;
		font-size: 12pt;
		font-weight: 600;
	}

	.advances {
		margin-top: 12pt;
		break-inside: avoid;
	}

	.advances-head {
		display: flex;
		align-items: baseline;
		gap: 8pt;
		padding-bottom: 4pt;
		border-bottom: 0.75pt solid var(--color-ink);
	}

	.advances-head .muted {
		flex: 1;
		font-size: 8.5pt;
		text-align: right;
	}

	.items td {
		padding: 4pt 8pt 4pt 0;
		border-bottom: 0.75pt solid var(--color-rule);
	}

	.items .date {
		width: 62pt;
		color: var(--color-ink-muted);
		white-space: nowrap;
	}

	.doc-foot {
		margin-top: 28pt;
		padding-top: 6pt;
		border-top: 0.75pt solid var(--color-rule);
		font-size: 8pt;
		color: var(--color-ink-muted);
	}

	@media print {
		.payouts {
			display: block;
			height: auto;
		}
	}
</style>
