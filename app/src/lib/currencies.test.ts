import { describe, expect, it } from 'vitest';

import { CURRENCY_OPTIONS, currencyLabel } from './currencies';

describe('CURRENCY_OPTIONS', () => {
	it('lists what can be chosen first, and the rest as unsupported', () => {
		const firstUnsupported = CURRENCY_OPTIONS.findIndex((o) => o.disabled);
		expect(CURRENCY_OPTIONS.slice(0, firstUnsupported).map((o) => o.value)).toEqual([
			'EUR',
			'USD',
			'GBP',
			'CHF'
		]);
		const rest = CURRENCY_OPTIONS.slice(firstUnsupported);
		expect(rest.length).toBeGreaterThan(100);
		expect(rest.every((o) => o.disabled && o.group === 'Unsupported')).toBe(true);
	});

	it('has each code once', () => {
		const codes = CURRENCY_OPTIONS.map((o) => o.value);
		expect(new Set(codes).size).toBe(codes.length);
	});

	it('names a currency', () => {
		expect(currencyLabel('CHF')).toBe('CHF · Swiss franc');
	});
});
