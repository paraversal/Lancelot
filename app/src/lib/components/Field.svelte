<script lang="ts">
	// The frame every input sits in: a label, a white box with a rule border (ink while focused,
	// minus when invalid), and an optional hint line that carries data, never prose.
	//
	// An `error` says why the input was not accepted: in a tooltip under the box while the field
	// has focus (so typing does not shift the layout), and in the hint line once it is left.
	import type { Snippet } from 'svelte';

	let {
		label,
		id,
		invalid = false,
		error = null,
		tall = false,
		hint,
		children
	}: {
		label?: string;
		id?: string;
		invalid?: boolean;
		error?: string | null;
		tall?: boolean;
		hint?: Snippet;
		children: Snippet;
	} = $props();

	const message = $derived(error ? error.charAt(0).toUpperCase() + error.slice(1) : null);
</script>

<div class="field">
	{#if label}<label for={id}>{label}</label>{/if}
	<div class="box" class:invalid={invalid || !!error} class:tall>
		{@render children()}
		{#if message}<span class="tip" role="tooltip" id={id ? `${id}-error` : undefined}
				>{message}</span
			>{/if}
	</div>
	{#if message}
		<div class="hint error">{message}</div>
	{:else if hint}
		<div class="hint">{@render hint()}</div>
	{/if}
</div>

<style>
	.field {
		display: flex;
		flex-direction: column;
		gap: 6px;
		min-width: 0;
	}

	label {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.box {
		position: relative;
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		height: 36px;
		padding: 0 10px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-field);
	}

	.box.tall {
		height: auto;
		min-height: 72px;
		align-items: stretch;
		padding: 9px 10px;
	}

	.box:focus-within {
		border-color: var(--color-ink);
	}

	.box.invalid {
		border-color: var(--color-minus);
	}

	.box :global(input),
	.box :global(textarea),
	.box :global(select) {
		flex: 1;
		min-width: 0;
		border: none;
		outline: none;
		background: none;
		padding: 0;
		user-select: text;
		-webkit-user-select: text;
	}

	.box :global(textarea) {
		resize: none;
	}

	.hint {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.hint.error {
		color: var(--color-minus);
	}

	.tip {
		position: absolute;
		top: calc(100% + 6px);
		left: 0;
		z-index: 10;
		max-width: 280px;
		padding: 4px 8px;
		border-radius: var(--radius-sm);
		background: var(--color-ink);
		color: var(--color-paper);
		font-size: var(--type-xs);
		line-height: 1.4;
		pointer-events: none;
		display: none;
	}

	/* While typing: the tooltip. After leaving the field: the hint line. */
	.field:focus-within .tip {
		display: block;
	}

	.field:focus-within .hint.error {
		visibility: hidden;
	}
</style>
