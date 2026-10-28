<script lang="ts">
	// Paperclip when a receipt is attached, attention warning when an expense has none.
	import Icon from './Icon.svelte';

	let { state }: { state: 'attached' | 'missing' | 'none' } = $props();
</script>

<span class="receipt {state}" title={state === 'missing' ? 'No receipt' : undefined}>
	{#if state === 'attached'}<Icon name="clip" size={14} />{/if}
	{#if state === 'missing'}<Icon name="warn" size={14} />{/if}
</span>

<style>
	.receipt {
		display: inline-flex;
		width: 16px;
		color: var(--color-ink-muted);
	}

	.missing {
		color: var(--color-attention);
	}
</style>
