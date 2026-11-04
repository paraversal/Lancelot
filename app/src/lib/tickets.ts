// Ticket helpers for the Tickets page. Fees are basis points (R-TKT-10): `250` is 2,5 %. The
// fee field takes a percentage with at most two decimals; the core checks it against the price.
import type { EventSnapshot } from './api/generated/EventSnapshot';
import type { NumberFormat } from './api/generated/NumberFormat';
import type { SalesPoint } from './api/generated/SalesPoint';
import type { TierFigures } from './api/generated/TierFigures';
import type { Tickets } from './api/generated/Tickets';

/**
 * Where tickets of the sheet belong: their order, or the sheet itself when its rows have no order
 * ID. The core keeps those under an empty order.
 */
export const orderName = (order: string) => (order === '' ? 'Sheet' : `Order ${order}`);

/**
 * Income typed into the Tickets category by hand. The sales don't know it, but it is in the
 * category's balance, so the Tickets page shows it to add up with the rest of the app.
 */
export function otherTicketIncome(snapshot: EventSnapshot): number {
	const category = snapshot.accounts.find((a) => a.role === 'tickets');
	if (!category) return 0;
	return snapshot.txns
		.filter((t) => t.origin.type === 'manual')
		.flatMap((t) => t.lines)
		.filter((l) => l.account === category.id)
		.reduce((sum, l) => sum - l.amount, 0);
}

/** `2,5 %` (De) or `2.5%` (En), without trailing zeros. */
export function formatFee(bp: number, format: NumberFormat): string {
	const text = String(bp / 100);
	return format === 'de' ? `${text.replace('.', ',')} %` : `${text}%`;
}

/** The number in the fee field, without the sign: `2,5` (De) or `2.5` (En). */
export function feeInput(bp: number, format: NumberFormat): string {
	const text = String(bp / 100);
	return format === 'de' ? text.replace('.', ',') : text;
}

/** `2,5`, `2.5`, `2,5 %` or empty (no fee) as basis points; null when it is not a percentage. */
export function parseFee(input: string): number | null {
	const s = input.replace(/\s|%/g, '');
	if (!s) return 0;
	const m = s.match(/^(\d{1,3})(?:[.,](\d{1,2}))?$/);
	if (!m) return null;
	return Number(m[1]) * 100 + Number((m[2] ?? '').padEnd(2, '0'));
}

/** Conflicts per order and tier: the navigation badge. */
export function conflictCount(tickets: Tickets): number {
	return tickets.conflicts.reduce((n, c) => n + c.removed.length, 0);
}

/** A count or quota: digits only. Empty is null; anything else is NaN. */
export function parseCount(input: string): number | null {
	const s = input.trim();
	if (!s) return null;
	return /^\d+$/.test(s) ? Number(s) : NaN;
}

/** `9 Jun`: the short date of an import. */
export function shortDay(iso: string): string {
	return new Date(`${iso}T12:00:00`).toLocaleDateString('en-GB', {
		day: 'numeric',
		month: 'short'
	});
}

export type SalesView = 'tickets' | 'revenue';

/** One column of the sales chart. A segment's `tier` is its index among the tiers. */
export type SalesBar = {
	point: SalesPoint;
	/** Places in tickets, net income in revenue, as written over the column. */
	value: number;
	segments: { tier: number; value: number }[];
	/** Sold but no longer taking a place. Only in tickets. */
	cancelled: number;
	/** The height to scale: the segments and what was cancelled. */
	size: number;
};

/** What the chart draws per point: places by tier under the cancelled, or net income by tier. */
export function salesBars(points: SalesPoint[], view: SalesView): SalesBar[] {
	return points.map((point) => {
		const segments = point.tiers.map((t, tier) => ({
			tier,
			value: view === 'tickets' ? t.places : Math.max(0, t.net)
		}));
		const cancelled = view === 'tickets' ? point.total.sold - point.total.places : 0;
		return {
			point,
			value: view === 'tickets' ? point.total.places : point.total.net,
			segments,
			cancelled,
			size: segments.reduce((n, s) => n + s.value, cancelled)
		};
	});
}

/** The places of all tiers, or null when a tier has no quota and the total means nothing. */
export function totalQuota(tiers: TierFigures[]): number | null {
	if (!tiers.length) return null;
	let total = 0;
	for (const t of tiers) {
		if (t.quota === null) return null;
		total += t.quota;
	}
	return total;
}

/** Which date labels fit: every `step`th, counted from the last, which always shows. */
export function labelStep(count: number, width: number, labelWidth = 48): number {
	return Math.max(1, Math.ceil(count / Math.max(1, Math.floor(width / labelWidth))));
}
