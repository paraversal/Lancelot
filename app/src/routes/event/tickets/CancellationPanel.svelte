<script lang="ts">
	// A cancellation recorded without a change in the sheet (R-TKT-14): on an order of the sheet,
	// e.g. a partial refund after the deadline, or on a manual sale. Also where a cancellation or
	// correction from a conflict is changed or taken back, which makes the conflict open again.
	import { untrack } from 'svelte';
	import { api, ApiError } from '#lib/api/index.ts';
	import type { AccountId } from '#lib/api/generated/AccountId.ts';
	import type { CancellationId } from '#lib/api/generated/CancellationId.ts';
	import type { ManualSaleId } from '#lib/api/generated/ManualSaleId.ts';
	import type { RefundTerms } from '#lib/api/generated/RefundTerms.ts';
	import type { TierId } from '#lib/api/generated/TierId.ts';
	import Button from '#lib/components/Button.svelte';
	import DateField, { datesRead } from '#lib/components/DateField.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Select from '#lib/components/Select.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import { feeInput, parseCount, parseFee } from '#lib/tickets.ts';
	import { ticketShare } from '#lib/tickets.svelte.ts';

	let {
		id,
		onclose
	}: {
		/** The one being edited; null for a new one. */
		id: CancellationId | null;
		onclose: () => void;
	} = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const tickets = $derived(snapshot.tickets);
	const tiers = $derived(tickets.tiers);
	const sales = $derived(tickets.manualSales);
	const current = $derived(tickets.cancellations.find((c) => c.id === id) ?? null);
	const correction = $derived(!!current && current.terms === null);
	const ticketAccount = $derived(snapshot.ticketSettings.account ?? 0);
	const tierName = (t: TierId) => tiers.find((x) => x.id === t)?.name ?? '';
	const accountName = (a: AccountId) => snapshot.accounts.find((x) => x.id === a)?.name ?? '';
	const saleLabel = (s: (typeof sales)[number]) =>
		`${s.count} × ${tierName(s.tier)} · ${accountName(s.destination)}`;
	const payers = $derived(
		snapshot.accounts
			.filter((a) => a.kind === 'asset' || a.kind === 'person')
			.map((a) => ({ value: a.id, label: a.name }))
	);

	// --- form state, filled once: the page remounts the panel ---------------------------
	type Source = 'order' | 'sale';
	let source = $state<Source>('order');
	let order = $state<string | null>(null);
	let tier = $state<TierId>(0);
	let sale = $state<ManualSaleId>(0);
	let count = $state('');
	let percent = $state('');
	let date = $state<string | null>(null);
	let payer = $state<AccountId>(0);
	let note = $state<string | null>(null);
	let confirming = $state(false);
	let error = $state<{ field: string | null; message: string } | null>(null);
	untrack(() => {
		const terms = current?.terms;
		source = current?.sale != null ? 'sale' : 'order';
		order = current?.order ?? null;
		tier = current?.tier ?? tiers[0]?.id ?? 0;
		sale = current?.sale ?? sales[0]?.id ?? 0;
		count = current ? String(current.count) : '1';
		percent = feeInput(terms?.refundBp ?? 10_000, format);
		date =
			terms?.date ??
			tickets.imports.at(-1)?.date ??
			snapshot.event.startsOn ??
			new Date().toISOString().slice(0, 10);
		payer = current?.payer ?? ticketAccount;
		note = current?.note ?? null;
	});

	const chosenSale = $derived(source === 'sale' ? sales.find((s) => s.id === sale) : undefined);
	const tierId = $derived(chosenSale?.tier ?? tier);
	// Whoever received the money, unless another payer is chosen (R-TKT-15).
	const receiver = $derived(chosenSale?.destination ?? ticketAccount);
	// A sale's destination pays by default: picking a new sale follows it.
	$effect(() => {
		if (!current && chosenSale) payer = chosenSale.destination;
	});

	const n = $derived(parseCount(count));
	const bp = $derived(percent.trim() ? parseFee(percent) : null);
	const share = ticketShare(() =>
		bp !== null && n !== null && !Number.isNaN(n)
			? { price: tiers.find((t) => t.id === tierId)?.price ?? 0, bp, count: n }
			: null
	);
	const refund = $derived(share.current?.part ?? null);
	const fieldError = (field: string) => (error?.field === field ? error.message : null);

	async function save() {
		error = null;
		await datesRead();
		const missing = (field: string, message: string) => void (error = { field, message });
		if (n === null || Number.isNaN(n)) return missing('count', 'enter a whole number');
		if (bp === null) return missing('refundBp', 'enter a percentage');
		if (date === null) return missing('date', 'enter a date');
		if (!current && source === 'order' && !order?.trim())
			return missing('order', 'enter the order ID');
		const terms: RefundTerms = {
			refundBp: bp,
			date,
			payer: payer === receiver ? null : payer
		};
		try {
			await session.change(() => {
				if (current) return api.updateCancellation(current.id, { count: n, terms, note });
				return api.addCancellation({
					source:
						source === 'sale'
							? { type: 'sale', sale }
							: { type: 'order', order: order ?? '', tier },
					count: n,
					terms,
					note
				});
			});
			onclose();
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = { field: e.field, message: e.message };
		}
	}

	async function remove() {
		if (!current) return;
		await session.change(() => api.deleteCancellation(current.id));
		onclose();
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !session.closed && !correction) {
			e.preventDefault();
			save();
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<Panel title={correction ? 'Correction' : 'Cancellation'} {onclose}>
	<fieldset disabled={session.closed}>
		{#if current}
			<p class="source">
				{current.order !== null
					? `Order ${current.order}`
					: `Sale: ${chosenSale ? saleLabel(chosenSale) : ''}`}
				· {current.count} × {tierName(current.tier)}
			</p>
		{:else}
			{#if sales.length}
				<Segmented
					options={[
						{ value: 'order' as Source, label: 'Order' },
						{ value: 'sale' as Source, label: 'Manual sale' }
					]}
					bind:value={source}
					label="Cancelled from"
				/>
			{/if}
			{#if source === 'order'}
				<div class="pair">
					<div class="form-field grow">
						<TextField
							label="Order ID"
							value={order}
							autofocus
							oninput={(v) => (order = v)}
							oncommit={(v) => (order = v)}
						/>
						{#if fieldError('order')}<span class="error">{fieldError('order')}</span>{/if}
					</div>
					<div class="form-field grow">
						<Select
							label="Tier"
							value={tier}
							options={tiers.map((t) => ({ value: t.id, label: t.name }))}
							onchange={(v) => (tier = v)}
						/>
					</div>
				</div>
			{:else}
				<Select
					label="Manual sale"
					value={sale}
					options={sales.map((s) => ({ value: s.id, label: saleLabel(s) }))}
					onchange={(v) => (sale = v)}
				/>
			{/if}
		{/if}

		{#if correction}
			<p class="meta">Never really sold: no income and no refund.</p>
			{#if current?.note}<p class="meta">{current.note}</p>{/if}
		{:else}
			<div class="pair">
				<div class="form-field small">
					<TextField
						label="Count"
						value={count}
						oninput={(v) => (count = v)}
						oncommit={(v) => (count = v ?? '')}
					/>
					{#if fieldError('count')}<span class="error">{fieldError('count')}</span>{/if}
				</div>
				<div class="form-field small">
					<TextField
						label="Refund %"
						value={percent}
						oninput={(v) => (percent = v)}
						oncommit={(v) => (percent = v ?? '')}
					/>
					{#if fieldError('refundBp')}<span class="error">{fieldError('refundBp')}</span>{/if}
				</div>
				<div class="form-field grow">
					<DateField
						label="Date"
						value={date}
						{format}
						required
						oninput={(v) => (date = v)}
						oncommit={(v) => (date = v ?? date)}
					/>
					{#if fieldError('date')}<span class="error">{fieldError('date')}</span>{/if}
				</div>
			</div>
			<div class="amount">
				<span class="meta">Refund</span>
				<strong>{formatAmount(-(refund ?? 0), format)}</strong>
			</div>
			<Select label="From" value={payer} options={payers} onchange={(v) => (payer = v)} />
			<TextField label="Note" value={note} oncommit={(v) => (note = v)} />
			{#if current && current.order !== null && current.goneNow < current.count}
				<p class="meta">
					{current.count - current.goneNow} still in the sheet. They stay sold until they leave it.
				</p>
			{/if}
		{/if}
	</fieldset>

	{#if error && !['count', 'refundBp', 'date', 'order'].includes(error.field ?? '')}
		<p class="error">{error.message}</p>
	{/if}

	{#snippet footer()}
		{#if session.closed}
			<span class="closed"><Icon name="lock" size={14} />Closed</span>
		{:else if confirming}
			<div class="confirm">
				<strong>Delete this {correction ? 'correction' : 'cancellation'}?</strong>
				<span class="spacer"></span>
				<Button size="sm" kind="primary" onclick={remove}>Delete</Button>
				<Button size="sm" onclick={() => (confirming = false)}>Keep</Button>
			</div>
		{:else}
			{#if !correction}<Button kind="primary" shortcut="⌘↵" onclick={save}>Save</Button>{/if}
			<Button shortcut="esc" onclick={onclose}>Cancel</Button>
			{#if current}
				<span class="spacer"></span>
				<Button size="sm" onclick={() => (confirming = true)}>Delete</Button>
			{/if}
		{/if}
	{/snippet}
</Panel>

<style>
	.source {
		margin: 0;
		font-weight: 600;
	}

	.meta {
		margin: 0;
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.pair {
		display: flex;
		gap: var(--spacing-3);
	}

	.small {
		width: 88px;
		flex: none;
	}

	.amount {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
	}

	.amount strong {
		font-size: var(--type-md);
		font-weight: 600;
	}

	.error {
		margin: 0;
		font-size: 13px;
		color: var(--color-attention);
	}

	.closed {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.confirm {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		width: 100%;
	}

	.spacer {
		flex: 1;
	}
</style>
