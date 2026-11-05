<script lang="ts">
	// The form behind a plugin's data source (technical design §10). A plugin only says which fields
	// it wants. The details are saved on this computer and not in the event, and the token goes to
	// the keychain and never comes back here, so it can only be replaced. Saving also tries the
	// source, since that is the only way to know the names are right.
	import { untrack } from 'svelte';
	import { api, ApiError, errorMessage } from '../api/index';
	import type { Pulled } from '../api/generated/Pulled';
	import Button from '../components/Button.svelte';
	import TextField from '../components/TextField.svelte';
	import type { DataSourceSpec } from './api';
	import { dataSources, optionalFields } from './sources.svelte';

	let {
		id,
		spec,
		open = false,
		ondirty
	}: {
		id: string;
		spec: DataSourceSpec;
		/** Whether it starts on its fields, when the panel that opens it means to configure it. */
		open?: boolean;
		/** Whether closing would lose changes. */
		ondirty?: (dirty: boolean) => void;
	} = $props();

	const view = $derived(dataSources.of(id));
	let editing = $state(false);
	let label = $state('');
	let url = $state('');
	let authHeader = $state('');
	let list = $state('');
	let fields = $state<Record<string, string>>({});
	let token = $state('');
	let replacing = $state(false);
	let busy = $state(false);
	let error = $state<{ field: string | null; message: string } | null>(null);
	let result = $state<Pulled | null>(null);
	const fieldError = (field: string) => (error?.field === field ? error.message : null);

	// Opened from the panel: ready to fill in, not a second button to press.
	let opened = false;
	$effect(() => {
		if (!open || opened) return;
		opened = true;
		edit();
	});

	// What the fields held when they were opened, so Esc can warn before the panel closes.
	const saved = $derived(view?.source ?? null);
	const dirty = $derived(
		editing &&
			(token.trim() !== '' ||
				label !== (saved?.label ?? '') ||
				url !== (saved?.url ?? '') ||
				authHeader !== (saved?.authHeader ?? 'Authorization') ||
				list !== (saved?.list ?? spec.list) ||
				spec.fields.some((f) => (fields[f.key] ?? '') !== (saved?.fields[f.key] ?? f.usually)))
	);
	// The panel only needs to hear about a change, not about every redraw of it.
	let reported: boolean | null = null;
	$effect(() => {
		if (dirty === reported) return;
		reported = dirty;
		ondirty?.(dirty);
	});

	function edit() {
		untrack(() => {
			const saved = view?.source;
			label = saved?.label ?? '';
			url = saved?.url ?? '';
			authHeader = saved?.authHeader ?? 'Authorization';
			list = saved?.list ?? spec.list;
			fields = Object.fromEntries(
				spec.fields.map((f) => [f.key, saved?.fields[f.key] ?? f.usually])
			);
		});
		token = '';
		replacing = !view?.hasToken;
		error = null;
		result = null;
		editing = true;
	}

	async function saveAndTest() {
		error = null;
		result = null;
		const blank = spec.fields.find((f) => !f.optional && !fields[f.key]?.trim());
		if (blank) {
			error = { field: `fields.${blank.key}`, message: "enter the provider's field name" };
			return;
		}
		busy = true;
		try {
			dataSources.set(
				await api.setDataSource(id, { label, url, authHeader, list, fields: { ...fields } })
			);
			if (token.trim()) {
				dataSources.set(await api.setDataSourceToken(id, token));
				token = '';
				replacing = false;
			}
			result = await api.pullDataSource(
				id,
				spec.fields.map((f) => f.key),
				optionalFields(spec)
			);
		} catch (e) {
			if (!(e instanceof ApiError)) error = { field: null, message: errorMessage(e) };
			else error = { field: e.field, message: e.message };
		} finally {
			busy = false;
		}
	}

	async function removeToken() {
		try {
			dataSources.set(await api.setDataSourceToken(id, null));
			replacing = true;
		} catch (e) {
			error = { field: null, message: errorMessage(e) };
		}
	}

	async function remove() {
		try {
			dataSources.set(await api.setDataSource(id, null));
			editing = false;
		} catch (e) {
			error = { field: null, message: errorMessage(e) };
		}
	}

	const known = new Set(['label', 'url', 'authHeader', 'list']);
	const general = $derived(
		error && !known.has(error.field ?? '') && !error.field?.startsWith('fields.')
	);
</script>

{#snippet field(
	title: string,
	key: string,
	value: string,
	set: (v: string) => void,
	hint: string | null = null
)}
	<div class="form-field">
		<TextField label={title} {value} oninput={set} oncommit={(v) => set(v ?? '')} />
		{#if hint}<span class="hint">{hint}</span>{/if}
		{#if fieldError(key)}<span class="form-error">{fieldError(key)}</span>{/if}
	</div>
{/snippet}

<div class="source" data-testid="data-source">
	{#if !editing}
		<div class="summary">
			<span class="note">
				{view ? `Pulls from ${view.source.label}` : 'Can pull from a data source.'}
			</span>
			<Button size="sm" onclick={edit}>{view ? 'Edit' : 'Set up'}</Button>
		</div>
	{:else}
		<p class="note">Saved on this computer, for all events. The token goes in the keychain.</p>
		{@render field('Name', 'label', label, (v) => (label = v))}
		{@render field('URL', 'url', url, (v) => (url = v))}
		<div class="pair">
			{@render field('Auth header', 'authHeader', authHeader, (v) => (authHeader = v))}
			<div class="form-field">
				{#if replacing}
					<label class="form-label" for="{id}-token">Token</label>
					<input
						id="{id}-token"
						class="token"
						type="password"
						autocomplete="off"
						bind:value={token}
					/>
					<span class="hint">The header's whole value, like Bearer abc123.</span>
				{:else}
					<span class="form-label">Token</span>
					<span class="saved">
						Saved
						<button type="button" class="link" onclick={() => (replacing = true)}>Replace</button>
						<button type="button" class="link" onclick={removeToken}>Remove</button>
					</span>
				{/if}
			</div>
		</div>
		<span class="note">What the provider calls things</span>
		<div class="pair">
			{@render field('List', 'list', list, (v) => (list = v))}
			{#each spec.fields as f (f.key)}
				{@render field(
					f.label,
					`fields.${f.key}`,
					fields[f.key] ?? '',
					(v) => (fields[f.key] = v),
					f.optional ? 'Optional. Leave it empty if the provider has none.' : null
				)}
			{/each}
		</div>

		{#if general}<p class="form-error" role="alert">{error?.message}</p>{/if}
		{#if result}
			<div class="result" data-testid="data-source-result">
				<strong>{result.rows.length} {result.rows.length === 1 ? 'row' : 'rows'} read</strong>
				{#each result.rows.slice(0, 3) as row, n (n)}
					<span
						>{spec.fields
							.map((f) => row[f.key])
							.filter((v) => v !== undefined)
							.join(' · ')}</span
					>
				{/each}
				{#if result.problems.length}
					<span class="attention">
						{result.problems.length}
						{result.problems.length === 1 ? 'field' : 'fields'} missing
					</span>
				{/if}
			</div>
		{/if}

		<div class="actions">
			<Button size="sm" kind="primary" disabled={busy} onclick={saveAndTest}>Save and test</Button>
			<Button size="sm" onclick={() => (editing = false)}>Close</Button>
			<span class="grow"></span>
			{#if view}<Button size="sm" onclick={remove}>Remove</Button>{/if}
		</div>
	{/if}
</div>

<style>
	.source {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-3);
	}

	.summary {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-3);
	}

	.note,
	.hint {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.pair {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
		gap: var(--spacing-3);
		align-items: start;
	}

	.token {
		height: 36px;
		padding: 0 10px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-field);
		font: inherit;
		color: var(--color-ink);
	}

	.token:focus {
		border-color: var(--color-ink);
		outline: none;
	}

	.saved {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		min-height: 36px;
	}

	.link {
		padding: 0;
		border: none;
		background: none;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
		cursor: pointer;
	}

	.link:hover {
		color: var(--color-ink);
		text-decoration: underline;
		text-underline-offset: 3px;
	}

	.result {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.result strong {
		font-weight: 600;
		color: var(--color-plus);
	}

	.attention {
		color: var(--color-attention);
	}

	.actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
	}

	.grow {
		flex: 1;
	}
</style>
