import { describe, expect, it } from 'vitest';

import type { ChangeLogEntry } from './api/generated/ChangeLogEntry';
import { historyChanges, localDay, localTime, matchingRows } from './history';

const NBSP = ' ';

function entry(
	id: number,
	at: string,
	entity: string,
	entityId: number,
	op: string,
	before: unknown,
	after: unknown
) {
	return { id, at, entity, entityId, op, before, after } as ChangeLogEntry;
}

const account = (id: number, name: string, kind = 'expense') => ({
	id,
	kind,
	name,
	role: null,
	assetType: null,
	netMovement: false,
	costCentre: null
});

const txn = (description: string, lines: unknown[], notes: string | null = null) => ({
	id: 7,
	kind: 'expense',
	date: '2026-06-13',
	description,
	counterparty: null,
	notes,
	lines,
	origin: { type: 'manual' }
});

const line = (account: number, amount: number, costCentre: number | null = null) => ({
	account,
	amount,
	budgetLine: null,
	costCentre
});

const setup = [
	entry(1, 't1', 'account', 1, 'create', null, account(1, 'Cash box', 'asset')),
	entry(2, 't1', 'account', 2, 'create', null, account(2, 'Kitchen')),
	entry(3, 't1', 'account', 3, 'create', null, account(3, 'Tech'))
];

describe('historyChanges', () => {
	it('makes one change of what one command wrote, newest first', () => {
		const changes = historyChanges(
			[
				entry(5, 't3', 'cost_centre', 1, 'create', null, { id: 1, name: 'Drinks' }),
				entry(4, 't2', 'cost_centre', 2, 'create', null, { id: 2, name: 'Food' }),
				...setup.toReversed()
			],
			'de'
		);
		expect(changes.map((c) => c.at)).toEqual(['t3', 't2', 't1']);
		expect(changes[2].rows.map((r) => r.name)).toEqual(['Cash box', 'Kitchen', 'Tech']);
	});

	it('shows the fields an edit moved, by account', () => {
		const before = txn('Bakery', [line(1, -2000), line(2, 2000)]);
		const after = txn('Cables', [line(1, -3500), line(3, 3500)], 'For the stage');
		const [change] = historyChanges(
			[entry(5, 't2', 'txn', 7, 'update', before, after), ...setup],
			'de'
		);
		const [row] = change.rows;
		expect([row.verb, row.noun, row.name]).toEqual(['Edited', 'Expense', 'Cables']);
		expect(row.fields).toEqual([
			{ label: 'Description', before: 'Bakery', after: 'Cables' },
			{ label: 'Cash box', before: `−20,00${NBSP}€`, after: `−35,00${NBSP}€` },
			{ label: 'Tech', before: null, after: `35,00${NBSP}€` },
			{ label: 'Notes', before: null, after: 'For the stage' },
			{ label: 'Kitchen', before: `20,00${NBSP}€`, after: null }
		]);
	});

	it('shows a line that only changed its cost centre', () => {
		const before = txn('Bakery', [line(1, -2000), line(2, 2000)]);
		const after = txn('Bakery', [line(1, -2000), line(2, 2000, 4)]);
		const [change] = historyChanges(
			[
				entry(6, 't3', 'txn', 7, 'update', before, after),
				entry(5, 't2', 'cost_centre', 4, 'create', null, { id: 4, name: 'Drinks' }),
				...setup
			],
			'de'
		);
		expect(change.rows[0].fields).toEqual([
			{ label: 'Kitchen', before: `20,00${NBSP}€`, after: `20,00${NBSP}€ · Drinks` }
		]);
	});

	it('shows income as money coming in', () => {
		const after = { ...txn('Donations', [line(1, 5000), line(8, -5000)]), kind: 'income' };
		const [change] = historyChanges(
			[
				entry(5, 't2', 'txn', 7, 'create', null, after),
				entry(4, 't1', 'account', 8, 'create', null, account(8, 'Bar', 'income')),
				...setup
			],
			'de'
		);
		expect(change.rows[0].fields).toContainEqual({
			label: 'Bar',
			before: null,
			after: `50,00${NBSP}€`
		});
	});

	it('keeps what was deleted, without repeating its name', () => {
		const gone = txn('Bakery', [line(1, -2000), line(2, 2000)]);
		const [change] = historyChanges(
			[entry(5, 't2', 'txn', 7, 'delete', gone, null), ...setup],
			'de'
		);
		const [row] = change.rows;
		expect([row.verb, row.name]).toEqual(['Deleted', 'Bakery']);
		expect(row.fields.map((f) => f.label)).toEqual(['Date', 'Cash box', 'Kitchen']);
		expect(row.fields.every((f) => f.after === null)).toBe(true);
	});

	it('names accounts as they were then, and still after they are gone', () => {
		const then = txn('Bakery', [line(1, -2000), line(2, 2000)]);
		const log = [
			entry(7, 't4', 'account', 2, 'delete', account(2, 'Food'), null),
			entry(6, 't3', 'account', 2, 'update', account(2, 'Kitchen'), account(2, 'Food')),
			entry(5, 't2', 'txn', 7, 'create', null, then),
			...setup
		];
		const changes = historyChanges(log, 'de');
		expect(changes[2].rows[0].fields.map((f) => f.label)).toContain('Kitchen');
		expect(changes[1].rows[0].fields).toEqual([
			{ label: 'Name', before: 'Kitchen', after: 'Food' }
		]);
		expect(changes[0].rows[0].name).toBe('Food');
	});

	it('makes one row of a person and their account', () => {
		const details = {
			holder: null,
			iban: null,
			otherMethod: null,
			bic: null,
			contact: 'clara@example.org',
			notes: null
		};
		const [change] = historyChanges(
			[
				entry(2, 't1', 'person', 9, 'create', null, details),
				entry(1, 't1', 'account', 9, 'create', null, account(9, 'Clara', 'person'))
			],
			'de'
		);
		expect(change.rows).toHaveLength(1);
		expect([change.rows[0].noun, change.rows[0].name]).toEqual(['Person', 'Clara']);
		expect(change.rows[0].fields).toEqual([
			{ label: 'Contact', before: null, after: 'clara@example.org' }
		]);
	});

	it('says closing and reopening with the notes on what stayed open', () => {
		const event = {
			name: 'Summer Festival',
			startsOn: null,
			endsOn: null,
			location: null,
			notes: null,
			closedAt: null,
			defaultReimbursementAccount: 1,
			closingOverrides: []
		};
		const closed = {
			...event,
			closedAt: 't2',
			closingOverrides: [{ item: { type: 'x' }, note: 'Clara pays later' }]
		};
		const [reopened, closing] = historyChanges(
			[
				entry(6, 't3', 'event', 1, 'reopen', closed, event),
				entry(5, 't2', 'event', 1, 'close', event, closed),
				...setup
			],
			'de'
		);
		expect(reopened.rows[0].verb).toBe('Reopened');
		expect(closing.rows[0].verb).toBe('Closed');
		expect(closing.rows[0].fields).toEqual([
			{ label: 'Accepted as open', before: null, after: 'Clara pays later' }
		]);
	});

	it('shows budget estimates and ticket fees in the chosen format', () => {
		const changes = historyChanges(
			[
				entry(6, 't3', 'ticket_tier', 1, 'create', null, {
					id: 1,
					name: 'Early',
					price: 2500,
					feeBp: 250,
					quota: null,
					aliases: []
				}),
				entry(5, 't2', 'budget_line', 1, 'create', null, {
					id: 1,
					category: 2,
					description: 'Beer',
					estimate: { type: 'quantity', quantity: 40, unitPrice: 1250 },
					costCentre: null,
					complete: false
				}),
				...setup
			],
			'en'
		);
		expect(changes[0].rows[0].fields).toContainEqual({
			label: 'Fee',
			before: null,
			after: '2.5 %'
		});
		expect(changes[1].rows[0].fields).toContainEqual({
			label: 'Estimate',
			before: null,
			after: '40 × €12.50'
		});
	});

	it('names a plugin and shows the values of its record', () => {
		const [change] = historyChanges(
			[
				entry(
					1,
					't1',
					'plugin_record',
					3,
					'update',
					{
						id: 3,
						plugin: 'drinks',
						key: 'count:5',
						scope: 'records',
						value: { taken: 96, leftOver: 6 }
					},
					{
						id: 3,
						plugin: 'drinks',
						key: 'count:5',
						scope: 'records',
						value: { taken: 96, leftOver: 7 }
					}
				)
			],
			'de',
			{ drinks: 'Stock and sales' }
		);
		expect([change.rows[0].noun, change.rows[0].name]).toEqual(['Stock and sales', 'count:5']);
		expect(change.rows[0].fields).toEqual([{ label: 'Left over', before: '6', after: '7' }]);
	});

	it('still shows an entity it does not know', () => {
		const [change] = historyChanges(
			[entry(1, 't1', 'new_thing', 3, 'create', null, { name: 'A', size: 2 })],
			'de'
		);
		expect([change.rows[0].noun, change.rows[0].name]).toEqual(['New thing', '#3']);
		expect(change.rows[0].fields.map((f) => f.label)).toEqual(['Name', 'Size']);
	});

	it('leaves out an edit nothing shown has moved in', () => {
		const a = { ...txn('Bakery', [line(1, -2000), line(2, 2000)]), origin: { type: 'manual' } };
		const b = { ...a, origin: { type: 'ticketSale', sale: 1 } };
		const changes = historyChanges([entry(5, 't2', 'txn', 7, 'update', a, b), ...setup], 'de');
		expect(changes.map((c) => c.at)).toEqual(['t1']);
	});
});

describe('matchingRows', () => {
	const [change] = historyChanges(setup, 'de');

	it('finds names and old values, and everything without a query', () => {
		expect(matchingRows(change, ' ')).toHaveLength(3);
		expect(matchingRows(change, 'kitch').map((r) => r.name)).toEqual(['Kitchen']);
		expect(matchingRows(change, 'nothing like this')).toEqual([]);
	});
});

describe('local time', () => {
	it('reads a timestamp in the local time zone', () => {
		const at = new Date(2026, 5, 13, 8, 5).toISOString();
		expect(localDay(at)).toBe('2026-06-13');
		expect(localTime(at)).toBe('08:05');
	});
});
