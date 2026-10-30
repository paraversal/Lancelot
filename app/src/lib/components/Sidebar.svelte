<script lang="ts">
	// The event's sidebar: name and dates, the pages with their counts, and the file name, which
	// opens Settings. Drinks joins later in phase 2.
	import { tick } from 'svelte';
	import { page } from '$app/state';
	import { markerTravel } from '../motion/effects';
	import { level } from '../motion/mode.svelte';
	import type { OpenEvent } from '../api/generated/OpenEvent';
	import { formatDateRange } from '../format';
	import { session } from '../state/session.svelte';
	import { conflictCount } from '../tickets';
	import Icon from './Icon.svelte';

	let { event }: { event: OpenEvent } = $props();

	const snapshot = $derived(event.snapshot);
	// Open ticket conflicts take the place of the count, as a badge (R-TKT-13).
	const conflicts = $derived(conflictCount(snapshot.tickets));
	const items = $derived([
		{ href: '/event/overview', label: 'Overview', count: null },
		{ href: '/event/ledger', label: 'Ledger', count: snapshot.txns.length || null },
		{ href: '/event/people', label: 'People', count: snapshot.persons.length || null },
		{
			href: '/event/tickets',
			label: 'Tickets',
			count: conflicts || snapshot.tickets.tiers.length || null,
			attention: conflicts > 0
		},
		{ href: '/event/budget', label: 'Budget', count: snapshot.budgetLines.length || null }
	]);
	const dates = $derived(
		formatDateRange(snapshot.event.startsOn, snapshot.event.endsOn, session.format)
	);
	const current = $derived(page.url.pathname);
	const active = $derived(items.findIndex((i) => current.startsWith(i.href)));

	// The yellow highlight is one element under the list that travels to the page (A1).
	let list: HTMLUListElement;
	let marker: HTMLLIElement;
	let box = $state<{ top: number; height: number } | null>(null);
	let placed = false;
	$effect(() => {
		const link = list?.querySelectorAll('a')[active];
		// A link's list item is what the marker covers.
		const item = link?.parentElement;
		const from = placed && marker ? marker.getBoundingClientRect() : null;
		const next = item ? { top: item.offsetTop, height: item.offsetHeight } : null;
		box = next;
		placed = !!next;
		if (from && next)
			tick().then(() => {
				if (marker) markerTravel(marker, from, marker.getBoundingClientRect(), level('A1'));
			});
	});
</script>

<nav class="sidebar">
	<div class="event">
		<strong>{snapshot.event.name}</strong>
		{#if snapshot.event.closedAt}
			<span class="meta"><Icon name="lock" size={12} />Closed{dates ? ` · ${dates}` : ''}</span>
		{:else if dates}
			<span class="meta">{dates}</span>
		{/if}
	</div>

	<ul bind:this={list}>
		<li
			class="marker"
			aria-hidden="true"
			bind:this={marker}
			style:top="{box?.top ?? 0}px"
			style:height="{box?.height ?? 0}px"
			style:opacity={box ? 1 : 0}
		></li>
		{#each items as item (item.href)}
			<li>
				<a href={item.href} aria-current={current.startsWith(item.href) ? 'page' : undefined}>
					{item.label}
					{#if item.count}<span
							class="count"
							class:attention={'attention' in item && item.attention}
							title={'attention' in item && item.attention ? 'Ticket conflicts' : undefined}
							>{item.count}</span
						>{/if}
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

	ul {
		position: relative;
	}

	.marker {
		position: absolute;
		left: 0;
		right: 0;
		border-radius: var(--radius-field);
		background: var(--color-marker);
		pointer-events: none;
	}

	li {
		position: relative;
	}

	a[aria-current='page'] {
		font-weight: 600;
	}

	a[aria-current='page']:hover {
		background: none;
	}

	.file[aria-current='page'] {
		background: var(--color-marker);
	}

	.count {
		font-size: var(--type-xs);
		font-weight: 400;
		color: var(--color-ink-muted);
	}

	.count.attention {
		padding: 0 6px;
		border-radius: 9px;
		background: var(--color-attention);
		font-weight: 600;
		color: var(--color-paper);
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
