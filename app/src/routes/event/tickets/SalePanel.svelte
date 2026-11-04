<script lang="ts">
	// A manual ticket sale (R-TKT-7): tickets sold outside the sheet, e.g. at the door. The money
	// is income in the account or with the person who took it; a person's balance moves by the
	// amount. Below, the
	// earlier manual sales, each opening here.
	import { untrack } from 'svelte';
	import { api, ApiError, errorMessage } from '#lib/api/index.ts';
	import type { AccountId } from '#lib/api/generated/AccountId.ts';
	import type { ManualSaleDraft } from '#lib/api/generated/ManualSaleDraft.ts';
	import type { ManualSaleId } from '#lib/api/generated/ManualSaleId.ts';
	import type { TierId } from '#lib/api/generated/TierId.ts';
	import { tierColor } from '#lib/colors.ts';
	import Button from '#lib/components/Button.svelte';
	import ChoiceField from '#lib/components/ChoiceField.svelte';
	import DateField, { datesRead } from '#lib/components/DateField.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { formatAmount, formatDate } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import { parseCount } from '#lib/tickets.ts';

	let {
		id,
		onopen,
		onclose
	}: {
		/** The sale being edited; null for a new one. */
		id: ManualSaleId | null;
		/** Opens another sale from the list. */
		onopen: (id: ManualSaleId) => void;
		onclose: () => void;
	} = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const tiers = $derived(snapshot.tickets.tiers);
	const sales = $derived(snapshot.tickets.manualSales);
	const sale = $derived(sales.find((s) => s.id === id) ?? null);
	const destinations = $derived([
		...snapshot.accounts
			.filter((a) => a.kind === 'asset')
			.map((a) => ({ value: a.id, label: a.name })),
		...snapshot.accounts
			.filter((a) => a.kind === 'person')
			.map((a) => ({ value: a.id, label: a.name, group: 'People' }))
	]);

	// --- form state, filled once: the page remounts the panel ---------------------------
	let tier = $state<TierId | null>(null);
	let count = $state('');
	let destination = $state<AccountId | null>(null);
	let date = $state<string | null>(null);
	let reference = $state<string | null>(null);
	untrack(() => {
		const last = sales.at(-1);
		tier = sale?.tier ?? last?.tier ?? tiers.at(-1)?.id ?? null;
		count = sale ? String(sale.count) : '';
		destination = sale?.destination ?? last?.destination ?? null;
		date =
			sale?.date ?? last?.date ?? snapshot.event.startsOn ?? new Date().toISOString().slice(0, 10);
		reference = sale?.reference ?? null;
	});
	const values = () => JSON.stringify([tier, count, destination, date, reference]);
	const clean = values();
	let confirming = $state(false);
	let error = $state<{ field: string | null; message: string } | null>(null);

	const price = $derived(tiers.find((t) => t.id === tier)?.price ?? null);
	const n = $derived(parseCount(count));
	const gross = $derived(price !== null && n !== null && !Number.isNaN(n) ? price * n : null);
	const fieldError = (field: string) => (error?.field === field ? error.message : null);

	// A person's balance without this sale, and with it: they hold the money.
	const before = $derived.by(() => {
		const holder = snapshot.persons.find((p) => p.account === destination);
		if (!holder) return null;
		const net = holder.summary.net;
		return sale && sale.destination === destination ? net + sale.gross : net;
	});
	const after = $derived(before !== null && gross !== null ? before - gross : null);
	const balance = (net: number) =>
		net > 0
			? `owed ${formatAmount(net, format)}`
			: net < 0
				? `owes ${formatAmount(-net, format)}`
				: 'settled';

	const tierName = (id: TierId) => tiers.find((t) => t.id === id)?.name ?? '';
	const accountName = (id: AccountId) => snapshot.accounts.find((a) => a.id === id)?.name ?? '';
	const tickets = $derived(sales.reduce((t, s) => t + s.count, 0));

	function draft(): ManualSaleDraft | null {
		const missing = (field: string, message: string) => ((error = { field, message }), null);
		if (tier === null) return missing('tier', 'choose a tier');
		if (n === null || Number.isNaN(n)) return missing('count', 'enter a whole number');
		if (destination === null) return missing('destination', 'choose who received the money');
		if (date === null) return missing('date', 'enter a date');
		return { tier, count: n, destination, date, reference };
	}

	async function save() {
		error = null;
		await datesRead();
		const d = draft();
		if (!d) return;
		try {
			await session.change(() => (sale ? api.updateManualSale(sale.id, d) : api.addManualSale(d)));
			onclose();
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = { field: e.field, message: e.message };
		}
	}

	async function remove() {
		if (!sale) return;
		await session.change(() => api.deleteManualSale(sale.id));
		onclose();
	}

	async function addPerson(name: string) {
		try {
			return (await session.change(() => api.addPerson(name))).id;
		} catch (e) {
			error = { field: 'destination', message: errorMessage(e) };
			return null;
		}
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !session.closed) {
			e.preventDefault();
			save();
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<Panel title="Manual sale" {onclose} dirty={values() !== clean}>
	<fieldset disabled={session.closed}>
		<div class="pair">
			<div class="form-field tier">
				<label class="form-label" for="sale-tier">Tier</label>
				<div class="select">
					{#if tier !== null}
						<span class="dot" style:background={tierColor(tiers.findIndex((t) => t.id === tier))}
						></span>
					{/if}
					<select
						id="sale-tier"
						value={tier === null ? '' : String(tier)}
						onchange={(e) => ((tier = Number(e.currentTarget.value)), (error = null))}
					>
						{#each tiers as t (t.id)}
							<option value={String(t.id)}>{t.name}</option>
						{/each}
					</select>
					<Icon name="down" size={14} />
				</div>
				{#if fieldError('tier')}<span class="form-error">{fieldError('tier')}</span>{/if}
			</div>
			<div class="form-field count">
				<TextField
					label="Count"
					value={count}
					autofocus
					oninput={(v) => (count = v)}
					oncommit={(v) => (count = v ?? '')}
				/>
				{#if fieldError('count')}<span class="form-error">{fieldError('count')}</span>{/if}
			</div>
		</div>

		<div class="amount">
			<span class="meta"
				>{n !== null && !Number.isNaN(n) ? n : '–'} × {price !== null
					? formatAmount(price, format)
					: '–'}</span
			>
			<strong class:plus={!!gross}>{formatAmount(gross ?? 0, format, 'always')}</strong>
		</div>

		<ChoiceField
			label="Received by"
			value={destination}
			options={destinations}
			addLabel="New person…"
			error={fieldError('destination')}
			onchange={(v) => ((destination = v), (error = null))}
			onadd={addPerson}
		>
			{#snippet hint()}
				{#if before !== null}
					{balance(before)}
					{#if after !== null && after !== before}
						→ <span class:owes={after < 0}>{balance(after)}</span>
					{/if}
				{/if}
			{/snippet}
		</ChoiceField>

		<div class="pair">
			<div class="form-field date">
				<DateField
					label="Date"
					value={date}
					{format}
					required
					oninput={(v) => (date = v)}
					oncommit={(v) => (date = v ?? date)}
				/>
				{#if fieldError('date')}<span class="form-error">{fieldError('date')}</span>{/if}
			</div>
			<div class="form-field reference">
				<TextField
					label="Reference"
					value={reference}
					oninput={(t) => (reference = t.trim() || null)}
					oncommit={(v) => (reference = v)}
				/>
			</div>
		</div>
	</fieldset>

	{#if error && !['tier', 'count', 'destination', 'date'].includes(error.field ?? '')}
		<p class="form-error">{error.message}</p>
	{/if}

	{#if sales.length}
		<div class="sales">
			<div class="head">
				<span>Manual sales</span><span>{tickets} {tickets === 1 ? 'ticket' : 'tickets'}</span>
			</div>
			{#each sales as s (s.id)}
				<button type="button" class="sale" class:current={s.id === id} onclick={() => onopen(s.id)}>
					<span class="day">{formatDate(s.date, format).slice(0, format === 'de' ? 6 : 10)}</span>
					<span class="what">{s.count} × {tierName(s.tier)} · {accountName(s.destination)}</span>
					<span class="num">{formatAmount(s.gross, format)}</span>
				</button>
			{/each}
		</div>
	{/if}

	{#snippet footer()}
		{#if session.closed}
			<span class="closed"><Icon name="lock" size={14} />Closed</span>
		{:else if confirming}
			<div class="form-confirm">
				<strong>Delete this sale?</strong>
				<span class="grow"></span>
				<Button size="sm" kind="primary" onclick={remove}>Delete</Button>
				<Button size="sm" onclick={() => (confirming = false)}>Keep</Button>
			</div>
		{:else}
			<Button kind="primary" shortcut="⌘↵" onclick={save}>Save</Button>
			<Button shortcut="esc" onclick={onclose}>Cancel</Button>
			{#if sale}
				<span class="grow"></span>
				<Button size="sm" onclick={() => (confirming = true)}>Delete</Button>
			{/if}
		{/if}
	{/snippet}
</Panel>

<style>
	.pair {
		display: flex;
		gap: var(--spacing-3);
	}

	.tier,
	.reference {
		flex: 1;
	}

	.count {
		width: 96px;
		flex: none;
	}

	.date {
		width: 140px;
		flex: none;
	}

	.select {
		position: relative;
		display: flex;
		align-items: center;
		height: 36px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-field);
	}

	.select:focus-within {
		border-color: var(--color-ink);
	}

	.select .dot {
		position: absolute;
		left: 10px;
		pointer-events: none;
	}

	.select select {
		flex: 1;
		height: 100%;
		padding: 0 32px 0 26px;
		border: none;
		outline: none;
		background: none;
		font: inherit;
		color: var(--color-ink);
		appearance: none;
	}

	.select :global(svg) {
		position: absolute;
		right: 10px;
		color: var(--color-ink-muted);
		pointer-events: none;
	}

	.dot {
		width: 8px;
		height: 8px;
		flex: none;
		border-radius: 50%;
	}

	.amount {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
	}

	.amount strong {
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.plus {
		color: var(--color-plus);
	}

	.owes {
		font-weight: 600;
		color: var(--color-attention);
	}

	.meta {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.sales {
		display: flex;
		flex-direction: column;
	}

	.head {
		display: flex;
		justify-content: space-between;
		padding-bottom: 6px;
		border-bottom: 1px solid var(--color-rule);
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.sale {
		display: flex;
		align-items: baseline;
		gap: var(--spacing-3);
		min-height: 36px;
		padding: 10px 0 0;
		border: none;
		border-bottom: 1px solid var(--color-rule);
		background: none;
		text-align: left;
		cursor: pointer;
	}

	.sale:hover .what {
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.sale.current {
		font-weight: 600;
	}

	.day {
		width: 48px;
		flex: none;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.what {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.num {
		flex: none;
	}

	.closed {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		font-weight: 600;
		color: var(--color-ink-muted);
	}
</style>
