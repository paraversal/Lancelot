// The ledger's rows, filters, search and grouping (R-TX-7). Pure functions over the snapshot;
// every amount shown comes from the core (a transaction's `amount`, a line's amount, an
// account's balance), so nothing here adds money up.
import type { AccountView } from './api/generated/AccountView';
import type { Cents } from './api/generated/Cents';
import type { EventSnapshot } from './api/generated/EventSnapshot';
import type { Flow } from './api/generated/Flow';
import type { NumberFormat } from './api/generated/NumberFormat';
import type { TxnView } from './api/generated/TxnView';
import { formatAmount } from './format';

export type Filter = 'all' | 'expense' | 'income' | 'transfer' | 'missing';

export interface Row {
	txn: TxnView;
	/** What the row says: the description, else the counterparty. */
	title: string;
	/** Who paid or received: the payer, the destination, the source, the person settling. */
	who: string;
	/** The income or expense categories the transaction books to. */
	categories: AccountView[];
	flow: Flow;
	receipt: 'attached' | 'missing' | 'none';
}

/** One line of the table: a row, the amount to show and whether it is part of a split. */
export interface Item {
	row: Row;
	amount: Cents;
	split: boolean;
}

export interface Group {
	key: string;
	label: string;
	/** For categories: the category's total (its displayed balance). */
	total: Cents | null;
	account: AccountView | null;
	items: Item[];
}

const isCategory = (a: AccountView | undefined) => a?.kind === 'income' || a?.kind === 'expense';

export function rows(snapshot: EventSnapshot): Row[] {
	const byId = new Map(snapshot.accounts.map((a) => [a.id, a]));
	return snapshot.txns.map((txn) => ({
		txn,
		title: txn.description || txn.counterparty || '',
		who: (txn.who !== null && byId.get(txn.who)?.name) || '',
		categories: txn.lines.map((l) => byId.get(l.account)!).filter(isCategory),
		flow: txn.flow,
		receipt: txn.attachments.length ? 'attached' : txn.needsReceipt ? 'missing' : 'none'
	}));
}

export function matches(row: Row, filter: Filter, query: string, format: NumberFormat): boolean {
	const kind = row.txn.kind;
	const passes =
		filter === 'all' ||
		(filter === 'expense' && kind === 'expense') ||
		(filter === 'income' && (kind === 'income' || kind === 'refund')) ||
		(filter === 'transfer' && (kind === 'transfer' || kind === 'settle' || kind === 'opening')) ||
		(filter === 'missing' && row.receipt === 'missing');
	if (!passes) return false;
	const words = query.toLowerCase().split(/\s+/).filter(Boolean);
	if (!words.length) return true;
	const haystack = [
		row.title,
		row.txn.counterparty,
		row.txn.notes,
		row.who,
		...row.categories.map((c) => c.name),
		formatAmount(row.txn.amount, format, 'magnitude')
	]
		.filter(Boolean)
		.join(' ')
		.toLowerCase();
	return words.every((w) => haystack.includes(w));
}

export function counts(all: Row[]): Record<Filter, number> {
	const n = (f: Filter) => all.filter((r) => matches(r, f, '', 'de')).length;
	return {
		all: all.length,
		expense: n('expense'),
		income: n('income'),
		transfer: n('transfer'),
		missing: n('missing')
	};
}

/** Newest day first; within a day, the latest entry first. */
export function byDay(visible: Row[]): Group[] {
	const groups = new Map<string, Item[]>();
	for (const row of [...visible].sort((a, b) =>
		a.txn.date === b.txn.date ? b.txn.id - a.txn.id : a.txn.date < b.txn.date ? 1 : -1
	)) {
		const items = groups.get(row.txn.date) ?? [];
		items.push({ row, amount: row.txn.amount, split: false });
		groups.set(row.txn.date, items);
	}
	return [...groups].map(([date, items]) => ({
		key: date,
		label: date,
		total: null,
		account: null,
		items
	}));
}

/**
 * One group per category in the event's order, with its total; a split transaction appears
 * under each of its categories with that line's amount. Transfers and settling, which have no
 * category, come last.
 */
export function byCategory(visible: Row[], snapshot: EventSnapshot): Group[] {
	const groups: Group[] = snapshot.accounts
		.filter((a) => isCategory(a))
		.map((a) => ({ key: String(a.id), label: a.name, total: a.balance, account: a, items: [] }));
	const moved: Group = { key: 'moved', label: 'Transfers', total: null, account: null, items: [] };
	const sorted = [...visible].sort((a, b) =>
		a.txn.date === b.txn.date ? b.txn.id - a.txn.id : a.txn.date < b.txn.date ? 1 : -1
	);
	for (const row of sorted) {
		if (!row.categories.length) {
			moved.items.push({ row, amount: row.txn.amount, split: false });
			continue;
		}
		for (const line of row.txn.lines) {
			const group = groups.find((g) => g.account?.id === line.account);
			if (group)
				group.items.push({ row, amount: Math.abs(line.amount), split: row.categories.length > 1 });
		}
	}
	return [...groups, moved].filter((g) => g.items.length);
}

/**
 * The counterparties of a file's entries, most used first, each once however it was capitalised.
 * Spelling one the same way matters: the Stock and sales invoice check goes by the name.
 */
export function counterparties(txns: readonly { counterparty: string | null }[]): string[] {
	const seen = new Map<string, { name: string; count: number }>();
	for (const { counterparty } of txns) {
		const name = counterparty?.trim();
		if (!name) continue;
		const known = seen.get(name.toLowerCase());
		if (known) known.count++;
		else seen.set(name.toLowerCase(), { name, count: 1 });
	}
	return [...seen.values()]
		.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))
		.map((c) => c.name);
}
