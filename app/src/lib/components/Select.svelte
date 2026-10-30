<script lang="ts" generics="V extends string | number">
	import Field from './Field.svelte';
	import Icon from './Icon.svelte';

	let {
		label,
		name,
		value,
		options,
		onchange
	}: {
		label?: string;
		/** The accessible name when there is no visible label. */
		name?: string;
		value: V;
		options: { value: V; label: string }[];
		onchange: (value: V) => void;
	} = $props();

	const id = $props.id();
</script>

<Field {label} {id}>
	<select
		{id}
		aria-label={name}
		value={String(value)}
		onchange={(e) => {
			const picked = options.find((o) => String(o.value) === e.currentTarget.value);
			if (picked) onchange(picked.value);
		}}
	>
		{#each options as o (o.value)}
			<option value={String(o.value)}>{o.label}</option>
		{/each}
	</select>
	<span class="chevron"><Icon name="down" size={14} /></span>
</Field>

<style>
	select {
		appearance: none;
		cursor: pointer;
	}

	.chevron {
		display: flex;
		color: var(--color-ink-muted);
		pointer-events: none;
	}
</style>
