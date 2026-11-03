<script lang="ts">
	// Which cost centres have stock and sales. Switching one off keeps its products, and products
	// whose cost centre was deleted can move to another: the core doesn't know about them.
	import Button from '../../components/Button.svelte';
	import Select from '../../components/Select.svelte';
	import TextField from '../../components/TextField.svelte';
	import type { SettingsSectionProps } from '../api';
	import { allProducts, centreSettings, NEW_CENTRE, setCentre, setProduct } from './records';

	let { ctx }: SettingsSectionProps = $props();

	const attached = (centre: number) => centreSettings(ctx.records, centre) !== null;
	const orphans = $derived(
		allProducts(ctx.records).filter((p) => !ctx.costCentres.some((c) => c.id === p.centre))
	);
	let error = $state<string | null>(null);

	async function run(action: () => Promise<unknown>) {
		error = null;
		try {
			await action();
		} catch (e) {
			if (!(e instanceof Error)) throw e;
			error = e.message;
		}
	}

	const toggle = (centre: number, on: boolean) =>
		run(() => ctx.apply([setCentre(centre, on ? NEW_CENTRE : null)]));

	/** The name of a new cost centre for goods, such as Drinks or Merch. */
	let name = $state<string | null>(null);

	// A new cost centre with its own income and expense categories, and a shared Deposit. A
	// category the event already has joins it, unless it belongs to another cost centre.
	const add = () =>
		run(async () => {
			const centreName = name?.trim();
			if (!centreName) throw new Error('Enter a name for the cost centre.');
			const centre = await ctx.createCostCentre(centreName);
			const category = async (kind: 'income' | 'expense', name: string) => {
				const found = ctx.accounts.find(
					(a) => a.kind === kind && a.name.toLowerCase() === name.toLowerCase()
				);
				if (!found) return ctx.createCategory(kind, name, centre);
				if (found.costCentre === null) await ctx.setCategoryCentre(found.id, centre);
				return found.id;
			};
			await category('income', `${centreName} income`);
			await category('expense', centreName);
			const deposit = await category('expense', 'Deposit');
			await ctx.apply([setCentre(centre, { ...NEW_CENTRE, deposit })]);
			name = null;
		});

	const move = (centre: number) =>
		run(() => ctx.apply(orphans.map((p) => setProduct({ ...p, centre }))));
</script>

<fieldset class="section" disabled={ctx.closed}>
	{#if ctx.costCentres.length}
		<span class="note">Show on these cost centres</span>
		<ul>
			{#each ctx.costCentres as c (c.id)}
				<li>
					<label>
						<input
							type="checkbox"
							checked={attached(c.id)}
							onchange={(e) => toggle(c.id, e.currentTarget.checked)}
						/>
						{c.name}
					</label>
				</li>
			{/each}
		</ul>
	{/if}
	{#if !ctx.closed}
		<div class="add">
			<TextField
				label="New cost centre for goods"
				value={name}
				oninput={(v) => (name = v)}
				oncommit={(v) => (name = v)}
			>
				{#snippet hint()}Such as Drinks or Merch. It gets an income and an expense category of its
					name, and Deposit.{/snippet}
			</TextField>
			<div><Button size="sm" icon="plus" onclick={add}>Add cost centre</Button></div>
		</div>
	{/if}
	{#if orphans.length}
		<p class="note" data-testid="plugin-drinks-orphans">
			{orphans.length}
			{orphans.length === 1 ? 'product belongs' : 'products belong'} to a deleted cost centre.
		</p>
		{#if ctx.costCentres.length}
			<Select
				label="Move to"
				value={0}
				options={[
					{ value: 0, label: 'Choose' },
					...ctx.costCentres.map((c) => ({ value: c.id, label: c.name }))
				]}
				onchange={(v) => {
					if (v !== 0) move(v);
				}}
			/>
		{/if}
	{/if}
	{#if error}<p class="form-error">{error}</p>{/if}
</fieldset>

<style>
	.section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
		padding: 0;
		border: none;
	}

	ul {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.add {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2);
		max-width: 360px;
		margin-top: var(--spacing-2);
	}

	label {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2);
	}

	.note {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}
</style>
