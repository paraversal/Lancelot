<script lang="ts">
	// Closing the event (R-SET-7): every open item is either fixed or accepted with a note.
	// After closing the event is read-only until it is reopened in Settings (R-EVT-2).
	import { untrack } from 'svelte';
	import { api, errorMessage } from '#lib/api/index.ts';
	import Button from '#lib/components/Button.svelte';
	import Field from '#lib/components/Field.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import { formatAmount } from '#lib/format.ts';
	import { centreOf, confetti } from '#lib/motion/confetti.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import { describe } from '#lib/openItems.ts';
	import { session } from '#lib/state/session.svelte.ts';

	let { onclose }: { onclose: () => void } = $props();

	const snapshot = $derived(session.event!.snapshot);
	const format = $derived(session.format);
	const items = $derived(snapshot.openItems);
	const id = $props.id();

	// Notes by item, so a note survives the list changing underneath (e.g. a fix elsewhere).
	let notes = $state<Record<string, string>>({});
	let error = $state<string | null>(null);
	const key = (i: unknown) => JSON.stringify(i);
	// After reopening, what was accepted before stands while the item is unchanged.
	untrack(() => {
		for (const o of snapshot.event.closingOverrides) notes[key(o.item)] = o.note;
	});
	const clean = JSON.stringify(notes);
	const missing = $derived(items.filter((i) => !notes[key(i)]?.trim()).length);

	let confirmButton = $state<HTMLElement>();

	async function confirm() {
		if (missing) return;
		const origin = confirmButton ? centreOf(confirmButton) : null;
		try {
			error = null;
			const overrides = items.map((item) => ({
				item: $state.snapshot(item),
				note: notes[key(item)].trim()
			}));
			await session.change(() => api.closeEvent(overrides));
			// E1: the big strength throws confetti from the button that was pressed.
			if (origin && level('E1') === 'big') confetti(origin);
			onclose();
		} catch (e) {
			error = errorMessage(e);
		}
	}
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			confirm();
		}
	}}
/>

<Panel title="Close event" {onclose} dirty={JSON.stringify(notes) !== clean}>
	{#each items as item, n (key(item))}
		{@const d = describe(item, snapshot, format)}
		<div class="item">
			<label class="line" for="{id}-{n}">
				<span>{d.text}</span>
				{#if d.amount !== null}<strong>{formatAmount(d.amount, format)}</strong>{/if}
			</label>
			<Field id="{id}-{n}">
				<input
					id="{id}-{n}"
					type="text"
					placeholder="Why it is fine"
					value={notes[key(item)] ?? ''}
					oninput={(e) => (notes[key(item)] = e.currentTarget.value)}
				/>
			</Field>
		</div>
	{:else}
		<p class="done"><Icon name="check" size={16} />Nothing open</p>
	{/each}
	<span class="grow"></span>
	{#if missing}
		<span class="attention"
			><Icon name="warn" size={14} />{missing === 1
				? '1 item without a note'
				: `${missing} items without a note`}</span
		>
	{/if}
	{#if error}<p class="form-error">{error}</p>{/if}
	{#snippet footer()}
		<span bind:this={confirmButton}>
			<Button kind="primary" icon="lock" shortcut="⌘↵" disabled={!!missing} onclick={confirm}
				>Close event</Button
			>
		</span>
		<Button shortcut="esc" onclick={onclose}>Cancel</Button>
	{/snippet}
</Panel>

<style>
	.item {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
	}

	.line {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-3);
	}

	strong {
		font-weight: 600;
	}

	.done {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		margin: 0;
		color: var(--color-plus);
	}

	.attention {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--color-attention);
	}
</style>
