<script lang="ts">
	// Counting by the piece, under one of the Count panel's amounts. Typing fills in the amount and
	// saves nothing; the panel's Save is the one action, and the pieces are kept after it.
	import { untrack } from 'svelte';
	import Button from '../../components/Button.svelte';
	import { formatAmount } from '../../format';
	import type { CountSectionProps } from '../api';
	import {
		hasPieces,
		hiddenOf,
		offered,
		piecesOf,
		setPieces,
		totalOf,
		type Pieces
	} from './records';

	let { ctx, account, which, amount, setAmount, onSaved }: CountSectionProps = $props();

	const format = $derived(ctx.format);
	// Read once: like the panel, this edits its own copy until Save.
	const stored = untrack(() => piecesOf(ctx.records, account, which));
	let pieces = $state<Pieces>(stored);
	let open = $state(false);

	const total = $derived(totalOf(pieces));
	const counted = $derived(hasPieces(pieces));
	const differs = $derived(counted && amount !== total);
	const denominations = $derived(offered(hiddenOf(ctx.records), pieces));

	function set(value: number, input: HTMLInputElement) {
		const digits = input.value.replace(/\D/g, '').slice(0, 6);
		input.value = digits;
		pieces = { ...pieces, [value]: Number(digits) };
		setAmount(counted ? total : null);
	}

	$effect(() =>
		onSaved(async () => {
			if (same(pieces, stored)) return;
			await ctx.apply([setPieces(account, which, pieces)]);
		})
	);

	function same(a: Pieces, b: Pieces) {
		return [...Object.keys(a), ...Object.keys(b)].every((v) => (a[+v] ?? 0) === (b[+v] ?? 0));
	}
</script>

<div class="summary">
	{#if counted}
		<span class="total" data-testid="plugin-cash-total">{formatAmount(total, format)}</span>
	{:else}
		<span class="muted">Notes and coins</span>
	{/if}
	<Button size="sm" onclick={() => (open = !open)}>{open ? 'Hide' : 'Count'}</Button>
</div>

{#if differs}
	<div class="differs">
		<span>Amount is {amount === null ? 'empty' : formatAmount(amount, format)}</span>
		<Button size="sm" onclick={() => setAmount(total)}>Use pieces</Button>
	</div>
{/if}

{#if open}
	<div class="rows" role="group" aria-label="Notes and coins">
		{#each denominations as value (value)}
			{@const label = formatAmount(value, format)}
			<label class="row">
				<span class="value">{label}</span>
				<input
					type="text"
					inputmode="numeric"
					aria-label={`Pieces of ${label}`}
					value={pieces[value] || ''}
					oninput={(e) => set(value, e.currentTarget)}
					onfocus={(e) => e.currentTarget.select()}
				/>
				<span class="sum">{pieces[value] ? formatAmount(value * pieces[value], format) : ''}</span>
			</label>
		{/each}
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
