import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError } from '#lib/api/index.ts';
import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import example from '#lib/api/fixtures/example-event.json';
import { session } from '#lib/state/session.svelte.ts';
import People from './+page.svelte';

const { addPerson } = vi.hoisted(() => ({ addPerson: vi.fn() }));

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/state', () => ({ page: { url: new URL('http://x/event/people') } }));
vi.mock('#lib/api/index.ts', async () => {
	const actual = await vi.importActual<typeof import('#lib/api/index.ts')>('#lib/api/index.ts');
	return { ...actual, api: { ...actual.api, addPerson } };
});

const snapshot = example as unknown as EventSnapshot;
const person = {
	holder: null,
	iban: null,
	bic: null,
	otherMethod: null,
	contact: null,
	notes: null
};

beforeEach(() => {
	addPerson.mockReset();
	session.update({
		revision: Date.now(),
		path: '/e.lancelot',
		fileName: 'e.lancelot',
		snapshot: structuredClone(snapshot),
		integrity: { issues: [] },
		backupError: null
	});
});

const openNew = async () => {
	render(People);
	await fireEvent.click(screen.getByRole('button', { name: /Person/ }));
	const name = screen.getByPlaceholderText('Name') as HTMLInputElement;
	return name;
};

const escape = (target: Element | Window = window) => fireEvent.keyDown(target, { key: 'Escape' });

describe('Adding a person', () => {
	it('opens a blank panel with the name focused and no record written', async () => {
		const name = await openNew();
		expect(screen.getByText('New person')).toBeTruthy();
		expect(document.activeElement).toBe(name);
		expect(name.getAttribute('aria-label')).toBe('Name');
		expect(addPerson).not.toHaveBeenCalled();
	});

	it('writes the person when Esc saves the typed name, and closes the panel', async () => {
		addPerson.mockImplementation(async () => ({ event: session.event!, id: 99 }));
		const name = await openNew();
		await fireEvent.input(name, { target: { value: 'Marta' } });
		await escape(name);
		await waitFor(() => expect(addPerson).toHaveBeenCalledWith('Marta', person));
		await waitFor(() => expect(screen.queryByText('New person')).toBeNull());
	});

	it('saves fields that never lost the focus', async () => {
		addPerson.mockImplementation(async () => ({ event: session.event!, id: 99 }));
		const name = await openNew();
		await fireEvent.input(name, { target: { value: 'Marta' } });
		await fireEvent.input(screen.getByLabelText('Contact'), {
			target: { value: 'marta@example.org' }
		});
		await escape(screen.getByLabelText('Contact'));
		await waitFor(() =>
			expect(addPerson).toHaveBeenCalledWith('Marta', {
				...person,
				contact: 'marta@example.org'
			})
		);
	});

	it('closes an empty panel without writing anything or warning', async () => {
		const name = await openNew();
		await escape(name);
		await waitFor(() => expect(screen.queryByText('New person')).toBeNull());
		expect(addPerson).not.toHaveBeenCalled();
		expect(screen.queryByText('Press Esc again to discard changes')).toBeNull();
	});

	it('warns once before discarding fields that have no name to save them under', async () => {
		await openNew();
		const contact = screen.getByLabelText('Contact');
		await fireEvent.input(contact, { target: { value: 'marta@example.org' } });
		await escape(contact);
		expect(screen.getByText('Press Esc again to discard changes')).toBeTruthy();
		expect(addPerson).not.toHaveBeenCalled();
		await escape();
		await waitFor(() => expect(screen.queryByText('New person')).toBeNull());
		expect(addPerson).not.toHaveBeenCalled();
	});

	it('asks for a name when Done has fields but no name', async () => {
		await openNew();
		await fireEvent.input(screen.getByLabelText('Contact'), {
			target: { value: 'marta@example.org' }
		});
		await fireEvent.click(screen.getByRole('button', { name: /Done/ }));
		expect(screen.getByText('a name is needed')).toBeTruthy();
		expect(screen.getByText('New person')).toBeTruthy();
		expect(addPerson).not.toHaveBeenCalled();
	});

	it('keeps a name the core refuses, then warns before discarding it', async () => {
		addPerson.mockRejectedValue(
			new ApiError({
				code: 'duplicate_name',
				message: 'this name is already used',
				field: 'name',
				report: null
			})
		);
		const name = await openNew();
		await fireEvent.input(name, { target: { value: 'Anna' } });
		await escape(name);
		expect(await screen.findByText('this name is already used')).toBeTruthy();
		expect(screen.getByText('New person')).toBeTruthy();
		await escape();
		expect(screen.getByText('Press Esc again to discard changes')).toBeTruthy();
		await escape();
		await waitFor(() => expect(screen.queryByText('New person')).toBeNull());
		expect(addPerson).toHaveBeenCalledOnce();
	});
});
