<script lang="ts">
	// The sheet's columns, and what came of reading it: the rows that could not be read are listed,
	// never dropped silently (R-TKT-4). Only these columns are read, the rest of the file never
	// reaches the event (R-TKT-17). Whether rows have an order ID is set until the first import:
	// an event's imports either all have them or none do.
	import Button from '../../components/Button.svelte';
	import Panel from '../../components/Panel.svelte';
	import TextField from '../../components/TextField.svelte';
	import type { PluginContext } from '../api';
	import { columnsError, columnsOf, setColumns, type Sheet, type TicketColumns } from './records';

	let {
		ctx,
		path,
		sheet,
		error,
		onchoose,
		oncolumns,
		onreview,
		onclose
	}: {
		ctx: PluginContext;
		/** The file read; null when only the columns were asked for. */
		path: string | null;
		sheet: Sheet | null;
		error: string | null;
		onchoose: () => void;
		/** The columns changed: the file is read again. */
		oncolumns: () => void;
		onreview: () => void;
		onclose: () => void;
	} = $props();

	const columns = $derived(columnsOf(ctx.records, ctx.ticketOrderIds));
	const locked = $derived(ctx.ticketOrderIds !== null);
	let columnError = $state<string | null>(null);
	const name = $derived(path?.split(/[\\/]/).pop() ?? null);
	const PROBLEMS = $derived({
		noTier: `no ${columns.tier}`,
		noOrder: `no ${columns.order}`,
		cellError: 'a cell with an error'
	});

	async function setColumn<K extends keyof TicketColumns>(key: K, value: TicketColumns[K]) {
		const trimmed = typeof value === 'string' ? value.trim() : value;
		if (columns[key] === trimmed) return;
		const next = { ...columns, [key]: value };
		columnError = columnsError(next);
		if (columnError) return;
		try {
			await ctx.apply([setColumns(next)]);
		} catch (e) {
			if (!(e instanceof Error)) throw e;
			columnError = e.message;
			return;
		}
		oncolumns();
	}
</script>

<Panel title="Ticket sheet" {onclose}>
	{#if name}
		<div class="file">
			<strong>{name}</strong>
			{#if sheet}<span class="meta">{sheet.batch.tickets.length} tickets</span>{/if}
			{#if error}<span class="form-error">{error}</span>{/if}
		</div>
	{/if}

	<div class="labelled">
		<span class="form-label">Columns read</span>
		<div class="columns">
			{#each [{ key: 'tier', label: 'tier' }, { key: 'order', label: 'order' }] as const as c (c.key)}
				{#if c.key === 'tier' || columns.orderIds}
					<div class="column">
						<TextField
							label={c.label}
							value={columns[c.key]}
							oncommit={(v) => setColumn(c.key, v ?? '')}
						/>
					</div>
				{/if}
			{/each}
		</div>
		{#if columnError}<span class="form-error">{columnError}</span>{/if}
	</div>

	<div class="labelled">
		<label class="check">
			<input
				type="checkbox"
				checked={columns.orderIds}
				disabled={locked || ctx.closed}
				onchange={(e) => setColumn('orderIds', e.currentTarget.checked)}
			/>
			Rows have an order ID
		</label>
		<span class="hint">
			{#if locked}
				The event's imports {columns.orderIds
					? 'have order IDs, so every sheet needs them'
					: 'have none, so every sheet is counted per tier'}. Delete the imports to change this.
			{:else if columns.orderIds}
				A ticket that leaves the sheet is traced to its order.
			{:else}
				Tickets are counted per tier. A tier with fewer tickets than before is a conflict, but a
				cancelled ticket and a new one in the same tier cancel out.
			{/if}
		</span>
	</div>

	{#if sheet?.problems.length}
		<div class="problems" data-testid="plugin-sheet-problems">
			<strong
				>{sheet.problems.length}
				{sheet.problems.length === 1 ? 'row' : 'rows'} not read</strong
			>
			{#each sheet.problems as p (p.row)}
				<span>Row {p.row} · {PROBLEMS[p.problem]}</span>
			{/each}
		</div>
	{/if}

	{#snippet footer()}
		{#if sheet}
			<Button kind="primary" onclick={onreview}>Review import</Button>
		{/if}
		<Button icon="upload" onclick={onchoose}>{name ? 'Choose another' : 'Import sheet'}</Button>
	{/snippet}
</Panel>

<style>
	.file {
		display: flex;
		flex-direction: column;
		gap: 4px;
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

	.labelled {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.columns {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
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

	.check {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2);
	}

	.hint {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
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
</style>
