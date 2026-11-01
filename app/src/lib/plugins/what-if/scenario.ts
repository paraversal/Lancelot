// What a scenario does to the forecast. Its figures are informational and never reach the ledger
// or the file (technical design §10). All amounts are integer cents.
import type { AccountView, Budget, TierFigures } from '../api';

/** A slider counts whole steps from where the figure is now, so its start is always on the grid. */
export type Range = { min: number; max: number };

export type TierKnob = {
	id: number;
	name: string;
	price: number;
	feeBp: number;
	/** More tickets, from none up to the places left, or a guess without a quota. */
	more: Range;
	/** Steps of `PRICE_STEP` from the tier's price. It applies only to tickets still to sell. */
	priceSteps: Range;
};

export type CategoryKnob = {
	id: number;
	name: string;
	kind: 'income' | 'expense';
	forecast: number;
	step: number;
	range: Range;
};

/** How far each slider has been moved. What isn't here hasn't been. */
export type Scenario = {
	more: Record<number, number>;
	price: Record<number, number>;
	category: Record<number, number>;
};

export const NOTHING: Scenario = { more: {}, price: {}, category: {} };

export const PRICE_STEP = 100;

/** Steps around a figure, without passing the floor or the ceiling. */
const around = (value: number, floor: number, ceiling: number, step: number): Range => ({
	min: -Math.floor((value - floor) / step),
	max: Math.floor((ceiling - value) / step)
});

const clamp = (value: number, { min, max }: Range) => Math.min(max, Math.max(min, value));

/** What one more ticket brings in, like the core's break-even: the price minus the platform's fee. */
export const netPrice = (price: number, feeBp: number) =>
	price - Math.floor((price * feeBp + 5000) / 10000);

export const tierKnobs = (tiers: TierFigures[]): TierKnob[] =>
	tiers.map((t) => ({
		id: t.id,
		name: t.name,
		price: t.price,
		feeBp: t.feeBp,
		more: {
			min: 0,
			max: t.quota === null ? Math.max(50, t.places) : Math.max(0, t.quota - t.places)
		},
		priceSteps: around(t.price, 0, Math.max(t.price * 2, 2000), PRICE_STEP)
	}));

function stepFor(forecast: number) {
	if (forecast >= 100000) return 1000;
	return forecast >= 20000 ? 500 : 100;
}

/**
 * The categories with a forecast, in account order. Ticket categories follow the sliders above
 * them. What is already booked has happened, so a category can't go below it.
 */
export function categoryKnobs(budget: Budget, accounts: AccountView[]): CategoryKnob[] {
	return budget.categories.flatMap((c) => {
		const account = accounts.find((a) => a.id === c.category);
		if (!account || account.role !== null || c.forecast <= 0) return [];
		if (account.kind !== 'income' && account.kind !== 'expense') return [];
		const step = stepFor(c.forecast);
		const floor = Math.min(c.actual, c.forecast);
		const ceiling = c.forecast + Math.max(c.forecast, 10000);
		return [
			{
				id: c.category,
				name: account.name,
				kind: account.kind,
				forecast: c.forecast,
				step,
				range: around(c.forecast, floor, ceiling, step)
			}
		];
	});
}

export const tierPrice = (knob: TierKnob, scenario: Scenario) =>
	knob.price + clamp(scenario.price[knob.id] ?? 0, knob.priceSteps) * PRICE_STEP;

export const moreOf = (knob: TierKnob, scenario: Scenario) =>
	clamp(scenario.more[knob.id] ?? 0, knob.more);

/** What the tickets still to sell of this tier would bring in. */
export const tierGain = (knob: TierKnob, scenario: Scenario) =>
	moreOf(knob, scenario) * netPrice(tierPrice(knob, scenario), knob.feeBp);

export const categoryValue = (knob: CategoryKnob, scenario: Scenario) =>
	knob.forecast + clamp(scenario.category[knob.id] ?? 0, knob.range) * knob.step;

/** The forecast with the scenario applied. */
export function outcome(
	forecast: number,
	tiers: TierKnob[],
	categories: CategoryKnob[],
	scenario: Scenario
) {
	const tickets = tiers.reduce((sum, t) => sum + tierGain(t, scenario), 0);
	return categories.reduce((sum, c) => {
		const change = categoryValue(c, scenario) - c.forecast;
		return c.kind === 'income' ? sum + change : sum - change;
	}, forecast + tickets);
}
