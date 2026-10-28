<script lang="ts">
	// The right-hand panel where editing happens (no modals for routine work). Esc closes it.
	import type { Snippet } from 'svelte';
	import { panelIn } from '../motion/effects';
	import { level } from '../motion/mode.svelte';
	import Icon from './Icon.svelte';

	let {
		title,
		onclose,
		onescape,
		header,
		footer,
		children
	}: {
		title?: string;
		onclose: () => void;
		/** What Esc does; defaults to closing. */
		onescape?: () => void;
		/** Replaces the title, e.g. with a segmented control. */
		header?: Snippet;
		footer?: Snippet;
		children: Snippet;
	} = $props();
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key !== 'Escape' || e.defaultPrevented) return;
		e.preventDefault();
		(onescape ?? onclose)();
	}}
/>

<!-- Panels slide in (A2); closing is immediate, so nothing waits on it. -->
<aside class="panel" in:panelIn|global={{ level: level('A2') }}>
	<header>
		{#if header}{@render header()}{:else}<h2>{title}</h2>{/if}
		<button type="button" class="close" aria-label="Close" onclick={onclose}>
			<Icon name="x" size={18} />
		</button>
	</header>
	<div class="body">{@render children()}</div>
	{#if footer}<footer>{@render footer()}</footer>{/if}
</aside>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		gap: 18px;
		width: 380px;
		flex: none;
		height: 100%;
		padding: var(--spacing-6);
		border-left: 1px solid var(--color-rule);
		background: var(--color-paper);
		overflow: auto;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	h2 {
		margin: 0;
		font-size: var(--type-md);
		font-weight: 600;
	}

	.close {
		display: flex;
		padding: 0;
		border: none;
		background: none;
		color: var(--color-ink-muted);
		cursor: pointer;
	}

	.body {
		display: flex;
		flex-direction: column;
		gap: 18px;
		flex: 1;
	}

	footer {
		display: flex;
		gap: var(--spacing-2);
	}
</style>
