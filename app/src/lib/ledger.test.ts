import { describe, expect, it } from 'vitest';

import type { EventSnapshot } from './api/generated/EventSnapshot';
import example from './api/mock/example-event.json';
import { byCategory, byDay, counts, matches, rows } from './ledger';

// The example event from the mockups, exported by the Rust tests.
const snapshot = example as EventSnapshot;
const all = rows(snapshot);
const find = (title: string) => all.find((r) => r.title === title)!;

describe('rows', () => {
	it('names who paid and what for', () => {
		const metro = find('Großmarkt');
		expect(metro.who).toBe('Cash box');
		expect(metro.categories.map((c) => c.name)).toEqual(['Kitchen', 'General']);
		expect(metro.flow).toBe('out');
		expect(find('Tickets, paid to Bob').who).toBe('Bob');
		expect(find('Reimbursement Anna').who).toBe('Anna');
		expect(find('Change money').flow).toBe('moved');
	});

	it('knows which expenses have no receipt', () => {
		expect(find('Cups and ice').receipt).toBe('missing');
		expect(find('Bakery').receipt).toBe('attached');
		expect(find('Drink donations').receipt).toBe('none');
	});
});

describe('filters and search', () => {
	it('counts each filter', () => {
		expect(counts(all)).toEqual({ all: 19, expense: 12, income: 4, transfer: 3, missing: 1 });
	});

	it('searches titles, people, categories and amounts, all words required', () => {
		const titles = (q: string) => all.filter((r) => matches(r, 'all', q, 'de')).map((r) => r.title);
		expect(titles('anna')).toEqual(['Groceries', 'Change money', 'Reimbursement Anna']);
		expect(titles('kitchen cash')).toEqual(['Großmarkt', 'Bakery']);
		expect(titles('391,25')).toEqual(['Großmarkt']);
		expect(titles('nothing like this')).toEqual([]);
	});
});

describe('grouping', () => {
	it('by day puts the newest first', () => {
		const days = byDay(all);
		expect(days.map((d) => d.key)).toEqual(['2026-06-13']);
		expect(days[0].items[0].row.title).toBe('Reimbursement Clara');
	});

	it('by category shows a split under each category with its line amount', () => {
		const groups = byCategory(all, snapshot);
		const kitchen = groups.find((g) => g.label === 'Kitchen')!;
		expect(kitchen.total).toBe(210000);
		const metro = kitchen.items.find((i) => i.row.title === 'Großmarkt')!;
		expect(metro).toMatchObject({ amount: 34275, split: true });
		const general = groups.find((g) => g.label === 'General' && g.account?.kind === 'expense')!;
		expect(general.items.find((i) => i.row.title === 'Großmarkt')!.amount).toBe(4850);
		expect(groups.at(-1)!.label).toBe('Transfers');
		expect(groups.at(-1)!.items.map((i) => i.row.title)).toEqual([
			'Reimbursement Clara',
			'Reimbursement Anna',
			'Change money'
		]);
	});
});
