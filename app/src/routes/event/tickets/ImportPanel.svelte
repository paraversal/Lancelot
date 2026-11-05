<script lang="ts">
	// Reviewing tickets a plugin read (R-TKT-3 to R-TKT-5): what they were read from and its hash,
	// the account the money arrives in, the day the counts stand for, tier names to map and what
	// changes per tier since the last import. Import stays blocked while a tier name matches no tier.
	// Without tickets to review, the earlier imports and their dates.
	import { untrack } from 'svelte';
	import { api, ApiError } from '#lib/api/index.ts';
	import type { ImportDeletion } from '#lib/api/generated/ImportDeletion.ts';
	import type { ImportPreview } from '#lib/api/generated/ImportPreview.ts';
	import type { TicketBatch } from '#lib/api/generated/TicketBatch.ts';
	import type { TierId } from '#lib/api/generated/TierId.ts';
	import { tierColor } from '#lib/colors.ts';
	import Button from '#lib/components/Button.svelte';
	import DateField, { datesRead } from '#lib/components/DateField.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import Select from '#lib/components/Select.svelte';
	import { formatAmount, formatDate } from '#lib/format.ts';
	import { boxOpen } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import { session } from '#lib/state/session.svelte.ts';

	let {
		batch,
		onclose,
		onnewtier
	}: {
		/** The tickets to review; null for the list of imports. */
		batch: TicketBatch | null;
		onclose: () => void;
		/** "New tier" for an unknown name; the panel comes back with the same tickets. */
		onnewtier: (name: string, batch: TicketBatch) => void;
	} = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const tiers = $derived(snapshot.tickets.tiers);
	const imports = $derived(snapshot.tickets.imports);
	const ticketAccount = $derived(snapshot.ticketSettings.account);
	const assets = $derived(
		snapshot.accounts.filter((a) => a.kind === 'asset').map((a) => ({ value: a.id, label: a.name }))
	);

	const today = new Date().toISOString().slice(0, 10);
	let date = $state<string | null>(today);
	let preview = $state<ImportPreview | null>(null);
	let error = $state<string | null>(null);
	// How many reviews this panel has shown. The first one comes with the panel opening, so the
	// boxes it brings in arrive with the panel, not after it; later reviews grow their boxes in.
	let previews = $state(0);
	let reads = 0;
	untrack(() => read());

	// The changes depend on the date, so typing one reads again; only the last read counts.
	async function read() {
		if (!batch) return;
		const mine = ++reads;
		error = null;
		try {
			const read = await api.previewTickets(batch, date ?? today);
			if (mine === reads) {
				preview = read;
				previews += 1;
			}
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			if (mine !== reads) return;
			preview = null;
			error = e.message;
		}
	}

	async function setAccount(account: number) {
		error = null;
		try {
			await session.change(() => api.setTicketAccount(account === 0 ? null : account));
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = e.message;
		}
	}

	async function mapTo(name: string, id: TierId) {
		const tier = tiers.find((t) => t.id === id);
		if (!tier) return;
		error = null;
		try {
			await session.change(() =>
				api.updateTier(tier.id, {
					name: tier.name,
					price: tier.price,
					feeBp: tier.feeBp,
					quota: tier.quota,
					aliases: [...tier.aliases, name]
				})
			);
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = e.message;
			return;
		}
		await read();
	}

	/** Tier names to map, or tickets of the other kind than the event's imports. */
	const blocked = (p: ImportPreview) => p.unknown.length > 0 || p.orderIdsDiffer;

	async function doImport() {
		await datesRead();
		if (!batch || !preview || blocked(preview) || !date || ticketAccount === null) return;
		error = null;
		try {
			await session.change(() => api.importTickets(batch, date!));
			onclose();
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = e.message;
			if (e.code === 'unknown_tiers') await read();
		}
	}

	async function setDate(id: number, value: string | null) {
		const current = imports.find((i) => i.id === id)?.date;
		if (!value || value === current) return;
		await session.change(() => api.setImportDate(id, value));
	}

	let deleting = $state<{ id: number; effect: ImportDeletion } | null>(null);

	async function askDelete(id: number) {
		error = null;
		try {
			deleting = { id, effect: await api.previewDeleteImport(id) };
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = e.message;
		}
	}

	async function removeImport() {
		const id = deleting?.id;
		if (id === undefined) return;
		deleting = null;
		try {
			await session.change(() => api.deleteImport(id));
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = e.message;
		}
	}

	const tierName = (id: TierId) => tiers.find((t) => t.id === id)?.name ?? '';
	const tierIndex = (id: TierId) => tiers.findIndex((t) => t.id === id);
	const previousNumber = $derived(
		preview?.previous ? imports.findIndex((i) => i.id === preview!.previous) + 1 : null
	);
	const shortHash = (h: string) => `${h.slice(0, 4)}…${h.slice(-4)}`;
	const signed = (n: number) => (n > 0 ? `+${n}` : `−${-n}`);

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !session.closed) {
			e.preventDefault();
			doImport();
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<Panel title={batch ? 'Review import' : 'Imports'} {onclose}>
	{#if batch}
		<div class="file">
			<strong>{batch.source}</strong>
			{#if preview}
				<span class="meta"
					>{preview.tickets} tickets · sha256 {shortHash(preview.sha256)}{#if preview.sameFile}
						· same as import {previousNumber}{/if}</span
				>
			{/if}
		</div>

		<fieldset disabled={session.closed}>
			<Select
				label="Money arrives in"
				value={ticketAccount ?? 0}
				options={ticketAccount === null
					? [{ value: 0, label: 'Choose an account' }, ...assets]
					: assets}
				onchange={setAccount}
			/>

			<div class="date">
				<DateField
					label="Snapshot date"
					value={date}
					{format}
					required
					oninput={(v) => {
						date = v;
						if (v) read();
					}}
					oncommit={(v) => (date = v ?? date)}
				/>
			</div>
		</fieldset>

		{#if preview}
			{#if preview.orderIdsDiffer}
				<div
					class="problems"
					role="alert"
					transition:boxOpen|global={{ level: previews > 1 ? level('A7') : 'off' }}
				>
					<strong
						>{snapshot.tickets.orderIds
							? 'These tickets have no order IDs'
							: 'These tickets have order IDs'}</strong
					>
					<span
						>The event's imports {snapshot.tickets.orderIds ? 'have them' : 'have none'}, and every
						import has to be the same. Delete the imports to switch.</span
					>
				</div>
			{/if}
			{#each preview.unknown as u (u.name)}
				<div
					class="unknown"
					transition:boxOpen|global={{ level: previews > 1 ? level('A7') : 'off' }}
				>
					<div class="top">
						<span class="name"><Icon name="warn" size={14} />„{u.name}“</span>
						<span class="meta">{u.rows} {u.rows === 1 ? 'ticket' : 'tickets'}</span>
					</div>
					<div class="map">
						<select
							aria-label="Alias of"
							disabled={!tiers.length || session.closed}
							onchange={(e) => mapTo(u.name, Number(e.currentTarget.value))}
						>
							<option value="" selected disabled>Alias of…</option>
							{#each tiers as t (t.id)}
								<option value={t.id}>{t.name}</option>
							{/each}
						</select>
						<Button
							size="sm"
							icon="plus"
							disabled={session.closed}
							onclick={() => onnewtier(u.name, batch)}>New tier</Button
						>
					</div>
				</div>
			{/each}

			{#if !preview.current}
				<p class="older" role="note">
					Dated before the latest import: it only adds to the sales over time, and the income stays
					as it is.
				</p>
			{/if}

			<div class="changes">
				<div class="row head">
					<span>{previousNumber ? `Since import ${previousNumber}` : 'First import'}</span>
					<span>{preview.tickets} tickets</span>
				</div>
				{#each preview.tiers as c (c.tier)}
					{@const removed = preview.removed.filter((r) => r.tier === c.tier)}
					<div class="row">
						<span class="dot" style:background={tierColor(tierIndex(c.tier))}></span>
						<span class="name">{tierName(c.tier)}</span>
						{#if c.added}<span class="delta plus">{signed(c.added)}</span>{/if}
						{#if c.removed}<span class="delta">{signed(-c.removed)}</span>{/if}
						{#if !c.added && !c.removed}<span class="delta muted">±0</span>{/if}
						{#if removed.length}
							<div class="conflicts">
								{#each removed as r (r.order)}
									<span class="conflict"
										>{r.order === '' ? 'Conflict' : `Order ${r.order} · conflict`}</span
									>
								{/each}
							</div>
						{/if}
					</div>
				{/each}
			</div>

			{#if preview.receiptsLost.length}
				<div
					class="problems"
					transition:boxOpen|global={{ level: previews > 1 ? level('A7') : 'off' }}
				>
					<strong
						>{preview.receiptsLost.length}
						{preview.receiptsLost.length === 1 ? 'receipt' : 'receipts'} would be deleted</strong
					>
					<span>Attached to ticket income this sheet takes to zero:</span>
					{#each preview.receiptsLost as name, n (n)}<span>{name}</span>{/each}
				</div>
			{/if}
		{/if}

		{#if error}<p class="form-error">{error}</p>{/if}
	{:else}
		<div class="imports">
			{#each [...imports].reverse() as i, n (i.id)}
				<div class="import">
					<div class="what">
						<strong>Import {imports.length - n}</strong>
						<span class="meta">{i.filename} · {i.tickets} tickets</span>
						<span class="meta">read {formatDate(i.importedAt.slice(0, 10), format)}</span>
					</div>
					<div class="when">
						<DateField
							label="Snapshot date"
							value={i.date}
							{format}
							required
							oncommit={(v) => setDate(i.id, v)}
						/>
					</div>
					{#if !session.closed}
						<div class="actions">
							{#if deleting?.id === i.id}
								<Button size="sm" onclick={() => (deleting = null)}>Keep</Button>
								<Button size="sm" kind="primary" onclick={removeImport}>Delete</Button>
							{:else}
								<Button size="sm" onclick={() => askDelete(i.id)}>Delete</Button>
							{/if}
						</div>
					{/if}
				</div>
				{#if deleting?.id === i.id}
					<div class="problems deleting" transition:boxOpen|global={{ level: level('A7') }}>
						<strong>Delete this import?</strong>
						<span>
							Sold: {deleting.effect.soldBefore} → {deleting.effect.soldAfter} · Net:
							{formatAmount(deleting.effect.netBefore, format)} →
							{formatAmount(deleting.effect.netAfter, format)}
						</span>
						<span>
							Conflicts: {deleting.effect.conflictsBefore} → {deleting.effect.conflictsAfter}
						</span>
						{#if deleting.effect.receiptsLost.length}
							<span>Receipts deleted: {deleting.effect.receiptsLost.join(', ')}</span>
						{/if}
					</div>
				{/if}
			{/each}
			{#if error}<p class="form-error">{error}</p>{/if}
			<p class="note">The import with the latest date is the current sheet.</p>
		</div>
	{/if}

	{#snippet footer()}
		{#if session.closed}
			<span class="closed"><Icon name="lock" size={14} />Closed</span>
		{:else if batch}
			<Button
				kind="primary"
				shortcut="⌘↵"
				disabled={!preview || blocked(preview) || !date || ticketAccount === null}
				onclick={doImport}>Import</Button
			>
			{#if preview?.unknown.length}
				<span class="todo"
					>{preview.unknown.length}
					{preview.unknown.length === 1 ? 'tier' : 'tiers'} to map</span
				>
			{/if}
		{/if}
	{/snippet}
</Panel>

<style>
	.file {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.top {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--spacing-3);
	}

	.file strong {
		overflow: hidden;
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.date {
		width: 160px;
	}

	.unknown {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 12px;
		border: 1px solid var(--color-attention);
		border-radius: var(--radius-md);
	}

	.unknown .name {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-weight: 600;
		color: var(--color-attention);
	}

	.map {
		display: flex;
		gap: var(--spacing-2);
	}

	select {
		flex: 1;
		min-width: 0;
		height: 32px;
		padding: 0 8px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-field);
		font: inherit;
		font-size: 13px;
		color: var(--color-ink);
	}

	.changes {
		display: flex;
		flex-direction: column;
	}

	.changes .row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--spacing-2);
		min-height: 32px;
	}

	.changes .row:last-child {
		border-bottom: 1px solid var(--color-rule);
	}

	.changes .row.head {
		justify-content: space-between;
		min-height: 0;
		padding-bottom: 6px;
		border-bottom: 1px solid var(--color-rule);
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.dot {
		width: 8px;
		height: 8px;
		flex: none;
		border-radius: 50%;
	}

	.changes .name {
		flex: 1;
		min-width: 0;
	}

	/* On their own line under the tier's name, however many orders there are. */
	.conflicts {
		display: flex;
		flex-basis: 100%;
		flex-wrap: wrap;
		gap: 2px var(--spacing-3);
		padding: 0 0 var(--spacing-2) var(--spacing-4);
	}

	.conflict {
		font-size: var(--type-xs);
		color: var(--color-attention);
	}

	.delta {
		min-width: 32px;
		font-weight: 600;
		text-align: right;
	}

	.delta.plus {
		color: var(--color-plus);
	}

	.delta.muted {
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	.note {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.older {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-attention);
	}

	.problems {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.problems strong {
		font-weight: 600;
		color: var(--color-attention);
	}

	.imports {
		display: flex;
		flex-direction: column;
	}

	.import {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		justify-content: space-between;
		gap: var(--spacing-3);
		padding: 12px 0;
		border-bottom: 1px solid var(--color-rule);
	}

	.what {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-2);
		width: 100%;
	}

	.deleting {
		padding: 8px 0 12px;
		border-bottom: 1px solid var(--color-rule);
	}

	.when {
		width: 140px;
		flex: none;
	}

	.todo {
		font-size: var(--type-xs);
		color: var(--color-attention);
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
