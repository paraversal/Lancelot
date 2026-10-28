<script lang="ts" generics="R">
	// A table with group headers: days in the ledger, categories (R-TX-7), people's entries.
	// Rows have a hairline under each; the caller renders their cells.
	import type { Snippet } from 'svelte';

	let {
		groups,
		row,
		head
	}: {
		groups: { key: string; rows: R[] }[];
		row: Snippet<[R]>;
		/** The group header; defaults to the key. */
		head?: Snippet<[{ key: string; rows: R[] }]>;
	} = $props();
</script>

<div class="table" role="table">
	{#each groups as group (group.key)}
		<div class="head" role="rowgroup">
			{#if head}{@render head(group)}{:else}{group.key}{/if}
		</div>
		{#each group.rows as r, i (i)}
			<div class="row" role="row">{@render row(r)}</div>
		{/each}
	{/each}
</div>

<style>
	.table {
		display: flex;
		flex-direction: column;
	}

	.head {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 14px 8px 4px;
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-ink-muted);
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		min-height: 38px;
		padding: 0 8px;
		border-bottom: 1px solid var(--color-rule);
	}
</style>
