<script lang="ts">
	// Payouts: the payout list for the Verein's treasurer (R-PER-4) and the repayment list
	// (R-PER-7). PDF prints a clean layout (saved from the print dialog), CSV follows the
	// number format, "Mark as paid" records the reimbursements, "Repaid" one repayment.
	import { api, ApiError } from '#lib/api/index.ts';
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

	/** A person's advances for the itemised appendix: what raised their balance. */
	function advances(person: number) {
		const p = snapshot.persons.find((x) => x.account === person);
		return (p?.entries ?? [])
			.filter((e) => e.effect > 0)
			.map((e) => {
				const txn = snapshot.txns.find((t) => t.id === e.txn);
				return { ...e, date: txn?.date ?? '', title: txn?.description || txn?.counterparty || '' };
			});
	}

	async function exportCsv() {
		error = null;
		try {
			const target = await chooseCsvPath(`${snapshot.event.name} payouts.csv`);
			if (target) await api.exportPayoutsCsv(target);
		} catch (e) {
			error = e instanceof ApiError ? e.message : String(e);
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
				<Button kind="primary" disabled={!list.payableTotal} onclick={() => (panel = 'mark')}
					>Mark as paid</Button
				>
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
					<span class="iban">IBAN</span>
					{#if !panel}<span class="reference">Reference</span>{:else}<span class="grow"></span>{/if}
					<span class="amount">Amount</span>
				</div>
				{#each list.payouts as p (p.person)}
					<div class="row">
						<span class="person"><strong>{p.name}</strong><small>{p.holder}</small></span>
						{#if p.iban}
							<span class="iban">{group(p.iban)}</span>
						{:else}
							<span class="iban attention"><Icon name="warn" size={14} />No IBAN</span>
						{/if}
						{#if !panel}<span class="reference muted">{p.reference}</span>{:else}<span class="grow"
							></span>{/if}
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
						<Button size="sm" onclick={() => (panel = { repay: r.person })}>Repaid</Button>
					</div>
				{/each}
			</section>
		{/if}

		{#if list.payouts.length}
			<label class="toggle">
				<input type="checkbox" bind:checked={itemised} />
				With itemised advances
			</label>
		{/if}
		{#if error}<p class="error">{error}</p>{/if}
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
	<article class="print-only">
		<h1>{snapshot.event.name} · Payout list</h1>
		<p class="muted">
			{formatDateRange(snapshot.event.startsOn, snapshot.event.endsOn, format)} · as of {formatDate(
				today,
				format
			)}
		</p>
		<table>
			<thead>
				<tr
					><th>Person</th><th>Account holder</th><th>IBAN</th><th>Reference</th><th class="num"
						>Amount</th
					></tr
				>
			</thead>
			<tbody>
				{#each list.payouts as p (p.person)}
					<tr>
						<td>{p.name}</td>
						<td>{p.holder}</td>
						<td>{p.iban ? group(p.iban) : '—'}{p.bic ? ` · ${p.bic}` : ''}</td>
						<td>{p.reference}</td>
						<td class="num">{formatAmount(p.amount, format)}</td>
					</tr>
				{/each}
			</tbody>
			<tfoot>
				<tr
					><td colspan="4">{people(list.payouts.length)}</td><td class="num"
						>{formatAmount(list.payoutTotal, format)}</td
					></tr
				>
			</tfoot>
		</table>
		{#if itemised}
			{#each list.payouts as p (p.person)}
				<h2>{p.name}</h2>
				<table>
					<tbody>
						{#each advances(p.person) as e (e.txn)}
							<tr
								><td class="date">{formatDate(e.date, format)}</td><td>{e.title}</td><td class="num"
									>{formatAmount(e.effect, format)}</td
								></tr
							>
						{/each}
						<tr class="sum"
							><td colspan="2">Open balance</td><td class="num">{formatAmount(p.amount, format)}</td
							></tr
						>
					</tbody>
				</table>
			{/each}
		{/if}
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

	.reference {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		font-size: 13px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.grow {
		flex: 1;
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

	.error {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-minus);
	}

	/* The printed page: plain ink on white, in the app's type. */
	.print-only {
		color: var(--color-ink);
		font-size: 11pt;
	}

	.print-only h1 {
		font-size: 18pt;
	}

	.print-only h2 {
		margin-top: 18pt;
		font-size: 12pt;
	}

	.print-only p {
		margin: 4pt 0 14pt;
	}

	table {
		width: 100%;
		border-collapse: collapse;
	}

	th {
		padding: 4pt 6pt;
		border-bottom: 1px solid var(--color-ink);
		font-size: 9pt;
		font-weight: 400;
		color: var(--color-ink-muted);
		text-align: left;
	}

	td {
		padding: 5pt 6pt;
		border-bottom: 1px solid var(--color-rule);
		vertical-align: top;
	}

	tfoot td,
	tr.sum td {
		border-bottom: none;
		font-weight: 600;
	}

	.num {
		text-align: right;
		white-space: nowrap;
	}

	.date {
		width: 70pt;
		white-space: nowrap;
	}

	@media print {
		.payouts {
			display: block;
			height: auto;
		}
	}
</style>
