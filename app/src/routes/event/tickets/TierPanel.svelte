<script lang="ts">
	// One ticket tier (R-TKT-1, R-TKT-2, R-TKT-10): name, price, the platform's fee, quota and the
	// other names the sheet uses for it. Below, what it sold and earned. A tier with imported or
	// sold tickets cannot be deleted.
	import { untrack } from 'svelte';
	import { api, ApiError } from '#lib/api/index.ts';
	import type { Cents } from '#lib/api/generated/Cents.ts';
	import type { TierDraft } from '#lib/api/generated/TierDraft.ts';
	import type { TierId } from '#lib/api/generated/TierId.ts';
	import AmountField from '#lib/components/AmountField.svelte';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import { feeInput, feePerTicket, parseCount, parseFee } from '#lib/tickets.ts';

	let {
		id,
		name: initialName = '',
		onclose
	}: {
		/** The tier being edited; null for a new one. */
		id: TierId | null;
		/** The name a new tier starts with, e.g. an unknown tier from the sheet. */
		name?: string;
		onclose: () => void;
	} = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const tier = $derived(snapshot.tickets.tiers.find((t) => t.id === id) ?? null);

	// --- form state, filled once: the page remounts the panel -----------------------------
	let name = $state('');
	let price = $state<Cents | null>(null);
	let fee = $state('');
	let quota = $state('');
	let aliases = $state<string[]>([]);
	let alias = $state('');
	untrack(() => {
		name = tier?.name ?? initialName;
		price = tier?.price ?? null;
		fee = tier?.feeBp ? feeInput(tier.feeBp, format) : '';
		quota = tier?.quota?.toString() ?? '';
		aliases = [...(tier?.aliases ?? [])];
	});
	let confirming = $state(false);
	let error = $state<{ field: string | null; message: string } | null>(null);

	const feeBp = $derived(parseFee(fee));
	const perTicket = $derived(price !== null && feeBp ? feePerTicket(price, feeBp) : null);
	const fieldError = (field: string) => (error?.field?.startsWith(field) ? error.message : null);
	const inUse = $derived(
		!!tier &&
			(snapshot.tickets.imports.some((i) => i.counts.some((c) => c.tier === tier.id)) ||
				snapshot.tickets.manualSales.some((m) => m.tier === tier.id))
	);

	function addAlias() {
		const a = alias.trim();
		if (a && !aliases.some((x) => x.toLowerCase() === a.toLowerCase())) aliases = [...aliases, a];
		alias = '';
	}

	function draft(): TierDraft | null {
		const missing = (field: string, message: string) => ((error = { field, message }), null);
		if (price === null) return missing('price', 'enter a price');
		if (feeBp === null) return missing('feeBp', 'enter a percentage, e.g. 2,5');
		const q = parseCount(quota);
		if (Number.isNaN(q)) return missing('quota', 'enter a whole number');
		if (alias.trim()) addAlias();
		return { name, price, feeBp, quota: q, aliases };
	}

	async function save() {
		error = null;
		const d = draft();
		if (!d) return;
		try {
			await session.change(() => (tier ? api.updateTier(tier.id, d) : api.addTier(d)));
			onclose();
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = { field: e.field, message: e.message };
		}
	}

	async function remove() {
		if (!tier) return;
		try {
			await session.change(() => api.deleteTier(tier.id));
			onclose();
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			confirming = false;
			error = { field: null, message: e.message };
		}
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !session.closed) {
			e.preventDefault();
			save();
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<Panel title={tier ? tier.name : 'New tier'} {onclose}>
	<fieldset disabled={session.closed}>
		<div class="form-field">
			<TextField
				label="Name"
				value={name}
				autofocus={!tier}
				oninput={(v) => (name = v)}
				oncommit={(v) => (name = v ?? '')}
			/>
			{#if fieldError('name')}<span class="form-error">{fieldError('name')}</span>{/if}
		</div>

		<div class="three">
			<div class="form-field">
				<AmountField
					label="Price"
					value={price}
					{format}
					previewBelow
					oninput={(v) => (price = v)}
				/>
				{#if fieldError('price')}<span class="form-error">{fieldError('price')}</span>{/if}
			</div>
			<div class="form-field">
				<div class="percent">
					<TextField
						label="Fee"
						value={fee}
						oninput={(v) => (fee = v)}
						oncommit={(v) => (fee = v ?? '')}
					/>
					<span class="sign">%</span>
				</div>
				{#if fieldError('feeBp')}
					<span class="form-error">{fieldError('feeBp')}</span>
				{:else if perTicket !== null}
					<span class="hint">= {formatAmount(perTicket, format)} per ticket</span>
				{/if}
			</div>
			<div class="form-field">
				<TextField
					label="Quota"
					value={quota}
					oninput={(v) => (quota = v)}
					oncommit={(v) => (quota = v ?? '')}
				/>
				{#if fieldError('quota')}<span class="form-error">{fieldError('quota')}</span>{/if}
			</div>
		</div>

		<div class="form-field">
			<span class="form-label">Names in the sheet</span>
			<div class="names" class:invalid={!!fieldError('aliases')}>
				{#if name.trim()}<span class="chip">{name.trim()}</span>{/if}
				{#each aliases as a, i (a)}
					<span class="chip">
						{a}
						<button
							type="button"
							aria-label="Remove {a}"
							onclick={() => (aliases = aliases.filter((_, j) => j !== i))}
						>
							<Icon name="x" size={12} />
						</button>
					</span>
				{/each}
				<input
					aria-label="Another name in the sheet"
					bind:value={alias}
					onkeydown={(e) => {
						if (e.key === 'Enter' && !e.metaKey && !e.ctrlKey) {
							e.preventDefault();
							addAlias();
						} else if (e.key === 'Backspace' && !alias && aliases.length) {
							aliases = aliases.slice(0, -1);
						}
					}}
					onblur={addAlias}
				/>
			</div>
			{#if fieldError('aliases')}<span class="form-error">{fieldError('aliases')}</span>{/if}
		</div>
	</fieldset>

	{#if error && !['name', 'price', 'feeBp', 'quota', 'aliases'].some( (f) => error?.field?.startsWith(f) )}
		<p class="form-error">{error.message}</p>
	{/if}

	{#if tier?.unresolved}
		<p class="unresolved">{tier.unresolved} removed, not counted until resolved</p>
	{/if}

	{#if tier && tier.sold}
		<div class="figures">
			<div class="line">
				<span
					>{tier.sold} sold · {[
						tier.sheet || !tier.manual ? `${tier.sheet} sheet` : null,
						tier.cancelled ? `${tier.cancelled} cancelled` : null,
						tier.manual ? `${tier.manual} manual` : null
					]
						.filter(Boolean)
						.join(', ')}</span
				>
				<span>{formatAmount(tier.gross, format)}</span>
			</div>
			{#if tier.fees}
				<div class="line">
					<span>Ticket fees</span><span>{formatAmount(-tier.fees, format)}</span>
				</div>
			{/if}
			{#if tier.refunds}
				<div class="line">
					<span>Refunds</span><span>{formatAmount(-tier.refunds, format)}</span>
				</div>
			{/if}
			<div class="line net">
				<span>Net</span><span>{formatAmount(tier.net, format)}</span>
			</div>
		</div>
	{/if}

	{#snippet footer()}
		{#if session.closed}
			<span class="closed"><Icon name="lock" size={14} />Closed</span>
		{:else if confirming}
			<div class="form-confirm">
				<strong>Delete this tier?</strong>
				<span class="grow"></span>
				<Button size="sm" kind="primary" onclick={remove}>Delete</Button>
				<Button size="sm" onclick={() => (confirming = false)}>Keep</Button>
			</div>
		{:else}
			<Button kind="primary" shortcut="⌘↵" onclick={save}>Save</Button>
			<span class="grow"></span>
			{#if inUse}
				<span class="note">in use · cannot delete</span>
			{:else if tier}
				<Button size="sm" onclick={() => (confirming = true)}>Delete</Button>
			{/if}
		{/if}
	{/snippet}
</Panel>

<style>
	.three {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--spacing-3);
	}

	.percent {
		position: relative;
	}

	.sign {
		position: absolute;
		right: 12px;
		bottom: 10px;
		font-size: 13px;
		color: var(--color-ink-muted);
		pointer-events: none;
	}

	.hint {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.names {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		min-height: 36px;
		padding: 5px 6px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-field);
	}

	.names:focus-within {
		border-color: var(--color-ink);
	}

	.names.invalid {
		border-color: var(--color-minus);
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		height: 24px;
		padding: 0 8px;
		border-radius: var(--radius-sm);
		background: var(--color-paper-sunk);
		font-size: 13px;
	}

	.chip button {
		display: flex;
		padding: 0;
		border: none;
		background: none;
		color: var(--color-ink-muted);
		cursor: pointer;
	}

	.chip button:hover {
		color: var(--color-ink);
	}

	.names input {
		flex: 1;
		min-width: 60px;
		height: 24px;
		padding: 0 4px;
		border: none;
		outline: none;
		background: none;
		font: inherit;
		font-size: 13px;
	}

	.figures {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--color-rule);
	}

	.line {
		display: flex;
		justify-content: space-between;
		min-height: 32px;
		align-items: center;
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.line span:last-child {
		color: var(--color-ink);
	}

	.unresolved {
		margin: 0;
		font-size: 13px;
		color: var(--color-attention);
	}

	.line.net {
		border-top: 1px solid var(--color-rule);
		font-size: var(--type-sm);
		font-weight: 600;
		color: var(--color-ink);
	}

	.note {
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
