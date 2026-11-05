// The change log as people read it (R-TX-6): the entries one command wrote become one change, and
// each entry says what it is and which of its fields moved. Names are as they were at the time,
// which also keeps them for what has been deleted since.
import type { Account } from './api/generated/Account';
import type { Assumption } from './api/generated/Assumption';
import type { Attachment } from './api/generated/Attachment';
import type { BudgetLine } from './api/generated/BudgetLine';
import type { Cancellation } from './api/generated/Cancellation';
import type { ChangeLogEntry } from './api/generated/ChangeLogEntry';
import type { ClosingCount } from './api/generated/ClosingCount';
import type { CostCentre } from './api/generated/CostCentre';
import type { Event } from './api/generated/Event';
import type { ManualSale } from './api/generated/ManualSale';
import type { NumberFormat } from './api/generated/NumberFormat';
import type { Person } from './api/generated/Person';
import type { PluginRecord } from './api/generated/PluginRecord';
import type { Reconciliation } from './api/generated/Reconciliation';
import type { TicketImport } from './api/generated/TicketImport';
import type { TicketSettings } from './api/generated/TicketSettings';
import type { Tier } from './api/generated/Tier';
import type { Txn } from './api/generated/Txn';
import type { TxnKind } from './api/generated/TxnKind';
import { formatAmount, formatDate, formatSize } from './format';

export type HistoryField = { label: string; before: string | null; after: string | null };

export type HistoryRow = {
	id: number;
	entity: string;
	entityId: number;
	verb: string;
	noun: string;
	name: string;
	fields: HistoryField[];
};

/** What one command changed. */
export type HistoryChange = { at: string; rows: HistoryRow[] };

type Field = { key: string; label: string; text: string | null };
type Spec = { noun: string; name: string; fields: Field[] };

const VERBS: Record<string, string> = {
	create: 'Added',
	update: 'Edited',
	delete: 'Deleted',
	close: 'Closed',
	reopen: 'Reopened'
};

const TXN_NOUNS: Record<TxnKind, string> = {
	expense: 'Expense',
	income: 'Income',
	transfer: 'Transfer',
	settle: 'Settlement',
	opening: 'Opening count',
	refund: 'Refund'
};

const field = (label: string, text: string | null | undefined): Field => ({
	key: label,
	label,
	text: text || null
});

const yes = (value: boolean) => (value ? 'Yes' : null);

function humanize(key: string): string {
	const words = key.replace(/([a-z])([A-Z])/g, '$1 $2').replace(/_/g, ' ');
	return words.charAt(0).toUpperCase() + words.slice(1).toLowerCase();
}

/** The top level of a value we know nothing about. */
function rawFields(value: unknown): Field[] {
	if (typeof value !== 'object' || value === null || Array.isArray(value))
		return [field('Value', JSON.stringify(value))];
	return Object.entries(value).map(([key, v]) =>
		field(humanize(key), typeof v === 'string' ? v : v === null ? null : JSON.stringify(v))
	);
}

/** Describes the entries of one file, oldest first, so that names can be followed. */
class Describer {
	private names = new Map<string, string>();
	private incomes = new Set<number>();

	constructor(
		private format: NumberFormat,
		private plugins: Record<string, string>
	) {}

	private named(entity: string, id: number | null): string | null {
		if (id === null) return null;
		return this.names.get(`${entity}:${id}`) ?? `#${id}`;
	}

	private amount(cents: number) {
		return formatAmount(cents, this.format);
	}

	private date(iso: string | null) {
		return iso ? formatDate(iso, this.format) : null;
	}

	private percent(bp: number) {
		const text = String(bp / 100);
		return `${this.format === 'de' ? text.replace('.', ',') : text} %`;
	}

	/** Remembers the names an entry brings, before any entry of its change is described. */
	learn(e: ChangeLogEntry) {
		const value = (e.after ?? e.before) as Record<string, unknown> | null;
		const name =
			e.entity === 'txn' || e.entity === 'budget_line' ? value?.description : value?.name;
		if (typeof name === 'string') this.names.set(`${e.entity}:${e.entityId}`, name);
		if (e.entity === 'account' && value?.kind === 'income') this.incomes.add(e.entityId);
	}

	/** One line per account, so an edit shows the accounts that moved, not the whole entry. */
	private lines(txn: Txn): Field[] {
		const byAccount = new Map<number, string[]>();
		for (const l of txn.lines) {
			const parts = [
				// Income is credited: its lines are negative in the book, and read as money coming in.
				this.amount(this.incomes.has(l.account) ? -l.amount : l.amount),
				this.named('budget_line', l.budgetLine),
				this.named('cost_centre', l.costCentre)
			];
			const text = parts.filter(Boolean).join(' · ');
			byAccount.set(l.account, [...(byAccount.get(l.account) ?? []), text]);
		}
		return [...byAccount].map(([account, texts]) => ({
			key: `line:${account}`,
			label: this.named('account', account)!,
			text: texts.sort().join(', ')
		}));
	}

	private spec(entity: string, id: number, value: unknown): Spec {
		switch (entity) {
			case 'event': {
				const v = value as Event;
				return {
					noun: 'Event',
					name: v.name,
					fields: [
						field('Name', v.name),
						field('Starts', this.date(v.startsOn)),
						field('Ends', this.date(v.endsOn)),
						field('Location', v.location),
						field('Notes', v.notes),
						field('Reimburses from', this.named('account', v.defaultReimbursementAccount)),
						field('Accepted as open', v.closingOverrides.map((o) => o.note).join(', '))
					]
				};
			}
			case 'account': {
				const v = value as Account;
				const nouns = {
					asset: 'Account',
					opening: 'Opening account',
					person: 'Person',
					income: 'Income category',
					expense: 'Expense category'
				};
				return {
					noun: nouns[v.kind],
					name: v.name,
					fields: [
						field('Name', v.name),
						field('Type', v.assetType && humanize(v.assetType)),
						field('Net movement', yes(v.netMovement)),
						field('Cost centre', this.named('cost_centre', v.costCentre))
					]
				};
			}
			case 'person': {
				const v = value as Person;
				return {
					noun: 'Person',
					name: this.named('account', id)!,
					fields: [
						field('Holder', v.holder),
						field('IBAN', v.iban),
						field('BIC', v.bic),
						field('Other method', v.otherMethod),
						field('Contact', v.contact),
						field('Notes', v.notes)
					]
				};
			}
			case 'txn': {
				const v = value as Txn;
				return {
					noun: TXN_NOUNS[v.kind],
					name: v.description,
					fields: [
						field('Description', v.description),
						field('Date', this.date(v.date)),
						field('Counterparty', v.counterparty),
						...this.lines(v),
						field('Notes', v.notes)
					]
				};
			}
			case 'attachment': {
				const v = value as Attachment;
				return {
					noun: 'Attachment',
					name: v.filename,
					fields: [
						field('On', this.named('txn', v.txn)),
						field('Size', formatSize(v.size, this.format))
					]
				};
			}
			case 'closing_count': {
				const v = value as ClosingCount;
				return {
					noun: 'Closing count',
					name: this.named('account', v.account)!,
					fields: [field('Date', this.date(v.date)), field('Amount', this.amount(v.amount))]
				};
			}
			case 'reconciliation': {
				const v = value as Reconciliation;
				return {
					noun: 'Statement',
					name: this.named('account', v.account)!,
					fields: [field('Date', this.date(v.date)), field('Amount', this.amount(v.amount))]
				};
			}
			case 'budget_line': {
				const v = value as BudgetLine;
				const estimate =
					v.estimate.type === 'amount'
						? this.amount(v.estimate.amount)
						: [
								String(v.estimate.quantity),
								...(v.estimate.per ?? []).map((a) => this.named('assumption', a)),
								this.amount(v.estimate.unitPrice)
							].join(' × ');
				return {
					noun: 'Budget line',
					name: v.description,
					fields: [
						field('Description', v.description),
						field('Category', this.named('account', v.category)),
						field('Estimate', estimate),
						field('Cost centre', this.named('cost_centre', v.costCentre)),
						field('Complete', yes(v.complete))
					]
				};
			}
			case 'assumption': {
				const v = value as Assumption;
				return {
					noun: 'Assumption',
					name: v.name,
					fields: [field('Name', v.name), field('Value', String(v.value))]
				};
			}
			case 'cost_centre': {
				const v = value as CostCentre;
				return { noun: 'Cost centre', name: v.name, fields: [field('Name', v.name)] };
			}
			case 'ticket_settings': {
				const v = value as TicketSettings;
				return {
					noun: 'Ticket settings',
					name: '',
					fields: [
						field('Sells tickets', yes(v.enabled)),
						field('Account', this.named('account', v.account))
					]
				};
			}
			case 'ticket_tier': {
				const v = value as Tier;
				return {
					noun: 'Ticket tier',
					name: v.name,
					fields: [
						field('Name', v.name),
						field('Price', this.amount(v.price)),
						field('Fee', this.percent(v.feeBp)),
						field('Quota', v.quota === null ? null : String(v.quota)),
						field('Also called', v.aliases.join(', '))
					]
				};
			}
			case 'ticket_import': {
				const v = value as TicketImport;
				return {
					noun: 'Ticket import',
					name: v.filename,
					fields: [field('Date', this.date(v.date))]
				};
			}
			case 'ticket_manual_sale': {
				const v = value as ManualSale;
				return {
					noun: 'Ticket sale',
					name: this.named('ticket_tier', v.tier)!,
					fields: [
						field('Tickets', String(v.count)),
						field('Received by', this.named('account', v.destination)),
						field('Date', this.date(v.date)),
						field('Reference', v.reference)
					]
				};
			}
			case 'ticket_cancellation': {
				const v = value as Cancellation;
				return {
					noun: 'Cancellation',
					name: this.named('ticket_tier', v.tier)!,
					fields: [
						field('Tickets', String(v.count)),
						field('Gone from the sheet', v.gone ? String(v.gone) : null),
						field('Refund', v.terms && this.percent(v.terms.refundBp)),
						field('Date', v.terms && this.date(v.terms.date)),
						field('Paid by', v.terms && this.named('account', v.terms.payer)),
						field('Note', v.note)
					]
				};
			}
			case 'plugin_record': {
				const v = value as PluginRecord;
				return {
					noun: this.plugins[v.plugin] ?? v.plugin,
					name: recordName(v.key),
					fields: rawFields(v.value)
				};
			}
			default:
				return { noun: humanize(entity), name: `#${id}`, fields: rawFields(value) };
		}
	}

	describe(e: ChangeLogEntry): HistoryRow {
		const before = e.before === null ? null : this.spec(e.entity, e.entityId, e.before);
		const after = e.after === null ? null : this.spec(e.entity, e.entityId, e.after);
		const { noun, name } = (after ?? before)!;
		return {
			id: e.id,
			entity: e.entity,
			entityId: e.entityId,
			verb: VERBS[e.op] ?? humanize(e.op),
			noun,
			name,
			fields: changedFields(before?.fields ?? null, after?.fields ?? null, name)
		};
	}
}

/** A plugin record's key, or what a data source saved in the event is called (`data-source:Drinks`). */
function recordName(key: string): string {
	if (key === 'data-source') return 'Data source';
	const tab = key.startsWith('data-source:') ? key.slice('data-source:'.length) : null;
	return tab ? `Data source · ${tab}` : key;
}

/**
 * What differs between two versions. Adding or deleting shows the one version there is, without
 * what the row's name already says.
 */
function changedFields(
	before: Field[] | null,
	after: Field[] | null,
	name: string
): HistoryField[] {
	const out: HistoryField[] = [];
	const keys = [...(after ?? []), ...(before ?? [])].map((f) => f.key);
	for (const key of new Set(keys)) {
		const b = before?.find((f) => f.key === key) ?? null;
		const a = after?.find((f) => f.key === key) ?? null;
		const was = b?.text ?? null;
		const is = a?.text ?? null;
		if (was === is) continue;
		if ((before === null || after === null) && (was ?? is) === name) continue;
		out.push({ label: (a ?? b)!.label, before: was, after: is });
	}
	return out;
}

/** A person is two rows of the file, its account and its details, and one row here. */
function joinPeople(rows: HistoryRow[]): HistoryRow[] {
	const accounts = new Map(
		rows.filter((r) => r.entity === 'account').map((r) => [r.entityId, r] as const)
	);
	return rows.filter((row) => {
		const account = row.entity === 'person' ? accounts.get(row.entityId) : undefined;
		if (account) account.fields.push(...row.fields);
		return !account;
	});
}

/** The log, newest first, as changes, newest first. An edit that moved nothing shows nothing. */
export function historyChanges(
	log: ChangeLogEntry[],
	format: NumberFormat,
	plugins: Record<string, string> = {}
): HistoryChange[] {
	const describer = new Describer(format, plugins);
	const entries = [...log].sort((a, b) => a.id - b.id);
	const changes: HistoryChange[] = [];
	for (let start = 0; start < entries.length;) {
		let end = start;
		while (end < entries.length && entries[end].at === entries[start].at) end++;
		const group = entries.slice(start, end);
		group.forEach((e) => describer.learn(e));
		const rows = joinPeople(group.map((e) => describer.describe(e))).filter(
			(r) => r.fields.length || r.verb !== 'Edited'
		);
		if (rows.length) changes.push({ at: group[0].at, rows });
		start = end;
	}
	return changes.reverse();
}

/** The rows of a change a search finds. */
export function matchingRows(change: HistoryChange, query: string): HistoryRow[] {
	const q = query.trim().toLowerCase();
	if (!q) return change.rows;
	return change.rows.filter((r) =>
		[r.verb, r.noun, r.name, ...r.fields.flatMap((f) => [f.label, f.before, f.after])]
			.join(' ')
			.toLowerCase()
			.includes(q)
	);
}

const two = (n: number) => String(n).padStart(2, '0');

/** The ISO date of a timestamp in the user's time zone. */
export function localDay(iso: string): string {
	const t = new Date(iso);
	return `${t.getFullYear()}-${two(t.getMonth() + 1)}-${two(t.getDate())}`;
}

/** `18:02` in the user's time zone. */
export function localTime(iso: string): string {
	const t = new Date(iso);
	return `${two(t.getHours())}:${two(t.getMinutes())}`;
}
