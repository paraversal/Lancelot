<script lang="ts">
	// A person before anything is written. The name has the focus and the panel writes nothing
	// until it is saved: Esc or Done adds the person, an empty form closes, and a form with
	// something in it that cannot be saved asks before it is discarded.
	import { api, ApiError } from '#lib/api/index.ts';
	import type { AccountId } from '#lib/api/generated/AccountId.ts';
	import type { Person } from '#lib/api/generated/Person.ts';
	import Button from '#lib/components/Button.svelte';
	import IbanField from '#lib/components/IbanField.svelte';
	import Panel from '#lib/components/Panel.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { session } from '#lib/state/session.svelte.ts';

	let {
		onclose,
		oncreated
	}: {
		onclose: () => void;
		oncreated: (id: AccountId) => void;
	} = $props();

	let name = $state('');
	let holder = $state('');
	let iban = $state<string | null>(null);
	let bic = $state('');
	let otherMethod = $state('');
	let contact = $state('');
	let notes = $state('');
	let method = $state<'bank' | 'other'>('bank');
	let error = $state<{ field: string | null; message: string } | null>(null);
	let failed = $state(false);
	let saving = $state(false);
	let ibanField = $state<ReturnType<typeof IbanField> | null>(null);

	const clean = (text: string) => text.trim() || null;
	const hasContent = $derived(
		!!name.trim() ||
			!!holder.trim() ||
			iban !== null ||
			!!bic.trim() ||
			!!contact.trim() ||
			!!notes.trim() ||
			(method === 'other' && !!otherMethod.trim())
	);
	const nameMissing = $derived(!name.trim());
	// Esc asks first when the form has something unsaved that it cannot write: a name still
	// missing while other fields are filled in, or a save that the core refused. An empty form
	// and one that can be saved go without a warning.
	const dirty = $derived(failed || (nameMissing && hasContent));

	function changed() {
		failed = false;
		error = null;
	}

	function chooseMethod(next: 'bank' | 'other') {
		if (next === method) return;
		method = next;
		if (next === 'bank') otherMethod = '';
		changed();
	}

	async function submit() {
		// One press is enough even while the IBAN is still being checked: a second press, or a
		// second click on Done, must not add the person twice.
		if (saving) return;
		saving = true;
		try {
			// The IBAN is still in its own field while it is being checked. Take it first, so a
			// valid one makes the form non-empty and an invalid one keeps the panel open.
			if (method === 'bank' && ibanField && !(await ibanField.flush())) {
				failed = true;
				return;
			}
			if (!hasContent) {
				onclose();
				return;
			}
			if (nameMissing) {
				failed = true;
				error = { field: 'name', message: 'a name is needed' };
				return;
			}
			const details: Person = {
				holder: clean(holder),
				iban,
				bic: clean(bic),
				otherMethod: method === 'other' ? clean(otherMethod) : null,
				contact: clean(contact),
				notes: clean(notes)
			};
			error = null;
			const added = await session.change(() => api.addPerson(name.trim(), details));
			oncreated(added.id);
		} catch (e) {
			if (!(e instanceof ApiError)) throw e;
			error = { field: e.field, message: e.message };
			failed = true;
		} finally {
			saving = false;
		}
	}

	function escape() {
		// The panel calls this once it has nothing to lose, or after its own warning.
		if (dirty) {
			onclose();
			return;
		}
		void submit();
	}
</script>

<Panel title="New person" {onclose} onescape={escape} {dirty}>
	<div class="form-field">
		<TextField
			label=""
			placeholder="Name"
			value={name}
			autofocus
			oninput={(v) => {
				name = v;
				changed();
			}}
			oncommit={(v) => (name = v ?? '')}
		/>
		{#if error?.field === 'name'}<span class="form-error">{error.message}</span>{/if}
	</div>
	<div class="labelled">
		<span class="form-label">Reimburse by</span>
		<Segmented
			label="Reimburse by"
			value={method}
			onchange={chooseMethod}
			options={[
				{ value: 'bank', label: 'Bank transfer' },
				{ value: 'other', label: 'Other' }
			]}
		/>
	</div>
	{#if method === 'other'}
		<TextField
			label="Method"
			value={otherMethod}
			oninput={(v) => {
				otherMethod = v;
				changed();
			}}
			oncommit={(v) => (otherMethod = v ?? '')}
		>
			{#snippet hint()}Such as a PayPal address, or cash.{/snippet}
		</TextField>
	{:else}
		<TextField
			label="Account holder"
			value={holder}
			oninput={(v) => {
				holder = v;
				changed();
			}}
			oncommit={(v) => (holder = v ?? '')}
		/>
		<IbanField
			bind:this={ibanField}
			value={iban}
			oncommit={(v) => {
				iban = v;
				changed();
			}}
		/>
		<TextField
			label="BIC"
			value={bic}
			oninput={(v) => {
				bic = v;
				changed();
			}}
			oncommit={(v) => (bic = v ?? '')}
		/>
	{/if}
	<TextField
		label="Contact"
		value={contact}
		oninput={(v) => {
			contact = v;
			changed();
		}}
		oncommit={(v) => (contact = v ?? '')}
	/>
	<TextField
		label="Notes"
		value={notes}
		multiline
		oninput={(v) => {
			notes = v;
			changed();
		}}
		oncommit={(v) => (notes = v ?? '')}
	/>
	{#if error && error.field !== 'name'}<p class="form-error">{error.message}</p>{/if}

	{#snippet footer()}
		<Button kind="primary" shortcut="esc" onclick={submit}>Done</Button>
	{/snippet}
</Panel>

<style>
	.labelled {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
</style>
