<script lang="ts" module>
	// A panel is often replaced by another while it stays open (the next entry, another plugin), and
	// the room it takes must not be given away and taken again then. Each panel counts itself in on
	// creation and out a moment later, so one that replaces another sees the room already taken.
	let open = 0;
</script>

<script lang="ts">
	// The right-hand panel where editing happens (no modals for routine work). Esc closes it,
	// unless that would lose changes: then it warns once, and the next Esc goes through.
	import { onDestroy, type Snippet } from 'svelte';
	import { panelSpace, shake, warnIn } from '../motion/effects';
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

	// The page gives way only when the panel opens, not when what it shows changes.
	const opening = open === 0;
	open++;
	onDestroy(() => setTimeout(() => open--));
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
	onpointerdown={(e) => {
		// Removing the warning under the pointer would let this click land on what it covers.
		if (!(e.target instanceof Element && e.target.closest('.warning'))) warned = false;
	}}
/>

<!-- A panel takes its room beside the page (A2), so the page gives way; closing is immediate, so
     nothing waits on it. -->
<div class="slot" in:panelSpace|global={{ level: level('A2'), opening }}>
	<aside class="panel" class:warned bind:this={panel}>
		{#if warned}
			<div class="warning">
				<p role="status" in:warnIn={{ level: level('A6') }}>Press Esc again to discard changes</p>
			</div>
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
</div>

<style>
	/* The room the panel takes. Its width opens, not the panel's own, so nothing inside the panel
	   reflows on the way in. `clip` rather than `hidden`: it is not a scroll container. */
	.slot {
		display: flex;
		flex: none;
		width: 380px;
		/* Pages that wrap the panel in a plain box still get a panel as tall as the page. */
		height: 100%;
		overflow: clip;
	}

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

	/* Takes no room, so the form stays where it is. Covers the header row, and stays at the top of
	   the panel while it scrolls. */
	.warning {
		position: sticky;
		top: 0;
		z-index: 1;
		height: 0;
		margin-bottom: -18px;
		cursor: default;
	}

	/* Paper behind the pill, so scrolled fields pass under it. Keeps the panel's outline showing. */
	.warning::before {
		content: '';
		position: absolute;
		top: calc(-1 * var(--spacing-6));
		inset-inline: calc(1px - var(--spacing-6));
		height: calc(var(--spacing-6) + 36px);
		border-top: 1px solid var(--color-attention);
		background: var(--color-paper);
	}

	.warning p {
		display: flex;
		align-items: center;
		justify-content: center;
		position: relative;
		height: 26px;
		margin: 0;
		border-radius: var(--radius-pill);
		background: var(--color-attention);
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-paper);
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
