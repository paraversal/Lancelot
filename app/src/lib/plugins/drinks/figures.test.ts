import { describe, expect, it } from 'vitest';

import type { PluginRecord, TxnView } from '../api';
import { computeFigures, loggedOf } from './figures';
import {
	allProducts,
	centreSettings,
	type Counts,
	countsOf,
	nextProductId,
	NO_COUNTS,
	type Product
} from './records';

const CENTRE = 1;
const DEPOSIT = 9;
const DRINKS = 8;

const item = (over: Partial<Product> & Partial<Counts>) => {
	const { taken, returned, leftOver, manual, imported, ...product } = over;
	return {
		product: { id: 1, centre: CENTRE, name: 'Pils', cost: 100, donation: 200, ...product },
		counts: {
			...NO_COUNTS,
			...(taken !== undefined && { taken }),
			...(returned !== undefined && { returned }),
			...(leftOver !== undefined && { leftOver }),
			...(manual !== undefined && { manual }),
			...(imported !== undefined && { imported })
		}
	};
};

const settings = { supplier: 'Hoffmann', commission: true, deposit: DEPOSIT };

const invoice = (
	counterparty: string | null,
	lines: { account: number; amount: number; costCentre: number | null }[],
	kind = 'expense'
) => ({ kind, counterparty, lines }) as unknown as TxnView;

describe('The drink figures', () => {
	it('splits what is billed into left over, logged and unaccounted', () => {
		const f = computeFigures(
			CENTRE,
			settings,
			[item({ taken: 240, returned: 96, leftOver: 10, manual: 131, cost: 115, donation: 150 })],
			[]
		);
		const [row] = f.rows;
		expect([row.billed, row.leftOver, row.gone, row.logged, row.gap]).toEqual([
			144, 10, 134, 131, 3
		]);
		expect(row.cost).toBe(16560);
		expect(row.expected).toBe(19650);
	});

	it('bills everything taken when the goods were bought outright', () => {
		const bought = { ...settings, commission: false };
		const f = computeFigures(CENTRE, bought, [item({ taken: 24, returned: 6, cost: 100 })], []);
		expect([f.billed, f.cost]).toEqual([24, 2400]);
	});

	it('counts a blank left over as none', () => {
		const f = computeFigures(CENTRE, settings, [item({ taken: 10 })], []);
		expect(f.gone).toBe(10);
		expect(f.rows[0].gap).toBeNull();
	});

	it('has no gap without a log, and none for a tally without stock', () => {
		const tally = item({ name: 'Tally', manual: 700, cost: 0, donation: 50 });
		const f = computeFigures(CENTRE, settings, [item({ id: 2, taken: 10 }), tally], []);
		expect(f.rows.map((r) => r.gap)).toEqual([null, null]);
		// The money can still be compared: 700 × 0,50 €.
		expect(f.expected).toBe(35000);
		expect(f.cost).toBe(1000);
		expect(f.gap).toBe(0);
	});

	it('has no expected money without a donation price', () => {
		const f = computeFigures(CENTRE, settings, [item({ taken: 5, manual: 5, donation: null })], []);
		expect(f.rows[0].expected).toBeNull();
		expect(f.expected).toBeNull();
	});

	it('adds up imported and typed-in figures, and never goes below zero', () => {
		expect(loggedOf(NO_COUNTS)).toBeNull();
		expect(loggedOf({ ...NO_COUNTS, imported: 10 })).toBe(10);
		expect(loggedOf({ ...NO_COUNTS, imported: 10, manual: -3 })).toBe(7);
		expect(loggedOf({ ...NO_COUNTS, manual: -3 })).toBe(0);
		expect(loggedOf({ ...NO_COUNTS, manual: 0 })).toBe(0);
	});

	it('leaves left-over stock out of the cost per unit', () => {
		const stock = [
			item({ id: 1, taken: 10, leftOver: 5, cost: 100 }),
			item({ id: 2, taken: 5, cost: 200 })
		];
		const f = computeFigures(CENTRE, settings, stock, []);
		// 5 × 1,00 € and 5 × 2,00 € are gone; the 5 left over are billed, but not drunk.
		expect(f.costPerUnit).toBe(150);
		expect(f.cost).toBe(2000);
		const none = computeFigures(CENTRE, settings, [item({ taken: 5, leftOver: 5 })], []);
		expect(none.costPerUnit).toBeNull();
	});

	it('rounds the cost per unit half up', () => {
		const stock = [item({ id: 1, taken: 1, cost: 100 }), item({ id: 2, taken: 1, cost: 101 })];
		expect(computeFigures(CENTRE, settings, stock, []).costPerUnit).toBe(101);
	});
});

describe('The invoice check', () => {
	const stock = [item({ taken: 4, cost: 100 })];

	it('compares the invoice without deposit with what is billed', () => {
		const txns = [
			invoice(' hoffmann ', [
				{ account: DRINKS, amount: 400, costCentre: CENTRE },
				{ account: DEPOSIT, amount: 32, costCentre: CENTRE }
			])
		];
		expect(computeFigures(CENTRE, settings, stock, txns).invoice).toEqual({
			amount: 432,
			deposit: 32,
			difference: 0
		});
	});

	it('shows by how much an invoice differs', () => {
		const txns = [invoice('Hoffmann', [{ account: DRINKS, amount: 450, costCentre: CENTRE }])];
		expect(computeFigures(CENTRE, settings, stock, txns).invoice?.difference).toBe(50);
	});

	it('takes only the supplier’s expenses tagged with this cost centre', () => {
		const line = { account: DRINKS, amount: 400, costCentre: CENTRE };
		const noCheck = (txns: TxnView[]) => computeFigures(CENTRE, settings, stock, txns).invoice;
		expect(noCheck([invoice('Aral', [line])])).toBeNull();
		expect(noCheck([invoice('Hoffmann', [{ ...line, costCentre: null }])])).toBeNull();
		expect(noCheck([invoice('Hoffmann', [{ ...line, costCentre: 2 }])])).toBeNull();
		expect(noCheck([invoice('Hoffmann', [line], 'income')])).toBeNull();
		const noSupplier = { supplier: null, commission: true, deposit: null };
		expect(computeFigures(CENTRE, noSupplier, stock, [invoice(null, [line])]).invoice).toBeNull();
	});

	it('gives no false warning without product records, which are optional', () => {
		const txns = [invoice('Hoffmann', [{ account: DRINKS, amount: 400, costCentre: CENTRE }])];
		const check = computeFigures(CENTRE, settings, [], txns).invoice;
		expect(check?.amount).toBe(400);
		expect(check?.difference).toBeNull();
	});
});

describe('The stored records', () => {
	const record = (key: string, value: unknown): PluginRecord => ({
		id: 1,
		plugin: 'drinks',
		key,
		scope: 'setup',
		value
	});

	it('reads what was written', () => {
		const records = [
			// Written before goods could be bought outright, so on commission.
			record('centre:1', { supplier: 'Hoffmann', deposit: 9 }),
			record('centre:2', { supplier: null, commission: false, deposit: null }),
			record('product:3', { centre: 1, name: 'Pils', cost: 95, donation: null }),
			record('count:3', { taken: 5, returned: 1, leftOver: null, manual: -2, imported: null })
		];
		expect(centreSettings(records, 1)).toEqual({
			supplier: 'Hoffmann',
			commission: true,
			deposit: 9
		});
		expect(centreSettings(records, 2)?.commission).toBe(false);
		expect(centreSettings(records, 4)).toBeNull();
		expect(allProducts(records)).toEqual([
			{ id: 3, centre: 1, name: 'Pils', cost: 95, donation: null }
		]);
		expect(countsOf(records, 3).manual).toBe(-2);
		expect(nextProductId(records)).toBe(4);
	});

	it('ignores a record that does not fit', () => {
		const records = [
			record('product:1', { centre: 1, name: 'No cost' }),
			record('product:x', { centre: 1, name: 'Bad id', cost: 1, donation: null }),
			record('product:2', { centre: 1, name: 'Negative', cost: -5, donation: null }),
			record('count:1', { taken: -1, returned: 0, leftOver: null, manual: null, imported: null }),
			record('something-else', [1, 2])
		];
		expect(allProducts(records)).toEqual([]);
		expect(countsOf(records, 1)).toEqual(NO_COUNTS);
		expect(nextProductId(records)).toBe(1);
	});
});
