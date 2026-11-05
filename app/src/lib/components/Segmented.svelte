<script lang="ts" generics="V extends string">
	// A choice of a few options. The marker pill sits under the active one and travels to the
	// next one when the choice changes (A4).
	import { tick } from 'svelte';
	import type { Strength } from '../motion/choices';
	import { pillTravel } from '../motion/effects';
	import { level } from '../motion/mode.svelte';

	let {
		options,
		value = $bindable(),
		label,
		onchange,
		strength,
		variant = 'segmented'
	}: {
		options: { value: V; label: string }[];
		value: V;
		/** For screen readers; the visible label is the field's. */
		label?: string;
		onchange?: (value: V) => void;
		/** Overrides how the pill moves (A4), for the trial page. */
		strength?: Strength;
		/**
		 * `tabs` fills its row and the chosen one joins the fields below it, for a box whose
		 * contents change with the choice (the budget line's estimate).
		 */
		variant?: 'segmented' | 'tabs';
	} = $props();

	let track: HTMLDivElement;
	let pill: HTMLSpanElement;
	let box = $state<{ left: number; width: number } | null>(null);

	function measure() {
		const active = track?.querySelector<HTMLElement>('[aria-checked="true"]');
		box = active ? { left: active.offsetLeft, width: active.offsetWidth } : null;
	}

	// Follow the value and the labels (their widths), however they change.
	$effect(() => {
		void value;
		void options.map((o) => o.label).join();
		const from = pill?.getBoundingClientRect();
		measure();
		tick().then(() => {
			if (from?.width && pill)
				pillTravel(pill, from, pill.getBoundingClientRect(), level('A4', strength));
		});
	});

	function select(v: V) {
		if (v === value) return;
		value = v;
		onchange?.(v);
	}
</script>

<svelte:window onresize={measure} />

<div
	class="track"
	class:tabs={variant === 'tabs'}
	role="radiogroup"
	aria-label={label}
	bind:this={track}
>
	<span
		class="pill"
		bind:this={pill}
		style:left="{box?.left ?? 0}px"
		style:width="{box?.width ?? 0}px"
		style:opacity={box ? 1 : 0}
	></span>
	{#each options as o (o.value)}
		<button
			type="button"
			role="radio"
			aria-checked={o.value === value}
			class:active={o.value === value}
			title={o.label}
			onclick={() => select(o.value)}
		>
			{o.label}
		</button>
	{/each}
</div>

<style>
	.track {
		position: relative;
		display: inline-flex;
		gap: 2px;
		padding: 2px;
		border-radius: var(--radius-md);
		background: var(--color-paper-sunk);
	}

	.pill {
		position: absolute;
		top: 2px;
		bottom: 2px;
		border-radius: var(--radius-field);
		background: var(--color-marker);
		transform-origin: left center;
	}

	button {
		position: relative;
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
		font-weight: 600;
	}

	button:disabled {
		cursor: default;
	}

	/* A tab bar: the whole row, the chosen one a block that the fields below continue from. */
	.track.tabs {
		display: flex;
		width: 100%;
		gap: 0;
		padding: 0;
		border-radius: 0;
		background: none;
	}

	.tabs .pill {
		top: 0;
		bottom: 0;
		border-radius: var(--radius-field) var(--radius-field) 0 0;
	}

	.tabs button {
		flex: 1 1 0;
		/* Several tabs share the row, so a long label gives way rather than pushing the rest out. */
		min-width: 0;
		height: 36px;
		padding: 0 8px;
		overflow: hidden;
		border-radius: 0;
		color: var(--color-ink-muted);
		text-overflow: ellipsis;
	}

	/* The ring stays inside the tab, which its box clips. */
	.tabs button:focus-visible {
		outline-offset: -4px;
	}

	.tabs button:hover {
		background: var(--color-paper-sunk);
		color: var(--color-ink);
	}

	.tabs .active,
	.tabs .active:hover {
		background: none;
		color: var(--color-ink);
	}
</style>
