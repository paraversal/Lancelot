<script lang="ts">
	// The event currency's notes and coins, shown as lists a group can read while it counts, and
	// the custom denominations it adds for packed money, such as a roll of coins. A template copies
	// the custom ones.
	import { untrack } from 'svelte';
	import Button from '../../components/Button.svelte';
	import Icon from '../../components/Icon.svelte';
	import TextField from '../../components/TextField.svelte';
	import { formatAmount } from '../../format';
	import type { SettingsSectionProps } from '../api';
	import {
		customOf,
		DENOMINATIONS,
		dropCustomPieces,
		MAX_CUSTOM_NAME,
		newCustomId,
		setCustom,
		type CustomDenomination
	} from './records';

	let { ctx }: SettingsSectionProps = $props();

	const currency = untrack(() => ctx.currency);
	const notes = DENOMINATIONS[currency].notes;
	const coins = DENOMINATIONS[currency].coins;
	const custom = $derived(customOf(ctx.records));

	let name = $state('');
	let amount = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);

	async function attempt(action: () => Promise<unknown>): Promise<boolean> {
		error = null;
		try {
			await action();
			return true;
		} catch (e) {
			if (!(e instanceof Error)) throw e;
			error = e.message;
			return false;
		}
	}

	async function add() {
		if (busy) return;
		error = null;
		const trimmed = name.trim();
		if (!trimmed) {
			error = 'Enter a name for the denomination.';
			return;
		}
		if (trimmed.length > MAX_CUSTOM_NAME) {
			error = `Keep the name within ${MAX_CUSTOM_NAME} characters.`;
			return;
		}
		const typed = amount.trim();
		if (!typed) {
			error = 'Enter what one is worth.';
			return;
		}
		if (custom.some((c) => c.name.toLowerCase() === trimmed.toLowerCase())) {
			error = 'A custom denomination already has that name.';
			return;
		}
		busy = true;
		let added = false;
		try {
			added = await attempt(async () => {
				const value = await ctx.parseAmount(typed);
				if (value <= 0) throw new Error('Enter what one is worth.');
				const id = newCustomId(custom);
				await ctx.apply([setCustom([...custom, { id, name: trimmed, value }])]);
			});
		} finally {
			busy = false;
		}
		if (added) {
			name = '';
			amount = '';
		}
	}

	async function remove(target: CustomDenomination) {
		if (busy) return;
		busy = true;
		try {
			await attempt(() =>
				ctx.apply([
					setCustom(custom.filter((c) => c.id !== target.id)),
					...dropCustomPieces(ctx.records, target.id)
				])
			);
		} finally {
			busy = false;
		}
	}
</script>

<fieldset disabled={ctx.closed}>
	<span class="note">Denominations · {currency}</span>

	<div class="group" data-testid="cash-settings-notes">
		<span class="label"><Icon name="banknote" size={14} />Notes</span>
		<ul class="values">
			{#each notes as denomination (denomination)}
				<li>{formatAmount(denomination, ctx.format)}</li>
			{/each}
		</ul>
	</div>

	<div class="group" data-testid="cash-settings-coins">
		<span class="label"><Icon name="coins" size={14} />Coins</span>
		<ul class="values">
			{#each coins as denomination (denomination)}
				<li>{formatAmount(denomination, ctx.format)}</li>
			{/each}
		</ul>
	</div>

	<div class="group" data-testid="cash-settings-custom">
		<span class="label"><Icon name="package" size={14} />Custom</span>
		{#if custom.length}
			<ul class="custom-list">
				{#each custom as denomination (denomination.id)}
					<li>
						<span class="custom-name">{denomination.name}</span>
						<span class="custom-value">{formatAmount(denomination.value, ctx.format)}</span>
						<button
							type="button"
							aria-label={`Remove ${denomination.name}`}
							onclick={() => remove(denomination)}
						>
							<Icon name="x" size={12} />
						</button>
					</li>
				{/each}
			</ul>
		{/if}
		<div class="add">
			<TextField
				label="Name"
				placeholder="Coin rolls"
				value={name}
				oninput={(v) => (name = v)}
				oncommit={(v) => (name = v ?? '')}
			/>
			<TextField
				label="Value each"
				placeholder={formatAmount(0, ctx.format)}
				value={amount}
				oninput={(v) => (amount = v)}
				oncommit={(v) => (amount = v ?? '')}
			/>
			<Button icon="plus" disabled={busy} onclick={add}>Add</Button>
		</div>
		{#if error}<p class="form-error" role="alert">{error}</p>{/if}
	</div>
</fieldset>

<style>
	fieldset {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
		min-width: 0;
		margin: 0;
		padding: 0;
		border: none;
	}

	.note {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.group {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.label {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-ink-muted);
	}

	.values {
		display: flex;
		flex-wrap: wrap;
		gap: 2px 14px;
		margin: 0;
		padding: 0;
		list-style: none;
		font-variant-numeric: tabular-nums;
	}

	.values li {
		white-space: nowrap;
	}

	.custom-list {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.custom-list li {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto 24px;
		align-items: center;
		gap: var(--spacing-2);
		min-height: 28px;
	}

	.custom-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.custom-value {
		font-variant-numeric: tabular-nums;
		color: var(--color-ink-muted);
	}

	.custom-list button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		padding: 0;
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--color-ink-muted);
		cursor: pointer;
	}

	.custom-list button:hover {
		background: var(--color-paper-sunk);
		color: var(--color-ink);
	}

	.add {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 104px auto;
		align-items: end;
		gap: var(--spacing-2);
		margin-top: var(--spacing-1);
	}
</style>
