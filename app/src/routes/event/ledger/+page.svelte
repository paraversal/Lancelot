<script lang="ts">
	// The ledger: every transaction, grouped by day or by category (R-TX-7), with type filters
	// and search. Entering and editing happens in the panel on the right.
	import type { TxnView } from '#lib/api/generated/TxnView.ts';
	import { categoryColor } from '#lib/colors.ts';
	import Button from '#lib/components/Button.svelte';
	import Chip from '#lib/components/Chip.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import ReceiptIndicator from '#lib/components/ReceiptIndicator.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import { formatAmount, formatDate } from '#lib/format.ts';
	import { byCategory, byDay, counts, matches, rows, type Filter, type Item } from '#lib/ledger.ts';
	import { session } from '#lib/state/session.svelte.ts';
	import type { Attachment } from '#lib/api/generated/Attachment.ts';
	import { onFileDrop } from '#lib/dragdrop.ts';
	import EntryPanel from './EntryPanel.svelte';
	import Preview from './Preview.svelte';

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);

	let grouping = $state<'day' | 'category'>('day');
	let filter = $state<Filter>('all');
	let query = $state('');
	let selected = $state<number | null>(null);
	/** `undefined`: closed; `null`: a new entry; otherwise the transaction being edited. */
	let editing = $state<TxnView | null | undefined>(undefined);
	let panelKey = $state(0);
	let pending = $state<string[]>([]);
	let preview = $state<Attachment | null>(null);
	let dragging = $state<string[] | null>(null);

	// Dropping files anywhere attaches them to the open entry, or starts a new expense with
	// them (R-ATT-5).
	$effect(() =>
		onFileDrop({
			over: (names) => (dragging = names),
			leave: () => (dragging = null),
			drop: (paths) => {
				dragging = null;
				if (!paths.length) return;
				if (editing === undefined) {
					open(null);
					pending = paths;
				} else pending = [...pending, ...paths.filter((p) => !pending.includes(p))];
			}
		})
	);

	const all = $derived(rows(snapshot));
	const visible = $derived(all.filter((r) => matches(r, filter, query, format)));
	const groups = $derived(grouping === 'day' ? byDay(visible) : byCategory(visible, snapshot));
	const order = $derived(groups.flatMap((g) => g.items.map((i) => i.row.txn.id)));
	const n = $derived(counts(all));

	const FILTERS: { value: Filter; label: string }[] = [
		{ value: 'all', label: 'All' },
		{ value: 'expense', label: 'Expenses' },
		{ value: 'income', label: 'Income' },
		{ value: 'transfer', label: 'Transfers' },
		{ value: 'missing', label: 'No receipt' }
	];

	function open(txn: TxnView | null) {
		selected = txn?.id ?? null;
		editing = txn;
		pending = [];
		preview = null;
		panelKey++;
	}

	function dayLabel(iso: string) {
		const day = new Date(`${iso}T12:00:00`).toLocaleDateString('en-GB', { weekday: 'short' });
		return `${day} ${formatDate(iso, format)}`;
	}

	function shortDate(iso: string) {
		const [, m, d] = iso.split('-');
		return format === 'de' ? `${d}.${m}` : `${m}-${d}`;
	}

	function amountText(item: Item) {
		const { flow } = item.row;
		if (flow === 'out') return formatAmount(-item.amount, format);
		if (flow === 'in') return formatAmount(item.amount, format, 'always');
		return formatAmount(item.amount, format);
	}

	function keydown(e: KeyboardEvent) {
		const target = e.target instanceof Element ? e.target : null;
		const typing = target?.closest('input, textarea, select, [role="radiogroup"]');
		if (typing || e.metaKey || e.ctrlKey) return;
		if (e.key === 'n' || e.key === 'N') {
			e.preventDefault();
			open(null);
		} else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			if (!order.length) return;
			e.preventDefault();
			const at = selected === null ? -1 : order.indexOf(selected);
			const next = e.key === 'ArrowDown' ? Math.min(order.length - 1, at + 1) : Math.max(0, at - 1);
			selected = order[next];
			document.getElementById(`txn-${selected}`)?.scrollIntoView({ block: 'nearest' });
		} else if (e.key === 'Enter' && selected !== null) {
			e.preventDefault();
			open(snapshot.txns.find((t) => t.id === selected) ?? null);
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<div class="ledger">
	{#if dragging}
		<section class="table">
			<header><h1>Ledger</h1></header>
			<div class="drop">
				<Icon name="upload" size={28} />
				<strong>{editing === undefined ? 'New expense' : 'Attach'}</strong>
				{#if dragging.filter(Boolean).length}<span>{dragging.filter(Boolean).join(', ')}</span>{/if}
			</div>
		</section>
	{:else if preview && editing !== undefined}
		<Preview attachment={preview} onclose={() => (preview = null)} />
	{:else}
		<section class="table">
			<header>
				<h1>Ledger</h1>
				{#if all.length}
					<Segmented
						label="Group by"
						bind:value={grouping}
						options={[
							{ value: 'day', label: 'Day' },
							{ value: 'category', label: 'Category' }
						]}
					/>
				{:else if editing === undefined}
					<Button kind="primary" icon="plus" shortcut="N" onclick={() => open(null)}
						>New entry</Button
					>
				{/if}
			</header>

			{#if all.length}
				<div class="toolbar">
					<label class="search">
						<Icon name="search" />
						<input type="search" placeholder="Search" bind:value={query} />
					</label>
					{#each FILTERS as f (f.value)}
						<Chip
							label={f.label}
							count={n[f.value]}
							active={filter === f.value}
							onclick={() => (filter = f.value)}
						/>
					{/each}
					<span class="grow"></span>
					{#if editing === undefined}
						<Button kind="primary" icon="plus" shortcut="N" onclick={() => open(null)}
							>New entry</Button
						>
					{/if}
				</div>

				<div class="rows" role="table">
					{#each groups as group (group.key)}
						<div class="group" role="rowgroup">
							{#if grouping === 'day'}
								<span>{dayLabel(group.key)}</span>
							{:else}
								{#if group.account}<span
										class="dot"
										style:background={categoryColor(group.account, snapshot.accounts)}
									></span>{/if}
								<span>{group.label}</span>
								<span class="grow"></span>
								{#if group.total !== null && group.account}
									<span class:plus={group.account.kind === 'income'}>
										{formatAmount(
											group.account.kind === 'income' ? group.total : -group.total,
											format,
											'always'
										)}
									</span>
								{/if}
							{/if}
						</div>
						{#each group.items as item (`${group.key}-${item.row.txn.id}`)}
							{@const txn = item.row.txn}
							<button
								type="button"
								id={grouping === 'day' ? `txn-${txn.id}` : undefined}
								class="row"
								class:selected={selected === txn.id}
								role="row"
								onclick={() => open(txn)}
							>
								<span class="date">{shortDate(txn.date)}</span>
								<span class="title">
									{item.row.title}
									{#if item.split}<span class="tag">SPLIT</span>{/if}
								</span>
								{#if grouping === 'day'}
									<span class="categories">
										{#each item.row.categories as c (c.id)}
											<span class="dot" style:background={categoryColor(c, snapshot.accounts)}
											></span>
										{/each}
										<span
											>{item.row.categories.map((c) => c.name).join(', ') ||
												(txn.kind === 'settle' ? 'Settle' : 'Transfer')}</span
										>
									</span>
								{/if}
								<span class="who">{item.row.who}</span>
								<span class="amount {item.row.flow}">{amountText(item)}</span>
								<ReceiptIndicator state={item.row.receipt} />
							</button>
						{/each}
					{/each}
				</div>
			{/if}
		</section>
	{/if}

	{#if editing !== undefined}
		{#key panelKey}
			<EntryPanel
				txn={editing}
				bind:pending
				previewing={preview?.id ?? null}
				onpreview={(a) => (preview = a)}
				onclose={() => ((editing = undefined), (preview = null))}
				onescape={() => (preview ? (preview = null) : (editing = undefined))}
				onsaved={(id) => {
					if (id !== null) {
						selected = id;
						editing = undefined;
					}
				}}
			/>
		{/key}
	{/if}
</div>

<style>
	.ledger {
		display: flex;
		height: 100%;
	}

	.table {
		display: flex;
		flex-direction: column;
		gap: 20px;
		flex: 1;
		min-width: 0;
		padding: var(--spacing-10) var(--spacing-8) var(--spacing-10) var(--spacing-10);
		overflow: auto;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		min-height: 40px;
	}

	h1 {
		margin: 0;
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.toolbar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
	}

	.search {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		width: 150px;
		height: 32px;
		margin-right: 6px;
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

	.grow {
		flex: 1;
	}

	.rows {
		display: flex;
		flex-direction: column;
	}

	.group {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 14px 44px 4px 8px;
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-ink-muted);
	}

	.group .plus {
		color: var(--color-plus);
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		height: 38px;
		padding: 0 8px;
		border: none;
		border-bottom: 1px solid var(--color-rule);
		background: none;
		text-align: left;
		cursor: pointer;
	}

	.row.selected {
		border-radius: var(--radius-sm);
		background: var(--color-marker);
	}

	.date {
		width: 44px;
		flex: none;
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.title {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		flex: 1;
		min-width: 0;
		overflow: hidden;
		white-space: nowrap;
	}

	.tag {
		padding: 0 5px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-sm);
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-ink-muted);
	}

	.categories {
		display: flex;
		align-items: center;
		gap: 2px;
		width: 130px;
		flex: none;
		overflow: hidden;
		font-size: 13px;
		color: var(--color-ink-muted);
		white-space: nowrap;
	}

	.categories > span:last-child {
		margin-left: 4px;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.dot {
		flex: none;
		width: 6px;
		height: 6px;
		border-radius: 50%;
	}

	.who {
		width: 100px;
		flex: none;
		overflow: hidden;
		font-size: 13px;
		color: var(--color-ink-muted);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.amount {
		width: 96px;
		flex: none;
		text-align: right;
	}

	.selected .amount {
		font-weight: 600;
	}

	.amount.in {
		color: var(--color-plus);
	}

	.amount.moved {
		color: var(--color-ink-muted);
	}

	.drop {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 10px;
		flex: 1;
		border: 1px solid var(--color-ink);
		border-radius: 12px;
	}

	.drop strong {
		font-size: var(--type-md);
		font-weight: 600;
	}

	.drop span {
		font-size: 13px;
		color: var(--color-ink-muted);
	}
</style>
