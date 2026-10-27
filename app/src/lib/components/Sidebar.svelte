<script lang="ts">
	import { page } from '$app/state';
	import Icon from './Icon.svelte';

	// Budget, Tickets and Drinks join in phase 2.
	const items = [
		{ href: '/event/overview', label: 'Overview' },
		{ href: '/event/ledger', label: 'Ledger' },
		{ href: '/event/people', label: 'People' }
	];

	const current = $derived(page.url.pathname);
</script>

<nav class="sidebar">
	<ul>
		{#each items as item (item.href)}
			<li>
				<a href={item.href} aria-current={current === item.href ? 'page' : undefined}>
					{item.label}
				</a>
			</li>
		{/each}
	</ul>

	<a
		class="foot"
		href="/event/settings"
		aria-current={current === '/event/settings' ? 'page' : undefined}
	>
		<Icon name="gear" />
		Settings
	</a>
</nav>

<style>
	.sidebar {
		display: flex;
		flex-direction: column;
		width: 220px;
		flex: none;
		height: 100%;
		padding: var(--spacing-4);
		background: var(--color-paper-sunk);
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

	.foot {
		gap: var(--spacing-2);
		margin-top: auto;
		color: var(--color-ink-muted);
		font-size: var(--type-xs);
	}

	.foot[aria-current='page'] {
		color: var(--color-ink);
	}
</style>
