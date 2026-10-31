<script lang="ts">
	// Counting an asset account (R-ISO-3): what was there at the start and what is there at the
	// end. The start brings money in as an opening entry; the end is compared with the books.
	import { api, errorMessage } from '#lib/api/index.ts';
	import AmountField from '#lib/components/AmountField.svelte';
	import Button from '#lib/components/Button.svelte';
	import DateField from '#lib/components/DateField.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { popIn } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import Check from '#lib/motion/Check.svelte';
	import { session } from '#lib/state/session.svelte.ts';

	let { account, onclose }: { account: number; onclose: () => void } = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const name = $derived(snapshot.accounts.find((a) => a.id === account)?.name ?? '');
	const status = $derived(snapshot.cash.find((c) => c.account === account) ?? null);
	const details = $derived(snapshot.accounts.find((a) => a.id === account));
	const balance = $derived(details?.balance ?? 0);
	// A bank account can end below zero, and a net-movement account holds only the event's movement
	// (R-ISO-2), so it has no opening count to enter.
	const bank = $derived(details?.assetType === 'bank');
	const movement = $derived(details?.netMovement ?? false);
	const today = new Date().toISOString().slice(0, 10);

	// Read once: the panel edits its own copy until Save.
	const initial = session.event!.snapshot;
	const start = initial.cash.find((c) => c.account === account)?.opening ?? null;
	const end = initial.closingCounts.find((c) => c.account === account) ?? null;
	let openingDate = $state<string | null>(start?.date ?? initial.event.startsOn ?? today);
	let opening = $state<number | null>(start?.amount ?? null);
	let closingDate = $state<string | null>(end?.date ?? initial.event.endsOn ?? today);
	let closing = $state<number | null>(end?.amount ?? null);
	let error = $state<string | null>(null);

	// The books as they will be with the opening count typed so far.
	const computed = $derived(
		balance - (status?.opening?.amount ?? 0) + (movement ? 0 : (opening ?? 0))
	);
	const difference = $derived(closing === null ? null : closing - computed);

	async function save() {
		try {
			error = null;
			await session.change(() =>
				api.setCounts(
					account,
					opening && openingDate ? { date: openingDate, amount: opening } : null,
					closing !== null && closingDate ? { date: closingDate, amount: closing } : null
				)
			);
			onclose();
		} catch (e) {
			error = errorMessage(e);
		}
	}
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			save();
		}
	}}
/>

<Panel title={name} {onclose}>
	{#if !movement}<div class="group">
			<h3>Opening count</h3>
			<div class="form-pair">
				<DateField
					label="Date"
					value={openingDate}
					{format}
					oninput={(v) => v && (openingDate = v)}
					oncommit={(v) => (openingDate = v)}
				/>
				<AmountField
					label="Amount"
					value={opening}
					{format}
					oninput={(v) => (opening = v)}
					oncommit={(v) => (opening = v)}
				/>
			</div>
		</div>{/if}
	<div class="group">
		<h3>Closing count</h3>
		<div class="form-pair">
			<DateField
				label="Date"
				value={closingDate}
				{format}
				oninput={(v) => v && (closingDate = v)}
				oncommit={(v) => (closingDate = v)}
			/>
			<AmountField
				label={movement ? 'Net movement' : 'Amount'}
				value={closing}
				{format}
				allowNegative={bank}
				autofocus
				oninput={(v) => (closing = v)}
				oncommit={(v) => (closing = v)}
			/>
		</div>
	</div>
	<div class="sums">
		<div class="line"><span>In the books</span><span>{formatAmount(computed, format)}</span></div>
		{#if closing !== null}
			<div class="line"><span>Counted</span><span>{formatAmount(closing, format)}</span></div>
			<div class="line total">
				{#if !difference}
					<span class="match" in:popIn={{ level: level('D4') }}
						><Check level={level('D4')} size={16} delay={120} />Matches the books</span
					>
				{:else}
					<strong>{difference < 0 ? 'Less than recorded' : 'More than recorded'}</strong>
					<strong class="minus">{formatAmount(difference, format, 'magnitude')}</strong>
				{/if}
			</div>
		{/if}
	</div>
	{#if error}<p class="form-error">{error}</p>{/if}
	{#snippet footer()}
		<Button kind="primary" shortcut="⌘↵" onclick={save}>Save</Button>
		<Button shortcut="esc" onclick={onclose}>Cancel</Button>
	{/snippet}
</Panel>

<style>
	.group {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	h3 {
		margin: 0;
		font-size: 13px;
		font-weight: 600;
	}

	.sums {
		display: flex;
		flex-direction: column;
	}

	.line {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 32px;
	}

	.line.total {
		border-top: 1px solid var(--color-rule);
	}

	strong {
		font-weight: 600;
	}

	.match {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-weight: 600;
		color: var(--color-plus);
	}

	.minus {
		color: var(--color-minus);
	}
</style>
