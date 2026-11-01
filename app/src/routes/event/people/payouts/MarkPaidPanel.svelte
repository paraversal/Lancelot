<script lang="ts">
	// "Mark as paid" (R-PER-5): records every reimbursement on the payout list that has an
	// IBAN, from one account on one date, in a single step. Repayments are not part of it.
	import { api, errorMessage } from '#lib/api/index.ts';
	import Button from '#lib/components/Button.svelte';
	import DateField, { datesRead } from '#lib/components/DateField.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import Select from '#lib/components/Select.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { wave } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import Check from '#lib/motion/Check.svelte';
	import MovingAmount from '#lib/motion/MovingAmount.svelte';
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
	let dateInvalid = $state(false);

	// C2: the rows tick off one after another. The change is saved first; the panel keeps the
	// rows it showed and closes when the last tick is drawn.
	let paid = $state<typeof payable | null>(null);
	const rows = $derived(paid ?? payable);
	const fx = $derived(level('C2'));

	async function confirm() {
		await datesRead();
		if (dateInvalid || !date || !payable.length || paid) return;
		const shown = payable.map((p) => ({ ...p }));
		try {
			await session.change(() => api.markPayoutsPaid(account, date!));
			paid = shown;
			setTimeout(onclose, 300 + wave(fx, shown.length));
		} catch (e) {
			error = errorMessage(e);
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
		{#each rows as p, i (p.person)}
			<div class="line" class:done={!!paid}>
				<span>{p.name}</span>
				<span class="pay">
					{#if paid}<span class="ok"><Check level={fx} size={14} delay={wave(fx, i)} /></span>{/if}
					{formatAmount(p.amount, format)}
				</span>
			</div>
		{/each}
		<div class="line total">
			<span class="muted"
				>{rows.length === 1 ? '1 reimbursement' : `${rows.length} reimbursements`}</span
			>
			<strong
				><MovingAmount
					value={paid ? 0 : rows.reduce((t, p) => t + p.amount, 0)}
					{format}
					level={level('A5')}
				/></strong
			>
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
		bind:invalid={dateInvalid}
		oninput={(v) => v && (date = v)}
		oncommit={(v) => (date = v)}
	/>
	{#if error}<p class="form-error">{error}</p>{/if}
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

	.line.done {
		color: var(--color-ink-muted);
		transition: color var(--motion-normal);
	}

	.pay {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}

	.ok {
		display: inline-flex;
		color: var(--color-plus);
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
</style>
