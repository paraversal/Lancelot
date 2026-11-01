<script lang="ts">
	// Which denominations a count offers. A template copies the choice.
	import { formatAmount } from '../../format';
	import type { SettingsSectionProps } from '../api';
	import { EURO, hiddenOf, setHidden } from './records';

	let { ctx }: SettingsSectionProps = $props();

	const hidden = $derived(hiddenOf(ctx.records));
	let error = $state<string | null>(null);

	async function toggle(value: number, on: boolean) {
		error = null;
		try {
			await ctx.apply([setHidden(on ? hidden.filter((v) => v !== value) : [...hidden, value])]);
		} catch (e) {
			if (!(e instanceof Error)) throw e;
			error = e.message;
		}
	}
</script>

<fieldset disabled={ctx.closed}>
	<span class="note">Denominations</span>
	<ul>
		{#each EURO as value (value)}
			<li>
				<label>
					<input
						type="checkbox"
						checked={!hidden.includes(value)}
						onchange={(e) => toggle(value, e.currentTarget.checked)}
					/>
					{formatAmount(value, ctx.format)}
				</label>
			</li>
		{/each}
	</ul>
	{#if error}<p class="form-error" role="alert">{error}</p>{/if}
</fieldset>

<style>
	fieldset {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
		min-width: 0;
		margin: 0;
		padding: 0;
		border: none;
	}

	.note {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	ul {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
		gap: var(--spacing-1) var(--spacing-3);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	label {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2);
		font-variant-numeric: tabular-nums;
		cursor: pointer;
	}

	input {
		accent-color: var(--color-ink);
	}
</style>
