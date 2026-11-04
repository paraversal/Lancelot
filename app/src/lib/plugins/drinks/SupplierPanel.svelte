<script lang="ts">
	// The supplier, whose expenses are checked against the records, whether unsold goods go back to
	// them, and the category of deposit, which the check leaves out. Booking an invoice without a
	// supplier starts here.
	import { untrack } from 'svelte';
	import Button from '../../components/Button.svelte';
	import Icon from '../../components/Icon.svelte';
	import Panel from '../../components/Panel.svelte';
	import Select from '../../components/Select.svelte';
	import TextField from '../../components/TextField.svelte';
	import { counterparties } from '../../ledger';
	import type { PluginContext } from '../api';
	import { type CentreSettings, setCentre } from './records';

	let {
		ctx,
		centre,
		settings,
		forInvoice,
		onclose,
		onsaved
	}: {
		ctx: PluginContext;
		centre: number;
		settings: CentreSettings;
		forInvoice: boolean;
		onclose: () => void;
		onsaved: (supplier: string | null) => void;
	} = $props();

	let supplier = $state<string | null>(null);
	let commission = $state(true);
	let deposit = $state(0);
	untrack(() => {
		supplier = settings.supplier;
		commission = settings.commission;
		deposit = settings.deposit ?? 0;
	});
	const values = () => JSON.stringify([supplier, commission, deposit]);
	const clean = values();
	let error = $state<string | null>(null);

	const expenses = $derived(ctx.accounts.filter((a) => a.kind === 'expense'));
	// A category that was deleted counts as none.
	const depositValue = $derived(expenses.some((a) => a.id === deposit) ? deposit : 0);

	async function save() {
		error = null;
		const name = supplier?.trim() || null;
		if (forInvoice && !name) return void (error = 'Enter who the invoice is from.');
		try {
			await ctx.apply([
				setCentre(centre, { supplier: name, commission, deposit: depositValue || null })
			]);
			onsaved(name);
		} catch (e) {
			if (!(e instanceof Error)) throw e;
			error = e.message;
		}
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !ctx.closed) {
			e.preventDefault();
			save();
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<Panel title={forInvoice ? 'Supplier invoice' : 'Supplier'} {onclose} dirty={values() !== clean}>
	<fieldset disabled={ctx.closed}>
		<TextField
			label="Supplier"
			value={supplier}
			autofocus
			suggestions={counterparties(ctx.txns)}
			oninput={(v) => (supplier = v)}
			oncommit={(v) => (supplier = v)}
		>
			{#snippet hint()}The invoice check finds expenses on this cost centre whose Counterparty has
				this name.{/snippet}
		</TextField>
		{#if !forInvoice}
			<div class="form-field">
				<label class="check">
					<input type="checkbox" bind:checked={commission} />
					On commission
				</label>
				<span class="hint">
					Unsold goods go back to the supplier, and you pay only for the rest. Off, you bought them
					outright, and the products have no Returned count.
				</span>
			</div>
			<Select
				label="Deposit category"
				value={depositValue}
				options={[
					{ value: 0, label: 'None' },
					...expenses.map((a) => ({ value: a.id, label: a.name }))
				]}
				onchange={(v) => (deposit = v)}
			>
				{#snippet hint()}Optional. Expenses in this category are left out of the invoice check. Use
					it for a returnable deposit on crates that couldn't be returned.{/snippet}
			</Select>
		{/if}
	</fieldset>

	{#if error}<p class="form-error">{error}</p>{/if}

	{#snippet footer()}
		{#if ctx.closed}
			<span class="closed"><Icon name="lock" size={14} />Closed</span>
		{:else}
			<Button kind="primary" shortcut="⌘↵" onclick={save}>
				{forInvoice ? 'Continue' : 'Save'}
			</Button>
		{/if}
	{/snippet}
</Panel>

<style>
	.check {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2);
	}

	.hint {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
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
