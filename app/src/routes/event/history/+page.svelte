<script lang="ts">
	// The change log (R-TX-6): what changed in this file, newest first, one block per command.
	import { api, errorMessage } from '#lib/api/index.ts';
	import type { ChangeLogEntry } from '#lib/api/generated/ChangeLogEntry.ts';
	import Icon from '#lib/components/Icon.svelte';
	import { formatDate } from '#lib/format.ts';
	import {
		historyChanges,
		localDay,
		localTime,
		matchingRows,
		type HistoryRow
	} from '#lib/history.ts';
	import { plugins } from '#lib/plugins/registry.ts';
	import { session } from '#lib/state/session.svelte.ts';

	// A command can write many rows, e.g. an import or a template. The rest wait behind a button.
	const SHOWN = 4;

	const open = $derived(session.event!);
	const format = $derived(session.format);

	let log = $state<ChangeLogEntry[]>([]);
	let error = $state<string | null>(null);
	let query = $state('');
	let expanded = $state<Record<string, boolean>>({});

	$effect(() => {
		void open.revision;
		api
			.changeLog()
			.then((entries) => {
				log = entries;
				error = null;
			})
			.catch((e) => (error = errorMessage(e)));
	});

	const pluginNames = Object.fromEntries(plugins.map((p) => [p.id, p.name]));
	const changes = $derived(historyChanges(log, format, pluginNames));
	const days = $derived.by(() => {
		const out: {
			day: string;
			changes: { at: string; rows: HistoryRow[]; more: number }[];
		}[] = [];
		for (const change of changes) {
			const found = matchingRows(change, query);
			if (!found.length) continue;
			const cut = found.length > SHOWN && !expanded[change.at];
			const rows = cut ? found.slice(0, SHOWN - 1) : found;
			const day = localDay(change.at);
			if (out.at(-1)?.day !== day) out.push({ day, changes: [] });
			out.at(-1)!.changes.push({ at: change.at, rows, more: found.length - rows.length });
		}
		return out;
	});

	function dayLabel(iso: string) {
		const day = new Date(`${iso}T12:00:00`).toLocaleDateString('en-GB', { weekday: 'short' });
		return `${day} ${formatDate(iso, format)}`;
	}

	/** An entry that is still in the ledger opens there. */
	function ledgerLink(row: HistoryRow) {
		const live = open.snapshot.txns.some((t) => t.id === row.entityId);
		return row.entity === 'txn' && row.verb !== 'Deleted' && live
			? `/event/ledger?txn=${row.entityId}`
			: null;
	}
</script>

<div class="history">
	<header>
		<h1>History</h1>
		<label class="search">
			<Icon name="search" />
			<input type="search" placeholder="Search" bind:value={query} />
		</label>
	</header>

	{#if error}<p class="error" role="alert">{error}</p>{/if}

	<div class="days">
		{#each days as { day, changes: ofDay } (day)}
			<h2>{dayLabel(day)}</h2>
			{#each ofDay as change (change.at)}
				<div class="change">
					<time datetime={change.at}>{localTime(change.at)}</time>
					<ul>
						{#each change.rows as row (row.id)}
							{@const href = ledgerLink(row)}
							<li>
								<div class="what">
									<span class="muted">{row.verb} {row.noun.toLowerCase()}</span>
									{#if href}<a {href}>{row.name}</a>{:else if row.name}<strong>{row.name}</strong
										>{/if}
								</div>
								{#if row.fields.length}
									<dl>
										{#each row.fields as f (f.label)}
											<div>
												<dt>{f.label}</dt>
												<dd>
													{#if row.verb === 'Deleted'}
														<span class="muted">{f.before}</span>
													{:else if row.verb === 'Added'}
														{f.after}
													{:else}
														<span class="muted">{f.before ?? '–'}</span>
														<span class="muted" aria-label="to">→</span>
														{f.after ?? '–'}
													{/if}
												</dd>
											</div>
										{/each}
									</dl>
								{/if}
							</li>
						{/each}
						{#if change.more}
							<li>
								<button type="button" onclick={() => (expanded[change.at] = true)}>
									{change.more} more
								</button>
							</li>
						{/if}
					</ul>
				</div>
			{/each}
		{/each}
	</div>
</div>

<style>
	.history {
		display: flex;
		flex-direction: column;
		gap: 20px;
		max-width: 860px;
		padding: var(--spacing-10);
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-3);
		min-height: 40px;
	}

	h1 {
		margin: 0;
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.search {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		width: 200px;
		height: 32px;
		padding: 0 10px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-md);
		color: var(--color-ink-muted);
	}

	.search input {
		flex: 1;
		min-width: 0;
		border: none;
		outline: none;
		background: none;
		user-select: text;
		-webkit-user-select: text;
	}

	.error {
		margin: 0;
		color: var(--color-attention);
	}

	.days {
		display: flex;
		flex-direction: column;
	}

	h2 {
		margin: 0;
		padding: 14px 8px 4px;
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-ink-muted);
	}

	.change {
		display: flex;
		gap: var(--spacing-3);
		padding: 10px 8px;
		border-bottom: 1px solid var(--color-rule);
	}

	time {
		width: 44px;
		flex: none;
		font-size: 13px;
		line-height: 20px;
		color: var(--color-ink-muted);
	}

	ul {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
		flex: 1;
		min-width: 0;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.what {
		display: flex;
		flex-wrap: wrap;
		gap: 0 6px;
		line-height: 20px;
	}

	.what strong,
	.what a {
		font-weight: 600;
	}

	.what a {
		text-decoration: underline;
		text-decoration-color: var(--color-rule);
		text-underline-offset: 3px;
	}

	.what a:hover {
		text-decoration-color: currentColor;
	}

	.muted {
		color: var(--color-ink-muted);
	}

	dl {
		display: flex;
		flex-wrap: wrap;
		gap: 2px var(--spacing-4);
		margin: 2px 0 0;
		font-size: 13px;
	}

	dl div {
		display: flex;
		gap: 6px;
		min-width: 0;
	}

	dt {
		flex: none;
		color: var(--color-ink-muted);
	}

	dt::after {
		content: ':';
	}

	dd {
		margin: 0;
		overflow-wrap: anywhere;
	}

	li button {
		padding: 0;
		border: none;
		background: none;
		font-size: 13px;
		color: var(--color-ink-muted);
		text-decoration: underline;
		text-underline-offset: 3px;
		cursor: pointer;
	}
</style>
