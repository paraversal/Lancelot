import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import { api } from '#lib/api/index.ts';
import example from '#lib/api/fixtures/example-event.json';
import type { ChangeLogEntry } from '#lib/api/generated/ChangeLogEntry.ts';
import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import { session } from '#lib/state/session.svelte.ts';
import Page from './+page.svelte';

const snapshot = example as unknown as EventSnapshot;

function open(log: unknown[]) {
	session.update({
		revision: Date.now(),
		path: '/x.lancelot',
		fileName: 'x.lancelot',
		snapshot: structuredClone(snapshot),
		integrity: { issues: [] },
		backupError: null
	});
	vi.spyOn(api, 'changeLog').mockResolvedValue(log as ChangeLogEntry[]);
	render(Page);
}

const centre = (id: number, at: string, name: string) => ({
	id,
	at,
	entity: 'cost_centre',
	entityId: id,
	op: 'create',
	before: null,
	after: { id, name }
});

describe('The history page', () => {
	it('shows an edit with its old and new value', async () => {
		open([
			{
				id: 2,
				at: '2026-06-13T10:00:01Z',
				entity: 'cost_centre',
				entityId: 1,
				op: 'update',
				before: { id: 1, name: 'Food' },
				after: { id: 1, name: 'Drinks' }
			},
			centre(1, '2026-06-13T10:00:00Z', 'Food')
		]);
		expect(await screen.findByText('Edited cost centre')).toBeTruthy();
		expect(screen.getByText('Added cost centre')).toBeTruthy();
		expect(screen.getByRole('term').textContent ?? '').toBe('Name');
	});

	it('links an entry that is still in the ledger, and not one that is gone', async () => {
		const txn = snapshot.txns[0];
		const after = {
			id: txn.id,
			kind: 'expense',
			date: '2026-06-13',
			description: 'Still here',
			counterparty: null,
			notes: null,
			lines: [],
			origin: { type: 'manual' }
		};
		const row = { at: '2026-06-13T10:00:00Z', entity: 'txn', entityId: txn.id, op: 'create' };
		open([
			{
				...row,
				id: 2,
				at: '2026-06-13T10:00:01Z',
				op: 'delete',
				before: { ...after, description: 'Gone' },
				after: null
			},
			{ ...row, id: 1, before: null, after }
		]);
		const link = await screen.findByRole('link', { name: 'Still here' });
		expect(link.getAttribute('href')).toBe(`/event/ledger?txn=${txn.id}`);
		expect(screen.queryByRole('link', { name: 'Gone' })).toBeNull();
	});

	it('folds a command that wrote many rows, until asked', async () => {
		const at = '2026-06-13T10:00:00Z';
		open([1, 2, 3, 4, 5, 6].map((id) => centre(id, at, `Centre ${id}`)));
		await fireEvent.click(await screen.findByRole('button', { name: '3 more' }));
		expect(screen.getByText('Centre 6')).toBeTruthy();
		expect(screen.queryByRole('button', { name: /more/ })).toBeNull();
	});

	it('searches, also behind the fold', async () => {
		const at = '2026-06-13T10:00:00Z';
		open([1, 2, 3, 4, 5, 6].map((id) => centre(id, at, `Centre ${id}`)));
		await screen.findByText('Centre 1');
		await fireEvent.input(screen.getByPlaceholderText('Search'), { target: { value: 'centre 6' } });
		expect(await screen.findByText('Centre 6')).toBeTruthy();
		expect(screen.queryByText('Centre 1')).toBeNull();
	});
});
