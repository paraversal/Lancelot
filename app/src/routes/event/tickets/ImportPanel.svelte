<script lang="ts">
	// Importing the ticket sheet (R-TKT-3 to R-TKT-5): the file and its hash, the columns read,
	// the day the counts stand for, tier names to map, what changes per tier since the last
	// import, and the rows that could not be read. Import stays blocked while a tier name matches
	// no tier. Without a file, the earlier imports and their dates.
	import { untrack } from 'svelte';
	import { chooseTicketSheet } from '#lib/api/dialogs.ts';
	import { api, ApiError } from '#lib/api/index.ts';
	import type { ImportDeletion } from '#lib/api/generated/ImportDeletion.ts';
	import type { ImportPreview } from '#lib/api/generated/ImportPreview.ts';
	import type { Problem } from '#lib/api/generated/Problem.ts';
	import type { TicketColumns } from '#lib/api/generated/TicketColumns.ts';
	import type { TierId } from '#lib/api/generated/TierId.ts';
	import { tierColor } from '#lib/colors.ts';
	import Button from '#lib/components/Button.svelte';
	import DateField from '#lib/components/DateField.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { formatAmount, formatDate } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';

	let {
		path: initialPath,
		onclose,
		onnewtier
	}: {
		/** The sheet to read; null to choose one. */
		path: string | null;
		onclose: () => void;
		/** "New tier" for an unknown name; the panel comes back with the same file. */
		onnewtier: (name: string, path: string) => void;
	} = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const tiers = $derived(snapshot.tickets.tiers);
	const imports = $derived(snapshot.tickets.imports);
	const columns = $derived(snapshot.event.ticketColumns);

	let path = $state<string | null>(null);
	const today = new Date().toISOString().slice(0, 10);
	let date = $state<string | null>(today);
	let preview = $state<ImportPreview | null>(null);
	let reading = $state(false);
	let error = $state<{ field: string | null; message: string } | null>(null);
	let reads = 0;
	untrack(() => {
		path = initialPath;
		if (path) read();
	});

	// The changes depend on the date, so typing one reads again; only the last read counts.
	async function read() {
		if (!path) return;
		const mine = ++reads;
		reading = true;
		error = null;
		try {
			const read = await api.previewTickets(path, date ?? today);
			if (mine === reads) preview = read;
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			if (mine !== reads) return;
			preview = null;
			error = { field: 'file', message: e.message };
		} finally {
			if (mine === reads) reading = false;
		}
	}

	async function choose() {
		const picked = await chooseTicketSheet();
		if (!picked) return;
		path = picked;
		preview = null;
		await read();
	}

	async function setColumn(key: keyof TicketColumns, value: string | null) {
		if ((columns[key] ?? '') === (value ?? '').trim()) return;
		error = null;
		try {
			await session.change(() => api.setTicketColumns({ ...columns, [key]: value }));
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = { field: `columns.${e.field ?? key}`, message: e.message };
			return;
		}
		await read();
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
			error = { field: null, message: e.message };
			return;
		}
		await read();
	}

	async function doImport() {
		if (!path || !preview || preview.unknown.length || !date) return;
		error = null;
		try {
			await session.change(() => api.importTickets(path!, date!, preview!.sha256));
			onclose();
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = { field: e.code === 'file_changed' ? 'file' : null, message: e.message };
			if (e.code === 'file_changed' || e.code === 'unknown_tiers') await read();
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
			error = { field: null, message: e.message };
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
			error = { field: null, message: e.message };
		}
	}

	const tierName = (id: TierId) => tiers.find((t) => t.id === id)?.name ?? '';
	const tierIndex = (id: TierId) => tiers.findIndex((t) => t.id === id);
	const previousNumber = $derived(
		preview?.previous ? imports.findIndex((i) => i.id === preview!.previous) + 1 : null
	);
	const PROBLEMS: Record<Problem, (c: TicketColumns) => string> = {
		noTier: (c) => `no ${c.tier}`,
		noOrder: (c) => `no ${c.order}`,
		cellError: () => 'a cell with an error'
	};
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

<Panel title={path ? 'Import sheet' : 'Imports'} {onclose}>
	{#if path}
		<div class="file">
			<div class="top">
				<strong>{preview?.filename ?? path.split(/[\\/]/).pop()}</strong>
				<button type="button" class="link" onclick={choose}>Choose…</button>
			</div>
			{#if preview}
				<span class="meta"
					>{preview.tickets} tickets · sha256 {shortHash(preview.sha256)}{#if preview.sameFile}
						· same file as import {previousNumber}{/if}</span
				>
			{:else if reading}
				<span class="meta">Reading…</span>
			{/if}
			{#if error?.field === 'file'}<span class="error">{error.message}</span>{/if}
		</div>

		<fieldset disabled={session.closed}>
			<div class="labelled">
				<span class="label">Columns read</span>
				<div class="columns">
					{#each [{ key: 'tier', label: 'tier' }, { key: 'order', label: 'order' }, { key: 'saleDate', label: 'sale date' }] as const as c (c.key)}
						<div class="column">
							<TextField
								label={c.label}
								value={columns[c.key] ?? ''}
								oncommit={(v) => setColumn(c.key, c.key === 'saleDate' ? v : (v ?? ''))}
							/>
						</div>
					{/each}
				</div>
				{#if error?.field?.startsWith('columns.')}<span class="error">{error.message}</span>{/if}
			</div>

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
			{#each preview.unknown as u (u.name)}
				<div class="unknown">
					<div class="top">
						<span class="name"><Icon name="warn" size={14} />„{u.name}“</span>
						<span class="meta">{u.rows} {u.rows === 1 ? 'row' : 'rows'}</span>
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
							onclick={() => onnewtier(u.name, path!)}>New tier</Button
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
						{#each removed as r (r.order)}
							<span class="conflict">Order {r.order} · conflict</span>
						{/each}
						{#if c.added}<span class="delta plus">{signed(c.added)}</span>{/if}
						{#if c.removed}<span class="delta">{signed(-c.removed)}</span>{/if}
						{#if !c.added && !c.removed}<span class="delta muted">±0</span>{/if}
					</div>
				{/each}
			</div>

			{#if preview.problems.length}
				<div class="problems">
					<strong
						>{preview.problems.length}
						{preview.problems.length === 1 ? 'row' : 'rows'} not read</strong
					>
					{#each preview.problems as p (p.row)}
						<span>Row {p.row} · {PROBLEMS[p.problem](columns)}</span>
					{/each}
				</div>
			{/if}

			{#if preview.receiptsLost.length}
				<div class="problems">
					<strong
						>{preview.receiptsLost.length}
						{preview.receiptsLost.length === 1 ? 'receipt' : 'receipts'} would be deleted</strong
					>
					<span>Attached to ticket income this sheet takes to zero:</span>
					{#each preview.receiptsLost as name, n (n)}<span>{name}</span>{/each}
				</div>
			{/if}
		{/if}

		{#if error && error.field !== 'file' && !error.field?.startsWith('columns.')}
			<p class="error">{error.message}</p>
		{/if}
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
					<div class="problems deleting">
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
			{#if error}<p class="error">{error.message}</p>{/if}
			<p class="note">The import with the latest date is the current sheet.</p>
		</div>
	{/if}

	{#snippet footer()}
		{#if session.closed}
			<span class="closed"><Icon name="lock" size={14} />Closed</span>
		{:else if path}
			<Button
				kind="primary"
				shortcut="⌘↵"
				disabled={!preview || !!preview.unknown.length || !date}
				onclick={doImport}>Import</Button
			>
			{#if preview?.unknown.length}
				<span class="todo"
					>{preview.unknown.length}
					{preview.unknown.length === 1 ? 'tier' : 'tiers'} to map</span
				>
			{/if}
		{:else}
			<Button kind="primary" icon="upload" onclick={choose}>Import sheet</Button>
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

	.top strong {
		overflow: hidden;
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.link {
		flex: none;
		padding: 0;
		border: none;
		background: none;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
		cursor: pointer;
	}

	.link:hover {
		color: var(--color-ink);
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.meta {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.labelled {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.label {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.columns {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--spacing-2);
	}

	/* The column's role sits under its field, as in UI.fig. */
	.column :global(.field) {
		display: flex;
		flex-direction: column;
	}

	.column :global(.field > label) {
		order: 2;
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

	.error {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-minus);
	}
</style>
