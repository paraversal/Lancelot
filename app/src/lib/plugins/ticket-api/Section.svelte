<script lang="ts">
	// Pulls the tickets sold and hands them to the review (R-TKT-3). Items it can't read are listed
	// first, never dropped silently (R-TKT-4). A source without an Order field gives tickets without
	// order IDs, counted per tier.
	import Button from '../../components/Button.svelte';
	import type { TicketBatch, TicketSectionProps } from '../api';
	import { ticketsOf } from './batch';

	let { ctx, proposeImport }: TicketSectionProps = $props();

	type Held = { batch: TicketBatch; problems: { row: number; field: string }[] };

	let pulling = $state(false);
	let error = $state<string | null>(null);
	let held = $state<Held | null>(null);

	function review(batch: TicketBatch) {
		held = null;
		proposeImport(batch);
	}

	async function pull() {
		pulling = true;
		error = null;
		held = null;
		try {
			const read = ticketsOf(await ctx.pull(['tier', 'order']), ctx.ticketOrderIds);
			if ('error' in read) error = read.error;
			else if (read.problems.length) held = read;
			else review(read.batch);
		} catch (e) {
			if (!(e instanceof Error)) throw e;
			error = e.message;
		} finally {
			pulling = false;
		}
	}
</script>

<div class="section">
	<div class="row">
		<span class="note">From {ctx.dataSource}</span>
		<Button size="sm" disabled={pulling} onclick={pull}>Pull tickets</Button>
	</div>
	{#if error}<p class="form-error">{error}</p>{/if}
	{#if held}
		<div class="problems" data-testid="plugin-ticket-api-problems">
			<strong>{held.problems.length} not read</strong>
			{#each held.problems as p, n (n)}
				<span>Item {p.row}: no {p.field}</span>
			{/each}
			<div><Button size="sm" onclick={() => review(held!.batch)}>Review the rest</Button></div>
		</div>
	{/if}
</div>

<style>
	.section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
	}

	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-2) var(--spacing-4);
	}

	.note {
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
