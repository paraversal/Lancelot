<script lang="ts">
	// The sheet's columns, and what came of reading it: the rows that could not be read are listed,
	// never dropped silently (R-TKT-4). Only these columns are read, the rest of the file never
	// reaches the event (R-TKT-17).
	import Button from '../../components/Button.svelte';
	import Panel from '../../components/Panel.svelte';
	import TextField from '../../components/TextField.svelte';
	import type { PluginContext, Sheet, TicketColumns } from '../api';
	import { columnsError, columnsOf, setColumns } from './records';

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

	const columns = $derived(columnsOf(ctx.records));
	let columnError = $state<string | null>(null);
	const name = $derived(path?.split(/[\\/]/).pop() ?? null);
	const PROBLEMS = $derived({
		noTier: `no ${columns.tier}`,
		noOrder: `no ${columns.order}`,
		cellError: 'a cell with an error'
	});

	async function setColumn(key: keyof TicketColumns, value: string) {
		if (columns[key] === value.trim()) return;
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
				<div class="column">
					<TextField
						label={c.label}
						value={columns[c.key]}
						oncommit={(v) => setColumn(c.key, v ?? '')}
					/>
				</div>
			{/each}
		</div>
		{#if columnError}<span class="form-error">{columnError}</span>{/if}
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
