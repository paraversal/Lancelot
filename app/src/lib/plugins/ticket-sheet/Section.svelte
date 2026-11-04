<script lang="ts">
	// Reads a ticket sheet and hands its tickets to the review (R-TKT-3). The panel only comes up when
	// there is something to sort out first: rows that can't be read, a file or columns that don't
	// work, or when asked for the columns.
	import { onDestroy } from 'svelte';
	import Button from '../../components/Button.svelte';
	import Icon from '../../components/Icon.svelte';
	import type { TicketSectionProps } from '../api';
	import {
		columnsOf,
		columnsRead,
		isSheet,
		SHEET_EXTENSIONS,
		sheetOf,
		type Sheet
	} from './records';
	import SheetPanel from './SheetPanel.svelte';

	let { ctx, proposeImport, setPanel }: TicketSectionProps = $props();

	const columns = $derived(columnsOf(ctx.records, ctx.ticketOrderIds));
	let dragging = $state(false);
	let path = $state<string | null>(null);
	let sheet = $state<Sheet | null>(null);
	let error = $state<string | null>(null);

	const openPanel = () => setPanel(panel);
	function forget() {
		path = null;
		sheet = null;
		error = null;
	}
	function closePanel() {
		setPanel(null);
		forget();
	}
	function showColumns() {
		forget();
		openPanel();
	}

	async function read(next: string, quiet = true) {
		path = next;
		sheet = null;
		error = null;
		try {
			sheet = sheetOf(await ctx.readTable(next, columnsRead(columns)), columns.orderIds);
		} catch (e) {
			if (!(e instanceof Error)) throw e;
			error = e.message;
			openPanel();
			return;
		}
		if (quiet && !sheet.problems.length) {
			const { batch } = sheet;
			closePanel();
			proposeImport(batch);
		} else openPanel();
	}

	async function choose() {
		const picked = await ctx.chooseFile('Ticket sheet', SHEET_EXTENSIONS);
		if (picked) await read(picked);
	}

	function review() {
		if (!sheet) return;
		const { batch } = sheet;
		closePanel();
		proposeImport(batch);
	}

	// A sheet dropped anywhere on the page is read like a chosen one.
	$effect(() =>
		ctx.onFileDrop({
			over: (names) => (dragging = names.some((n) => !n || isSheet(n))),
			leave: () => (dragging = false),
			drop: (paths) => {
				dragging = false;
				// In a plain browser the names stand in for paths.
				const dropped = paths.find(isSheet);
				if (dropped) read(dropped);
			}
		})
	);
	onDestroy(() => setPanel(null));
</script>

{#snippet panel()}
	<SheetPanel
		{ctx}
		{path}
		{sheet}
		{error}
		onchoose={choose}
		oncolumns={() => {
			if (path) read(path, false);
		}}
		onreview={review}
		onclose={closePanel}
	/>
{/snippet}

{#if dragging}
	<div class="drop" data-testid="plugin-sheet-drop">
		<Icon name="upload" size={20} />
		<strong>Import sheet</strong>
		<span>.ods · .xlsx · .csv</span>
	</div>
{:else}
	<div class="row">
		<button type="button" class="link" title="Columns read" onclick={showColumns}>
			Reads {columns.tier} · {columns.orderIds ? columns.order : 'no order IDs'}
		</button>
		<Button size="sm" icon="upload" onclick={choose}>Import sheet</Button>
	</div>
{/if}

<style>
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-2) var(--spacing-4);
	}

	.link {
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

	.drop {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-2);
		min-height: 120px;
		border: 1.5px dashed var(--color-ink-muted);
		border-radius: var(--radius-md);
		background: color-mix(in srgb, var(--color-marker) 25%, transparent);
	}

	.drop strong {
		font-size: var(--type-md);
	}

	.drop span {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}
</style>
