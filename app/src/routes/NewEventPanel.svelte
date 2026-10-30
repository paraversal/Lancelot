<script lang="ts">
	import { goto } from '$app/navigation';
	import { api, ApiError } from '#lib/api/index.ts';
	import { chooseEventFile, chooseNewEventFile } from '#lib/api/dialogs.ts';
	import type { TemplateSummary } from '#lib/api/generated/TemplateSummary.ts';
	import Button from '#lib/components/Button.svelte';
	import DateField from '#lib/components/DateField.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { session } from '#lib/state/session.svelte.ts';

	let { onclose }: { onclose: () => void } = $props();

	let name = $state<string | null>(null);
	let startsOn = $state<string | null>(null);
	let endsOn = $state<string | null>(null);
	let location = $state<string | null>(null);
	let template = $state<{ path: string; summary: TemplateSummary } | null>(null);
	let error = $state<string | null>(null);

	const counted = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

	async function chooseTemplate() {
		try {
			error = null;
			const path = await chooseEventFile();
			if (path) template = { path, summary: await api.templatePreview(path) };
		} catch (e) {
			error = e instanceof ApiError ? e.message : String(e);
		}
	}

	async function create() {
		// Let the focused field commit first.
		(document.activeElement as HTMLElement | null)?.blur();
		await new Promise((r) => setTimeout(r));
		if (!name) return;
		const path = await chooseNewEventFile(name);
		if (!path) return;
		try {
			const opened = await api.createEvent(
				path,
				{ name, startsOn, endsOn, location, notes: null },
				template?.path ?? null
			);
			session.update(opened);
			await goto('/event/overview');
		} catch (e) {
			error = e instanceof ApiError ? e.message : String(e);
		}
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Enter' && e.metaKey && create()} />

<Panel title="New event" {onclose}>
	<TextField
		label="Name"
		value={name}
		autofocus
		oninput={(t) => (name = t.trim() || null)}
		oncommit={(v) => (name = v)}
	/>
	<div class="pair">
		<DateField
			label="From"
			value={startsOn}
			format={session.format}
			oncommit={(v) => (startsOn = v)}
		/>
		<DateField label="To" value={endsOn} format={session.format} oncommit={(v) => (endsOn = v)} />
	</div>
	<TextField label="Location" value={location} oncommit={(v) => (location = v)} />
	<div class="template">
		<span class="label">Template</span>
		{#if template}
			<div class="chosen">
				<strong>{template.summary.name}</strong>
				<Button size="sm" onclick={() => (template = null)}>Remove</Button>
			</div>
			<span class="label" data-testid="template-counts">
				{counted(template.summary.categories, 'category', 'categories')} ·
				{counted(template.summary.persons, 'person', 'people')} ·
				{counted(template.summary.budgetLines, 'budget line', 'budget lines')} ·
				{counted(template.summary.tiers, 'tier', 'tiers')} ·
				{counted(template.summary.drinkProducts, 'product', 'products')}
			</span>
		{:else}
			<div><Button size="sm" onclick={chooseTemplate}>Choose event</Button></div>
		{/if}
	</div>
	{#if error}<p class="error">{error}</p>{/if}
	{#snippet footer()}
		<Button kind="primary" shortcut="⌘↵" disabled={!name} onclick={create}>Create</Button>
		<Button shortcut="esc" onclick={onclose}>Cancel</Button>
	{/snippet}
</Panel>

<style>
	.pair {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--spacing-3);
	}

	.template {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.label {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.chosen {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-3);
	}

	strong {
		font-weight: 600;
	}

	.error {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-minus);
	}
</style>
