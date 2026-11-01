// Fitting what a data source read onto a cost centre's products (R-DRK-6). Only totals per product
// are kept, so the event doesn't record who drank what.
import type { Pulled } from '../api';
import type { Product } from './records';

export const PULL_FIELDS = ['name', 'quantity'];

/** The most units of one product: far above any event, and safe to multiply by a price. */
const MAX_QUANTITY = 1_000_000;

export type PullMatch = {
	/** The total per product id. */
	totals: Map<number, number>;
	/** Names that match no product, with their totals. They block the import until mapped. */
	unknown: { name: string; quantity: number }[];
	/** Rows whose quantity isn't a whole number. Listed and left out. */
	unreadable: { name: string; quantity: string }[];
};

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

export function matchPull(products: Product[], rows: Pulled['rows']): PullMatch {
	const totals = new Map<number, number>();
	const unknown = new Map<string, { name: string; quantity: number }>();
	const unreadable: PullMatch['unreadable'] = [];
	for (const row of rows) {
		const name = row.name ?? '';
		const text = row.quantity ?? '';
		const quantity = /^\d+$/.test(text) ? Number(text) : null;
		if (quantity === null || quantity > MAX_QUANTITY) {
			unreadable.push({ name, quantity: text });
			continue;
		}
		const product = products.find(
			(p) => same(p.name, name) || p.aliases?.some((a) => same(a, name))
		);
		if (product) {
			totals.set(product.id, Math.min(MAX_QUANTITY, (totals.get(product.id) ?? 0) + quantity));
			continue;
		}
		const key = name.trim().toLowerCase();
		const seen = unknown.get(key);
		if (seen) seen.quantity += quantity;
		else unknown.set(key, { name, quantity });
	}
	return { totals, unknown: [...unknown.values()], unreadable };
}
