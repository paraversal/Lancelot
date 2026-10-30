<script lang="ts">
	// One order with tickets that left the sheet (R-TKT-12, R-TKT-13). Each tier is resolved on its
	// own, wholly or in part: a refund, a cancellation recorded earlier, or a correction. A
	// cancellation waiting for these tickets only needs confirming (R-TKT-14).
	import { untrack } from 'svelte';
	import { api, ApiError } from '#lib/api/index.ts';
	import type { AccountId } from '#lib/api/generated/AccountId.ts';
	import type { ConflictMatch } from '#lib/api/generated/ConflictMatch.ts';
	import type { OrderConflict } from '#lib/api/generated/OrderConflict.ts';
	import type { RemovedTickets } from '#lib/api/generated/RemovedTickets.ts';
	import type { Resolution } from '#lib/api/generated/Resolution.ts';
	import Button from '#lib/components/Button.svelte';
	import DateField from '#lib/components/DateField.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Select from '#lib/components/Select.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import { feeInput, formatFee, parseCount, parseFee, refundPerTicket } from '#lib/tickets.ts';

	let { conflict, removed }: { conflict: OrderConflict; removed: RemovedTickets } = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const tickets = $derived(snapshot.tickets);
	const tier = $derived(tickets.tiers.find((t) => t.id === removed.tier));
	const tierName = (id: number) => tickets.tiers.find((t) => t.id === id)?.name ?? '';
	const since = $derived(tickets.imports.findIndex((i) => i.id === removed.since) + 1);
	const verein = $derived(snapshot.accounts.find((a) => a.role === 'verein')?.id ?? null);
	const payers = $derived(
		snapshot.accounts
			.filter((a) => a.kind === 'asset' || a.kind === 'person')
			.map((a) => ({ value: a.id, label: a.name }))
	);

	type Mode = 'refund' | 'match' | 'correction';
	const options = $derived([
		{ value: 'refund' as Mode, label: 'Refund' },
		...(removed.matches.length ? [{ value: 'match' as Mode, label: 'Match' }] : []),
		{ value: 'correction' as Mode, label: 'Correction' }
	]);

	// --- form state: the page keys each card by its order and tier ------------------------
	let mode = $state<Mode>('refund');
	let count = $state('');
	let percent = $state('');
	let date = $state<string | null>(null);
	let payer = $state<AccountId | null>(null);
	let note = $state<string | null>(null);
	let error = $state<{ field: string | null; message: string } | null>(null);
	untrack(() => {
		// A cancellation waiting: confirm it. Tickets moved to another tier: normally a correction.
		mode = removed.matches.length ? 'match' : conflict.added.length ? 'correction' : 'refund';
		count = String(removed.count);
		percent = feeInput(10_000, format);
		date = tickets.imports.at(-1)?.date ?? new Date().toISOString().slice(0, 10);
		payer = verein;
	});

	const n = $derived(parseCount(count));
	const bp = $derived(percent.trim() ? parseFee(percent) : null);
	const price = $derived(tier?.price ?? 0);
	const refund = $derived(
		bp !== null && n !== null && !Number.isNaN(n) ? refundPerTicket(price, bp) * n : null
	);
	const keep = $derived(
		n !== null && !Number.isNaN(n) && refund !== null ? price * n - refund : null
	);

	async function resolve(resolution: Resolution) {
		error = null;
		try {
			await session.change(() => api.resolveConflict(conflict.order, removed.tier, resolution));
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = { field: e.field, message: e.message };
		}
	}

	const confirm = (m: ConflictMatch) =>
		resolve({ type: 'match', cancellation: m.cancellation, count: m.count });

	function submit() {
		const missing = (field: string, message: string) => (error = { field, message });
		if (n === null || Number.isNaN(n)) return missing('count', 'enter a whole number');
		if (mode === 'correction') return resolve({ type: 'correction', count: n, note });
		if (bp === null) return missing('refundBp', 'enter a percentage');
		if (date === null) return missing('date', 'enter a date');
		return resolve({
			type: 'refund',
			count: n,
			terms: { refundBp: bp, date, payer: payer === verein ? null : payer },
			note
		});
	}
</script>

<div class="line">
	<div class="what">
		<span>{removed.count} × {tierName(removed.tier)} removed</span>
		<span class="meta">since import {since}</span>
	</div>
	{#if conflict.added.length}
		<p class="meta">
			{conflict.added.map((a) => `${a.count} × ${tierName(a.tier)}`).join(', ')} added to this order
		</p>
	{/if}

	{#if !session.closed}
		{#if mode === 'match'}
			{#each removed.matches as m (m.cancellation)}
				<div class="match">
					<span>matches manual cancellation · {formatFee(m.refundBp, format)}</span>
					<Button size="sm" kind="primary" onclick={() => confirm(m)}>Confirm</Button>
				</div>
			{/each}
			<button type="button" class="other" onclick={() => (mode = 'refund')}
				>Resolve another way</button
			>
		{:else}
			<Segmented {options} bind:value={mode} label="How to resolve" />
			<div class="fields">
				{#if removed.count > 1}
					<div class="field count">
						<TextField
							label="Tickets"
							value={count}
							oninput={(v) => (count = v)}
							oncommit={(v) => (count = v ?? '')}
						/>
					</div>
				{/if}
				{#if mode === 'refund'}
					<div class="field percent">
						<TextField
							label="Refund %"
							value={percent}
							oninput={(v) => (percent = v)}
							oncommit={(v) => (percent = v ?? '')}
						/>
					</div>
					<div class="field date">
						<DateField
							label="Date"
							value={date}
							{format}
							required
							oninput={(v) => (date = v)}
							oncommit={(v) => (date = v ?? date)}
						/>
					</div>
					<div class="field payer">
						<Select
							label="From"
							value={payer ?? 0}
							options={payers}
							onchange={(v) => (payer = v)}
						/>
					</div>
				{/if}
				<div class="field note">
					<TextField label="Note" value={note} oncommit={(v) => (note = v)} />
				</div>
			</div>
			<div class="foot">
				{#if mode === 'refund'}
					<span class="meta">
						{#if refund !== null && keep !== null}
							refund <strong>{formatAmount(refund, format)}</strong> · keep
							<strong>{formatAmount(keep, format)}</strong>
						{/if}
					</span>
				{:else}
					<span class="meta">Never really sold: no income and no refund</span>
				{/if}
				<Button size="sm" kind="primary" onclick={submit}>Resolve</Button>
			</div>
		{/if}
		{#if error}<p class="error">{error.message}</p>{/if}
	{/if}
</div>

<style>
	.line {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.what,
	.match,
	.foot {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-3);
	}

	.meta {
		margin: 0;
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.meta strong {
		font-weight: 600;
		color: var(--color-ink);
	}

	.fields {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-3);
	}

	.field {
		min-width: 0;
	}

	.count,
	.percent {
		width: 72px;
	}

	.date {
		width: 120px;
	}

	.payer {
		width: 150px;
	}

	.note {
		flex: 1 1 140px;
	}

	.other {
		align-self: flex-start;
		padding: 0;
		border: none;
		background: none;
		font-size: 13px;
		color: var(--color-ink-muted);
		cursor: pointer;
	}

	.other:hover {
		color: var(--color-ink);
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.error {
		margin: 0;
		font-size: 13px;
		color: var(--color-attention);
	}
</style>
