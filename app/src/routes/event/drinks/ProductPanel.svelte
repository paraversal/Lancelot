<script lang="ts">
	// One product taken on commission (R-DRK-5): units taken and returned, and the price per unit.
	// The kept units and the cost come from the core once it is saved; the preview is display only.
	import { untrack } from 'svelte';
	import { api, ApiError } from '#lib/api/index.ts';
	import type { Cents } from '#lib/api/generated/Cents.ts';
	import type { DrinkProduct } from '#lib/api/generated/DrinkProduct.ts';
	import AmountField from '#lib/components/AmountField.svelte';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';

	let { product, onclose }: { product: DrinkProduct | null; onclose: () => void } = $props();

	const format = $derived(session.format);

	let name = $state('');
	let taken = $state('');
	let returned = $state('');
	let unitPrice = $state<Cents | null>(null);
	untrack(() => {
		name = product?.name ?? '';
		taken = product ? String(product.taken) : '';
		returned = product ? String(product.returned) : '0';
		unitPrice = product?.unitPrice ?? null;
	});
	let confirming = $state(false);
	let error = $state<{ field: string | null; message: string } | null>(null);

	const parse = (text: string) => (/^\d+$/.test(text.trim()) ? Number(text.trim()) : null);
	const kept = $derived.by(() => {
		const t = parse(taken);
		const r = parse(returned);
		return t !== null && r !== null && r <= t ? t - r : null;
	});
	const fieldError = (field: string) => (error?.field === field ? error.message : null);

	async function save() {
		error = null;
		const t = parse(taken);
		const r = parse(returned);
		if (t === null) return void (error = { field: 'taken', message: 'enter a whole number' });
		if (r === null) return void (error = { field: 'returned', message: 'enter a whole number' });
		if (unitPrice === null) return void (error = { field: 'unitPrice', message: 'enter a price' });
		const draft = { name, taken: t, returned: r, unitPrice };
		try {
			await session.change(() =>
				product ? api.updateDrinkProduct(product.id, draft) : api.addDrinkProduct(draft)
			);
			onclose();
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = { field: e.field, message: e.message };
		}
	}

	async function remove() {
		if (!product) return;
		await session.change(() => api.deleteDrinkProduct(product.id));
		onclose();
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !session.closed) {
			e.preventDefault();
			save();
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<Panel title={product ? product.name : 'New product'} {onclose}>
	<fieldset disabled={session.closed}>
		<div class="labelled">
			<TextField
				label="Product"
				value={name}
				autofocus={!product}
				oninput={(v) => (name = v)}
				oncommit={(v) => (name = v ?? '')}
			/>
			{#if fieldError('name')}<span class="error">{fieldError('name')}</span>{/if}
		</div>
		<div class="pair">
			<div class="labelled">
				<TextField
					label="Taken"
					value={taken}
					oninput={(v) => (taken = v)}
					oncommit={(v) => (taken = v ?? '')}
				/>
				{#if fieldError('taken')}<span class="error">{fieldError('taken')}</span>{/if}
			</div>
			<div class="labelled">
				<TextField
					label="Returned"
					value={returned}
					oninput={(v) => (returned = v)}
					oncommit={(v) => (returned = v ?? '')}
				/>
				{#if fieldError('returned')}<span class="error">{fieldError('returned')}</span>{/if}
			</div>
		</div>
		<div class="labelled">
			<AmountField
				label="Price per unit"
				value={unitPrice}
				{format}
				oninput={(v) => (unitPrice = v)}
			/>
			{#if fieldError('unitPrice')}<span class="error">{fieldError('unitPrice')}</span>{/if}
			{#if kept !== null && unitPrice !== null}
				<span class="hint">{kept} kept = {formatAmount(kept * unitPrice, format)}</span>
			{/if}
		</div>
	</fieldset>

	{#if error && !['name', 'taken', 'returned', 'unitPrice'].includes(error.field ?? '')}
		<p class="error">{error.message}</p>
	{/if}

	{#snippet footer()}
		{#if session.closed}
			<span class="closed"><Icon name="lock" size={14} />Closed</span>
		{:else if confirming}
			<div class="confirm">
				<strong>Delete this product?</strong>
				<span class="grow"></span>
				<Button size="sm" kind="primary" onclick={remove}>Delete</Button>
				<Button size="sm" onclick={() => (confirming = false)}>Keep</Button>
			</div>
		{:else}
			<Button kind="primary" shortcut="⌘↵" onclick={save}>Save</Button>
			{#if product}
				<Button size="sm" onclick={() => (confirming = true)}>Delete</Button>
			{/if}
		{/if}
	{/snippet}
</Panel>

<style>
	fieldset {
		display: flex;
		flex-direction: column;
		gap: 18px;
		min-width: 0;
		margin: 0;
		padding: 0;
		border: none;
	}

	.labelled {
		display: flex;
		flex-direction: column;
		gap: 6px;
		min-width: 0;
	}

	.pair {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--spacing-3);
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

	.error {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-minus);
	}

	.confirm {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		width: 100%;
		font-size: 13px;
	}

	.grow {
		flex: 1;
	}
</style>
