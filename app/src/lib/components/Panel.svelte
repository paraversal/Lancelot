<script lang="ts">
	// The right-hand panel where editing happens (no modals for routine work). Esc closes it,
	// unless that would lose changes: then it warns once, and the next Esc goes through.
	import type { Snippet } from 'svelte';
	import { panelIn, shake } from '../motion/effects';
	import { level } from '../motion/mode.svelte';
	import Icon from './Icon.svelte';

	let {
		title,
		onclose,
		onescape,
		dirty = false,
		header,
		footer,
		children
	}: {
		title?: string;
		onclose: () => void;
		/** What Esc does; defaults to closing. */
		onescape?: () => void;
		/** Whether Esc would lose changes. */
		dirty?: boolean;
		/** Replaces the title, e.g. with a segmented control. */
		header?: Snippet;
		footer?: Snippet;
		children: Snippet;
	} = $props();

	let panel: HTMLElement;
	let warned = $state(false);
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key !== 'Escape') {
			warned = false;
			return;
		}
		if (e.defaultPrevented) return;
		e.preventDefault();
		// Holding Esc down is one press.
		if (dirty && (!warned || e.repeat)) {
			if (!warned) shake(panel, level('A6'));
			warned = true;
			return;
		}
		(onescape ?? onclose)();
	}}
	onpointerdown={() => (warned = false)}
/>

<!-- Panels slide in (A2); closing is immediate, so nothing waits on it. -->
<aside class="panel" class:warned bind:this={panel} in:panelIn|global={{ level: level('A2') }}>
	{#if warned}
		<div class="warning"><p role="status">Press Esc again to discard changes</p></div>
	{/if}
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

	.panel.warned {
		box-shadow: inset 0 0 0 1px var(--color-attention);
	}

	/* Sits in the panel's top padding, so the form stays where it is. */
	.warning {
		position: sticky;
		top: 0;
		z-index: 1;
		height: 0;
		margin-bottom: -18px;
	}

	.warning p {
		margin: 0;
		transform: translateY(calc(-100% - 2px));
		background: var(--color-paper);
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-attention);
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
