<script lang="ts">
	// The event's sidebar: name and dates, the pages with their counts, and the file name, which
	// opens Settings. Budget, Tickets and Drinks join in phase 2.
	import { page } from '$app/state';
	import type { OpenEvent } from '../api/generated/OpenEvent';
	import { formatDateRange } from '../format';
	import { session } from '../state/session.svelte';
	import Icon from './Icon.svelte';

	let { event }: { event: OpenEvent } = $props();

	const snapshot = $derived(event.snapshot);
	const items = $derived([
		{ href: '/event/overview', label: 'Overview', count: null },
		{ href: '/event/ledger', label: 'Ledger', count: snapshot.txns.length || null },
		{ href: '/event/people', label: 'People', count: snapshot.persons.length || null }
	]);
	const dates = $derived(
		formatDateRange(snapshot.event.startsOn, snapshot.event.endsOn, session.format)
	);
	const current = $derived(page.url.pathname);
</script>

<nav class="sidebar">
	<div class="event">
		<strong>{snapshot.event.name}</strong>
		{#if snapshot.event.closed}
			<span class="meta"><Icon name="lock" size={12} />Closed{dates ? ` · ${dates}` : ''}</span>
		{:else if dates}
			<span class="meta">{dates}</span>
		{/if}
	</div>

	<ul>
		{#each items as item (item.href)}
			<li>
				<a href={item.href} aria-current={current === item.href ? 'page' : undefined}>
					{item.label}
					{#if item.count}<span class="count">{item.count}</span>{/if}
				</a>
			</li>
		{/each}
	</ul>

	<a
		class="file"
		href="/event/settings"
		title={event.path}
		aria-current={current === '/event/settings' ? 'page' : undefined}
	>
		<Icon name="gear" />
		<span>{event.fileName}</span>
	</a>
</nav>

<style>
	.sidebar {
		display: flex;
		flex-direction: column;
		gap: 20px;
		width: 220px;
		flex: none;
		height: 100%;
		padding: var(--spacing-4);
		background: var(--color-paper-sunk);
	}

	.event {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 0 10px;
	}

	.event strong {
		font-size: var(--type-md);
		font-weight: 600;
	}

	.meta {
		display: flex;
		align-items: center;
		gap: 4px;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	ul {
		display: flex;
		flex-direction: column;
		gap: 2px;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	a {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 32px;
		padding: 0 10px;
		border-radius: var(--radius-field);
	}

	a:hover {
		background: color-mix(in srgb, var(--color-rule) 60%, transparent);
	}

	a[aria-current='page'] {
		background: var(--color-marker);
		font-weight: 600;
	}

	.count {
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	.file {
		justify-content: flex-start;
		gap: var(--spacing-2);
		margin-top: auto;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.file span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.file[aria-current='page'] {
		color: var(--color-ink);
	}
</style>
