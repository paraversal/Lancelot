// What the plugin keeps in the event file: the pieces of each count, the notes and coins of the
// event's currency, and the custom denominations a group adds for packed money, such as a roll of
// coins. A record that doesn't fit is ignored and kept. The totals are the plugin's own: they only
// reach the books when the treasurer saves the count they filled in. All amounts are integer cents.
import type { Currency } from '../../api/generated/Currency';
import type { PluginChange, PluginRecord } from '../api';

export type Which = 'opening' | 'closing';

/**
 * How many of each: a note or coin by its value in cents, a custom denomination by
 * `custom:<id>`. Keys are strings because JSON object keys always are.
 */
export type Pieces = Record<string, number>;

/** A denomination a group added, such as a roll of coins. A template copies them. */
export type CustomDenomination = {
	/** Stable within the event, so a count keeps pointing at the denomination by the value typed. */
	id: string;
	name: string;
	/** What one of them is worth, in cents. */
	value: number;
};

/** The largest name a custom denomination may have. */
export const MAX_CUSTOM_NAME = 40;

/** The notes and coins of every currency the app supports, largest first. */
export const DENOMINATIONS: Record<Currency, { notes: number[]; coins: number[] }> = {
	EUR: {
		notes: [50000, 20000, 10000, 5000, 2000, 1000, 500],
		coins: [200, 100, 50, 20, 10, 5, 2, 1]
	},
	USD: {
		notes: [10000, 5000, 2000, 1000, 500, 200, 100],
		// The dollar coin shares its value with the note, so it is left out.
		coins: [50, 25, 10, 5, 1]
	},
	GBP: {
		notes: [5000, 2000, 1000, 500],
		coins: [200, 100, 50, 20, 10, 5, 2, 1]
	},
	CHF: {
		notes: [100000, 20000, 10000, 5000, 2000, 1000],
		coins: [500, 200, 100, 50, 20, 10, 5]
	}
};

const PLAIN = /^\d+$/;
const CUSTOM = /^custom:[a-z0-9-]+$/;
const ID = /^[a-z0-9-]+$/;

const isCount = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;
const isValue = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) > 0;

const isKey = (key: string) => (PLAIN.test(key) && Number(key) > 0) || CUSTOM.test(key);

const key = (account: number, which: Which) => `pieces:${account}:${which}`;

export const customKey = (id: string) => `custom:${id}`;

/** The pieces in a record's value; null when it doesn't fit. */
function piecesIn(value: unknown): Pieces | null {
	if (typeof value !== 'object' || value === null || Array.isArray(value)) return null;
	const pieces: Pieces = {};
	for (const [k, count] of Object.entries(value)) {
		if (!isKey(k) || !isCount(count)) return null;
		if (count > 0) pieces[k] = count;
	}
	return pieces;
}

export function piecesOf(records: PluginRecord[], account: number, which: Which): Pieces {
	const v = records.find((r) => r.key === key(account, which))?.value;
	return piecesIn(v) ?? {};
}

export const hasPieces = (pieces: Pieces) => Object.values(pieces).some((n) => n > 0);

/** What the pieces add up to. A custom denomination no longer in the list adds nothing. */
export function totalOf(pieces: Pieces, custom: CustomDenomination[] = []): number {
	const added = new Map(custom.map((c) => [customKey(c.id), c.value]));
	let sum = 0;
	for (const [k, count] of Object.entries(pieces)) {
		if (CUSTOM.test(k)) sum += (added.get(k) ?? 0) * count;
		else if (PLAIN.test(k)) sum += Number(k) * count;
	}
	return sum;
}

/** A null value removes the record, so an event with no pieces has none. */
export const setPieces = (account: number, which: Which, pieces: Pieces): PluginChange => ({
	key: key(account, which),
	scope: 'records',
	value: hasPieces(pieces) ? pieces : null
});

const isCustom = (v: unknown): v is CustomDenomination => {
	if (typeof v !== 'object' || v === null || Array.isArray(v)) return false;
	const c = v as Record<string, unknown>;
	return (
		typeof c.id === 'string' &&
		ID.test(c.id) &&
		typeof c.name === 'string' &&
		!!c.name.trim() &&
		isValue(c.value)
	);
};

export function customOf(records: PluginRecord[]): CustomDenomination[] {
	const v = records.find((r) => r.key === 'custom')?.value;
	if (!Array.isArray(v)) return [];
	const custom: CustomDenomination[] = [];
	const ids = new Set<string>();
	const names = new Set<string>();
	for (const item of v) {
		if (!isCustom(item)) continue;
		const name = item.name.trim().slice(0, MAX_CUSTOM_NAME);
		if (ids.has(item.id) || names.has(name.toLowerCase())) continue;
		ids.add(item.id);
		names.add(name.toLowerCase());
		custom.push({ id: item.id, name, value: item.value });
	}
	return custom;
}

export const setCustom = (custom: CustomDenomination[]): PluginChange => ({
	key: 'custom',
	scope: 'setup',
	value: custom.length ? custom : null
});

/** An id no custom denomination in the list has yet. */
export function newCustomId(custom: CustomDenomination[]): string {
	const taken = new Set(custom.map((c) => c.id));
	let id: string;
	do {
		id = `c${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
	} while (taken.has(id));
	return id;
}

/** A custom denomination's pieces, taken out of the counts that hold them. */
export function dropCustomPieces(records: PluginRecord[], id: string): PluginChange[] {
	const gone = customKey(id);
	const changes: PluginChange[] = [];
	for (const record of records) {
		if (!record.key.startsWith('pieces:')) continue;
		const pieces = piecesIn(record.value);
		if (!pieces || !(gone in pieces)) continue;
		const kept = { ...pieces };
		delete kept[gone];
		changes.push({ key: record.key, scope: 'records', value: hasPieces(kept) ? kept : null });
	}
	return changes;
}
