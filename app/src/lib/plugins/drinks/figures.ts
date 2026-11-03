// The plugin's figures. They are informational: they never reach the ledger (technical design §10),
// and the real money is what the Result on the page shows. All amounts are integer cents.
import type { TxnView } from '../api';
import type { CentreSettings, Counts, Product } from './records';

/** What each count means, for the person typing it in. */
export const EXPLAINERS = {
	taken: 'Received from the supplier.',
	returned: 'Given back to the supplier unsold. Not billed.',
	leftOver: 'Paid for, but still here after the event.',
	logged: 'What people logged as taken.'
};

export type Row = {
	product: Product;
	counts: Counts;
	/** Taken minus returned: what the supplier bills. Bought outright, everything taken. */
	billed: number;
	leftOver: number;
	/** Billed minus left over: sold, given away or lost. */
	gone: number;
	/** What people logged. Null until a figure is entered or imported. */
	logged: number | null;
	/** Gone minus logged. Null without a log, or without stock to compare it with. */
	gap: number | null;
	cost: number;
	/** Logged at the donation price. Null when either is missing. */
	expected: number | null;
};

export type InvoiceCheck = {
	/** Everything booked to the cost centre with the supplier as counterparty. */
	amount: number;
	/** The part of `amount` in the Deposit category. */
	deposit: number;
	/** `amount` without deposit, minus the billed cost. Null without products to compare with. */
	difference: number | null;
};

export type Figures = {
	rows: Row[];
	billed: number;
	leftOver: number;
	gone: number;
	logged: number;
	gap: number;
	cost: number;
	/** Cost of what is gone, per unit, rounded half up. Null while nothing is gone. */
	costPerUnit: number | null;
	/** Null until something has a donation price and a log. */
	expected: number | null;
	invoice: InvoiceCheck | null;
};

const sum = (values: number[]) => values.reduce((a, b) => a + b, 0);

export function loggedOf(counts: Counts): number | null {
	if (counts.manual === null && counts.imported === null) return null;
	return Math.max(0, (counts.imported ?? 0) + (counts.manual ?? 0));
}

function rowOf(product: Product, counts: Counts, commission: boolean): Row {
	const billed = Math.max(0, counts.taken - (commission ? counts.returned : 0));
	const leftOver = counts.leftOver ?? 0;
	const gone = Math.max(0, billed - leftOver);
	const logged = loggedOf(counts);
	return {
		product,
		counts,
		billed,
		leftOver,
		gone,
		logged,
		// A tally from a paper list has no stock of its own, so there is nothing to compare.
		gap: logged !== null && counts.taken > 0 ? gone - logged : null,
		cost: billed * product.cost,
		expected: logged !== null && product.donation !== null ? logged * product.donation : null
	};
}

function checkInvoice(
	centre: number,
	settings: CentreSettings,
	txns: TxnView[],
	cost: number,
	hasProducts: boolean
): InvoiceCheck | null {
	const supplier = settings.supplier?.trim().toLowerCase();
	if (!supplier) return null;
	const lines = txns
		.filter((t) => t.kind === 'expense' && t.counterparty?.trim().toLowerCase() === supplier)
		.flatMap((t) => t.lines)
		.filter((l) => l.costCentre === centre);
	if (lines.length === 0) return null;
	const amount = sum(lines.map((l) => l.amount));
	const deposit = sum(lines.filter((l) => l.account === settings.deposit).map((l) => l.amount));
	return { amount, deposit, difference: hasProducts ? amount - deposit - cost : null };
}

export function computeFigures(
	centre: number,
	settings: CentreSettings,
	products: { product: Product; counts: Counts }[],
	txns: TxnView[]
): Figures {
	const rows = products.map((p) => rowOf(p.product, p.counts, settings.commission));
	const gone = sum(rows.map((r) => r.gone));
	const goneCost = sum(rows.map((r) => r.gone * r.product.cost));
	const cost = sum(rows.map((r) => r.cost));
	const expected = rows.flatMap((r) => r.expected ?? []);
	return {
		rows,
		billed: sum(rows.map((r) => r.billed)),
		leftOver: sum(rows.map((r) => r.leftOver)),
		gone,
		logged: sum(rows.map((r) => r.logged ?? 0)),
		gap: sum(rows.map((r) => r.gap ?? 0)),
		cost,
		costPerUnit: gone > 0 ? Math.floor((goneCost * 2 + gone) / (gone * 2)) : null,
		expected: expected.length ? sum(expected) : null,
		invoice: checkInvoice(centre, settings, txns, cost, rows.length > 0)
	};
}
