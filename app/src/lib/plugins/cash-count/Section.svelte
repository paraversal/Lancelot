<script lang="ts">
	// Counting by the piece, under one of the Count panel's amounts. Typing fills in the amount and
	// saves nothing; the panel's Save is the one action, and the pieces are kept after it.
	import { untrack } from 'svelte';
	import Button from '../../components/Button.svelte';
	import Icon from '../../components/Icon.svelte';
	import { formatAmount } from '../../format';
	import { boxOpen } from '../../motion/effects';
	import { level } from '../../motion/mode.svelte';
	import type { CountSectionProps } from '../api';
	import {
		customKey,
		customOf,
		DENOMINATIONS,
		hasPieces,
		piecesOf,
		setPieces,
		totalOf,
		type Pieces
	} from './records';

	let { ctx, account, which, amount, setAmount, onSaved }: CountSectionProps = $props();

	const format = $derived(ctx.format);
	// The currency is set when the event is made and never changes, so it is read once.
	const notes = untrack(() => DENOMINATIONS[ctx.currency].notes);
	const coins = untrack(() => DENOMINATIONS[ctx.currency].coins);

	// Read once: like the panel, this edits its own copy until Save. A custom denomination the
	// settings no longer list has no row anymore, so its pieces would not be part of the total.
	const stored = untrack(() => piecesOf(ctx.records, account, which));
	const custom = untrack(() => customOf(ctx.records));
	const known = new Set([
		...notes.map(String),
		...coins.map(String),
		...custom.map((c) => customKey(c.id))
	]);
	let pieces = $state<Pieces>(
		Object.fromEntries(Object.entries(stored).filter(([key]) => known.has(key)))
	);
	let open = $state(false);

	const total = $derived(totalOf(pieces, custom));
	const counted = $derived(hasPieces(pieces));
	const differs = $derived(counted && amount !== total);

	function set(key: string, value: number, input: HTMLInputElement) {
		const digits = input.value.replace(/\D/g, '').slice(0, 6);
		input.value = digits;
		pieces = { ...pieces, [key]: Number(digits) };
		setAmount(counted ? total : null);
	}

	$effect(() =>
		onSaved(async () => {
			if (same(pieces, stored)) return;
			await ctx.apply([setPieces(account, which, pieces)]);
		})
	);

	function same(a: Pieces, b: Pieces) {
		return [...Object.keys(a), ...Object.keys(b)].every((key) => (a[key] ?? 0) === (b[key] ?? 0));
	}
</script>

{#snippet group(label: string, icon: 'banknote' | 'coins', values: number[], testid: string)}
	<div class="group" data-testid={testid}>
		<span class="group-label"><Icon name={icon} size={14} />{label}</span>
		<div class="rows">
			{#each values as value (value)}
				{@const key = String(value)}
				{@const text = formatAmount(value, format)}
				<label class="row">
					<span class="value">{text}</span>
					<input
						type="text"
						inputmode="numeric"
						aria-label={`Pieces of ${text}`}
						value={pieces[key] || ''}
						oninput={(e) => set(key, value, e.currentTarget)}
						onfocus={(e) => e.currentTarget.select()}
					/>
					<span class="sum">{pieces[key] ? formatAmount(value * pieces[key], format) : ''}</span>
				</label>
			{/each}
		</div>
	</div>
{/snippet}

{#snippet customGroup()}
	<div class="group" data-testid="cash-custom">
		<span class="group-label"><Icon name="package" size={14} />Custom</span>
		<div class="rows">
			{#each custom as denomination (denomination.id)}
				{@const key = customKey(denomination.id)}
				{@const text = formatAmount(denomination.value, format)}
				<label class="row">
					<span class="value custom">
						<span class="name">{denomination.name}</span>
						<span class="unit">{text}</span>
					</span>
					<input
						type="text"
						inputmode="numeric"
						aria-label={`Pieces of ${denomination.name} (${text})`}
						value={pieces[key] || ''}
						oninput={(e) => set(key, denomination.value, e.currentTarget)}
						onfocus={(e) => e.currentTarget.select()}
					/>
					<span class="sum"
						>{pieces[key] ? formatAmount(denomination.value * pieces[key], format) : ''}</span
					>
				</label>
			{/each}
		</div>
	</div>
{/snippet}

<div class="summary">
	{#if counted}
		<span class="total" data-testid="plugin-cash-total">{formatAmount(total, format)}</span>
	{:else}
		<span class="muted">{custom.length ? 'Notes, coins and custom' : 'Notes and coins'}</span>
	{/if}
	<Button size="sm" onclick={() => (open = !open)}>{open ? 'Hide' : 'Count'}</Button>
</div>

{#if differs}
	<div class="differs" transition:boxOpen={{ level: level('A7') }}>
		<span>Amount is {amount === null ? 'empty' : formatAmount(amount, format)}</span>
		<Button size="sm" onclick={() => setAmount(total)}>Use pieces</Button>
	</div>
{/if}

{#if open}
	<div
		class="groups"
		role="group"
		aria-label="Denominations"
		transition:boxOpen={{ level: level('A7') }}
	>
		{@render group('Notes', 'banknote', notes, 'cash-notes')}
		{@render group('Coins', 'coins', coins, 'cash-coins')}
		{#if custom.length}{@render customGroup()}{/if}
	</div>
{/if}

<style>
	.summary,
	.differs {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-3);
	}

	.total {
		font-weight: 600;
	}

	.muted {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.differs {
		font-size: var(--type-xs);
		color: var(--color-attention);
	}

	.groups {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-3);
	}

	.group {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-1);
	}

	.group-label {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-ink-muted);
	}

	.rows {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-1);
		font-variant-numeric: tabular-nums;
	}

	.row {
		display: grid;
		grid-template-columns: 1fr 56px 1fr;
		align-items: center;
		gap: var(--spacing-3);
	}

	.value,
	.sum {
		text-align: right;
		white-space: nowrap;
	}

	.value.custom {
		display: flex;
		min-width: 0;
		flex-direction: column;
		align-items: flex-end;
		line-height: 1.25;
	}

	.name {
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.unit {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.sum {
		color: var(--color-ink-muted);
	}

	input {
		box-sizing: border-box;
		width: 100%;
		height: 28px;
		padding: 0 var(--spacing-2);
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-sm);
		background: var(--color-paper);
		font: inherit;
		color: inherit;
		text-align: right;
	}
</style>
