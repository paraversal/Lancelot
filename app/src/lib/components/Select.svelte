<script lang="ts" generics="V extends string | number">
	import Field from './Field.svelte';
	import Icon from './Icon.svelte';

	let {
		label,
		name,
		value,
		options,
		hint,
		onchange
	}: {
		label?: string;
		/** The accessible name when there is no visible label. */
		name?: string;
		value: V;
		options: { value: V; label: string; group?: string; disabled?: boolean }[];
		hint?: import('svelte').Snippet;
		onchange: (value: V) => void;
	} = $props();

	const id = $props.id();
	const groups = $derived(
		[...new Set(options.map((o) => o.group ?? ''))].map((g) => ({
			group: g,
			options: options.filter((o) => (o.group ?? '') === g)
		}))
	);
</script>

<Field {label} {id} {hint}>
	<select
		{id}
		aria-label={name}
		value={String(value)}
		onchange={(e) => {
			const picked = options.find((o) => String(o.value) === e.currentTarget.value);
			if (picked) onchange(picked.value);
		}}
	>
		{#each groups as g (g.group)}
			{#if g.group}
				<optgroup label={g.group}>
					{#each g.options as o (o.value)}<option value={String(o.value)} disabled={o.disabled}
							>{o.label}</option
						>{/each}
				</optgroup>
			{:else}
				{#each g.options as o (o.value)}<option value={String(o.value)} disabled={o.disabled}
						>{o.label}</option
					>{/each}
			{/if}
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
