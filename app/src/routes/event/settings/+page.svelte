<script lang="ts">
	// Settings: the event's details, the display format, the file and its integrity, backups.
	// Every field saves when it is left; there is no Save button.
	import { goto } from '$app/navigation';
	import { api, ApiError } from '#lib/api/index.ts';
	import { chooseNewEventFile, revealFile } from '#lib/api/dialogs.ts';
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
	import { formatSize, formatTimestamp } from '#lib/format.ts';
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
			error = e instanceof ApiError ? e.message : String(e);
		}
	}

	function details(patch: Partial<EventDetails>): EventDetails {
		const { name, startsOn, endsOn, location, notes } = event;
		return { name, startsOn, endsOn, location, notes, ...patch };
	}

	const update = (patch: Partial<EventDetails>) =>
		run(() => session.change(() => api.updateEvent(details(patch))));

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
			error = e instanceof ApiError ? e.message : String(e);
		}
	}

	const check = () => run(() => session.change(() => api.checkIntegrity(), true));

	const restore = (b: BackupInfo) =>
		run(async () => {
			const target = await chooseNewEventFile(`${event.name} restored`);
			if (!target) return;
			session.update(await api.restoreBackup(open.path, b.path, target));
			await goto('/event/overview');
		});

	const assets = $derived(
		open.snapshot.accounts
			.filter((a) => a.kind === 'asset')
			.map((a) => ({ value: a.id, label: a.name }))
	);
	const issues = $derived(open.integrity.issues.length);
	const shortPath = $derived(open.path.replace(/^\/Users\/[^/]+/, '~'));
	const checked = $derived(
		session.checkedAt
			? formatTimestamp(session.checkedAt.toISOString(), format).replace('Today ', '')
			: null
	);
</script>

<h1>Settings</h1>

<div class="columns">
	<section>
		<h2>Event</h2>
		<TextField
			label="Name"
			value={event.name}
			required
			oncommit={(v) => v && update({ name: v })}
		/>
		<div class="pair">
			<DateField
				label="From"
				value={event.startsOn}
				{format}
				oncommit={(v) => update({ startsOn: v })}
			/>
			<DateField label="To" value={event.endsOn} {format} oncommit={(v) => update({ endsOn: v })} />
		</div>
		<TextField label="Location" value={event.location} oncommit={(v) => update({ location: v })} />
		<TextField label="Notes" value={event.notes} multiline oncommit={(v) => update({ notes: v })} />
		<Select
			label="Reimburse from"
			value={event.defaultReimbursementAccount}
			options={assets}
			onchange={(account) =>
				run(() => session.change(() => api.setDefaultReimbursementAccount(account)))}
		/>
	</section>

	<div class="stack">
		<section>
			<h2>Display</h2>
			<div class="labelled">
				<span class="label">Numbers and dates</span>
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
					<span class="ok"><Icon name="check" /></span>
					<strong>No issues</strong>
				{/if}
				{#if checked}<span class="muted">Checked {checked}</span>{/if}
				<span class="grow"></span>
				<Button size="sm" icon="refresh" onclick={check}>Check again</Button>
			</div>
		</section>

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

{#if error}<p class="error" role="alert">{error}</p>{/if}

<style>
	h1 {
		margin: 0 0 28px;
		font-size: var(--type-lg);
		font-weight: 600;
	}

	.columns {
		display: grid;
		grid-template-columns: minmax(0, 440px) minmax(0, 460px);
		gap: 60px;
	}

	.stack {
		display: flex;
		flex-direction: column;
		gap: 36px;
	}

	section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-4);
	}

	h2 {
		margin: 0;
		padding-bottom: var(--spacing-4);
		border-bottom: 1px solid var(--color-rule);
		font-size: var(--type-md);
		font-weight: 600;
	}

	.pair {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--spacing-3);
	}

	.labelled {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 6px;
	}

	.label {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
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

	.grow {
		flex: 1;
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

	.error {
		margin: var(--spacing-6) 0 0;
		font-size: var(--type-xs);
		color: var(--color-minus);
	}
</style>
