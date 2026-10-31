import { describe, expect, it } from 'vitest';

import type { SalesPoint } from './api/generated/SalesPoint';
import type { TierFigures } from './api/generated/TierFigures';
import type { Tickets } from './api/generated/Tickets';
import {
	conflictCount,
	feeInput,
	formatFee,
	labelStep,
	parseCount,
	parseFee,
	salesBars,
	totalQuota
} from './tickets';

describe('conflicts', () => {
	it('count per order and tier', () => {
		const removed = (tier: number) => ({ tier, count: 2, since: 1, matches: [] });
		const tickets = {
			conflicts: [
				{ order: '1', removed: [removed(1), removed(2)], added: [] },
				{ order: '2', removed: [removed(1)], added: [] }
			]
		} as unknown as Tickets;
		expect(conflictCount(tickets)).toBe(3);
	});
});

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
});

describe('parseCount', () => {
	it('takes digits only', () => {
		expect(parseCount(' 12 ')).toBe(12);
		expect(parseCount('')).toBeNull();
		expect(parseCount('1,5')).toBeNaN();
	});
});

describe('sales chart', () => {
	const figures = (sold: number, places: number, net: number) => ({
		sheet: sold,
		cancelled: sold - places,
		manual: 0,
		sold,
		places,
		unresolved: 0,
		gross: net,
		fees: 0,
		refunds: 0,
		net
	});
	const point = {
		date: '2026-06-09',
		import: 1,
		tiers: [
			{ tier: 1, ...figures(10, 8, 25000) },
			{ tier: 2, ...figures(5, 5, -80) }
		],
		total: figures(15, 13, 24920)
	} as SalesPoint;

	it('stacks places by tier under what was cancelled', () => {
		const [bar] = salesBars([point], 'tickets');
		expect(bar.value).toBe(13);
		expect(bar.segments.map((s) => s.value)).toEqual([8, 5]);
		expect(bar.cancelled).toBe(2);
		expect(bar.size).toBe(15);
	});

	it('stacks net income by tier and draws nothing below zero', () => {
		const [bar] = salesBars([point], 'revenue');
		expect(bar.value).toBe(24920);
		expect(bar.segments.map((s) => s.value)).toEqual([25000, 0]);
		expect(bar.cancelled).toBe(0);
		expect(bar.size).toBe(25000);
	});

	it('has a quota line only when every tier has a quota', () => {
		const tier = (quota: number | null) => ({ quota }) as TierFigures;
		expect(totalQuota([tier(60), tier(200)])).toBe(260);
		expect(totalQuota([tier(60), tier(null)])).toBeNull();
		expect(totalQuota([])).toBeNull();
	});

	it('thins the date labels when they do not fit', () => {
		expect(labelStep(6, 355)).toBe(1);
		expect(labelStep(30, 355)).toBe(5);
		expect(labelStep(1, 10)).toBe(1);
	});
});
