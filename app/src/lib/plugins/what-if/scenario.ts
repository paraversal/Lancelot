// What a scenario does to the forecast. Its figures are informational and never reach the ledger
// or the file (technical design §10). All amounts are integer cents.
import type { AccountView, Assumption, Budget, BudgetLine, TierFigures } from '../api';

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
	/** Where it starts: the forecast, or the estimate while planning. */
	forecast: number;
	/** What is booked already. A category can't go below it. */
	actual: number;
	step: number;
	/** The lowest and highest value, both a whole number of steps from the forecast. */
	min: number;
	max: number;
};

/** An assumption to try other values of, such as the guests expected (R-BUD-6). */
export type AssumptionKnob = { id: number; name: string; value: number; max: number };

/** A budget line that scales with assumptions: its unit price can change, and its estimate follows. */
export type LineKnob = {
	id: number;
	name: string;
	category: number;
	kind: 'income' | 'expense';
	quantity: number;
	unitPrice: number;
	per: number[];
	actual: number;
	/** Complete, or ticket income once tickets sell: its forecast is what is booked. */
	fixed: boolean;
	/** Where it starts, like the categories. */
	base: number;
	step: number;
	maxPrice: number;
};

/** The assumptions, and the lines that scale with them. */
export type Scaled = { assumptions: AssumptionKnob[]; lines: LineKnob[]; planning: boolean };
export const UNSCALED: Scaled = { assumptions: [], lines: [], planning: false };

/** What has been changed. What isn't here hasn't been. */
export type Scenario = {
	/** More tickets per tier. */
	more: Record<number, number>;
	/** Another price per tier, for the tickets still to sell. */
	price: Record<number, number>;
	/** Another figure per category, for what doesn't scale. */
	category: Record<number, number>;
	/** Another value per assumption. */
	assumption: Record<number, number>;
	/** Another unit price per scaled line. */
	unit: Record<number, number>;
};

export const NOTHING: Scenario = { more: {}, price: {}, category: {}, assumption: {}, unit: {} };

/**
 * Before anything is booked or sold, the forecast only counts the expenses: income counts once it
 * comes in (R-BUD-4). What if then starts from the plan, and tickets come from the budget.
 */
export const isPlanning = (budget: Budget, tiers: TierFigures[]) =>
	budget.income.actual === 0 && budget.expense.actual === 0 && tiers.every((t) => t.places === 0);

/** Where What if starts: the planned result while planning, the forecast after. */
export const startOf = (budget: Budget, planning: boolean) =>
	planning ? budget.result.estimate : budget.result.forecast;

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

const unitStep = (price: number) => (price < 200 ? 10 : price < 2000 ? 50 : 100);

/** The assumptions some line scales with, and those lines. */
export function scaledKnobs(
	budget: Budget,
	lines: BudgetLine[],
	assumptions: Assumption[],
	accounts: AccountView[],
	planning = false
): Scaled {
	const out: LineKnob[] = [];
	for (const l of lines) {
		const e = l.estimate;
		if (e.type !== 'quantity' || !e.per.length) continue;
		const account = accounts.find((a) => a.id === l.category);
		if (!account || (account.kind !== 'income' && account.kind !== 'expense')) continue;
		const figures = budget.lines.find((f) => f.line === l.id);
		if (!figures) continue;
		out.push({
			id: l.id,
			name: l.description,
			category: l.category,
			kind: account.kind,
			quantity: e.quantity,
			unitPrice: e.unitPrice,
			per: e.per,
			actual: figures.actual,
			fixed: l.complete || (account.role !== null && !planning),
			base: planning ? figures.estimate : figures.forecast,
			step: unitStep(e.unitPrice),
			maxPrice: Math.max(e.unitPrice * 2, 500)
		});
	}
	const used = new Set(out.flatMap((l) => l.per));
	return {
		assumptions: assumptions
			.filter((a) => used.has(a.id))
			.map((a) => ({ ...a, max: Math.max(a.value * 2, a.value + 20) })),
		lines: out,
		planning
	};
}

export const assumptionValue = (knob: AssumptionKnob, s: Scenario) =>
	clamp(s.assumption[knob.id] ?? knob.value, 0, knob.max);

export const unitPrice = (line: LineKnob, s: Scenario) =>
	clamp(s.unit[line.id] ?? line.unitPrice, 0, line.maxPrice);

/** What a scaled line would come to, by the forecast's rule (R-BUD-4), or its estimate while planning. */
function lineFigure(line: LineKnob, scaled: Scaled, s: Scenario, price: number) {
	if (line.fixed) return line.base;
	const estimate = line.per.reduce((n, id) => {
		const knob = scaled.assumptions.find((a) => a.id === id);
		return n * (knob ? assumptionValue(knob, s) : 0);
	}, line.quantity * price);
	if (scaled.planning) return estimate;
	return line.kind === 'income' ? Math.min(estimate, line.actual) : Math.max(estimate, line.actual);
}

/** How far a scaled line moves: from the assumptions, then from its own unit price. */
export function lineChange(line: LineKnob, scaled: Scaled, s: Scenario) {
	const values = lineFigure(line, scaled, s, line.unitPrice);
	const priced = lineFigure(line, scaled, s, unitPrice(line, s));
	return { values: values - line.base, price: priced - values, value: priced };
}

/** What the scaled lines of a category add to it in the scenario. */
export const scaledIn = (category: number, scaled: Scaled, s: Scenario) =>
	scaled.lines
		.filter((l) => l.category === category)
		.reduce((n, l) => {
			const c = lineChange(l, scaled, s);
			return n + c.values + c.price;
		}, 0);

function stepFor(forecast: number) {
	if (forecast >= 100000) return 1000;
	return forecast >= 20000 ? 500 : 100;
}

/**
 * The categories with a forecast, in account order. Ticket categories follow the tiers above
 * them, except while planning, when they are budgeted like the rest. What is already booked has
 * happened, so a category can't go below it.
 */
export function categoryKnobs(
	budget: Budget,
	accounts: AccountView[],
	planning = false
): CategoryKnob[] {
	return budget.categories.flatMap((c) => {
		const account = accounts.find((a) => a.id === c.category);
		const start = planning ? c.estimate : c.forecast;
		if (!account || (account.role !== null && !planning) || start <= 0) return [];
		if (account.kind !== 'income' && account.kind !== 'expense') return [];
		const step = stepFor(start);
		const floor = Math.min(c.actual, start);
		const ceiling = start + Math.max(start, 10000);
		return [
			{
				id: c.category,
				name: account.name,
				kind: account.kind,
				forecast: start,
				actual: c.actual,
				step,
				min: start - Math.floor((start - floor) / step) * step,
				max: start + Math.floor((ceiling - start) / step) * step
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
	/** What other values of the assumptions do to the lines that scale with them. */
	assumptions: number;
	income: number;
	/** Already signed: more expenses are negative. */
	expenses: number;
	result: number;
};

export function bridge(
	forecast: number,
	tiers: TierKnob[],
	categories: CategoryKnob[],
	s: Scenario,
	scaled: Scaled = UNSCALED
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
	let assumptions = 0;
	for (const l of scaled.lines) {
		const c = lineChange(l, scaled, s);
		const sign = l.kind === 'income' ? 1 : -1;
		assumptions += sign * c.values;
		if (l.kind === 'income') income += c.price;
		else expenses -= c.price;
	}
	return {
		forecast,
		tickets,
		prices,
		assumptions,
		income,
		expenses,
		result: forecast + tickets + prices + assumptions + income + expenses
	};
}

/** The forecast with the scenario applied. */
export const outcome = (
	forecast: number,
	tiers: TierKnob[],
	categories: CategoryKnob[],
	s: Scenario,
	scaled: Scaled = UNSCALED
) => bridge(forecast, tiers, categories, s, scaled).result;

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
	| { kind: 'assumption'; assumption: AssumptionKnob; to: number }
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
	scaled: Scaled = UNSCALED,
	limit = 6
): Suggestion[] {
	if (gap <= 0) return [];
	const out: Suggestion[] = [];
	// Another value of an assumption: the nearest that gets there, or the one that gets closest.
	const now = outcome(0, tiers, categories, s, scaled);
	for (const a of scaled.assumptions) {
		const value = assumptionValue(a, s);
		const at = (v: number) => ({ ...s, assumption: { ...s.assumption, [a.id]: v } });
		const gainAt = (v: number) => outcome(0, tiers, categories, at(v), scaled) - now;
		let best: { to: number; gain: number } | null = null;
		for (let d = 1; d <= a.max; d++) {
			for (const v of [value + d, value - d]) {
				if (v < 0 || v > a.max) continue;
				const gain = gainAt(v);
				if (gain >= gap) {
					best = { to: v, gain };
					break;
				}
				if (gain > 0 && (!best || gain > best.gain)) best = { to: v, gain };
			}
			if (best && best.gain >= gap) break;
		}
		if (best)
			out.push({
				kind: 'assumption',
				assumption: a,
				to: best.to,
				covers: best.gain >= gap,
				gain: best.gain,
				scenario: at(best.to)
			});
	}
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
export function changes(
	tiers: TierKnob[],
	categories: CategoryKnob[],
	s: Scenario,
	scaled: Scaled = UNSCALED
): Change[] {
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
	for (const a of scaled.assumptions) {
		const value = assumptionValue(a, s);
		if (value !== a.value) out.push({ part: 'assumption', id: a.id, name: a.name, value });
	}
	for (const l of scaled.lines) {
		const price = unitPrice(l, s);
		if (price !== l.unitPrice) out.push({ part: 'unit', id: l.id, name: l.name, value: price });
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
