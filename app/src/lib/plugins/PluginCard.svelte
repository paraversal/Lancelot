<script lang="ts">
	// A plugin in Settings: what it is, whether it is on, and how it is set up. Its settings open in
	// the panel beside the page, so every card keeps the same size: two lines of description, one
	// line of summary.
	import Button from '../components/Button.svelte';
	import PluginTag from './PluginTag.svelte';

	let {
		name,
		description,
		summary,
		on,
		open = false,
		onopen,
		ontoggle
	}: {
		name: string;
		description: string;
		/** What it is wired to, or that it has nothing to set up. */
		summary: string;
		on: boolean;
		/** Whether the panel beside the page shows its settings. */
		open?: boolean;
		/** Left out when there is nothing to configure, or while the plugin is off. */
		onopen?: () => void;
		ontoggle: (on: boolean) => void;
	} = $props();
</script>

<div class="card" class:selected={open} data-testid="plugin-card">
	<div class="head">
		<div class="title">
			<strong>{name}</strong>
			<PluginTag />
			<span class="grow"></span>
			<label class="toggle">
				<input
					type="checkbox"
					aria-label="{name} on"
					checked={on}
					onchange={(e) => ontoggle(e.currentTarget.checked)}
				/>
				On
			</label>
		</div>
		<p class="description">{description}</p>
	</div>
	<div class="foot">
		<span class="summary" title={summary}>{summary}</span>
		{#if on && onopen}
			<Button size="sm" onclick={onopen}>Configure</Button>
		{/if}
	</div>
</div>

<style>
	.card {
		display: flex;
		flex-direction: column;
		border: 1px solid var(--color-rule);
		border-radius: 16px;
		background: var(--color-field);
	}

	.card.selected {
		border-color: var(--color-ink);
	}

	.head {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-1);
		padding: var(--spacing-3) var(--spacing-4) 0;
	}

	.title {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		min-width: 0;
	}

	strong {
		overflow: hidden;
		font-size: var(--type-sm);
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.grow {
		flex: 1;
	}

	.toggle {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2);
		font-size: 13px;
		cursor: pointer;
	}

	.toggle input {
		accent-color: var(--color-ink);
	}

	/* Two lines whatever the text, so the cards in a row line up. */
	.description {
		display: -webkit-box;
		overflow: hidden;
		margin: 0;
		min-height: 2.9em;
		font-size: var(--type-xs);
		line-height: 1.45;
		color: var(--color-ink-muted);
		-webkit-box-orient: vertical;
		line-clamp: 2;
		-webkit-line-clamp: 2;
	}

	.foot {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		min-height: 53px;
		margin-top: var(--spacing-3);
		padding: var(--spacing-3) var(--spacing-4);
		border-top: 1px solid var(--color-rule);
	}

	.summary {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
