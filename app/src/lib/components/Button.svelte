<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon, { type IconName } from './Icon.svelte';

	let {
		kind = 'ghost',
		size = 'md',
		icon,
		shortcut,
		type = 'button',
		disabled = false,
		onclick,
		children
	}: {
		kind?: 'primary' | 'ghost' | 'marker';
		size?: 'md' | 'sm';
		icon?: IconName;
		/** Shown after the label, e.g. `⌘↵`. */
		shortcut?: string;
		type?: 'button' | 'submit';
		disabled?: boolean;
		onclick?: (e: MouseEvent) => void;
		children: Snippet;
	} = $props();
</script>

<button {type} {disabled} {onclick} class="{kind} {size}">
	{#if icon}<Icon name={icon} size={size === 'sm' ? 14 : 16} />{/if}
	<span class="label">{@render children()}</span>
	{#if shortcut}<kbd>{shortcut}</kbd>{/if}
</button>

<style>
	button {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2);
		height: 34px;
		padding: 0 14px;
		border: 1px solid transparent;
		border-radius: var(--radius-md);
		background: none;
		font-weight: 600;
		white-space: nowrap;
		cursor: pointer;
	}

	.sm {
		height: 28px;
		padding: 0 10px;
		gap: 6px;
		border-radius: var(--radius-field);
		font-size: 13px;
	}

	.primary {
		background: var(--color-ink);
		color: var(--color-paper);
	}

	.ghost {
		border-color: var(--color-rule);
	}

	.ghost:hover:not(:disabled) {
		background: var(--color-paper-sunk);
	}

	.marker {
		background: var(--color-marker);
	}

	button:disabled {
		opacity: 0.5;
		cursor: default;
	}

	kbd {
		font: inherit;
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	.primary kbd {
		color: var(--color-on-ink-muted);
	}
</style>
