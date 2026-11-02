<script lang="ts">
	// One product: its prices, which a template copies, and this event's counts, which it doesn't.
	import { untrack } from 'svelte';
	import AmountField, { amountsRead } from '../../components/AmountField.svelte';
	import Button from '../../components/Button.svelte';
	import Icon from '../../components/Icon.svelte';
	import Panel from '../../components/Panel.svelte';
	import TextField from '../../components/TextField.svelte';
	import { EXPLAINERS } from './figures';
	import { formatAmount } from '../../format';
	import type { PluginContext } from '../api';
	import {
		type Counts,
		NO_COUNTS,
		nextProductId,
		type Product,
		removeProduct,
		setCounts,
		setProduct
	} from './records';

	let {
		ctx,
		centre,
		product,
		counts = NO_COUNTS,
		onclose
	}: {
		ctx: PluginContext;
		centre: number;
		product: Product | null;
		counts?: Counts;
		onclose: () => void;
	} = $props();

	const format = $derived(ctx.format);
	const text = (n: number | null) => (n === null ? '' : String(n));

	let name = $state('');
	let cost = $state<number | null>(null);
	let donation = $state<number | null>(null);
	let costInvalid = $state(false);
	let donationInvalid = $state(false);
	let taken = $state('');
	let returned = $state('');
	let leftOver = $state('');
	let logged = $state('');
	untrack(() => {
		name = product?.name ?? '';
		cost = product?.cost ?? null;
		donation = product?.donation ?? null;
		taken = product ? String(counts.taken) : '';
		returned = String(counts.returned);
		leftOver = text(counts.leftOver);
		logged = text(counts.manual);
	});
	const values = () =>
		JSON.stringify([
			name,
			cost,
			donation,
			costInvalid,
			donationInvalid,
			taken,
			returned,
			leftOver,
			logged
		]);
	const clean = values();
	let confirming = $state(false);
	let error = $state<{ field: string | null; message: string } | null>(null);
	const fieldError = (field: string) => (error?.field === field ? error.message : null);

	/** The most units of one product: far above any event, and safe to multiply by a price. */
	const MAX_COUNT = 1_000_000;
	const whole = (value: string) =>
		/^\d+$/.test(value.trim()) && Number(value) <= MAX_COUNT ? Number(value) : null;
	const signed = (value: string) =>
		/^[-−]?\d+$/.test(value.trim()) && Math.abs(Number(value.replace('−', '-'))) <= MAX_COUNT
			? Number(value.replace('−', '-'))
			: null;

	const billed = $derived.by(() => {
		const t = whole(taken);
		const r = whole(returned);
		return t !== null && r !== null && r <= t ? t - r : null;
	});

	async function save() {
		error = null;
		await amountsRead();
		// The field already says why.
		if (costInvalid || donationInvalid) return;
		const fail = (field: string, message: string) => void (error = { field, message });
		const t = taken.trim() === '' ? 0 : whole(taken);
		const r = returned.trim() === '' ? 0 : whole(returned);
		const left = leftOver.trim() === '' ? null : whole(leftOver);
		const manual = logged.trim() === '' ? null : signed(logged);
		if (!name.trim()) return fail('name', 'enter a name');
		// A Strich from a paper list has no stock, so it has no cost either.
		if (cost === null && t !== 0) return fail('cost', 'enter a price');
		if (cost !== null && cost < 0) return fail('cost', 'a price cannot be negative');
		if (donation !== null && donation < 0) return fail('donation', 'a price cannot be negative');
		if (t === null) return fail('taken', 'enter a whole number');
		if (r === null) return fail('returned', 'enter a whole number');
		if (r > t) return fail('returned', 'more units returned than taken');
		if (leftOver.trim() !== '' && left === null) return fail('leftOver', 'enter a whole number');
		if (left !== null && left > t - r) return fail('leftOver', 'more left over than billed');
		if (logged.trim() !== '' && manual === null) return fail('logged', 'enter a whole number');

		const id = product?.id ?? nextProductId(ctx.records);
		try {
			await ctx.apply([
				setProduct({ ...product, id, centre, name: name.trim(), cost: cost ?? 0, donation }),
				setCounts(id, { ...counts, taken: t, returned: r, leftOver: left, manual })
			]);
			onclose();
		} catch (e) {
			if (!(e instanceof Error)) throw e;
			error = { field: null, message: e.message };
		}
	}

	async function remove() {
		if (!product) return;
		await ctx.apply(removeProduct(product.id));
		onclose();
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !ctx.closed) {
			e.preventDefault();
			save();
		}
	}
</script>

<svelte:window onkeydown={keydown} />

{#snippet count(
	label: string,
	field: keyof typeof EXPLAINERS,
	value: string,
	set: (v: string) => void
)}
	<div class="form-field">
		<TextField {label} {value} oninput={set} oncommit={(v) => set(v ?? '')} />
		<span class="hint">{EXPLAINERS[field]}</span>
		{#if fieldError(field)}<span class="form-error">{fieldError(field)}</span>{/if}
	</div>
{/snippet}

<Panel title={product ? product.name : 'New product'} {onclose} dirty={values() !== clean}>
	<fieldset disabled={ctx.closed}>
		<div class="form-field">
			<TextField
				label="Product"
				value={name}
				autofocus={!product}
				oninput={(v) => (name = v)}
				oncommit={(v) => (name = v ?? '')}
			/>
			{#if fieldError('name')}<span class="form-error">{fieldError('name')}</span>{/if}
		</div>
		<div class="form-pair">
			<div class="form-field">
				<AmountField
					label="Cost per unit"
					value={cost}
					{format}
					bind:invalid={costInvalid}
					oninput={(v) => (cost = v)}
				/>
				<span class="hint">Without deposit, such as Pfand.</span>
				{#if fieldError('cost')}<span class="form-error">{fieldError('cost')}</span>{/if}
			</div>
			<div class="form-field">
				<AmountField
					label="Donation per unit"
					value={donation}
					{format}
					bind:invalid={donationInvalid}
					oninput={(v) => (donation = v)}
				/>
				{#if fieldError('donation')}<span class="form-error">{fieldError('donation')}</span>{/if}
			</div>
		</div>
		<div class="form-pair">
			{@render count('Taken', 'taken', taken, (v) => (taken = v))}
			{@render count('Returned', 'returned', returned, (v) => (returned = v))}
		</div>
		<div class="form-pair">
			{@render count('Left over', 'leftOver', leftOver, (v) => (leftOver = v))}
			{@render count('Logged', 'logged', logged, (v) => (logged = v))}
		</div>
		{#if billed !== null && cost !== null}
			<span class="hint">{billed} billed = {formatAmount(billed * cost, format)}</span>
		{/if}
	</fieldset>

	{#if error?.field === null}<p class="form-error">{error.message}</p>{/if}

	{#snippet footer()}
		{#if ctx.closed}
			<span class="closed"><Icon name="lock" size={14} />Closed</span>
		{:else if confirming}
			<div class="form-confirm">
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
