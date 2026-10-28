import { describe, expect, it } from 'vitest';

import amountCases from '../../../fixtures/amount-format.json';
import { formatAmount, formatDate } from './format';
import type { NumberFormat } from './api/generated/NumberFormat';
import type { SignStyle } from './api/generated/SignStyle';

describe('formatAmount', () => {
	it('matches the shared fixture', () => {
		expect(amountCases.length).toBeGreaterThanOrEqual(100);
		for (const c of amountCases) {
			expect(
				formatAmount(c.cents, c.format as NumberFormat, c.sign as SignStyle),
				String(c.cents)
			).toBe(c.text);
		}
	});

	it('defaults to a minus for negative amounts only', () => {
		expect(formatAmount(-31240, 'de')).toBe('−312,40 €');
		expect(formatAmount(31240, 'en')).toBe('€312.40');
	});
});

describe('formatDate', () => {
	it('formats both styles', () => {
		expect(formatDate('2026-06-03', 'de')).toBe('03.06.2026');
		expect(formatDate('2026-06-03', 'en')).toBe('2026-06-03');
	});
});
