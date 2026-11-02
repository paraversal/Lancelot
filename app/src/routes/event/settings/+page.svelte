<script lang="ts">
	// Settings: the event's details, its accounts, the display format, the file and its
	// integrity, backups. Every field saves when it is left; there is no Save button.
	import { tick } from 'svelte';
	import { goto } from '$app/navigation';
	import { api, errorMessage } from '#lib/api/index.ts';
	import { chooseEventFile, chooseNewEventFile, revealFile } from '#lib/api/dialogs.ts';
	import type { CopySummary } from '#lib/api/generated/CopySummary.ts';
	import type { Appearance } from '#lib/api/generated/Appearance.ts';
	import type { AssetType } from '#lib/api/generated/AssetType.ts';
	import type { BackupInfo } from '#lib/api/generated/BackupInfo.ts';
	import type { EventDetails } from '#lib/api/generated/EventDetails.ts';
	import type { NumberFormat } from '#lib/api/generated/NumberFormat.ts';
	import Button from '#lib/components/Button.svelte';
	import DateField from '#lib/components/DateField.svelte';
	import Field from '#lib/components/Field.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Select from '#lib/components/Select.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { formatAmount, formatDate, formatSize, formatTimestamp } from '#lib/format.ts';
	import { lingerOut, spin } from '#lib/motion/effects.ts';
	import Check from '#lib/motion/Check.svelte';
	import Lock from '#lib/motion/Lock.svelte';
	import { level, reduced } from '#lib/motion/mode.svelte.ts';
	import { pluginContext, pluginEnabled, setPluginEnabled } from '#lib/plugins/host.ts';
	import DataSourceForm from '#lib/plugins/DataSourceForm.svelte';
	import PluginFrame from '#lib/plugins/PluginFrame.svelte';
	import { plugins } from '#lib/plugins/registry.ts';
	import { session } from '#lib/state/session.svelte.ts';

	// The layout only renders pages while an event is open.
	const open = $derived(session.event!);
	const event = $derived(open.snapshot.event);
	const format = $derived(session.format);

	let error = $state<string | null>(null);
	let backups = $state<BackupInfo[]>([]);
	let keepText = $state('');
	let keepInvalid = $state(false);

	$effect(() => {
		keepText = String(session.settings?.backupCount ?? 20);
	});

	$effect(() => {
		// Reload the list after every change, which may have written a backup.
		void open.revision;
		api.backups(open.path).then((b) => (backups = b));
	});

	async function run(f: () => Promise<unknown>) {
		try {
			error = null;
			await f();
		} catch (e) {
			error = errorMessage(e);
		}
	}

	function details(patch: Partial<EventDetails>): EventDetails {
		const { name, startsOn, endsOn, location, notes } = event;
		return { name, startsOn, endsOn, location, notes, ...patch };
	}

	const update = (patch: Partial<EventDetails>) =>
		run(() => session.change(() => api.updateEvent(details(patch))));

	const setReducedMotion = (reducedMotion: boolean) =>
		run(async () => {
			if (!session.settings) return;
			session.settings = await api.setSettings({ ...session.settings, reducedMotion });
		});

	const setAppearance = (appearance: Appearance) =>
		run(async () => {
			if (!session.settings) return;
			session.settings = await api.setSettings({ ...session.settings, appearance });
		});

	const setFormat = (numberFormat: NumberFormat) =>
		run(async () => {
			if (!session.settings) return;
			session.settings = await api.setSettings({ ...session.settings, numberFormat });
		});

	async function setKeep() {
		const backupCount = Number(keepText);
		if (!session.settings || backupCount === session.settings.backupCount) return;
		try {
			session.settings = await api.setSettings({ ...session.settings, backupCount });
			keepInvalid = false;
		} catch (e) {
			keepInvalid = true;
			error = errorMessage(e);
		}
	}

	// F2: the arrow turns once, and a clean result draws its check again.
	const check = (e: MouseEvent) => {
		const arrow = (e.currentTarget as HTMLElement).querySelector('svg');
		if (arrow) spin(arrow, level('F2'));
		return run(() => session.change(() => api.checkIntegrity(), true));
	};

	const restore = (b: BackupInfo) =>
		run(async () => {
			const target = await chooseNewEventFile(`${event.name} restored`);
			if (!target) return;
			session.update(await api.restoreBackup(open.path, b.path, target));
			await goto('/event/overview');
		});

	const closed = $derived(session.closed);
	const closedOn = $derived(
		event.closedAt ? formatDate(event.closedAt.slice(0, 10), format) : null
	);
	let reopening = $state(false);
	// E2: the lock opens while the status lingers; the event is already reopened underneath.
	let unlocking = $state(false);
	const reopen = () =>
		run(async () => {
			unlocking = true;
			try {
				await session.change(() => api.reopenEvent());
			} finally {
				unlocking = false;
				reopening = false;
			}
		});

	// An account or category can go while nothing refers to it; the core has the last word.
	let deletingAccount = $state<number | null>(null);
	const accounts = $derived(open.snapshot.accounts.filter((a) => a.kind === 'asset'));
	const assets = $derived(accounts.map((a) => ({ value: a.id, label: a.name })));
	const TYPES: { value: AssetType; label: string }[] = [
		{ value: 'bank', label: 'Bank' },
		{ value: 'cash', label: 'Cash' }
	];

	function rename(id: number, input: HTMLInputElement) {
		const name = input.value.trim();
		const current = accounts.find((a) => a.id === id)?.name ?? '';
		if (!name || name === current) {
			input.value = current;
			return;
		}
		run(async () => {
			try {
				await session.change(() => api.renameAccount(id, name));
			} catch (e) {
				input.value = current;
				throw e;
			}
		});
	}

	const addAccount = () =>
		run(async () => {
			let name = 'New account';
			for (let n = 2; accounts.some((a) => a.name.toLowerCase() === name.toLowerCase()); n++)
				name = `New account ${n}`;
			const added = await session.change(() => api.addAsset(name, 'cash'));
			await tick();
			const input = document.getElementById(`account-${added.id}`) as HTMLInputElement | null;
			input?.focus();
			input?.select();
		});
	// Cost centres (R-CC-1): names save when left, like the accounts. Deleting one leaves what was
	// tagged without a tag, so it asks first.
	const centres = $derived(open.snapshot.costCentres);
	const centreOptions = $derived([
		{ value: 0, label: 'None' },
		...centres.map((c) => ({ value: c.id, label: c.name }))
	]);
	const categories = $derived(
		open.snapshot.accounts.filter((a) => a.kind === 'income' || a.kind === 'expense')
	);
	const expenses = $derived(categories.filter((a) => a.kind === 'expense'));
	const income = $derived(categories.filter((a) => a.kind === 'income'));
	let deletingCentre = $state<number | null>(null);

	function renameCentre(id: number, input: HTMLInputElement) {
		const name = input.value.trim();
		const current = centres.find((c) => c.id === id)?.name ?? '';
		if (!name || name === current) {
			input.value = current;
			return;
		}
		run(async () => {
			try {
				await session.change(() => api.renameCostCentre(id, name));
			} catch (e) {
				input.value = current;
				throw e;
			}
		});
	}

	const addCentre = () =>
		run(async () => {
			let name = 'New cost centre';
			for (let n = 2; centres.some((c) => c.name.toLowerCase() === name.toLowerCase()); n++)
				name = `New cost centre ${n}`;
			await session.change(() => api.addCostCentre(name));
			await tick();
			const inputs = document.querySelectorAll<HTMLInputElement>('.centres input');
			const input = inputs[inputs.length - 1];
			input?.focus();
			input?.select();
		});

	// Copying from another event (R-ISO-4): what it would add, then what was chosen.
	let source = $state<{ path: string; preview: CopySummary } | null>(null);
	let copyPersons = $state(true);
	let copyBudget = $state(true);
	let copied = $state<string | null>(null);
	const fileName = (path: string) => path.split('/').pop() ?? path;
	const counted = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

	const chooseSource = () =>
		run(async () => {
			copied = null;
			const path = await chooseEventFile();
			if (!path) return;
			source = { path, preview: await api.copyPreview(path) };
			copyPersons = source.preview.personsAdded > 0;
			copyBudget = source.preview.budgetLinesAdded > 0;
		});

	const copyFrom = () =>
		run(async () => {
			if (!source) return;
			const { path, preview } = source;
			const persons = copyPersons;
			const budget = copyBudget;
			await session.change(() => api.copyFromEvent(path, { persons, budget }));
			const added = [
				persons ? counted(preview.personsAdded, 'person', 'people') : null,
				budget ? counted(preview.budgetLinesAdded, 'budget line', 'budget lines') : null
			].filter(Boolean);
			copied = `Added ${added.join(' and ')}`;
			source = null;
		});

	const issues = $derived(open.integrity.issues.length);
	const shortPath = $derived(open.path.replace(/^\/Users\/[^/]+/, '~'));
	const checked = $derived(
		session.checkedAt
			? formatTimestamp(session.checkedAt.toISOString(), format).replace('Today ', '')
			: null
	);
</script>

{#snippet deleteAccount(a: { id: number; role: unknown; balance: number })}
	{#if !a.role && !closed && !a.balance && a.id !== event.defaultReimbursementAccount && a.id !== open.snapshot.ticketSettings.account}
		{#if deletingAccount === a.id}
			<Button
				size="sm"
				kind="primary"
				onclick={() => {
					deletingAccount = null;
					return run(() => session.change(() => api.deleteAccount(a.id)));
				}}>Delete</Button
			>
			<Button size="sm" onclick={() => (deletingAccount = null)}>Keep</Button>
		{:else}
			<Button size="sm" onclick={() => (deletingAccount = a.id)}>Delete</Button>
		{/if}
	{/if}
{/snippet}

{#snippet categoryGroup(title: string, list: typeof categories)}
	{#if list.length}
		<span class="form-label group">{title}</span>
		<ul class="accounts">
			{#each list as a (a.id)}
				<li>
					<span class="name" title={a.name}>{a.name}</span>
					<span class="delete" class:deleting={deletingAccount === a.id}>
						{@render deleteAccount(a)}
					</span>
					{#if centres.length}
						<Select
							name="{a.name} starts in"
							value={a.costCentre ?? 0}
							options={centreOptions}
							onchange={(v) =>
								run(() =>
									session.change(() => api.setCategoryCostCentre(a.id, v === 0 ? null : v))
								)}
						/>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
{/snippet}

<div class="page">
	<h1>Settings</h1>

	<div class="columns">
		<fieldset class="plain" disabled={closed}>
			<div class="stack">
				<section>
					<h2>Event</h2>
					<TextField
						label="Name"
						value={event.name}
						required
						oncommit={(v) => v && update({ name: v })}
					/>
					<div class="form-pair">
						<DateField
							label="From"
							value={event.startsOn}
							{format}
							oncommit={(v) => update({ startsOn: v })}
						/>
						<DateField
							label="To"
							value={event.endsOn}
							{format}
							oncommit={(v) => update({ endsOn: v })}
						/>
					</div>
					<TextField
						label="Location"
						value={event.location}
						oncommit={(v) => update({ location: v })}
					/>
					<TextField
						label="Notes"
						value={event.notes}
						multiline
						oncommit={(v) => update({ notes: v })}
					/>
					<Select
						label="Reimburse from"
						value={event.defaultReimbursementAccount}
						options={assets}
						onchange={(account) =>
							run(() => session.change(() => api.setDefaultReimbursementAccount(account)))}
					/>
					<label class="toggle">
						<input
							type="checkbox"
							checked={open.snapshot.ticketSettings.enabled}
							onchange={(e) => {
								const { checked } = e.currentTarget;
								const box = e.currentTarget;
								run(async () => {
									try {
										await session.change(() => api.setTicketsEnabled(checked));
									} catch (err) {
										box.checked = !checked;
										throw err;
									}
								});
							}}
						/>
						Sells tickets
					</label>
				</section>

				<section>
					<h2>Accounts</h2>
					<ul class="accounts">
						{#each accounts as a (a.id)}
							<li>
								<input
									id="account-{a.id}"
									class="name"
									type="text"
									aria-label="Account name"
									value={a.name}
									onblur={(e) => rename(a.id, e.currentTarget)}
									onkeydown={(e) => {
										if (e.key === 'Enter') e.currentTarget.blur();
										if (e.key === 'Escape') {
											e.currentTarget.value = a.name;
											e.currentTarget.blur();
										}
									}}
								/>
								<Segmented
									label="{a.name} holds"
									value={a.assetType ?? 'bank'}
									options={TYPES}
									onchange={(t) => run(() => session.change(() => api.setAssetType(a.id, t)))}
								/>
								<label class="toggle" title="Counts and statements leave out opening counts">
									<input
										type="checkbox"
										checked={a.netMovement}
										onchange={(e) => {
											const { checked } = e.currentTarget;
											run(() => session.change(() => api.setNetMovement(a.id, checked)));
										}}
									/>
									Net movement
								</label>
								<span class="balance">{formatAmount(a.balance, format)}</span>
								{@render deleteAccount(a)}
							</li>
						{/each}
					</ul>
					{#if !closed}
						<div><Button size="sm" icon="plus" onclick={addAccount}>Account</Button></div>
					{/if}
				</section>

				<section>
					<h2>Cost centres</h2>
					<ul class="accounts centres">
						{#each centres as c (c.id)}
							<li>
								<input
									class="name"
									type="text"
									aria-label="Cost centre name"
									value={c.name}
									onblur={(e) => renameCentre(c.id, e.currentTarget)}
									onkeydown={(e) => {
										if (e.key === 'Enter') e.currentTarget.blur();
										if (e.key === 'Escape') {
											e.currentTarget.value = c.name;
											e.currentTarget.blur();
										}
									}}
								/>
								{#if !closed}
									{#if deletingCentre === c.id}
										<Button
											size="sm"
											kind="primary"
											onclick={() => {
												deletingCentre = null;
												return run(() => session.change(() => api.deleteCostCentre(c.id)));
											}}>Delete</Button
										>
										<Button size="sm" onclick={() => (deletingCentre = null)}>Keep</Button>
									{:else}
										<Button size="sm" onclick={() => (deletingCentre = c.id)}>Delete</Button>
									{/if}
								{/if}
							</li>
						{/each}
					</ul>
					{#if !closed}
						<div><Button size="sm" icon="plus" onclick={addCentre}>Cost centre</Button></div>
					{/if}
				</section>
			</div>
		</fieldset>

		<div class="stack">
			{#if closedOn}
				<section out:lingerOut={{ level: level('E2') }}>
					<h2>Status</h2>
					<div class="line">
						<span class="muted"><Lock closed={!unlocking} level={level('E2')} size={16} /></span>
						<strong>Closed {closedOn}</strong>
						<span class="grow"></span>
						{#if reopening}
							<span>Reopen for changes?</span>
							<Button size="sm" kind="primary" onclick={reopen}>Reopen</Button>
							<Button size="sm" onclick={() => (reopening = false)}>Keep closed</Button>
						{:else}
							<Button size="sm" onclick={() => (reopening = true)}>Reopen</Button>
						{/if}
					</div>
				</section>
			{/if}
			<section>
				<h2>Display</h2>
				<div class="labelled">
					<span class="form-label">Appearance</span>
					<Segmented
						label="Appearance"
						value={session.settings?.appearance ?? 'system'}
						onchange={setAppearance}
						options={[
							{ value: 'system', label: 'System' },
							{ value: 'light', label: 'Light' },
							{ value: 'dark', label: 'Dark' }
						]}
					/>
				</div>
				<div class="labelled">
					<span class="form-label">Numbers and dates</span>
					<Segmented
						label="Numbers and dates"
						value={format}
						onchange={setFormat}
						options={[
							{ value: 'de', label: '1.234,56 €  ·  31.12.2026' },
							{ value: 'en', label: '€1,234.56  ·  2026-12-31' }
						]}
					/>
				</div>
				<label class="toggle">
					<input
						type="checkbox"
						checked={reduced()}
						onchange={(e) => setReducedMotion(e.currentTarget.checked)}
					/>
					Reduced motion
				</label>
			</section>

			<section>
				<h2>File</h2>
				<div class="line">
					<span class="muted"><Icon name="file" /></span>
					<span class="path" title={open.path}>{shortPath}</span>
					<Button size="sm" onclick={() => run(() => revealFile(open.path))}>Show in Finder</Button>
				</div>
				<div class="line">
					{#if issues}
						<span class="warn"><Icon name="warn" /></span>
						<strong class="warn">{issues === 1 ? '1 issue' : `${issues} issues`}</strong>
					{:else}
						{#key session.checkedAt}
							<span class="ok"><Check level={level('F2')} size={16} delay={220} /></span>
						{/key}
						<strong>No issues</strong>
					{/if}
					{#if checked}<span class="muted">Checked {checked}</span>{/if}
					<span class="grow"></span>
					<Button size="sm" icon="refresh" onclick={check}>Check again</Button>
				</div>
			</section>

			{#if !closed}
				<section id="copy">
					<h2>Copy from another event</h2>
					{#if source}
						<div class="line">
							<span class="muted"><Icon name="file" /></span>
							<span class="path">{fileName(source.path)}</span>
							<Button size="sm" onclick={() => (source = null)}>Cancel</Button>
						</div>
						<label class="toggle">
							<input type="checkbox" bind:checked={copyPersons} />
							People
							<span class="counts"
								>{source.preview.personsAdded} new · {source.preview.personsSkipped} already here</span
							>
						</label>
						<label class="toggle">
							<input type="checkbox" bind:checked={copyBudget} />
							Budget lines
							<span class="counts"
								>{source.preview.budgetLinesAdded} new · {source.preview.budgetLinesSkipped} already here{#if source.preview.categoriesAdded}
									· {counted(
										source.preview.categoriesAdded,
										'new category',
										'new categories'
									)}{/if}{#if source.preview.costCentresAdded}
									· {counted(
										source.preview.costCentresAdded,
										'new cost centre',
										'new cost centres'
									)}{/if}</span
							>
						</label>
						<div>
							<Button
								size="sm"
								kind="primary"
								disabled={!copyPersons && !copyBudget}
								onclick={copyFrom}>Copy</Button
							>
						</div>
					{:else}
						<div class="line">
							<Button size="sm" onclick={chooseSource}>Choose event</Button>
							{#if copied}<span class="muted">{copied}</span>{/if}
						</div>
					{/if}
				</section>
			{/if}

			<section>
				<h2>Backups</h2>
				<div class="keep">
					<span>Keep</span>
					<div class="count">
						<Field invalid={keepInvalid}>
							<input
								type="text"
								inputmode="numeric"
								aria-label="Backups to keep"
								bind:value={keepText}
								onblur={setKeep}
								onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
							/>
						</Field>
					</div>
				</div>
				<ul class="backups">
					{#each backups as b (b.path)}
						<li>
							<span>{b.takenAt ? formatTimestamp(b.takenAt, format) : b.path}</span>
							<span class="grow"></span>
							<span class="muted size">{formatSize(b.size, format)}</span>
							<span class="restore"
								><Button size="sm" onclick={() => restore(b)}>Restore</Button></span
							>
						</li>
					{/each}
				</ul>
			</section>
		</div>
	</div>

	<fieldset class="plain" disabled={closed}>
		<section class="wide">
			<h2>Categories</h2>
			{#if centres.length}
				<p class="note">The cost centre beside a category is where its new lines start.</p>
			{/if}
			<div class="categories">
				{@render categoryGroup('Expenses', expenses)}
				{@render categoryGroup('Income', income)}
			</div>
		</section>
	</fieldset>

	<fieldset class="plain" disabled={closed}>
		<section class="wide plugins">
			<h2>Plugins</h2>
			<p class="note">
				Optional additions to the app. Switching one off hides it for this event and keeps its data
				in the file.
			</p>
			<div class="plugin-cards">
				{#each plugins as plugin (plugin.id)}
					{@const on = pluginEnabled(plugin.id)}
					<PluginFrame name={plugin.name} description={plugin.description}>
						{#snippet actions()}
							<label class="toggle">
								<input
									type="checkbox"
									checked={on}
									onchange={(e) => {
										const { checked } = e.currentTarget;
										run(() => setPluginEnabled(plugin.id, checked));
									}}
								/>
								On
							</label>
						{/snippet}
						{#if on && (plugin.SettingsSection || plugin.dataSource)}
							{#if plugin.SettingsSection}
								<plugin.SettingsSection ctx={pluginContext(plugin.id)} />
							{/if}
							{#if plugin.dataSource}
								<DataSourceForm id={plugin.id} spec={plugin.dataSource} />
							{/if}
						{/if}
					</PluginFrame>
				{/each}
			</div>
		</section>
	</fieldset>

	{#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

<style>
	.page {
		padding: var(--spacing-10);
	}

	h1 {
		margin: 0 0 28px;
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.columns {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 470px));
		gap: 60px;
	}

	.stack {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-12);
	}

	/* An ink rule opens each block, so it can't be mistaken for the hairlines between its rows. */
	section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-4);
		padding-top: var(--spacing-3);
		border-top: 1px solid var(--color-ink);
	}

	h2 {
		margin: 0;
		font-size: var(--type-md);
		font-weight: 600;
	}

	.wide {
		max-width: 1000px;
		margin-top: var(--spacing-12);
	}

	/* What follows is added to the app rather than part of it: a ledger's double rule. */
	.plugins {
		margin-top: 72px;
		padding-top: var(--spacing-6);
		border-top: 3px double var(--color-ink);
	}

	.plugins h2 {
		font-size: var(--type-lg);
	}

	.plugin-cards {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(380px, 100%), 1fr));
		align-items: start;
		gap: var(--spacing-6) 60px;
	}

	.note {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	/* Flows over two columns, so a long list stays short and fills the width. */
	.categories {
		columns: 2;
		column-gap: 60px;
	}

	/* As tall as a row, so the rows of both columns line up. */
	.group {
		display: flex;
		align-items: flex-end;
		height: 45px;
		padding-bottom: var(--spacing-1);
		break-after: avoid;
	}

	.categories .accounts li {
		flex-wrap: nowrap;
		break-inside: avoid;
	}

	.categories span.name {
		overflow: hidden;
		margin-left: 0;
		padding: 0;
		height: auto;
		border: none;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.categories li > :global(.field) {
		width: 150px;
		flex: none;
	}

	/* Like Restore on a backup: there when the row is in use. */
	.delete {
		display: none;
		gap: var(--spacing-2);
	}

	.categories li:hover .delete,
	.categories li:focus-within .delete,
	.delete.deleting {
		display: flex;
	}

	.labelled {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 6px;
	}

	.toggle {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2);
		align-self: flex-start;
		font-size: 13px;
		cursor: pointer;
	}

	.counts {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.accounts .toggle {
		align-self: center;
		white-space: nowrap;
	}

	.toggle input {
		accent-color: var(--color-ink);
	}

	.line {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		font-size: 13px;
	}

	.path {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		color: var(--color-ink-muted);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.muted {
		display: flex;
		color: var(--color-ink-muted);
	}

	.ok {
		display: flex;
		color: var(--color-plus);
	}

	.warn {
		display: flex;
		color: var(--color-attention);
	}

	strong {
		font-weight: 600;
	}

	.keep {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		font-size: 13px;
	}

	.count {
		width: 64px;
	}

	.accounts {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.accounts li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px var(--spacing-4);
		min-height: 44px;
		padding: 4px 0;
		border-bottom: 1px solid var(--color-rule);
	}

	.accounts .name {
		flex: 1 1 140px;
		min-width: 0;
		height: 34px;
		margin-left: -9px;
		padding: 0 8px;
		border: 1px solid transparent;
		border-radius: var(--radius-field);
		background: none;
		font: inherit;
		color: inherit;
	}

	.accounts .name:hover {
		border-color: var(--color-rule);
	}

	.accounts .name:focus {
		border-color: var(--color-ink);
		outline: none;
		background: var(--color-field);
	}

	.accounts .balance {
		width: 110px;
		flex: none;
		margin-left: auto;
		text-align: right;
	}

	.backups {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.backups li {
		display: flex;
		align-items: center;
		gap: var(--spacing-3);
		height: 36px;
		padding: 0 8px;
		border-bottom: 1px solid var(--color-rule);
		border-radius: 0;
		font-size: 13px;
	}

	.backups li:hover {
		border-radius: var(--radius-field);
		background: var(--color-paper-sunk);
	}

	.size {
		font-size: var(--type-xs);
	}

	.restore {
		display: none;
	}

	.backups li:hover .restore,
	.backups li:focus-within .restore {
		display: flex;
	}

	/* Stays in view: the page is long, and what was refused may be far above its end. */
	.error {
		position: sticky;
		bottom: 0;
		margin: var(--spacing-6) 0 0;
		padding: var(--spacing-3) 0;
		border-top: 1px solid var(--color-rule);
		background: var(--color-paper);
		font-size: var(--type-xs);
		color: var(--color-minus);
	}
</style>
