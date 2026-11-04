import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '../../api/index';
import example from '../../api/fixtures/example-event.json';
import type { EventSnapshot } from '../../api/generated/EventSnapshot';
import type { TierFigures } from '../../api/generated/TierFigures';
import { session } from '../../state/session.svelte';
import Budget from '../../../routes/event/budget/+page.svelte';
import {
	bridge,
	categoryKnobs,
	categoryValue,
	changes,
	moreOf,
	netPrice,
	NOTHING,
	outcome,
	places,
	isPlanning,
	reach,
	scaledKnobs,
	snapCategory,
	startOf,
	tierKnobs,
	tierPrice,
	undo,
	type CategoryKnob
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

const kitchen = (over: Partial<CategoryKnob> = {}): CategoryKnob => {
	const budget = {
		...snapshot.budget,
		categories: [{ category: 7, estimate: 50000, actual: 30000, forecast: 50000, unbudgeted: 0 }]
	};
	const accounts = [{ id: 7, name: 'Kitchen', kind: 'expense', role: null }] as never;
	return { ...categoryKnobs(budget, accounts)[0], ...over };
};

describe('A scenario', () => {
	it('changes nothing until something moves', () => {
		const tiers = tierKnobs(snapshot.tickets.tiers);
		const categories = categoryKnobs(snapshot.budget, snapshot.accounts);
		expect(outcome(1000, tiers, categories, NOTHING)).toBe(1000);
		expect(changes(tiers, categories, NOTHING)).toEqual([]);
	});

	it('sells more tickets at their price less the fee, only up to the quota', () => {
		const [knob] = tierKnobs([tier({})]);
		expect(knob.maxMore).toBe(40);
		expect(netPrice(3200, 250)).toBe(3120);
		expect(outcome(0, [knob], [], { ...NOTHING, more: { 1: 10 } })).toBe(31200);
		expect(moreOf(knob, { ...NOTHING, more: { 1: 500 } })).toBe(40);
	});

	it('prices only the tickets still to sell, and never below zero', () => {
		const [knob] = tierKnobs([tier({ feeBp: 0 })]);
		expect(outcome(0, [knob], [], { ...NOTHING, price: { 1: 3700 } })).toBe(0);
		const both = { ...NOTHING, more: { 1: 2 }, price: { 1: 3700 } };
		expect(outcome(0, [knob], [], both)).toBe(7400);
		// The bridge tells the tickets from what their new price adds.
		expect(bridge(0, [knob], [], both)).toMatchObject({ tickets: 6400, prices: 1000 });
		expect(tierPrice(knob, { ...NOTHING, price: { 1: -1000 } })).toBe(0);
	});

	it('guesses a range for a tier without a quota, and has none for a full one', () => {
		expect(tierKnobs([tier({ quota: null, places: 10 })])[0].maxMore).toBe(50);
		expect(tierKnobs([tier({ quota: null, places: 80 })])[0].maxMore).toBe(80);
		expect(tierKnobs([tier({ quota: 60 })])[0].maxMore).toBe(0);
		expect(tierKnobs([tier({ quota: 50 })])[0].maxMore).toBe(0);
	});

	it('draws a place per ticket, and a box for several in a big tier', () => {
		const [small] = tierKnobs([tier({})]);
		expect(places(small, 5)).toEqual({ per: 1, boxes: 100, sold: 60, added: 5 });
		const [big] = tierKnobs([tier({ quota: 1000, places: 400 })]);
		expect(places(big, 10)).toEqual({ per: 4, boxes: 250, sold: 100, added: 3 });
	});

	it('leaves out ticket categories and those with no forecast', () => {
		const names = categoryKnobs(snapshot.budget, snapshot.accounts).map((k) => k.name);
		expect(names).not.toContain('Tickets');
		expect(names).toContain('Venue');
		for (const k of categoryKnobs(snapshot.budget, snapshot.accounts)) {
			expect(k.forecast).toBeGreaterThan(0);
		}
	});

	it('cannot take a category below what is booked, and moves it in steps', () => {
		const knob = kitchen();
		expect(knob.step).toBe(500);
		expect(knob.min).toBe(30000);
		expect(categoryValue(knob, { ...NOTHING, category: { 7: 0 } })).toBe(30000);
		expect(snapCategory(knob, 51234)).toBe(51000);
		expect(snapCategory(knob, 10)).toBe(30000);
		// An expense going up takes from the result; income going up adds to it.
		expect(outcome(100, [], [knob], { ...NOTHING, category: { 7: 51000 } })).toBe(100 - 1000);
		expect(
			outcome(100, [], [{ ...knob, kind: 'income' }], { ...NOTHING, category: { 7: 51000 } })
		).toBe(100 + 1000);
	});

	it('lists its changes, and undoes one at a time', () => {
		const [knob] = tierKnobs([tier({})]);
		const s = { ...NOTHING, more: { 1: 3 }, price: { 1: 3500 }, category: { 7: 51000 } };
		expect(changes([knob], [kitchen()], s).map((c) => [c.part, c.value])).toEqual([
			['more', 3],
			['price', 3500],
			['category', 1000]
		]);
		expect(undo(s, 'price', 1)).toEqual({ ...s, price: {} });
	});
});

describe('Reaching a target', () => {
	const [regular] = tierKnobs([tier({ feeBp: 0 })]);

	it('suggests nothing once the target is reached', () => {
		expect(reach(0, [regular], [kitchen()], NOTHING)).toEqual([]);
	});

	it('puts what gets there alone first, the smallest change first', () => {
		const out = reach(9000, [regular], [kitchen()], NOTHING);
		expect(out.map((s) => [s.kind, s.covers, s.gain])).toEqual([
			['cut', true, 9000],
			['tickets', true, 9600]
		]);
		const tickets = out.find((s) => s.kind === 'tickets')!;
		expect(tickets.kind === 'tickets' && tickets.count).toBe(3);
		expect(tickets.scenario.more).toEqual({ 1: 3 });
	});

	it('asks for a higher price when the places left are too few', () => {
		const out = reach(140000, [regular], [], NOTHING);
		const sell = out.find((s) => s.kind === 'tickets')!;
		expect(sell.covers).toBe(false);
		expect(sell.gain).toBe(40 * 3200);
		const price = out.find((s) => s.kind === 'price')!;
		expect(price.kind === 'price' && price.price).toBe(3500);
		expect(price.scenario).toMatchObject({ more: { 1: 40 }, price: { 1: 3500 } });
		expect(out[0]).toBe(price);
	});

	it('cuts no further than what is booked', () => {
		const [cut] = reach(50000, [], [kitchen()], NOTHING);
		expect(cut).toMatchObject({ kind: 'cut', by: 20000, to: 30000, covers: false });
	});
});

describe('Assumptions in a scenario', () => {
	const accounts = [
		{ id: 4, name: 'Tickets', kind: 'income', role: 'tickets' },
		{ id: 6, name: 'Kitchen', kind: 'expense', role: null }
	] as never;
	const lines = [
		{
			id: 1,
			category: 4,
			description: 'Tickets',
			estimate: { type: 'quantity', quantity: 1, unitPrice: 2000, per: [1] },
			complete: false,
			costCentre: null
		},
		{
			id: 2,
			category: 6,
			description: 'Kitchen',
			estimate: { type: 'quantity', quantity: 1, unitPrice: 400, per: [1] },
			complete: false,
			costCentre: null
		}
	] as never;
	const guests = [{ id: 1, name: 'Guests', value: 150 }];
	const figures = (line: number, estimate: number, actual: number, forecast: number) => ({
		line,
		estimate,
		actual,
		forecast,
		fromPlan: 0
	});
	// Nothing booked: the forecast leaves out the ticket income, the plan doesn't.
	const planned = {
		...snapshot.budget,
		income: { estimate: 300000, actual: 0, forecast: 0 },
		expense: { estimate: 60000, actual: 0, forecast: 60000 },
		result: { estimate: 240000, actual: 0, forecast: -60000 },
		lines: [figures(1, 300000, 0, 0), figures(2, 60000, 0, 60000)],
		categories: [
			{ category: 4, estimate: 300000, actual: 0, forecast: 0, unbudgeted: 0 },
			{ category: 6, estimate: 60000, actual: 0, forecast: 60000, unbudgeted: 0 }
		]
	};

	it('start from the plan while nothing is booked, ticket income included', () => {
		expect(isPlanning(planned, [])).toBe(true);
		expect(startOf(planned, true)).toBe(240000);
		expect(categoryKnobs(planned, accounts, true).map((c) => [c.name, c.forecast])).toEqual([
			['Tickets', 300000],
			['Kitchen', 60000]
		]);
		const spent = { ...planned, expense: { ...planned.expense, actual: 100 } };
		expect(isPlanning(spent, [])).toBe(false);
	});

	it('move every line that scales, and each unit price on its own', () => {
		const scaled = scaledKnobs(planned, lines, guests, accounts, true);
		const more = { ...NOTHING, assumption: { 1: 200 } };
		// 50 more guests: 50 × 20 € in, 50 × 4 € out.
		expect(bridge(240000, [], [], more, scaled)).toMatchObject({
			assumptions: 80000,
			result: 320000
		});
		const pricier = { ...more, unit: { 2: 450 } };
		expect(bridge(240000, [], [], pricier, scaled)).toMatchObject({
			assumptions: 80000,
			expenses: -10000,
			result: 310000
		});
		expect(changes([], [], pricier, scaled).map((c) => [c.part, c.name, c.value])).toEqual([
			['assumption', 'Guests', 200],
			['unit', 'Kitchen', 450]
		]);
	});

	it('follow the forecast once money is booked, and ticket income what is sold', () => {
		const booked = {
			...planned,
			lines: [figures(1, 300000, 100000, 100000), figures(2, 60000, 70000, 70000)]
		};
		const scaled = scaledKnobs(booked, lines, guests, accounts, false);
		expect(scaled.lines.map((l) => l.fixed)).toEqual([true, false]);
		// Kitchen has spent 700 € already: up to 175 guests, the forecast stays there.
		expect(bridge(0, [], [], { ...NOTHING, assumption: { 1: 170 } }, scaled).result).toBe(0);
		expect(bridge(0, [], [], { ...NOTHING, assumption: { 1: 200 } }, scaled).result).toBe(-10000);
	});

	it('suggest the value that reaches a target', () => {
		const scaled = scaledKnobs(planned, lines, guests, accounts, true);
		const [s] = reach(32000, [], [], NOTHING, scaled);
		// Each guest adds 16 €: 20 guests more.
		expect(s).toMatchObject({ kind: 'assumption', to: 170, covers: true, gain: 32000 });
	});
});

describe('The What if section', () => {
	beforeEach(() => vi.restoreAllMocks());

	const money = (id: string) => screen.getByTestId(id).textContent?.replace(/\s+/g, ' ').trim();
	const adjust = () => fireEvent.click(screen.getByRole('button', { name: 'Adjust' }));

	it('starts folded at the forecast', () => {
		open();
		render(Budget);
		expect(money('plugin-what-if-result')).toBe('+1.765,00 €');
		const toggle = screen.getByRole('button', { name: 'Adjust' });
		expect(toggle.getAttribute('aria-expanded')).toBe('false');
		expect(screen.queryByRole('slider')).toBeNull();
		expect(screen.queryByTestId('plugin-what-if-change')).toBeNull();
	});

	it('follows a bar, undoes the change, and writes nothing to the file', async () => {
		open();
		const apply = vi.spyOn(api, 'applyPluginRecords');
		render(Budget);
		await adjust();
		expect(screen.getByRole('button', { name: 'Hide' }).getAttribute('aria-expanded')).toBe('true');
		const reset = screen.getByRole('button', { name: 'Reset' });
		expect(reset).toHaveProperty('disabled', true);

		const venue = screen.getByRole<HTMLInputElement>('slider', { name: 'Venue' });
		await fireEvent.input(venue, { target: { value: '190000' } });
		// Venue moves in steps of 10 €, and an expense going up lowers the result.
		expect(money('plugin-what-if-result')).toBe('+1.665,00 €');
		expect(money('plugin-what-if-change')).toContain('−100,00 €');
		expect(money('plugin-what-if-bridge-result')).toBe('+1.665,00 €');

		await fireEvent.click(screen.getByRole('button', { name: /Undo Venue \+100,00/ }));
		expect(money('plugin-what-if-result')).toBe('+1.765,00 €');

		await fireEvent.input(venue, { target: { value: '190000' } });
		await fireEvent.click(screen.getByRole('button', { name: 'Reset' }));
		expect(money('plugin-what-if-result')).toBe('+1.765,00 €');
		expect(apply).not.toHaveBeenCalled();
	});

	it('sells more tickets of a tier, one at a time or by place', async () => {
		open((s) => {
			s.tickets.tiers = [tier({ feeBp: 0, quota: 10, places: 8 })];
		});
		render(Budget);
		await adjust();
		await fireEvent.click(screen.getByRole('button', { name: 'One more ticket of Regular' }));
		expect(money('plugin-what-if-result')).toBe('+1.797,00 €');
		const free = screen.getAllByRole('button', { name: 'Sell up to here', hidden: true });
		expect(free).toHaveLength(2);
		await fireEvent.click(free[1]);
		expect(money('plugin-what-if-result')).toBe('+1.829,00 €');
		expect(screen.getByRole('button', { name: 'One more ticket of Regular' })).toHaveProperty(
			'disabled',
			true
		);
	});

	it('tries other values of an assumption and the unit prices that scale with it', async () => {
		open((s) => {
			s.budgetLines.push({
				id: 99,
				category: 6,
				description: 'Breakfast',
				estimate: { type: 'quantity', quantity: 1, unitPrice: 400, per: [1] },
				complete: false,
				costCentre: null
			});
			s.budget.lines.push({ line: 99, estimate: 104000, actual: 0, forecast: 104000, fromPlan: 0 });
		});
		render(Budget);
		await adjust();
		expect(screen.getByTestId('plugin-what-if-assumption').textContent).toBe('260');
		await fireEvent.click(screen.getByRole('button', { name: 'One more Guests' }));
		// Ticket sales are complete, so only breakfast moves: 4 € more.
		expect(money('plugin-what-if-result')).toBe('+1.761,00 €');
		await fireEvent.click(
			screen.getByRole('button', { name: 'Raise the unit price of Breakfast' })
		);
		expect(money('plugin-what-if-result')).toBe('+1.630,50 €');
		await fireEvent.click(screen.getByRole('button', { name: /Undo Guests 261/ }));
		expect(money('plugin-what-if-result')).toBe('+1.635,00 €');
	});

	it('works back from a target and applies a way there', async () => {
		open();
		render(Budget);
		await adjust();
		await fireEvent.click(screen.getByRole('radio', { name: 'Reach' }));
		// The target starts at the plan, 540 € above the forecast.
		expect(money('plugin-what-if-gap')).toBe('540,00 € to go');
		const mobility = screen.getByText('Mobility down by 150,00 €').closest('li')!;
		await fireEvent.click(mobility.querySelector('button')!);
		expect(money('plugin-what-if-result')).toBe('+1.915,00 €');
		expect(screen.getByRole('radio', { name: 'Try' }).getAttribute('aria-checked')).toBe('true');
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
