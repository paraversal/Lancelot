// The ledger's rows, filters, search and grouping (R-TX-7). Pure functions over the snapshot;
// every amount shown comes from the core (a transaction's `amount`, a line's amount, an
// account's balance), so nothing here adds money up.
import type { AccountView } from './api/generated/AccountView';
import type { Cents } from './api/generated/Cents';
import type { EventSnapshot } from './api/generated/EventSnapshot';
import type { NumberFormat } from './api/generated/NumberFormat';
import type { TxnView } from './api/generated/TxnView';
import { formatAmount } from './format';

export type Filter = 'all' | 'expense' | 'income' | 'transfer' | 'missing';

export interface Row {
	txn: TxnView;
	/** What the row says: the description, else the counterparty. */
	title: string;
	/** Who paid or received: the payer, the destination, the person settling. */
	who: string;
	/** The income or expense categories the transaction books to. */
	categories: AccountView[];
	/** Money in (income), out (expense, ticket refund) or moved (transfer, settle, opening). */
	flow: 'in' | 'out' | 'moved';
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
	return snapshot.txns.map((txn) => {
		const accounts = txn.lines.map((l) => ({ line: l, account: byId.get(l.account) }));
		const categories = accounts.filter((x) => isCategory(x.account)).map((x) => x.account!);
		const parties = accounts.filter((x) => !isCategory(x.account) && x.account?.kind !== 'opening');
		// A ticket refund is negative income: money out, from whoever pays it (R-TKT-15).
		const refund = txn.origin.type === 'ticketRefund';
		const party =
			txn.kind === 'settle'
				? parties.find((x) => x.account?.kind === 'person')
				: txn.kind === 'income' && !refund
					? parties.find((x) => x.line.amount > 0)
					: (parties.find((x) => x.line.amount < 0) ?? parties[0]);
		const flow = txn.kind === 'expense' || refund ? 'out' : txn.kind === 'income' ? 'in' : 'moved';
		const receipt = txn.attachments.length
			? 'attached'
			: txn.kind === 'expense'
				? 'missing'
				: 'none';
		return {
			txn,
			title: txn.description || txn.counterparty || '',
			who: party?.account?.name ?? '',
			categories,
			flow,
			receipt
		};
	});
}

export function matches(row: Row, filter: Filter, query: string, format: NumberFormat): boolean {
	const kind = row.txn.kind;
	const passes =
		filter === 'all' ||
		(filter === 'expense' && kind === 'expense') ||
		(filter === 'income' && kind === 'income') ||
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
