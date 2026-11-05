<script lang="ts">
	// The event's sidebar: name and dates, the pages with their counts, the cost centres, the
	// History link, and the file name, which opens Settings. Close file leaves the event for the
	// start screen without quitting the app.
	import { tick } from 'svelte';
	import { page } from '$app/state';
	import { markerTravel } from '../motion/effects';
	import { level } from '../motion/mode.svelte';
	import type { OpenEvent } from '../api/generated/OpenEvent';
	import { formatDateRange } from '../format';
	import { centreLink, pageLinks } from '../nav';
	import { session } from '../state/session.svelte';
	import { conflictCount } from '../tickets';
	import Icon from './Icon.svelte';

	let { event, onclose }: { event: OpenEvent; onclose: () => void } = $props();

	const snapshot = $derived(event.snapshot);
	// Open ticket conflicts take the place of the count, as a badge (R-TKT-13).
	const conflicts = $derived(conflictCount(snapshot.tickets));
	const counts = $derived<Record<string, number>>({
		'/event/ledger': snapshot.txns.length,
		'/event/people': snapshot.persons.length,
		'/event/tickets': conflicts || snapshot.tickets.tiers.length,
		'/event/budget': snapshot.budgetLines.length
	});
	const pages = $derived(
		pageLinks(snapshot).map((p) => ({
			...p,
			count: counts[p.href] || null,
			attention: p.href === '/event/tickets' && conflicts > 0
		}))
	);
	// A cost centre counts the lines tagged with it.
	const centres = $derived(
		snapshot.costCentres.map((c) => ({
			href: centreLink(c.id),
			label: c.name,
			count:
				snapshot.txns.flatMap((t) => t.lines).filter((l) => l.costCentre === c.id).length || null,
			attention: false
		}))
	);
	const dates = $derived(
		formatDateRange(snapshot.event.startsOn, snapshot.event.endsOn, session.format)
	);
	const current = $derived(page.url.pathname + page.url.search);
	const isCurrent = (href: string) =>
		href.includes('?') ? current === href : page.url.pathname.startsWith(href);
	const items = $derived([...pages, ...centres]);
	const active = $derived(items.findIndex((i) => isCurrent(i.href)));

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

{#snippet link(item: { href: string; label: string; count: number | null; attention: boolean })}
	<li>
		<a href={item.href} aria-current={isCurrent(item.href) ? 'page' : undefined}>
			<span class="label">{item.label}</span>
			{#if item.count}<span
					class="count"
					class:attention={item.attention}
					title={item.attention ? 'Ticket conflicts' : undefined}>{item.count}</span
				>{/if}
		</a>
	</li>
{/snippet}

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
		{#each pages as item (item.href)}
			{@render link(item)}
		{/each}
		{#if centres.length}
			<li class="heading">Cost centres</li>
			{#each centres as item (item.href)}
				{@render link(item)}
			{/each}
		{/if}
	</ul>

	<div class="foot">
		<a
			class="file"
			href="/event/history"
			aria-current={page.url.pathname === '/event/history' ? 'page' : undefined}
		>
			<Icon name="history" />
			<span>History</span>
		</a>
		<a
			class="file"
			href="/event/settings"
			title={event.path}
			aria-current={page.url.pathname === '/event/settings' ? 'page' : undefined}
		>
			<Icon name="gear" />
			<span>{event.fileName}</span>
		</a>
		<button
			type="button"
			class="file close"
			title="Close the file and return to the start screen (⌘W)"
			onclick={onclose}
		>
			<Icon name="x" />
			<span>Close file</span>
		</button>
	</div>
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

	.heading {
		margin-top: var(--spacing-3);
		padding: var(--spacing-3) 10px 6px;
		border-top: 1px solid var(--color-rule);
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.label {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
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

	.foot {
		display: flex;
		flex-direction: column;
		gap: 2px;
		margin-top: auto;
	}

	.file {
		justify-content: flex-start;
		gap: var(--spacing-2);
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	/* A file link is an anchor; closing is an action, so it needs the link's row by hand. */
	.close {
		display: flex;
		align-items: center;
		width: 100%;
		height: 32px;
		padding: 0 10px;
		border: none;
		border-radius: var(--radius-field);
		background: none;
		cursor: pointer;
	}

	.close:hover {
		background: color-mix(in srgb, var(--color-rule) 60%, transparent);
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
