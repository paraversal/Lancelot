import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '../../api/index';
import example from '../../api/fixtures/example-event.json';
import type { EventSnapshot } from '../../api/generated/EventSnapshot';
import type { TierFigures } from '../../api/generated/TierFigures';
import { session } from '../../state/session.svelte';
import Budget from '../../../routes/event/budget/+page.svelte';
import {
	categoryKnobs,
	categoryValue,
	moreOf,
	netPrice,
	NOTHING,
	outcome,
	tierKnobs,
	tierPrice
} from './scenario';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

const snapshot = example as unknown as EventSnapshot;

function open(change: (s: EventSnapshot) => void = () => {}) {
	const copy = structuredClone(snapshot);
	change(copy);
	session.update({
		revision: Date.now(),
		path: '/x.lancelot',
		fileName: 'x.lancelot',
		snapshot: copy,
		integrity: { issues: [] },
		backupError: null
	});
}

const tier = (over: Partial<TierFigures>) =>
	({
		id: 1,
		name: 'Regular',
		price: 3200,
		feeBp: 250,
		quota: 100,
		places: 60,
		...over
	}) as TierFigures;

describe('A scenario', () => {
	it('changes nothing until a slider moves', () => {
		const tiers = tierKnobs(snapshot.tickets.tiers);
		const categories = categoryKnobs(snapshot.budget, snapshot.accounts);
		expect(outcome(1000, tiers, categories, NOTHING)).toBe(1000);
	});

	it('sells more tickets at their price less the fee, only up to the quota', () => {
		const [knob] = tierKnobs([tier({})]);
		expect(knob.more).toEqual({ min: 0, max: 40 });
		expect(netPrice(3200, 250)).toBe(3120);
		expect(outcome(0, [knob], [], { ...NOTHING, more: { 1: 10 } })).toBe(31200);
		expect(moreOf(knob, { ...NOTHING, more: { 1: 500 } })).toBe(40);
	});

	it('prices only the tickets still to sell, and never below zero', () => {
		const [knob] = tierKnobs([tier({ feeBp: 0 })]);
		expect(outcome(0, [knob], [], { ...NOTHING, price: { 1: 5 } })).toBe(0);
		expect(outcome(0, [knob], [], { ...NOTHING, more: { 1: 2 }, price: { 1: 5 } })).toBe(7400);
		expect(tierPrice(knob, { ...NOTHING, price: { 1: -1000 } })).toBe(0);
	});

	it('guesses a range for a tier without a quota, and has none for a full one', () => {
		expect(tierKnobs([tier({ quota: null, places: 10 })])[0].more.max).toBe(50);
		expect(tierKnobs([tier({ quota: null, places: 80 })])[0].more.max).toBe(80);
		expect(tierKnobs([tier({ quota: 60 })])[0].more.max).toBe(0);
		expect(tierKnobs([tier({ quota: 50 })])[0].more.max).toBe(0);
	});

	it('leaves out ticket categories and those with no forecast', () => {
		const names = categoryKnobs(snapshot.budget, snapshot.accounts).map((k) => k.name);
		expect(names).not.toContain('Tickets');
		expect(names).toContain('Venue');
		for (const k of categoryKnobs(snapshot.budget, snapshot.accounts)) {
			expect(k.forecast).toBeGreaterThan(0);
		}
	});

	it('cannot take a category below what is booked', () => {
		const budget = {
			...snapshot.budget,
			categories: [{ category: 7, estimate: 50000, actual: 30000, forecast: 50000, unbudgeted: 0 }]
		};
		const accounts = [{ id: 7, name: 'Kitchen', kind: 'expense', role: null }] as never;
		const [knob] = categoryKnobs(budget, accounts);
		expect(knob.step).toBe(500);
		expect(knob.range.min).toBe(-40);
		expect(categoryValue(knob, { ...NOTHING, category: { 7: -999 } })).toBe(30000);
		// An expense going up takes from the result; income going up adds to it.
		expect(outcome(100, [], [knob], { ...NOTHING, category: { 7: 2 } })).toBe(100 - 1000);
		expect(
			outcome(100, [], [{ ...knob, kind: 'income' }], { ...NOTHING, category: { 7: 2 } })
		).toBe(100 + 1000);
	});
});

describe('The What if section', () => {
	beforeEach(() => vi.restoreAllMocks());

	const money = (id: string) => screen.getByTestId(id).textContent?.replace(/\s+/g, ' ').trim();

	it('starts at the forecast and shows no sliders', () => {
		open();
		render(Budget);
		expect(money('plugin-what-if-result')).toBe('+1.765,00 €');
		expect(screen.queryByRole('slider')).toBeNull();
		expect(screen.queryByTestId('plugin-what-if-change')).toBeNull();
	});

	it('follows a slider, resets, and writes nothing to the file', async () => {
		open();
		const apply = vi.spyOn(api, 'applyPluginRecords');
		render(Budget);
		await fireEvent.click(screen.getByRole('button', { name: 'Adjust' }));
		const venue = screen.getByRole<HTMLInputElement>('slider', { name: /Venue/ });
		await fireEvent.input(venue, { target: { value: '10' } });
		// Venue moves in steps of 10 €, and an expense going up lowers the result.
		expect(money('plugin-what-if-result')).toBe('+1.665,00 €');
		expect(money('plugin-what-if-change')).toContain('−100,00 €');

		await fireEvent.click(screen.getByRole('button', { name: 'Reset' }));
		expect(money('plugin-what-if-result')).toBe('+1.765,00 €');
		expect(apply).not.toHaveBeenCalled();
	});

	it('is gone from a closed event and when switched off', () => {
		open((s) => (s.event.closedAt = '2026-06-20T10:00:00Z'));
		const closed = render(Budget);
		expect(screen.queryByTestId('plugin-what-if-result')).toBeNull();
		closed.unmount();

		open((s) =>
			s.pluginRecords.push({
				id: 999,
				plugin: 'what-if',
				key: 'host:off',
				scope: 'setup',
				value: true
			})
		);
		render(Budget);
		expect(screen.queryByTestId('plugin-what-if-result')).toBeNull();
	});
});
