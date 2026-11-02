<script lang="ts">
	import { tick } from 'svelte';
	import { goto } from '$app/navigation';
	import { api, errorMessage } from '#lib/api/index.ts';
	import { chooseEventFile, chooseNewEventFile } from '#lib/api/dialogs.ts';
	import type { AssetType } from '#lib/api/generated/AssetType.ts';
	import type { Currency } from '#lib/api/generated/Currency.ts';
	import type { TemplateSummary } from '#lib/api/generated/TemplateSummary.ts';
	import Button from '#lib/components/Button.svelte';
	import DateField, { datesRead } from '#lib/components/DateField.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Select from '#lib/components/Select.svelte';
	import { CURRENCY_OPTIONS, currencyLabel } from '#lib/currencies.ts';
	import TextField from '#lib/components/TextField.svelte';
	import { session } from '#lib/state/session.svelte.ts';

	let { onclose }: { onclose: () => void } = $props();

	let name = $state<string | null>(null);
	let startsOn = $state<string | null>(null);
	let endsOn = $state<string | null>(null);
	let location = $state<string | null>(null);
	let currency = $state<Currency>('EUR');
	let template = $state<{ path: string; summary: TemplateSummary } | null>(null);
	let error = $state<string | null>(null);
	let startsInvalid = $state(false);
	let endsInvalid = $state(false);

	// The accounts to start with, unless a template brings its own. An empty row is not an account.
	const TYPES: { value: AssetType; label: string }[] = [
		{ value: 'bank', label: 'Bank' },
		{ value: 'cash', label: 'Cash' }
	];
	let nextRow = 1;
	let rows = $state([{ row: 0, name: '', assetType: 'bank' as AssetType }]);
	const accounts = $derived(
		rows.map((r) => ({ name: r.name.trim(), assetType: r.assetType })).filter((a) => a.name)
	);
	const duplicate = $derived(
		!template && new Set(accounts.map((a) => a.name.toLowerCase())).size !== accounts.length
	);
	const ready = $derived(!!name && !duplicate && (!!template || accounts.length > 0));

	async function addRow() {
		const row = nextRow++;
		rows.push({ row, name: '', assetType: 'bank' });
		await tick();
		document.getElementById(`new-account-${row}`)?.focus();
	}

	const values = () =>
		JSON.stringify([
			name,
			startsOn,
			startsInvalid,
			endsOn,
			endsInvalid,
			location,
			currency,
			template,
			rows
		]);
	const clean = values();

	const counted = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

	async function chooseTemplate() {
		try {
			error = null;
			const path = await chooseEventFile();
			if (path) template = { path, summary: await api.templatePreview(path) };
		} catch (e) {
			error = errorMessage(e);
		}
	}

	async function create() {
		// Let the focused field commit first.
		(document.activeElement as HTMLElement | null)?.blur();
		await new Promise((r) => setTimeout(r));
		await datesRead();
		if (!name || !ready || startsInvalid || endsInvalid) return;
		const path = await chooseNewEventFile(name);
		if (!path) return;
		try {
			const opened = await api.createEvent(
				path,
				{ name, currency, startsOn, endsOn, location, notes: null },
				accounts,
				template?.path ?? null
			);
			session.update(opened);
			await goto('/event/overview');
		} catch (e) {
			error = errorMessage(e);
		}
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Enter' && e.metaKey && create()} />

<Panel title="New event" {onclose} dirty={values() !== clean}>
	<TextField
		label="Name"
		value={name}
		autofocus
		oninput={(t) => (name = t.trim() || null)}
		oncommit={(v) => (name = v)}
	/>
	<div class="form-pair">
		<DateField
			label="From"
			value={startsOn}
			format={session.format}
			bind:invalid={startsInvalid}
			oncommit={(v) => (startsOn = v)}
		/>
		<DateField
			label="To"
			value={endsOn}
			format={session.format}
			bind:invalid={endsInvalid}
			oncommit={(v) => (endsOn = v)}
		/>
	</div>
	<TextField
		label="Location"
		value={location}
		oninput={(t) => (location = t.trim() || null)}
		oncommit={(v) => (location = v)}
	/>
	{#if template}
		<div class="template">
			<span class="form-label">Currency</span>
			<span data-testid="template-currency"
				>{currencyLabel(template.summary.currency)}, from the template</span
			>
		</div>
	{:else}
		<Select
			label="Currency"
			value={currency as string}
			options={CURRENCY_OPTIONS}
			onchange={(v) => (currency = v as Currency)}
		>
			{#snippet hint()}Can't be changed once the event exists.{/snippet}
		</Select>
	{/if}
	<div class="template">
		<span class="form-label">Template</span>
		{#if template}
			<div class="chosen">
				<strong>{template.summary.name}</strong>
				<Button size="sm" onclick={() => (template = null)}>Remove</Button>
			</div>
			<span class="form-label" data-testid="template-counts">
				{counted(template.summary.categories, 'category', 'categories')} ·
				{counted(template.summary.persons, 'person', 'people')} ·
				{counted(template.summary.budgetLines, 'budget line', 'budget lines')} ·
				{counted(template.summary.tiers, 'tier', 'tiers')}
			</span>
		{:else}
			<div><Button size="sm" onclick={chooseTemplate}>Choose event</Button></div>
		{/if}
	</div>
	<div class="template">
		<span class="form-label">Accounts</span>
		{#if template}
			<ul class="accounts" data-testid="template-accounts">
				{#each template.summary.accounts as account (account)}
					<li><span class="name">{account}</span></li>
				{/each}
			</ul>
		{:else}
			<ul class="accounts">
				{#each rows as r (r.row)}
					<li>
						<input
							id="new-account-{r.row}"
							class="name"
							type="text"
							aria-label="Account name"
							placeholder="Name"
							bind:value={r.name}
						/>
						<Segmented
							label="{r.name.trim() || 'Account'} holds"
							bind:value={r.assetType}
							options={TYPES}
						/>
						{#if rows.length > 1}
							<Button size="sm" onclick={() => rows.splice(rows.indexOf(r), 1)}>Remove</Button>
						{/if}
					</li>
				{/each}
			</ul>
			<div><Button size="sm" icon="plus" onclick={addRow}>Account</Button></div>
			{#if duplicate}<p class="form-error">Two accounts have the same name</p>{/if}
		{/if}
	</div>
	{#if error}<p class="form-error">{error}</p>{/if}
	{#snippet footer()}
		<Button kind="primary" shortcut="⌘↵" disabled={!ready} onclick={create}>Create</Button>
		<Button shortcut="esc" onclick={onclose}>Cancel</Button>
	{/snippet}
</Panel>

<style>
	.template {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.chosen {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-3);
	}

	strong {
		font-weight: 600;
	}

	.accounts {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.accounts li {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		min-height: 44px;
		padding: 4px 0;
		border-bottom: 1px solid var(--color-rule);
	}

	.name {
		flex: 1 1 0;
		min-width: 0;
	}

	input.name {
		height: 34px;
		padding: 0 8px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: none;
		font: inherit;
		color: inherit;
	}
</style>
