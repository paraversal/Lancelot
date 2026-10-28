<script lang="ts">
	let {
		label,
		count,
		active = false,
		onclick
	}: { label: string; count?: number; active?: boolean; onclick?: () => void } = $props();
</script>

<button type="button" class:active aria-pressed={active} {onclick}>
	{label}
	{#if count !== undefined}<span class="count">{count}</span>{/if}
</button>

<style>
	button {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 28px;
		padding: 0 10px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-pill);
		background: none;
		font-size: 13px;
		cursor: pointer;
	}

	.active {
		border-color: var(--color-ink);
		background: var(--color-ink);
		color: var(--color-paper);
		font-weight: 600;
	}

	.count {
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	.active .count {
		color: var(--color-on-ink-muted);
	}
</style>
