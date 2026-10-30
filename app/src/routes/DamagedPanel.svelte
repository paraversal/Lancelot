<script lang="ts">
	// A file that cannot be opened: what the integrity check found, and its backups.
	import { goto } from '$app/navigation';
	import { api, ApiError } from '#lib/api/index.ts';
	import type { BackupInfo } from '#lib/api/generated/BackupInfo.ts';
	import { chooseNewEventFile } from '#lib/api/dialogs.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import { formatTimestamp } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';

	let {
		path,
		name,
		error,
		onclose
	}: { path: string; name: string; error: ApiError; onclose: () => void } = $props();

	let backups = $state<BackupInfo[]>([]);
	let failure = $state<string | null>(null);
	$effect(() => {
		api.backups(path).then(
			(b) => (backups = b),
			() => (backups = [])
		);
	});

	const fileName = $derived(path.split('/').pop() ?? path);
	const issues = $derived(error.report?.issues.filter((i) => i.severity === 'error') ?? []);

	async function restore(b: BackupInfo) {
		const target = await chooseNewEventFile(`${name} restored`);
		if (!target) return;
		try {
			session.update(await api.restoreBackup(path, b.path, target));
			await goto('/event/overview');
		} catch (e) {
			failure = e instanceof ApiError ? e.message : String(e);
		}
	}

	const capitalised = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
</script>

<Panel title={fileName} {onclose}>
	<span class="state"
		><Icon name="warn" size={14} />{error.code === 'damaged' ? 'Damaged' : 'Cannot be opened'}</span
	>
	{#if issues.length}
		<section>
			<h3>Issues</h3>
			<ul class="issues">
				{#each issues as issue, i (i)}<li>{capitalised(issue.detail)}</li>{/each}
			</ul>
		</section>
	{/if}
	{#if backups.length}
		<section>
			<h3>Backups</h3>
			<ul class="backups">
				{#each backups as b, i (b.path)}
					<li>
						<span>{b.takenAt ? formatTimestamp(b.takenAt, session.format) : b.path}</span>
						<Button size="sm" kind={i === 0 ? 'primary' : 'ghost'} onclick={() => restore(b)}
							>Restore</Button
						>
					</li>
				{/each}
			</ul>
		</section>
	{/if}
	{#if failure}<p class="error">{failure}</p>{/if}
</Panel>

<style>
	.state {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-attention);
	}

	h3 {
		margin: 0 0 4px;
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.issues li {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 28px;
		font-size: 13px;
	}

	.issues li::before {
		content: '';
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: var(--color-attention);
	}

	.backups li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 40px;
		border-bottom: 1px solid var(--color-rule);
		font-size: 13px;
	}

	.error {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-minus);
	}
</style>
