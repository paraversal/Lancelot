<script lang="ts" generics="V extends string">
	let {
		options,
		value = $bindable(),
		label,
		onchange
	}: {
		options: { value: V; label: string }[];
		value: V;
		/** For screen readers; the visible label is the field's. */
		label?: string;
		onchange?: (value: V) => void;
	} = $props();

	function select(v: V) {
		if (v === value) return;
		value = v;
		onchange?.(v);
	}
</script>

<div class="track" role="radiogroup" aria-label={label}>
	{#each options as o (o.value)}
		<button
			type="button"
			role="radio"
			aria-checked={o.value === value}
			class:active={o.value === value}
			onclick={() => select(o.value)}
		>
			{o.label}
		</button>
	{/each}
</div>

<style>
	.track {
		display: inline-flex;
		gap: 2px;
		padding: 2px;
		border-radius: var(--radius-md);
		background: var(--color-paper-sunk);
	}

	button {
		height: 28px;
		padding: 0 10px;
		border: none;
		border-radius: var(--radius-field);
		background: none;
		font-size: 13px;
		white-space: nowrap;
		cursor: pointer;
	}

	.active {
		background: var(--color-marker);
		font-weight: 600;
	}
</style>
