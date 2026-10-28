<script lang="ts">
	// The frame every input sits in: a label, a white box with a rule border (ink while focused,
	// minus when invalid), and an optional hint line that carries data, never prose.
	import type { Snippet } from 'svelte';

	let {
		label,
		id,
		invalid = false,
		tall = false,
		hint,
		children
	}: {
		label?: string;
		id?: string;
		invalid?: boolean;
		tall?: boolean;
		hint?: Snippet;
		children: Snippet;
	} = $props();
</script>

<div class="field">
	{#if label}<label for={id}>{label}</label>{/if}
	<div class="box" class:invalid class:tall>
		{@render children()}
	</div>
	{#if hint}<div class="hint">{@render hint()}</div>{/if}
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
</style>
