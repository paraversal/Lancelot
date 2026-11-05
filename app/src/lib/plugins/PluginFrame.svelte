<script lang="ts">
	// What marks something as a plugin, wherever one shows. The app draws it around the plugin's
	// content, so a plugin can't leave it out and every plugin looks the same.
	import type { Snippet } from 'svelte';
	import PluginTag from './PluginTag.svelte';

	let {
		name,
		description,
		actions,
		children
	}: {
		name: string;
		description?: string;
		/** At the right of the header, like a switch. */
		actions?: Snippet;
		children?: Snippet;
	} = $props();
</script>

<div class="card" data-testid="plugin-frame">
	<div class="head">
		<div class="title">
			<strong>{name}</strong>
			<PluginTag />
			<span class="grow"></span>
			{@render actions?.()}
		</div>
		{#if description}<p>{description}</p>{/if}
	</div>
	<div class="body">{@render children?.()}</div>
</div>

<style>
	.card {
		display: flex;
		flex-direction: column;
		border: 1px solid var(--color-rule);
		border-radius: 16px;
		background: var(--color-field);
	}

	.head {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-1);
		padding: var(--spacing-3) var(--spacing-4);
	}

	.title {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
	}

	.grow {
		flex: 1;
	}

	p {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.body {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
		padding: var(--spacing-4);
		border-top: 1px solid var(--color-rule);
	}

	/* A plugin that is switched off has only its header. */
	.body:empty {
		display: none;
	}
</style>
