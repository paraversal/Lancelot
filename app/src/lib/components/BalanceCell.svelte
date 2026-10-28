<script lang="ts">
	// A person's net balance: `owed 212,40 €`, `owes 180,00 €` (attention) or `✓ settled`.
	import type { Cents } from '../api/generated/Cents';
	import type { NumberFormat } from '../api/generated/NumberFormat';
	import { stampIn } from '../motion/effects';
	import { level } from '../motion/mode.svelte';
	import MovingAmount from '../motion/MovingAmount.svelte';
	import Icon from './Icon.svelte';

	let { net, format }: { net: Cents; format: NumberFormat } = $props();
</script>

<span class="balance">
	{#if net > 0}
		<span class="word">owed</span><strong
			><MovingAmount value={Math.abs(net)} {format} level={level('A5')} /></strong
		>
	{:else if net < 0}
		<span class="word">owes</span><strong class="owes"
			><MovingAmount value={Math.abs(net)} {format} level={level('A5')} /></strong
		>
	{:else}
		<span class="stamp" in:stampIn={{ level: level('C1') }}>
			<span class="settled"><Icon name="check" size={14} /></span><span class="word">settled</span>
		</span>
	{/if}
</span>

<style>
	.balance {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		white-space: nowrap;
	}

	.word {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	strong {
		font-weight: 600;
	}

	.owes {
		color: var(--color-attention);
	}

	.stamp {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}

	.settled {
		display: flex;
		color: var(--color-plus);
	}
</style>
