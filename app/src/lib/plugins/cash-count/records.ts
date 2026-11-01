// What the plugin keeps in the event file: the pieces of each count, which a template leaves out,
// and the denominations a group doesn't use, which it copies. A record that doesn't fit is ignored
// and kept. The totals are the plugin's own: they only reach the books when the treasurer saves the
// count they filled in. All amounts are integer cents.
import type { PluginChange, PluginRecord } from '../api';

export type Which = 'opening' | 'closing';

/** How many of each denomination, by its value in cents. */
export type Pieces = Record<number, number>;

/** Largest first. The app only knows the euro so far; another currency needs its own list. */
export const EURO = [50000, 20000, 10000, 5000, 2000, 1000, 500, 200, 100, 50, 20, 10, 5, 2, 1];

const isCount = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;

const key = (account: number, which: Which) => `pieces:${account}:${which}`;

export function piecesOf(records: PluginRecord[], account: number, which: Which): Pieces {
	const v = records.find((r) => r.key === key(account, which))?.value;
	if (typeof v !== 'object' || v === null || Array.isArray(v)) return {};
	const pieces: Pieces = {};
	for (const [value, count] of Object.entries(v)) {
		if (!isCount(Number(value)) || !isCount(count)) return {};
		if (count > 0) pieces[Number(value)] = count;
	}
	return pieces;
}

export const hasPieces = (pieces: Pieces) => Object.values(pieces).some((n) => n > 0);

export const totalOf = (pieces: Pieces) =>
	Object.entries(pieces).reduce((sum, [value, count]) => sum + Number(value) * count, 0);

/** A null value removes the record, so an event with no pieces has none. */
export const setPieces = (account: number, which: Which, pieces: Pieces): PluginChange => ({
	key: key(account, which),
	scope: 'records',
	value: hasPieces(pieces) ? pieces : null
});

export function hiddenOf(records: PluginRecord[]): number[] {
	const v = records.find((r) => r.key === 'hidden')?.value;
	return Array.isArray(v) ? v.filter(isCount) : [];
}

export const setHidden = (hidden: number[]): PluginChange => ({
	key: 'hidden',
	scope: 'setup',
	value: hidden.length ? hidden : null
});

/** The denominations to offer: those in use, and any hidden one that was counted. */
export const offered = (hidden: number[], pieces: Pieces) =>
	EURO.filter((value) => !hidden.includes(value) || (pieces[value] ?? 0) > 0);
