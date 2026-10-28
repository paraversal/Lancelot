<script lang="ts">
	// "Mark as paid" (R-PER-5): records every reimbursement on the payout list that has an
	// IBAN, from one account on one date, in a single step. Repayments are not part of it.
	import { api, ApiError } from '#lib/api/index.ts';
	import Button from '#lib/components/Button.svelte';
	import DateField from '#lib/components/DateField.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import Select from '#lib/components/Select.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';

	let { onclose }: { onclose: () => void } = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const list = $derived(snapshot.payouts);
	const payable = $derived(list.payouts.filter((p) => p.iban));
	const missing = $derived(list.payouts.filter((p) => !p.iban));
	const assets = $derived(
		snapshot.accounts.filter((a) => a.kind === 'asset').map((a) => ({ value: a.id, label: a.name }))
	);

	let account = $state(session.event!.snapshot.event.defaultReimbursementAccount);
	let date = $state<string | null>(new Date().toISOString().slice(0, 10));
	let error = $state<string | null>(null);

	async function confirm() {
		if (!date || !payable.length) return;
		try {
			await session.change(() => api.markPayoutsPaid(account, date!));
			onclose();
		} catch (e) {
			error = e instanceof ApiError ? e.message : String(e);
		}
	}
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			confirm();
		}
	}}
/>

<Panel title="Mark as paid" {onclose}>
	<div class="list">
		{#each payable as p (p.person)}
			<div class="line"><span>{p.name}</span><span>{formatAmount(p.amount, format)}</span></div>
		{/each}
		<div class="line total">
			<span class="muted"
				>{payable.length === 1 ? '1 reimbursement' : `${payable.length} reimbursements`}</span
			>
			<strong>{formatAmount(list.payableTotal, format)}</strong>
		</div>
	</div>
	{#each missing as p (p.person)}
		<span class="attention"><Icon name="warn" size={14} />{p.name} has no IBAN</span>
	{/each}
	<Select label="From" value={account} options={assets} onchange={(v) => (account = v)} />
	<DateField
		label="Date"
		value={date}
		{format}
		required
		oninput={(v) => v && (date = v)}
		oncommit={(v) => (date = v)}
	/>
	{#if error}<p class="error">{error}</p>{/if}
	{#snippet footer()}
		<Button kind="primary" shortcut="⌘↵" disabled={!payable.length} onclick={confirm}
			>Mark as paid</Button
		>
		<Button shortcut="esc" onclick={onclose}>Cancel</Button>
	{/snippet}
</Panel>

<style>
	.list {
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

	.muted {
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	strong {
		font-weight: 600;
	}

	.attention {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--color-attention);
	}

	.error {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-minus);
	}
</style>
