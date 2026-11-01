import { describe, expect, it } from 'vitest';

import { matchPull } from './pull';
import { allProducts, type Product, setProduct } from './records';

const mate: Product = { id: 1, centre: 1, name: 'Club-Mate', cost: 100, donation: 200 };
const beer: Product = { ...mate, id: 2, name: 'Beer', aliases: ['Pils'] };

describe('Fitting a pull onto the products', () => {
	it('adds up by name, ignoring case and through other names', () => {
		const { totals, unknown } = matchPull(
			[mate, beer],
			[
				{ name: ' club-mate ', quantity: '3' },
				{ name: 'Beer', quantity: '2' },
				{ name: 'pils', quantity: '5' }
			]
		);
		expect(totals.get(1)).toBe(3);
		expect(totals.get(2)).toBe(7);
		expect(unknown).toEqual([]);
	});

	it('holds on to names it does not know, with their totals', () => {
		const { totals, unknown } = matchPull(
			[mate],
			[
				{ name: 'Radler', quantity: '1' },
				{ name: 'radler', quantity: '4' }
			]
		);
		expect(totals.size).toBe(0);
		expect(unknown).toEqual([{ name: 'Radler', quantity: 5 }]);
	});

	it('lists a quantity that is not a whole number', () => {
		for (const quantity of ['-1', '1.5', 'many', '1000001']) {
			const { totals, unreadable } = matchPull([mate], [{ name: 'Club-Mate', quantity }]);
			expect(totals.size).toBe(0);
			expect(unreadable).toEqual([{ name: 'Club-Mate', quantity }]);
		}
	});

	it('keeps other names with the product, and reads a product without any', () => {
		const record = (value: unknown) => ({
			id: 1,
			plugin: 'drinks',
			key: setProduct(beer).key,
			scope: 'setup' as const,
			value
		});
		expect(allProducts([record(setProduct(beer).value)])[0]?.aliases).toEqual(['Pils']);
		expect(allProducts([record(setProduct(mate).value)])[0]?.aliases).toBeUndefined();
	});
});
