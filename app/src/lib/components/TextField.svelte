<script lang="ts">
	// A text input that commits on blur or Enter, the way every edit in Lancelot is saved. With
	// suggestions, what matches the text opens under it; arrows and Enter or a click pick one.
	import type { Snippet } from 'svelte';
	import Field from './Field.svelte';

	let {
		label,
		value,
		multiline = false,
		required = false,
		autofocus = false,
		suggestions = [],
		hint,
		oninput,
		oncommit
	}: {
		label: string;
		value: string | null;
		multiline?: boolean;
		/** An empty required field goes back to its value instead of committing. */
		required?: boolean;
		autofocus?: boolean;
		/** What people typed here before. Typing something else stays possible. */
		suggestions?: string[];
		hint?: Snippet;
		/** Every keystroke, for callers that react before the commit. */
		oninput?: (text: string) => void;
		oncommit: (value: string | null) => void;
	} = $props();

	const id = $props.id();
	let text = $state('');
	// Props passed as expressions re-run effects whenever the parent's snapshot changes, so reset
	// the text only when the value itself changed; otherwise typing would be wiped by saves
	// elsewhere.
	let synced: string | null | undefined;
	$effect.pre(() => {
		if (value === synced) return;
		synced = value;
		text = value ?? '';
	});

	let open = $state(false);
	let active = $state(-1);
	const matches = $derived.by(() => {
		const typed = text.trim().toLowerCase();
		const found = suggestions.filter((s) => s.toLowerCase().includes(typed));
		return found.length === 1 && found[0].toLowerCase() === typed ? [] : found.slice(0, 8);
	});
	const showing = $derived(open && matches.length > 0);

	function pick(suggestion: string) {
		text = suggestion;
		oninput?.(text);
		open = false;
		active = -1;
		commit();
	}

	function commit() {
		const next = text.trim();
		if (required && !next) {
			text = value ?? '';
			return;
		}
		if (next !== (value ?? '')) oncommit(next || null);
	}

	function keydown(e: KeyboardEvent) {
		if (showing && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
			e.preventDefault();
			const step = e.key === 'ArrowDown' ? 1 : -1;
			active = (active + step + matches.length + (active < 0 && step < 0 ? 1 : 0)) % matches.length;
			return;
		}
		if (showing && e.key === 'Enter' && active >= 0) {
			e.preventDefault();
			pick(matches[active]);
			return;
		}
		if (showing && e.key === 'Escape') {
			// Closing the list is all this Esc does.
			e.preventDefault();
			open = false;
			active = -1;
			return;
		}
		if (e.key === 'Enter' && (!multiline || e.metaKey)) {
			e.preventDefault();
			(e.currentTarget as HTMLElement).blur();
		}
		if (e.key === 'Escape') {
			// Undoing what was typed is all this Esc does; the next one reaches the panel.
			if (text.trim() !== (value ?? '').trim()) e.preventDefault();
			text = value ?? '';
			(e.currentTarget as HTMLElement).blur();
		}
	}
</script>

<Field {label} {id} {hint} tall={multiline}>
	{#if multiline}
		<textarea
			{id}
			bind:value={text}
			oninput={() => oninput?.(text)}
			onblur={commit}
			onkeydown={keydown}></textarea>
	{:else}
		<!-- svelte-ignore a11y_autofocus -->
		<input
			{id}
			type="text"
			bind:value={text}
			role={suggestions.length ? 'combobox' : undefined}
			aria-expanded={suggestions.length ? showing : undefined}
			aria-controls={suggestions.length ? `${id}-suggestions` : undefined}
			aria-autocomplete={suggestions.length ? 'list' : undefined}
			autocomplete={suggestions.length ? 'off' : undefined}
			oninput={() => {
				open = true;
				active = -1;
				oninput?.(text);
			}}
			onfocus={() => (open = true)}
			onblur={() => {
				open = false;
				active = -1;
				commit();
			}}
			onkeydown={keydown}
			{autofocus}
		/>
	{/if}
	{#if showing}
		<ul class="suggestions" role="listbox" id="{id}-suggestions">
			{#each matches as match, i (match)}
				<!-- Pressing keeps the focus in the input, so leaving it doesn't commit the half-typed text. -->
				<li
					role="option"
					aria-selected={i === active}
					class:active={i === active}
					onmousedown={(e) => {
						e.preventDefault();
						pick(match);
					}}
				>
					{match}
				</li>
			{/each}
		</ul>
	{/if}
</Field>

<style>
	/* Inside the input's box, so the list opens right under the input rather than under the hint. */
	.suggestions {
		position: absolute;
		z-index: 10;
		top: 100%;
		left: -1px;
		right: -1px;
		margin: 2px 0 0;
		padding: 4px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-field);
		box-shadow: 0 4px 12px rgb(0 0 0 / 12%);
		list-style: none;
	}

	li {
		padding: 5px 8px;
		border-radius: var(--radius-field);
		cursor: pointer;
	}

	li:hover,
	li.active {
		background: var(--color-rule);
	}
</style>
