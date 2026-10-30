// Ticket helpers for the Tickets page. Fees are basis points (R-TKT-10): `250` is 2,5 %. The
// fee field takes a percentage with at most two decimals; the core checks it against the price.
import type { NumberFormat } from './api/generated/NumberFormat';
import type { Tickets } from './api/generated/Tickets';

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

/** The fee on one ticket, rounded half-up like the core's `percent_bp`. */
export function feePerTicket(price: number, bp: number): number {
	return Math.round((price * bp) / 10_000);
}

/** The refund on one ticket: the same rounding as the fee, per ticket (R-TKT-15). */
export const refundPerTicket = feePerTicket;

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
