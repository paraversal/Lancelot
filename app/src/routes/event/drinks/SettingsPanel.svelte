<script lang="ts">
	// The commission supplier, whose expenses are checked against the records (R-DRK-5), and the
	// one price per Strich (R-DRK-6).
	import { untrack } from 'svelte';
	import { api, ApiError } from '#lib/api/index.ts';
	import type { Cents } from '#lib/api/generated/Cents.ts';
	import AmountField from '#lib/components/AmountField.svelte';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { session } from '#lib/state/session.svelte.ts';

	let { onclose }: { onclose: () => void } = $props();

	const format = $derived(session.format);
	const settings = $derived(session.event!.snapshot.event.drinks);

	let supplier = $state<string | null>(null);
	let pricePerStrich = $state<Cents | null>(null);
	untrack(() => {
		supplier = settings.supplier;
		pricePerStrich = settings.pricePerStrich;
	});
	let error = $state<string | null>(null);

	async function save() {
		error = null;
		try {
			await session.change(() => api.setDrinkSettings({ supplier, pricePerStrich }));
			onclose();
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = e.message;
		}
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !session.closed) {
			e.preventDefault();
			save();
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<Panel title="Drinks" {onclose}>
	<fieldset disabled={session.closed}>
		<TextField
			label="Supplier"
			value={supplier}
			autofocus
			oninput={(v) => (supplier = v.trim() || null)}
			oncommit={(v) => (supplier = v)}
		/>
		<AmountField
			label="Price per Strich"
			value={pricePerStrich}
			{format}
			oninput={(v) => (pricePerStrich = v)}
		/>
	</fieldset>

	{#if error}<p class="form-error">{error}</p>{/if}

	{#snippet footer()}
		{#if session.closed}
			<span class="closed"><Icon name="lock" size={14} />Closed</span>
		{:else}
			<Button kind="primary" shortcut="⌘↵" onclick={save}>Save</Button>
		{/if}
	{/snippet}
</Panel>

<style>
	.closed {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		font-weight: 600;
		color: var(--color-ink-muted);
	}
</style>
