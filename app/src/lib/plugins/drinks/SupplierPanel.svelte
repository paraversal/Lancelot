<script lang="ts">
	// The commission supplier, whose expenses are checked against the records, and the category of
	// Pfand, which the check leaves out. Booking an invoice without a supplier starts here.
	import { untrack } from 'svelte';
	import Button from '../../components/Button.svelte';
	import Icon from '../../components/Icon.svelte';
	import Panel from '../../components/Panel.svelte';
	import Select from '../../components/Select.svelte';
	import TextField from '../../components/TextField.svelte';
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
	let pfand = $state(0);
	untrack(() => {
		supplier = settings.supplier;
		pfand = settings.pfand ?? 0;
	});
	let error = $state<string | null>(null);

	const expenses = $derived(ctx.accounts.filter((a) => a.kind === 'expense'));
	// A category that was deleted counts as none.
	const pfandValue = $derived(expenses.some((a) => a.id === pfand) ? pfand : 0);

	async function save() {
		error = null;
		const name = supplier?.trim() || null;
		if (forInvoice && !name) return void (error = 'Enter who the invoice is from.');
		try {
			await ctx.apply([setCentre(centre, { supplier: name, pfand: pfandValue || null })]);
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

<Panel title={forInvoice ? 'Supplier invoice' : 'Commission'} {onclose}>
	<fieldset disabled={ctx.closed}>
		<TextField
			label="Supplier"
			value={supplier}
			autofocus
			oninput={(v) => (supplier = v)}
			oncommit={(v) => (supplier = v)}
		/>
		{#if !forInvoice}
			<Select
				label="Pfand category"
				value={pfandValue}
				options={[
					{ value: 0, label: 'None' },
					...expenses.map((a) => ({ value: a.id, label: a.name }))
				]}
				onchange={(v) => (pfand = v)}
			/>
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
	.closed {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		font-weight: 600;
		color: var(--color-ink-muted);
	}
</style>
