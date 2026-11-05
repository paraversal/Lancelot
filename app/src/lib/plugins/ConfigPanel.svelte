<script lang="ts">
	// A plugin's settings, in the panel beside Settings, so its card can stay a summary. A plugin
	// with a source per tab, like Stock and sales one per cost centre, gets a tab each. The panel
	// warns before Esc closes it while a data source form has changes to lose.
	import { untrack } from 'svelte';
	import Panel from '../components/Panel.svelte';
	import Segmented from '../components/Segmented.svelte';
	import type { Plugin, SourceTab } from './api';
	import DataSourceForm from './DataSourceForm.svelte';
	import { pluginContext } from './host';
	import PluginTag from './PluginTag.svelte';
	import { sourceId } from './sources.svelte';

	let { plugin, onclose }: { plugin: Plugin; onclose: () => void } = $props();

	// The panel is keyed on the plugin, so this is made once for it.
	const ctx = untrack(() => pluginContext(plugin.id));

	/** The plugin's one source, or one per tab such as a cost centre. */
	const tabs = $derived<SourceTab[]>(plugin.sourceTabs?.(ctx) ?? [{ key: '', title: plugin.name }]);
	const tabbed = $derived(Boolean(plugin.sourceTabs) && tabs.length > 1);

	// The tab shown: the first, or the one that just appeared when a cost centre was added.
	let selected = $state('');
	let counted = untrack(() => tabs.length);
	$effect(() => {
		const keys = tabs.map((t) => t.key);
		const added = tabs.length > counted;
		counted = tabs.length;
		if (!keys.includes(selected)) selected = keys[0] ?? '';
		else if (added) selected = keys[keys.length - 1];
	});

	// Every form stays mounted, so what was typed in a tab is still there when it is chosen again.
	// Only a change matters, so the state is written in place and the panel isn't redrawn for it.
	let dirty = $state<Record<string, boolean>>({});
	const anyDirty = $derived(tabs.some((t) => dirty[t.key]));
</script>

{#snippet head()}
	<div class="title">
		<h2>{plugin.name}</h2>
		<PluginTag />
	</div>
{/snippet}

<Panel header={head} {onclose} dirty={anyDirty}>
	{#if plugin.SettingsSection}
		<plugin.SettingsSection {ctx} />
	{/if}
	{#if plugin.dataSource && tabs.length}
		<div class="data-source">
			<h3>
				Data source{plugin.sourceTabs && tabs.length === 1 ? ` for ${tabs[0].title}` : ''}
			</h3>
			<div class="tabs" class:boxed={tabbed}>
				{#if tabbed}
					<div class="tab-strip">
						<Segmented
							label="Data source for"
							variant="tabs"
							bind:value={selected}
							options={tabs.map((t) => ({ value: t.key, label: t.title }))}
						/>
					</div>
				{/if}
				{#each tabs as tab (tab.key)}
					<div class="tab-body" hidden={tabbed && tab.key !== selected}>
						<DataSourceForm
							id={sourceId(plugin.id, tab.key)}
							spec={plugin.dataSource}
							open
							ondirty={(on) => (dirty[tab.key] = on)}
						/>
					</div>
				{/each}
			</div>
		</div>
	{/if}
</Panel>

<style>
	.title {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
	}

	h2 {
		margin: 0;
		font-size: var(--type-md);
		font-weight: 600;
	}

	.data-source {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
	}

	/* The plugin's own settings come first, so the source sits under a rule of its own. */
	.data-source:not(:first-child) {
		margin-top: var(--spacing-3);
		padding-top: var(--spacing-3);
		border-top: 1px solid var(--color-rule);
	}

	h3 {
		margin: 0;
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-ink-muted);
	}

	/* A tab per source: the chosen one joins the form below it. */
	.tabs.boxed {
		overflow: hidden;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-md);
		background: var(--color-paper);
	}

	.tab-strip {
		border-bottom: 1px solid var(--color-rule);
	}

	.tab-body {
		display: flex;
		flex-direction: column;
	}

	.boxed .tab-body {
		padding: 12px;
	}

	/* The other tabs' forms stay as they are, so nothing typed in them is lost. */
	.tab-body[hidden] {
		display: none;
	}
</style>
