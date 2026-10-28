<script lang="ts">
	// R-COR-5: what the integrity check found, one line per issue, above the page.
	import type { Report } from '../api/generated/Report';
	import Button from './Button.svelte';
	import Icon from './Icon.svelte';

	let { report, oncheck }: { report: Report; oncheck: () => void } = $props();

	const count = $derived(report.issues.length);
	const capitalised = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
</script>

{#if count}
	<div class="banner" role="status">
		<span class="count"><Icon name="warn" />{count === 1 ? '1 issue' : `${count} issues`}</span>
		<span class="issues">
			{#each report.issues as issue, i (i)}
				{#if i > 0}<span class="dot">·</span>{/if}<span>{capitalised(issue.detail)}</span>
			{/each}
		</span>
		<Button size="sm" icon="refresh" onclick={oncheck}>Check again</Button>
	</div>
{/if}

<style>
	.banner {
		display: flex;
		align-items: center;
		gap: 10px;
		min-height: 52px;
		padding: 0 var(--spacing-10);
		border-bottom: 1px solid var(--color-rule);
		font-size: 13px;
	}

	.count {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: var(--type-sm);
		font-weight: 600;
		color: var(--color-attention);
		white-space: nowrap;
	}

	.issues {
		display: flex;
		flex-wrap: wrap;
		gap: 0 10px;
		flex: 1;
	}

	.dot {
		color: var(--color-ink-muted);
	}
</style>
