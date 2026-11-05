<script lang="ts">
	// A figure the steppers change, which also opens in place to be typed. An amount is read by
	// the app, in the event's number format; a count is whole digits. What can't be read, or is
	// out of reach, leaves the figure as it was.
	import { tick } from 'svelte';
	import type { NumberFormat } from '../../api/generated/NumberFormat';
	import { formatPlain } from '../../format';

	let {
		value,
		text,
		label,
		format,
		parse,
		min = 0,
		max = Number.MAX_SAFE_INTEGER,
		wide = false,
		moved = false,
		disabled = false,
		testid,
		onchange
	}: {
		value: number;
		/** What it shows shut: `260`, `+3`, `4,50 €`. */
		text: string;
		/** Names the figure, and its input while it is open: `Guests 260`. */
		label: string;
		/** The format an amount is typed in. */
		format?: NumberFormat;
		/** Reads an amount, in cents. Without it the figure is a whole number. */
		parse?: (text: string) => Promise<number>;
		min?: number;
		max?: number;
		/** Room for an amount. */
		wide?: boolean;
		moved?: boolean;
		disabled?: boolean;
		testid?: string;
		onchange: (value: number) => void;
	} = $props();

	let open = $state(false);
	let draft = $state('');
	let reading = $state(false);
	let input = $state<HTMLInputElement | null>(null);

	async function start() {
		draft = parse && format ? formatPlain(value, format) : String(value);
		open = true;
		await tick();
		input?.focus();
		input?.select();
	}

	/** Shuts without a change. Escape, and a blur after a commit, land here too. */
	function stop() {
		open = false;
		draft = '';
	}

	async function commit() {
		if (!open || reading) return;
		const typed = draft.trim();
		if (!typed) return stop();
		let next: number | null = null;
		if (parse) {
			reading = true;
			try {
				next = await parse(typed);
			} catch {
				next = null;
			} finally {
				reading = false;
			}
		} else if (/^\d+$/.test(typed)) {
			next = Number(typed);
		}
		if (next === null || !Number.isFinite(next)) return stop();
		// Escape, or a blur, may have closed it while the amount was being read.
		if (!open) return;
		stop();
		const clamped = Math.min(max, Math.max(min, next));
		if (clamped !== value) onchange(clamped);
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			commit();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			stop();
		}
	}
</script>

{#if open}
	<input
		bind:this={input}
		bind:value={draft}
		class="edit"
		class:wide
		aria-label={label}
		data-testid={testid}
		inputmode={parse ? 'decimal' : 'numeric'}
		onkeydown={keydown}
		onblur={commit}
	/>
{:else}
	<button
		type="button"
		class="value"
		class:wide
		class:moved
		aria-label={label}
		{disabled}
		data-testid={testid}
		onclick={start}>{text}</button
	>
{/if}

<style>
	.value {
		min-width: 44px;
		height: 28px;
		padding: 0 4px;
		border: 1px solid transparent;
		border-radius: var(--radius-sm);
		background: none;
		font: inherit;
		color: inherit;
		text-align: center;
		font-variant-numeric: tabular-nums;
		line-height: 1;
		cursor: text;
	}

	.value:hover:not(:disabled) {
		border-color: var(--color-rule);
		background: var(--color-field);
	}

	.value:disabled {
		cursor: default;
	}

	.value.wide {
		min-width: 72px;
	}

	.moved {
		font-weight: 600;
	}

	.edit {
		width: 52px;
		height: 28px;
		padding: 0 4px;
		border: 1px solid var(--color-ink);
		border-radius: var(--radius-sm);
		background: var(--color-field);
		font: inherit;
		color: var(--color-ink);
		text-align: right;
		font-variant-numeric: tabular-nums;
	}

	.edit.wide {
		width: 84px;
	}
</style>
