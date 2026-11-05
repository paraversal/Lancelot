import { describe, expect, it } from 'vitest';

import amountCases from '../../../fixtures/amount-format.json';
import {
	differenceText,
	formatAmount,
	formatDate,
	formatDateRange,
	formatPlain,
	formatSize,
	formatTimestamp,
	setEventCurrency,
	todayISO
} from './format';
import type { Currency } from './api/generated/Currency';
import type { NumberFormat } from './api/generated/NumberFormat';
import type { SignStyle } from './api/generated/SignStyle';

describe('formatAmount', () => {
	it('matches the shared fixture', () => {
		expect(amountCases.length).toBeGreaterThanOrEqual(100);
		for (const c of amountCases) {
			const currency = ('currency' in c ? c.currency : 'EUR') as Currency;
			expect(
				formatAmount(c.cents, c.format as NumberFormat, c.sign as SignStyle, currency),
				String(c.cents)
			).toBe(c.text);
		}
	});

	it('defaults to a minus for negative amounts only', () => {
		expect(formatAmount(-31240, 'de')).toBe('−312,40 €');
		expect(formatAmount(31240, 'en')).toBe('€312.40');
	});

	it('uses the currency of the open event', () => {
		setEventCurrency('USD');
		expect(formatAmount(31240, 'en')).toBe('$312.40');
		setEventCurrency('EUR');
	});

	it('leaves the symbol out when asked to', () => {
		expect(formatPlain(-123456, 'de')).toBe('−1.234,56');
		expect(formatPlain(123456, 'en')).toBe('1,234.56');
	});
});

describe('formatDate', () => {
	it('formats both styles', () => {
		expect(formatDate('2026-06-03', 'de')).toBe('03.06.2026');
		expect(formatDate('2026-06-03', 'en')).toBe('2026-06-03');
	});
});

describe('todayISO', () => {
	it('is the local calendar date', () => {
		expect(todayISO(new Date(2026, 10, 5, 0, 30))).toBe('2026-11-05');
		expect(todayISO(new Date(2026, 11, 31, 23, 59))).toBe('2026-12-31');
	});
});

describe('formatDateRange', () => {
	it('shortens the shared parts in De', () => {
		expect(formatDateRange('2026-06-12', '2026-06-14', 'de')).toBe('12.–14.06.2026');
		expect(formatDateRange('2026-06-28', '2026-07-02', 'de')).toBe('28.06.–02.07.2026');
		expect(formatDateRange('2026-12-30', '2027-01-02', 'de')).toBe('30.12.2026–02.01.2027');
		expect(formatDateRange('2026-06-12', '2026-06-12', 'de')).toBe('12.06.2026');
	});

	it('handles En and missing dates', () => {
		expect(formatDateRange('2026-06-12', '2026-06-14', 'en')).toBe('2026-06-12 – 2026-06-14');
		expect(formatDateRange('2026-06-12', null, 'de')).toBe('12.06.2026');
		expect(formatDateRange(null, null, 'de')).toBe('');
	});
});

describe('formatTimestamp and formatSize', () => {
	it('shows today as Today', () => {
		const now = new Date(2026, 5, 14, 20, 0);
		expect(formatTimestamp(new Date(2026, 5, 14, 18, 2).toISOString(), 'de', now)).toBe(
			'Today 18:02'
		);
		expect(formatTimestamp(new Date(2026, 5, 13, 23, 10).toISOString(), 'de', now)).toBe(
			'13.06.2026 23:10'
		);
	});

	it('formats sizes in the number format', () => {
		expect(formatSize(2_412_000, 'de')).toBe('2,4 MB');
		expect(formatSize(2_412_000, 'en')).toBe('2.4 MB');
		expect(formatSize(512, 'en')).toBe('512 B');
	});
});

describe('differenceText', () => {
	it('says which way a count is off', () => {
		expect(differenceText(-290, 'de')).toBe('2,90 € less than recorded');
		expect(differenceText(750, 'en')).toBe('€7.50 more than recorded');
	});
});
