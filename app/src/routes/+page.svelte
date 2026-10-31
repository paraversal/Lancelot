<script lang="ts">
	// The start screen: recent events, New and Open. The new-event panel and the panel for a
	// damaged file open on the right.
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount, untrack } from 'svelte';
	import { openEvent, chooseAndOpen } from '#lib/actions.ts';
	import { api, ApiError } from '#lib/api/index.ts';
	import type { RecentEvent } from '#lib/api/generated/RecentEvent.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import DamagedPanel from './DamagedPanel.svelte';
	import NewEventPanel from './NewEventPanel.svelte';
	import { formatDateRange, formatTimestamp } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';

	type Problem = 'locked' | 'missing' | { damaged: ApiError };

	let recent = $state<RecentEvent[]>([]);
	let problems = $state<Record<string, Problem>>({});
	let selected = $state(0);
	let damaged = $state<{ path: string; name: string; error: ApiError } | null>(null);
	let failed = $state<string | null>(null);
	const creating = $derived(page.url.searchParams.has('new'));

	onMount(async () => {
		// After a reload inside an event, the backend still has it open.
		const current = await api.currentEvent();
		if (current) {
			session.update(current);
			goto('/event/overview', { replaceState: true });
			return;
		}
		recent = await api.recentEvents();
	});

	// Open asks through the address, also while this screen is already showing. Clearing it lets
	// the same file be asked for again after it failed.
	$effect(() => {
		const asked = page.url.searchParams.get('open');
		if (asked) untrack(() => openAsked(asked));
	});

	async function openAsked(path: string) {
		await goto('/', { replaceState: true });
		await open(path, path.split('/').pop() ?? path);
	}

	async function open(path: string, name: string) {
		failed = null;
		damaged = null;
		try {
			await openEvent(path);
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			recent = await api.recentEvents();
			const listed = recent.find((r) => r.path === path);
			if (e.code === 'damaged' || e.code === 'inconsistent') {
				problems[path] = { damaged: e };
				damaged = { path, name, error: e };
			} else if (e.code === 'already_open' && listed) problems[path] = 'locked';
			else if (!listed || listed.exists) failed = `${name}: ${e.message}`;
		}
	}

	async function forget(path: string) {
		recent = await api.forgetRecent(path);
		selected = Math.min(selected, recent.length - 1);
	}

	function keydown(e: KeyboardEvent) {
		if (creating || damaged || !recent.length) return;
		if (e.target instanceof HTMLInputElement) return;
		if (e.key === 'ArrowDown') selected = (selected + 1) % recent.length;
		else if (e.key === 'ArrowUp') selected = (selected - 1 + recent.length) % recent.length;
		else if (e.key === 'Enter') open(recent[selected].path, recent[selected].name);
		else return;
		e.preventDefault();
	}

	function shortPath(path: string) {
		return path.replace(/^\/Users\/[^/]+/, '~');
	}

	const problemOf = (r: RecentEvent): Problem | undefined =>
		r.exists ? problems[r.path] : 'missing';
</script>

<svelte:window onkeydown={keydown} />

<div class="window">
	<div class="start">
		<div class="column">
			<header>
				<h1>Lancelot</h1>
				<div class="actions">
					<Button kind="primary" icon="plus" shortcut="⇧⌘N" onclick={() => goto('/?new')}
						>New event</Button
					>
					<Button icon="folder" shortcut="⌘O" onclick={chooseAndOpen}>Open</Button>
				</div>
			</header>

			{#if failed}
				<p class="failed" role="alert"><Icon name="warn" size={14} />{failed}</p>
			{/if}

			{#if recent.length}
				<h2>Recent</h2>
				<ul>
					{#each recent as r, i (r.path)}
						{@const problem = problemOf(r)}
						<li class:selected={i === selected && !creating} class:missing={problem === 'missing'}>
							<button
								type="button"
								class="open"
								onclick={() => {
									selected = i;
									open(r.path, r.name);
								}}
							>
								<span class="name">{r.name}</span>
								<span class="path">
									{[formatDateRange(r.startsOn, r.endsOn, session.format), shortPath(r.path)]
										.filter(Boolean)
										.join('  ·  ')}
								</span>
							</button>
							{#if problem === 'missing'}
								<span class="state"><Icon name="warn" size={14} />Not found</span>
								<button
									type="button"
									class="forget"
									aria-label="Remove from list"
									onclick={() => forget(r.path)}
								>
									<Icon name="x" size={14} />
								</button>
							{:else if problem === 'locked'}
								<span class="state"><Icon name="lock" size={14} />Open in another window</span>
							{:else if problem}
								<span class="state"
									><Icon name="warn" size={14} />{problem.damaged.code === 'damaged'
										? 'Damaged'
										: 'Cannot be opened'}</span
								>
							{:else}
								<span class="when">{formatTimestamp(r.lastOpened, session.format)}</span>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
		</div>
	</div>

	{#if creating}
		<NewEventPanel onclose={() => goto('/')} />
	{:else if damaged}
		<DamagedPanel {...damaged} onclose={() => (damaged = null)} />
	{/if}
</div>

<style>
	.window {
		display: flex;
		height: 100vh;
	}

	.start {
		flex: 1;
		display: flex;
		justify-content: center;
		padding: 96px var(--spacing-10) var(--spacing-10);
		overflow: auto;
	}

	.column {
		width: 100%;
		max-width: 640px;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: var(--spacing-10);
	}

	h1 {
		margin: 0;
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.actions {
		display: flex;
		gap: var(--spacing-2);
	}

	h2 {
		margin: 0 0 6px;
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	li {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		height: 56px;
		padding: 0 12px;
		border-bottom: 1px solid var(--color-rule);
	}

	li.selected {
		border-radius: var(--radius-field);
		background: var(--color-marker);
	}

	.open {
		display: flex;
		flex-direction: column;
		gap: 3px;
		flex: 1;
		min-width: 0;
		padding: 0;
		border: none;
		background: none;
		text-align: left;
		cursor: pointer;
	}

	.name {
		font-weight: 600;
	}

	.missing .name {
		color: var(--color-ink-muted);
	}

	.path {
		overflow: hidden;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.when {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.failed {
		display: flex;
		align-items: center;
		gap: 6px;
		margin: 0 0 16px;
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-attention);
	}

	.state {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-attention);
		white-space: nowrap;
	}

	.forget {
		display: flex;
		padding: 0;
		border: none;
		background: none;
		color: var(--color-ink-muted);
		cursor: pointer;
	}
</style>
