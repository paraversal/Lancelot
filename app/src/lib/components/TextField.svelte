<script lang="ts">
	// A text input that commits on blur or Enter, the way every edit in Lancelot is saved.
	import Field from './Field.svelte';

	let {
		label,
		value,
		multiline = false,
		required = false,
		autofocus = false,
		oninput,
		oncommit
	}: {
		label: string;
		value: string | null;
		multiline?: boolean;
		/** An empty required field goes back to its value instead of committing. */
		required?: boolean;
		autofocus?: boolean;
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

	function commit() {
		const next = text.trim();
		if (required && !next) {
			text = value ?? '';
			return;
		}
		if (next !== (value ?? '')) oncommit(next || null);
	}

	function keydown(e: KeyboardEvent) {
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

<Field {label} {id} tall={multiline}>
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
			oninput={() => oninput?.(text)}
			onblur={commit}
			onkeydown={keydown}
			{autofocus}
		/>
	{/if}
</Field>
