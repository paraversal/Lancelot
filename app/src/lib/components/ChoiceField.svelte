<script lang="ts">
	// A field for choosing one of the event's accounts by typing. On focus the list opens; what is
	// typed narrows it; arrows and Enter or a click choose one. What is typed is only ever a
	// search: it becomes a value when one is chosen, or, when it is not already a name, through
	// the last row that offers to add it (`New person “Nora”`). So a letter press searches first,
	// and adding is the fallback rather than what a letter press turns into. Used for the payer,
	// a split line's category and the accounts of a transfer or a settle.
	import type { AccountId } from '../api/generated/AccountId';
	import Field from './Field.svelte';
	import Icon from './Icon.svelte';

	type Option = { value: AccountId; label: string; group?: string };
	type Row =
		| { kind: 'group'; label: string }
		| { kind: 'option'; option: Option; index: number }
		| { kind: 'create'; name: string; index: number };

	let {
		label,
		ariaLabel,
		value,
		options,
		addLabel,
		invalid = false,
		error = null,
		hint,
		onchange,
		onadd
	}: {
		label?: string;
		/** The name for a field without a label of its own, such as a split line's category. */
		ariaLabel?: string;
		value: AccountId | null;
		options: { value: AccountId; label: string; group?: string }[];
		/** What the adding row offers, e.g. `New person…`; the ellipsis makes way for the typed name. */
		addLabel: string;
		invalid?: boolean;
		/** Why the choice was not accepted, e.g. from the core when saving. */
		error?: string | null;
		hint?: import('svelte').Snippet;
		onchange: (value: AccountId) => void;
		/** Creates the entry and returns its id, or null when it could not be added. */
		onadd: (name: string) => Promise<AccountId | null>;
	} = $props();

	const id = $props.id();
	let el = $state<HTMLInputElement | null>(null);
	let panel = $state<HTMLElement | null>(null);
	let text = $state('');
	let open = $state(false);
	/** Which row of the list is under the keys; -1 while the chosen one is not in it. */
	let active = $state(-1);
	let focused = $state(false);
	/** Whether the field holds a search rather than the chosen name. */
	let typed = $state(false);
	let adding = $state(false);

	const chosen = $derived(options.find((o) => o.value === value));
	const shown = $derived(chosen?.label ?? '');

	// The chosen name is what the field shows whenever it is not being typed in.
	$effect.pre(() => {
		const name = shown;
		if (!focused) text = name;
	});

	const groups = $derived([...new Set(options.map((o) => o.group ?? ''))]);
	const matches = $derived.by(() => {
		// Each group is shown under its name once, so its accounts stay together even when the
		// options arrive interleaved.
		const inGroup = (a: Option, b: Option) =>
			groups.indexOf(a.group ?? '') - groups.indexOf(b.group ?? '');
		const q = typed ? text.trim().toLowerCase() : '';
		if (!q) return [...options].sort(inGroup);
		// Names that start with what is typed come first, then names with such a word in them,
		// then the rest. A group goes where its best match goes, so typing a person's first
		// letter puts people above accounts that merely contain it, and each group stays
		// together under its own name.
		const rank = (label: string) => {
			const lower = label.toLowerCase();
			if (lower.startsWith(q)) return 0;
			if (lower.split(/[\s\-–—/]+/).some((word) => word.startsWith(q))) return 1;
			return lower.includes(q) ? 2 : -1;
		};
		const found = options
			.map((option, order) => ({
				option,
				order,
				group: option.group ?? '',
				rank: rank(option.label)
			}))
			.filter((match) => match.rank >= 0);
		const best = new Map<string, number>();
		for (const match of found)
			best.set(match.group, Math.min(best.get(match.group) ?? 2, match.rank));
		return found
			.sort(
				(a, b) =>
					(best.get(a.group) ?? 2) - (best.get(b.group) ?? 2) ||
					groups.indexOf(a.group) - groups.indexOf(b.group) ||
					a.rank - b.rank ||
					a.order - b.order
			)
			.map((match) => match.option);
	});

	// What can be added: the typed name, unless it is already one of the names on offer. It stays
	// below the matches, so Enter on a match never adds a second one.
	const addName = $derived.by(() => {
		const name = typed ? text.trim() : '';
		if (!name) return null;
		const lower = name.toLowerCase();
		return options.some((o) => o.label.trim().toLowerCase() === lower) ? null : name;
	});
	const addPrefix = $derived(addLabel.replace(/…\s*$/, '').trim());

	const entries = $derived.by(() => {
		const rows: Row[] = [];
		let group: string | null = null;
		let index = 0;
		for (const option of matches) {
			const name = option.group ?? '';
			if (name && name !== group) rows.push({ kind: 'group', label: name });
			group = name;
			rows.push({ kind: 'option', option, index: index++ });
		}
		if (addName !== null) rows.push({ kind: 'create', name: addName, index: index++ });
		return { rows, count: index };
	});
	const showing = $derived(open && entries.count > 0);

	const indexOf = (account: AccountId | null) => {
		for (const row of entries.rows)
			if (row.kind === 'option' && row.option.value === account) return row.index;
		return -1;
	};

	const rowAt = (index: number) => {
		for (const row of entries.rows) if (row.kind !== 'group' && row.index === index) return row;
		return undefined;
	};

	function focus() {
		if (focused) return;
		focused = true;
		open = true;
		typed = false;
		text = shown;
		active = indexOf(value);
		// Selecting what is there lets a letter replace it, so typing searches instead of
		// appending to the name that is already chosen.
		el?.select();
	}

	function blur() {
		focused = false;
		restore();
	}

	function search() {
		typed = true;
		open = true;
		active = entries.count ? 0 : -1;
	}

	function close() {
		open = false;
		typed = false;
		active = -1;
	}

	/** Drops a search and puts the chosen name back in the field. */
	function restore() {
		text = shown;
		close();
	}

	function pick(row: Row) {
		if (row.kind !== 'option') return;
		close();
		if (row.option.value !== value) onchange(row.option.value);
		text = row.option.label;
	}

	async function create(name: string) {
		if (adding) return;
		adding = true;
		try {
			const created = await onadd(name);
			if (created === null) return;
			close();
			onchange(created);
			text = name;
		} finally {
			adding = false;
		}
	}

	function openList() {
		open = true;
		typed = false;
		text = shown;
		active = indexOf(value);
	}

	// A click starts a search: the chosen name is selected, so a letter replaces it. The caret
	// the browser would place is prevented, or it would collapse that selection. A click on a
	// field that already has the keys opens the list again, e.g. after a choice closed it.
	function grab(e: MouseEvent) {
		if (document.activeElement === el) {
			if (!open) {
				e.preventDefault();
				openList();
				el?.select();
			}
			return;
		}
		e.preventDefault();
		el?.focus(); // which puts the chosen name in and selects it
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Tab') {
			if (!showing) return;
			// Leaving the field with a name typed in full chooses it. A partial search is only a
			// search, so it is dropped, as any other picker's would be.
			const exact = typed
				? entries.rows.find(
						(row) =>
							row.kind === 'option' &&
							row.option.label.trim().toLowerCase() === text.trim().toLowerCase()
					)
				: undefined;
			if (exact) pick(exact);
			else restore();
			return;
		}
		if (e.key === 'Escape') {
			if (!showing) return; // the panel sees this one
			e.preventDefault();
			restore();
			return;
		}
		if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			if (!open) openList();
			if (!entries.count) return;
			e.preventDefault();
			const step = e.key === 'ArrowDown' ? 1 : -1;
			active =
				active < 0
					? step > 0
						? 0
						: entries.count - 1
					: (active + step + entries.count) % entries.count;
			return;
		}
		// ⌘↵ and ⇧⌘↵ belong to the panel's Save; only a plain Enter picks or leaves.
		if (e.key === 'Enter' && !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey) {
			if (!open) {
				e.preventDefault();
				el?.blur();
				return;
			}
			e.preventDefault();
			const row = rowAt(active);
			if (row?.kind === 'create') void create(row.name);
			else if (row) pick(row);
			else restore();
		}
	}

	// Keep the row under the keys in view in a long list.
	$effect(() => {
		if (!showing || active < 0) return;
		panel?.querySelector(`[data-row="${active}"]`)?.scrollIntoView?.({ block: 'nearest' });
	});
</script>

<Field {label} {id} {invalid} {error} {hint}>
	<input
		{id}
		bind:this={el}
		type="text"
		bind:value={text}
		aria-label={label ? undefined : ariaLabel}
		role="combobox"
		aria-expanded={showing}
		aria-controls={showing ? `${id}-list` : undefined}
		aria-autocomplete="list"
		aria-activedescendant={showing && active >= 0 ? `${id}-row-${active}` : undefined}
		aria-busy={adding}
		autocomplete="off"
		spellcheck="false"
		onfocus={focus}
		onblur={blur}
		oninput={search}
		onkeydown={keydown}
		onmousedown={grab}
	/>
	{#if showing}
		<ul class="list" id="{id}-list" role="listbox" bind:this={panel}>
			{#each entries.rows as row (row.kind === 'group' ? `g-${row.label}` : row.kind === 'create' ? 'create' : `o-${row.option.value}`)}
				{#if row.kind === 'group'}
					<li class="group" role="presentation">{row.label}</li>
				{:else if row.kind === 'create'}
					<li
						role="option"
						id="{id}-row-{row.index}"
						data-row={row.index}
						aria-selected={active === row.index}
						class="create"
						class:active={active === row.index}
						onmousedown={(e) => {
							e.preventDefault();
							void create(row.name);
						}}
					>
						<Icon name="plus" size={14} />{addPrefix} “{row.name}”
					</li>
				{:else}
					<li
						role="option"
						id="{id}-row-{row.index}"
						data-row={row.index}
						aria-selected={active === row.index}
						class:active={active === row.index}
						onmousedown={(e) => {
							e.preventDefault();
							pick(row);
						}}
					>
						{row.option.label}
					</li>
				{/if}
			{/each}
		</ul>
	{/if}
	<span class="chevron"><Icon name="down" size={14} /></span>
</Field>

<style>
	input {
		cursor: text;
	}

	.chevron {
		display: flex;
		flex: none;
		color: var(--color-ink-muted);
		pointer-events: none;
	}

	/* Inside the input's box, so the list opens right under the field rather than under the hint. */
	.list {
		position: absolute;
		z-index: 10;
		top: 100%;
		left: -1px;
		right: -1px;
		max-height: 264px;
		margin: 2px 0 0;
		padding: 4px;
		overflow-y: auto;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-field);
		box-shadow: 0 4px 12px rgb(0 0 0 / 12%);
		list-style: none;
	}

	li {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 5px 8px;
		border-radius: var(--radius-field);
		cursor: pointer;
	}

	li:hover,
	li.active {
		background: var(--color-rule);
	}

	.group {
		padding: 6px 8px 2px;
		font-size: var(--type-xs);
		font-weight: 600;
		color: var(--color-ink-muted);
		cursor: default;
	}

	.group:hover {
		background: none;
	}

	/* The fallback is set apart from the names it follows, so it never reads as one of them. */
	.create {
		margin-top: 4px;
		border-top: 1px solid var(--color-rule);
		border-radius: 0 0 var(--radius-field) var(--radius-field);
		color: var(--color-ink-muted);
	}

	.create:first-child {
		margin-top: 0;
		border-top: none;
		border-radius: var(--radius-field);
	}
</style>
