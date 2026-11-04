// What a scenario does to the forecast. Its figures are informational and never reach the ledger
// or the file (technical design §10). All amounts are integer cents.
import type { AccountView, Budget, TierFigures } from '../api';

export type TierKnob = {
	id: number;
	name: string;
	price: number;
	feeBp: number;
	quota: number | null;
	/** Places taken against the quota. */
	places: number;
	/** More tickets, from none up to the places left, or a guess without a quota. */
	maxMore: number;
	/** The highest price to try. It applies only to tickets still to sell. */
	maxPrice: number;
};

export type CategoryKnob = {
	id: number;
	name: string;
	kind: 'income' | 'expense';
	forecast: number;
	/** What is booked already. A category can't go below it. */
	actual: number;
	step: number;
	/** The lowest and highest value, both a whole number of steps from the forecast. */
	min: number;
	max: number;
};

/** What has been changed. What isn't here hasn't been. */
export type Scenario = {
	/** More tickets per tier. */
	more: Record<number, number>;
	/** Another price per tier, for the tickets still to sell. */
	price: Record<number, number>;
	/** Another figure per category. */
	category: Record<number, number>;
};

export const NOTHING: Scenario = { more: {}, price: {}, category: {} };

export const PRICE_STEP = 100;

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

/** What one more ticket brings in, like the core's break-even: the price minus the platform's fee. */
export const netPrice = (price: number, feeBp: number) =>
	price - Math.floor((price * feeBp + 5000) / 10000);

export const tierKnobs = (tiers: TierFigures[]): TierKnob[] =>
	tiers.map((t) => ({
		id: t.id,
		name: t.name,
		price: t.price,
		feeBp: t.feeBp,
		quota: t.quota,
		places: t.places,
		maxMore: t.quota === null ? Math.max(50, t.places) : Math.max(0, t.quota - t.places),
		maxPrice: Math.max(t.price * 2, 2000)
	}));

function stepFor(forecast: number) {
	if (forecast >= 100000) return 1000;
	return forecast >= 20000 ? 500 : 100;
}

/**
 * The categories with a forecast, in account order. Ticket categories follow the tiers above
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
				actual: c.actual,
				step,
				min: c.forecast - Math.floor((c.forecast - floor) / step) * step,
				max: c.forecast + Math.floor((ceiling - c.forecast) / step) * step
			}
		];
	});
}

export const moreOf = (knob: TierKnob, s: Scenario) => clamp(s.more[knob.id] ?? 0, 0, knob.maxMore);

export const tierPrice = (knob: TierKnob, s: Scenario) =>
	clamp(s.price[knob.id] ?? knob.price, 0, knob.maxPrice);

export const categoryValue = (knob: CategoryKnob, s: Scenario) =>
	clamp(s.category[knob.id] ?? knob.forecast, knob.min, knob.max);

/** Any figure, as the nearest whole number of steps from the forecast that the category allows. */
export const snapCategory = (knob: CategoryKnob, value: number) =>
	clamp(
		knob.forecast + Math.round((value - knob.forecast) / knob.step) * knob.step,
		knob.min,
		knob.max
	);

/** The forecast, each kind of change, and where they lead: the bars of the bridge. */
export type Bridge = {
	forecast: number;
	/** More tickets, at the prices they have now. */
	tickets: number;
	/** What other prices change on those tickets. */
	prices: number;
	income: number;
	/** Already signed: more expenses are negative. */
	expenses: number;
	result: number;
};

export function bridge(
	forecast: number,
	tiers: TierKnob[],
	categories: CategoryKnob[],
	s: Scenario
): Bridge {
	let tickets = 0;
	let prices = 0;
	for (const t of tiers) {
		const more = moreOf(t, s);
		const before = netPrice(t.price, t.feeBp);
		tickets += more * before;
		prices += more * (netPrice(tierPrice(t, s), t.feeBp) - before);
	}
	let income = 0;
	let expenses = 0;
	for (const c of categories) {
		const change = categoryValue(c, s) - c.forecast;
		if (c.kind === 'income') income += change;
		else expenses -= change;
	}
	return {
		forecast,
		tickets,
		prices,
		income,
		expenses,
		result: forecast + tickets + prices + income + expenses
	};
}

/** The forecast with the scenario applied. */
export const outcome = (
	forecast: number,
	tiers: TierKnob[],
	categories: CategoryKnob[],
	s: Scenario
) => bridge(forecast, tiers, categories, s).result;

/** One way towards a target, on top of the scenario. */
export type Suggestion = {
	/** Whether it reaches the target alone. */
	covers: boolean;
	/** What it adds to the result. */
	gain: number;
	/** The scenario with it applied. */
	scenario: Scenario;
} & (
	| { kind: 'tickets'; tier: TierKnob; count: number; net: number; leftAfter: number }
	| { kind: 'price'; tier: TierKnob; count: number; price: number }
	| { kind: 'cut'; category: CategoryKnob; by: number; to: number }
);

/**
 * What would close the gap between the scenario's result and a target: more tickets of one
 * tier, all the tickets left at a higher price, or a smaller expense. Those that get there alone
 * come first, the smallest change first; then the ones that get closest.
 */
export function reach(
	gap: number,
	tiers: TierKnob[],
	categories: CategoryKnob[],
	s: Scenario,
	limit = 6
): Suggestion[] {
	if (gap <= 0) return [];
	const out: Suggestion[] = [];
	for (const t of tiers) {
		const more = moreOf(t, s);
		const left = t.maxMore - more;
		const price = tierPrice(t, s);
		const net = netPrice(price, t.feeBp);
		if (left <= 0 || net <= 0) continue;
		const need = Math.ceil(gap / net);
		const count = Math.min(need, left);
		const with_ = (n: number, p = price): Scenario => ({
			...s,
			more: { ...s.more, [t.id]: more + n },
			price: p === t.price ? omit(s.price, t.id) : { ...s.price, [t.id]: p }
		});
		out.push({
			kind: 'tickets',
			tier: t,
			count,
			net,
			leftAfter: left - count,
			covers: need <= left,
			gain: count * net,
			scenario: with_(count)
		});
		// Not enough places: what all of them would have to cost. Without a quota, the places
		// are a guess, so a price on top of a guess is left out.
		if (need > left && t.quota !== null) {
			const all = more + left;
			const gained = more * net;
			for (let p = price + PRICE_STEP; p <= t.maxPrice; p += PRICE_STEP) {
				const gain = all * netPrice(p, t.feeBp) - gained;
				if (gain < gap) continue;
				out.push({
					kind: 'price',
					tier: t,
					count: all,
					price: p,
					covers: true,
					gain,
					scenario: with_(left, p)
				});
				break;
			}
		}
	}
	for (const c of categories) {
		if (c.kind !== 'expense') continue;
		const value = categoryValue(c, s);
		const room = value - c.min;
		if (room <= 0) continue;
		const by = Math.min(room, Math.ceil(gap / c.step) * c.step);
		out.push({
			kind: 'cut',
			category: c,
			by,
			to: value - by,
			covers: by >= gap,
			gain: by,
			scenario: { ...s, category: { ...s.category, [c.id]: value - by } }
		});
	}
	out.sort((a, b) =>
		a.covers !== b.covers ? (a.covers ? -1 : 1) : a.covers ? a.gain - b.gain : b.gain - a.gain
	);
	return out.slice(0, limit);
}

function omit(record: Record<number, number>, id: number) {
	const copy = { ...record };
	delete copy[id];
	return copy;
}

/** A change in a scenario, to show and undo on its own. */
export type Change = { part: keyof Scenario; id: number; name: string; value: number };

/** Every change the scenario makes, tier by tier and then category by category. */
export function changes(tiers: TierKnob[], categories: CategoryKnob[], s: Scenario): Change[] {
	const out: Change[] = [];
	for (const t of tiers) {
		const more = moreOf(t, s);
		if (more) out.push({ part: 'more', id: t.id, name: t.name, value: more });
		const price = tierPrice(t, s);
		if (price !== t.price) out.push({ part: 'price', id: t.id, name: t.name, value: price });
	}
	for (const c of categories) {
		const value = categoryValue(c, s);
		if (value !== c.forecast)
			out.push({ part: 'category', id: c.id, name: c.name, value: value - c.forecast });
	}
	return out;
}

/** The scenario without one of its changes. */
export function undo(s: Scenario, part: keyof Scenario, id: number): Scenario {
	return { ...s, [part]: omit(s[part], id) };
}

/**
 * The places of a tier as boxes: sold, added in the scenario, still free, or guessed for a tier
 * without a quota. A big tier gets one box for several tickets, so it stays one short row.
 */
export function places(knob: TierKnob, more: number, most = 250) {
	const total = knob.places + knob.maxMore;
	const per = Math.max(1, Math.ceil(total / most));
	const boxes = Math.ceil(total / per);
	const sold = Math.ceil(knob.places / per);
	const added = Math.ceil((knob.places + more) / per) - sold;
	return { per, boxes, sold, added };
}
