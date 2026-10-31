<script lang="ts">
	// Pulls the tickets sold and hands them to the review (R-TKT-3). Items it can't read are listed
	// first, never dropped silently (R-TKT-4).
	import Button from '../../components/Button.svelte';
	import type { Pulled, TicketBatch, TicketSectionProps } from '../api';

	let { ctx, proposeImport }: TicketSectionProps = $props();

	let pulling = $state(false);
	let error = $state<string | null>(null);
	let held = $state<Pulled | null>(null);

	const toBatch = (pulled: Pulled): TicketBatch => ({
		source: pulled.label,
		sha256: pulled.sha256,
		tickets: pulled.rows.map((row) => ({ tier: row.tier ?? '', order: row.order ?? '' }))
	});

	function review(pulled: Pulled) {
		held = null;
		proposeImport(toBatch(pulled));
	}

	async function pull() {
		pulling = true;
		error = null;
		held = null;
		try {
			const pulled = await ctx.pull(['tier', 'order']);
			if (pulled.problems.length) held = pulled;
			else review(pulled);
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
			<div><Button size="sm" onclick={() => review(held!)}>Review the rest</Button></div>
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
