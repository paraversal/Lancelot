import { describe, expect, it } from 'vitest';

import { feeInput, feePerTicket, formatFee, parseCount, parseFee } from './tickets';

describe('fees', () => {
	it('read a percentage in either format as basis points', () => {
		expect(parseFee('2,5')).toBe(250);
		expect(parseFee('2.5 %')).toBe(250);
		expect(parseFee('0,25')).toBe(25);
		expect(parseFee('10')).toBe(1000);
		expect(parseFee('')).toBe(0);
		expect(parseFee('2,555')).toBeNull();
		expect(parseFee('-1')).toBeNull();
		expect(parseFee('1.000')).toBeNull();
	});

	it('show without trailing zeros', () => {
		expect(formatFee(250, 'de')).toBe('2,5 %');
		expect(formatFee(250, 'en')).toBe('2.5%');
		expect(formatFee(0, 'de')).toBe('0 %');
		expect(feeInput(1025, 'de')).toBe('10,25');
	});

	it('round half up per ticket', () => {
		expect(feePerTicket(3200, 250)).toBe(80);
		expect(feePerTicket(1850, 250)).toBe(46); // 46,25 ct
		expect(feePerTicket(1000, 25)).toBe(3); // 2,5 ct
	});
});

describe('parseCount', () => {
	it('takes digits only', () => {
		expect(parseCount(' 12 ')).toBe(12);
		expect(parseCount('')).toBeNull();
		expect(parseCount('1,5')).toBeNaN();
	});
});
