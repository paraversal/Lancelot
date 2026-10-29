// A development-only stand-in for the Tauri backend, so the screens can be looked at in a plain
// browser (`pnpm dev`). It serves the example event exported by the Rust tests
// (crates/lancelot-core/tests/mock_event.rs) and imitates the commands closely enough to click
// through the app. It is never part of a Tauri build: see ../index.ts.
//
// The amount and date parsers below are rough imitations for clicking around. The real ones
// live in Rust and are what the app uses.
import type { ApiError } from '../generated/ApiError';
import type { AppSettings } from '../generated/AppSettings';
import type { BackupInfo } from '../generated/BackupInfo';
import type { BlockerOverride } from '../generated/BlockerOverride';
import type { BudgetLineDraft } from '../generated/BudgetLineDraft';
import type { AccountKind } from '../generated/AccountKind';
import type { AssetType } from '../generated/AssetType';
import type { CountInput } from '../generated/CountInput';
import type { EventDetails } from '../generated/EventDetails';
import type { EventSnapshot } from '../generated/EventSnapshot';
import type { OpenEvent } from '../generated/OpenEvent';
import type { RecentEvent } from '../generated/RecentEvent';
import type { Line } from '../generated/Line';
import type { TxnDraft } from '../generated/TxnDraft';
import empty from './empty-event.json';
import example from './example-event.json';

const DIR = '/Users/you/Documents/Lancelot';
const EXAMPLE = `${DIR}/sommerfest-2026.lancelot`;
const DAMAGED = `${DIR}/winterfest-2025.lancelot`;
const LOCKED = `${DIR}/sommerfest-2025.lancelot`;

let settings: AppSettings = { numberFormat: 'de', backupCount: 20, reducedMotion: null };
let recent: RecentEvent[] = [
	{
		path: EXAMPLE,
		name: 'Sommerfest 2026',
		startsOn: '2026-06-12',
		endsOn: '2026-06-14',
		lastOpened: today(18, 2),
		exists: true
	},
	{
		path: DAMAGED,
		name: 'Winterfest 2025',
		startsOn: '2025-12-05',
		endsOn: '2025-12-07',
		lastOpened: '2026-01-08T20:15:00Z',
		exists: true
	},
	{
		path: LOCKED,
		name: 'Sommerfest 2025',
		startsOn: '2025-06-13',
		endsOn: '2025-06-15',
		lastOpened: '2025-07-02T09:30:00Z',
		exists: true
	},
	{
		path: '/Users/you/Desktop/kennenlernen.lancelot',
		name: 'Kennenlernabend',
		startsOn: '2025-04-18',
		endsOn: null,
		lastOpened: '2025-04-19T10:00:00Z',
		exists: false
	}
];
let open: OpenEvent | null = null;

function today(h: number, m: number): string {
	const d = new Date();
	d.setHours(h, m, 0, 0);
	return d.toISOString();
}

function fail(code: string, message: string, extra: Partial<ApiError> = {}): never {
	throw { code, message, field: null, report: null, ...extra } satisfies ApiError;
}

function opened(path: string, snapshot: EventSnapshot): OpenEvent {
	open = {
		revision: 1,
		path,
		fileName: path.split('/').pop() ?? path,
		snapshot: structuredClone(snapshot),
		integrity: { issues: [] },
		backupError: null
	};
	recent = [
		{
			path,
			name: snapshot.event.name,
			startsOn: snapshot.event.startsOn,
			endsOn: snapshot.event.endsOn,
			lastOpened: new Date().toISOString(),
			exists: true
		},
		...recent.filter((r) => r.path !== path)
	].slice(0, 10);
	return structuredClone(open);
}

function changed(f: (s: EventSnapshot) => void, whileClosed = false): OpenEvent {
	if (!open) fail('no_event_open', 'no event is open');
	if (open.snapshot.event.closedAt && !whileClosed)
		fail('event_closed', 'the event is closed; reopen it to make changes');
	f(open.snapshot);
	open.revision += 1;
	return structuredClone(open);
}

const backups: BackupInfo[] = [
	['2026-01-08T21:15:00Z'],
	['2026-01-08T20:02:00Z'],
	['2025-12-07T23:40:00Z']
].map(([t], i) => ({ path: `/backups/${t}.lancelot`, takenAt: t, size: 2_460_000 - i * 90_000 }));

function parseAmount(input: string, allowNegative: boolean) {
	let s = input.replace(/[\s€]/g, '');
	const negative = /^[-−]/.test(s);
	s = s.replace(/^[-+−]/, '');
	if (!s) fail('empty', 'no amount entered');
	if (/[^0-9.,]/.test(s)) fail('invalid_character', 'only digits, `.` and `,` are allowed');
	const last = Math.max(s.lastIndexOf('.'), s.lastIndexOf(','));
	const tail = last < 0 ? '' : s.slice(last + 1);
	let ambiguous = false;
	let euros = s;
	let cents = '00';
	const grouped = (head: string, sep: string) => {
		if (/[.,]/.test(head.replaceAll(sep, '')))
			fail('bad_grouping', 'thousands separators must form groups of three');
		const groups = head.split(sep);
		const ok =
			groups.length === 1 ||
			(groups[0].length >= 1 &&
				groups[0].length <= 3 &&
				groups.slice(1).every((g) => g.length === 3));
		if (!ok) fail('bad_grouping', 'thousands separators must form groups of three');
		return groups.join('');
	};
	if (last >= 0 && tail.length >= 1 && tail.length <= 2) {
		euros = grouped(s.slice(0, last), s[last] === '.' ? ',' : '.');
		cents = tail.padEnd(2, '0');
	} else if (last >= 0 && tail.length === 3) {
		const thousands = settings.numberFormat === 'de' ? '.' : ',';
		if (s.match(/[.,]/g)?.length === 1) {
			if (s[last] !== thousands) fail('too_many_decimals', 'at most two decimals are allowed');
			ambiguous = true;
		}
		euros = grouped(s, s[last]);
	} else if (last >= 0) {
		fail(tail.length ? 'too_many_decimals' : 'bad_grouping', 'cannot read this amount');
	}
	if (negative && !allowNegative)
		fail('negative_not_allowed', 'negative amounts are not allowed here');
	const value = Number(euros || '0') * 100 + Number(cents);
	return { cents: negative ? -value : value, ambiguous };
}

function parseDate(input: string) {
	const s = input.trim();
	if (!s) fail('empty', 'no date entered');
	let m = s.match(/^(\d{1,2})\.(\d{1,2})\.(\d{2}|\d{4})$/);
	if (m) {
		const y = m[3].length === 2 ? `20${m[3]}` : m[3];
		return check(`${y}-${m[2].padStart(2, '0')}-${m[1].padStart(2, '0')}`);
	}
	m = s.match(/^(\d{4})-(\d{1,2})-(\d{1,2})$/);
	if (m) return check(`${m[1]}-${m[2].padStart(2, '0')}-${m[3].padStart(2, '0')}`);
	return fail('format', 'enter a date as 31.12.2026 or 2026-12-31');

	function check(iso: string) {
		const d = new Date(`${iso}T00:00:00Z`);
		if (Number.isNaN(d.getTime()) || d.toISOString().slice(0, 10) !== iso)
			fail('no_such_day', 'this day does not exist');
		return { date: iso };
	}
}

// Transactions in the mock: lines in the shape the core builds, but no balances are
// recomputed. Good enough to click through the ledger.
function lines(draft: TxnDraft): Line[] {
	const e = draft.entry;
	const line = (account: number, amount: number, budgetLine: number | null = null): Line => ({
		account,
		amount,
		memo: null,
		budgetLine
	});
	switch (e.type) {
		case 'expense':
			return [
				line(e.payer, -e.lines.reduce((s, l) => s + l.amount, 0)),
				...e.lines.map((l) => line(l.account, l.amount, l.budgetLine))
			];
		case 'income':
			return [
				...e.lines.map((l) => line(l.account, -l.amount, l.budgetLine)),
				line(
					e.destination,
					e.lines.reduce((s, l) => s + l.amount, 0)
				)
			];
		case 'transfer':
			return [line(e.from, -e.amount), line(e.to, e.amount)];
		case 'settle':
			return e.direction === 'reimburse'
				? [line(e.account, -e.amount), line(e.person, e.amount)]
				: [line(e.person, -e.amount), line(e.account, e.amount)];
		case 'opening':
			return [line(3, -e.amount), line(e.account, e.amount)];
	}
}

function saveTxn(draft: TxnDraft, id: number | null, files: string[], remove: number[]) {
	const built = lines(draft);
	const amount = built.filter((l) => l.amount > 0).reduce((s, l) => s + l.amount, 0);
	if (draft.total !== null && draft.total !== undefined && draft.total !== amount)
		fail('split_mismatch', 'the lines do not add up to the total', { field: 'lines' });
	let saved = id ?? 0;
	const event = changed((s) => {
		saved = id ?? Math.max(0, ...s.txns.map((t) => t.id)) + 1;
		const txn = {
			id: saved,
			kind: draft.entry.type,
			date: draft.date,
			description: draft.description,
			counterparty: draft.counterparty,
			notes: draft.notes,
			lines: built,
			amount,
			attachments: [
				...(s.txns.find((t) => t.id === id)?.attachments ?? []).filter(
					(x) => !remove.includes(x.id)
				),
				...files.map((path, i) => {
					const filename = path.split('/').pop() ?? path;
					return {
						id: Date.now() + i,
						txn: saved,
						filename,
						mime: /\.pdf$/i.test(filename)
							? 'application/pdf'
							: /\.(jpe?g|png|heic)$/i.test(filename)
								? 'image/jpeg'
								: 'application/octet-stream',
						size: 184_000,
						sha256: '',
						addedAt: new Date().toISOString()
					};
				})
			]
		};
		s.txns = [...s.txns.filter((t) => t.id !== saved), txn].sort((a, b) =>
			a.date === b.date ? a.id - b.id : a.date < b.date ? -1 : 1
		);
		rebudget(s);
	});
	return { event, id: saved, warnings: [] };
}

function addAccount(kind: AccountKind, name: string, assetType: AssetType | null = null) {
	let id = 0;
	const event = changed((s) => {
		if (
			s.accounts.some((a) => a.kind === kind && a.name.toLowerCase() === name.trim().toLowerCase())
		)
			fail('duplicate_name', 'this name is already used', { field: 'name' });
		id = Math.max(...s.accounts.map((a) => a.id)) + 1;
		s.accounts.push({
			id,
			kind,
			name: name.trim(),
			role: null,
			assetType: kind === 'asset' ? (assetType ?? 'bank') : null,
			balance: 0
		});
		if (kind === 'asset') s.whereTheMoneyIs.assets.push({ account: id, balance: 0 });
		if (kind === 'person')
			s.persons.push({
				account: id,
				name: name.trim(),
				holder: null,
				iban: null,
				bic: null,
				contact: null,
				notes: null,
				summary: {
					advanced: 0,
					held: 0,
					reimbursed: 0,
					repaid: 0,
					settled: 0,
					net: 0,
					status: 'settled'
				},
				entries: []
			});
	});
	return { event, id };
}

// The counts of an asset account, roughly as the core derives them: the opening count moves the
// balance, the closing count is compared with it, and cash accounts need a closing count.
function recount(
	s: EventSnapshot,
	id: number,
	opening?: CountInput | null,
	closing?: CountInput | null
) {
	const account = s.accounts.find((a) => a.id === id);
	if (!account?.assetType) fail('wrong_account_kind', 'not an asset account', { field: 'account' });
	let status = s.cash.find((c) => c.account === id);
	if (!status) {
		status = {
			account: id,
			assetType: account.assetType,
			opening: null,
			computed: account.balance,
			counted: null,
			difference: null
		};
		s.cash.push(status);
	}
	status.assetType = account.assetType;
	if (opening !== undefined) {
		const amount = opening && opening.amount > 0 ? opening.amount : 0;
		const delta = amount - (status.opening?.amount ?? 0);
		account.balance += delta;
		s.whereTheMoneyIs.opening += delta;
		const where = s.whereTheMoneyIs.assets.find((a) => a.account === id);
		if (where) where.balance = account.balance;
		status.opening = amount
			? { txn: status.opening?.txn ?? 0, date: parseDate(opening!.date).date, amount }
			: null;
	}
	if (closing !== undefined) {
		s.closingCounts = s.closingCounts.filter((c) => c.account !== id);
		if (closing)
			s.closingCounts.push({
				account: id,
				date: parseDate(closing.date).date,
				amount: closing.amount
			});
		status.counted = closing?.amount ?? null;
	}
	status.computed = account.balance;
	status.difference = status.counted === null ? null : status.counted - status.computed;
	if (account.assetType === 'bank' && !status.opening && status.counted === null)
		s.cash = s.cash.filter((c) => c.account !== id);
	s.openItems = s.openItems.filter(
		(i) => !((i.type === 'closingCountMissing' || i.type === 'cashDifference') && i.account === id)
	);
	if (account.assetType === 'cash' && status.counted === null)
		s.openItems.push({ type: 'closingCountMissing', account: id });
	else if (status.difference)
		s.openItems.push({ type: 'cashDifference', account: id, difference: status.difference });
}

type Args = Record<string, unknown>;

// Budget lines in the mock. The figures are redone with the core's rules (R-BUD-4) so the
// Budget page can be clicked through; the app gets them from the core.
function checkBudgetLine(s: EventSnapshot, d: BudgetLineDraft) {
	const kind = s.accounts.find((a) => a.id === d.category)?.kind;
	if (kind !== 'income' && kind !== 'expense')
		fail('wrong_account_kind', 'this account cannot be used here', { field: 'category' });
	if (!d.description.trim()) fail('empty_name', 'a name is needed', { field: 'description' });
	if (d.estimate.type === 'quantity' && d.estimate.quantity < 1)
		fail('non_positive_amount', 'amount must be greater than zero', {
			field: 'estimate.quantity'
		});
}

function rebudget(s: EventSnapshot) {
	const kindOf = (id: number) => s.accounts.find((a) => a.id === id)?.kind;
	const linked = new Map<number, number>();
	for (const l of s.txns.flatMap((t) => t.lines))
		if (l.budgetLine !== null)
			linked.set(
				l.budgetLine,
				(linked.get(l.budgetLine) ?? 0) + (kindOf(l.account) === 'income' ? -l.amount : l.amount)
			);
	s.budget.lines = s.budgetLines.map((b) => {
		const estimate =
			b.estimate.type === 'amount' ? b.estimate.amount : b.estimate.quantity * b.estimate.unitPrice;
		const actual = linked.get(b.id) ?? 0;
		const forecast = b.complete
			? actual
			: kindOf(b.category) === 'income'
				? Math.min(estimate, actual)
				: Math.max(estimate, actual);
		return { line: b.id, estimate, actual, forecast };
	});
	const zero = () => ({ estimate: 0, actual: 0, forecast: 0 });
	const totals = { income: zero(), expense: zero() };
	s.budget.categories = s.budget.categories.map((c) => {
		const own = s.budget.lines.filter(
			(f) => s.budgetLines.find((b) => b.id === f.line)?.category === c.category
		);
		const sum = (k: 'estimate' | 'actual' | 'forecast') => own.reduce((t, f) => t + f[k], 0);
		const unbudgeted = c.actual - sum('actual');
		const next = {
			...c,
			estimate: sum('estimate'),
			forecast: sum('forecast') + unbudgeted,
			unbudgeted
		};
		const total = totals[kindOf(c.category) === 'income' ? 'income' : 'expense'];
		for (const k of ['estimate', 'actual', 'forecast'] as const) total[k] += next[k];
		return next;
	});
	s.budget.income = totals.income;
	s.budget.expense = totals.expense;
	for (const k of ['estimate', 'actual', 'forecast'] as const)
		s.budget.result[k] = totals.income[k] - totals.expense[k];
}

const commands: Record<string, (a: Args) => unknown> = {
	app_info: () => ({ version: 'dev', schemaVersion: 1 }),
	settings_get: () => settings,
	settings_set: (a) => (settings = a.settings as AppSettings),
	recent_events: () => recent,
	recent_forget: (a) => (recent = recent.filter((r) => r.path !== a.path)),
	event_create: (a) => {
		const path = a.path as string;
		if (recent.some((r) => r.path === path))
			fail('already_exists', 'a file with this name already exists');
		const snapshot = structuredClone(empty as EventSnapshot);
		snapshot.event = { ...snapshot.event, ...(a.details as EventDetails) };
		return opened(path, snapshot);
	},
	event_open: (a) => {
		const path = a.path as string;
		if (path === DAMAGED)
			fail('damaged', 'the event file is damaged and cannot be opened', {
				report: {
					issues: [
						{ severity: 'error', code: 'unsealed', detail: 'transaction 14 is not sealed' },
						{ severity: 'error', code: 'unbalanced', detail: 'transaction 14 does not balance' }
					]
				}
			});
		if (path === LOCKED) fail('already_open', 'this event file is already open');
		if (!recent.find((r) => r.path === path)?.exists && path !== EXAMPLE)
			fail('io', 'No such file or directory');
		return opened(path, example as EventSnapshot);
	},
	file_close: () => (open = null),
	event_current: () => (open ? structuredClone(open) : null),
	event_update: (a) => changed((s) => (s.event = { ...s.event, ...(a.details as EventDetails) })),
	event_set_default_reimbursement_account: (a) =>
		changed((s) => (s.event.defaultReimbursementAccount = a.account as number)),
	integrity_check: () => changed(() => {}),
	backups_list: () => backups,
	backup_restore: (a) => opened(a.target as string, example as EventSnapshot),
	amount_parse: (a) => parseAmount(a.input as string, a.allowNegative as boolean),
	date_parse: (a) => parseDate(a.input as string),
	txn_save: (a) =>
		saveTxn(
			a.draft as TxnDraft,
			a.id as number | null,
			(a.files as string[]) ?? [],
			(a.remove as number[]) ?? []
		),
	attachment_open: () => null,
	attachment_export: () => null,
	txn_delete: (a) =>
		changed((s) => {
			s.txns = s.txns.filter((t) => t.id !== a.id);
			rebudget(s);
		}),
	account_add: (a) => addAccount(a.kind as AccountKind, a.name as string),
	account_rename: (a) =>
		changed((s) => {
			const account = s.accounts.find((x) => x.id === a.id);
			if (account) account.name = a.name as string;
		}),
	asset_add: (a) => {
		const added = addAccount('asset', a.name as string, a.assetType as AssetType);
		return { ...added, event: changed((s) => recount(s, added.id)) };
	},
	asset_set_type: (a) =>
		changed((s) => {
			const account = s.accounts.find((x) => x.id === a.id);
			if (account?.kind !== 'asset') fail('wrong_account_kind', 'not an asset account');
			account.assetType = a.assetType as AssetType;
			recount(s, account.id);
		}),
	counts_set: (a) =>
		changed((s) =>
			recount(
				s,
				a.account as number,
				a.opening as CountInput | null,
				a.closing as CountInput | null
			)
		),
	person_add: (a) => addAccount('person', a.name as string),
	person_update: (a) =>
		changed((s) => {
			const id = a.id as number;
			const name = (a.name as string).trim();
			const person = s.persons.find((p) => p.account === id);
			const account = s.accounts.find((x) => x.id === id);
			if (!person || !account) fail('unknown_person', 'unknown person');
			Object.assign(person, a.person as object, { name });
			account.name = name;
			// Keep the payout list in step with the new details.
			const row = s.payouts.payouts.find((r) => r.person === id);
			if (row) {
				row.name = name;
				row.iban = person.iban;
				row.holder = person.holder ?? name;
				row.bic = person.bic;
				s.payouts.payableTotal = s.payouts.payouts
					.filter((r) => r.iban)
					.reduce((t, r) => t + r.amount, 0);
			}
		}),
	budget_line_add: (a) =>
		changed((s) => {
			const draft = a.draft as BudgetLineDraft;
			checkBudgetLine(s, draft);
			const id = Math.max(0, ...s.budgetLines.map((b) => b.id)) + 1;
			s.budgetLines.push({ id, ...draft, description: draft.description.trim() });
			rebudget(s);
		}),
	budget_line_update: (a) =>
		changed((s) => {
			const draft = a.draft as BudgetLineDraft;
			checkBudgetLine(s, draft);
			const line = s.budgetLines.find((b) => b.id === a.id);
			if (!line) fail('unknown_budget_line', 'unknown budget line');
			const linked = s.txns.some((t) => t.lines.some((l) => l.budgetLine === line.id));
			if (linked && draft.category !== line.category)
				fail(
					'budget_line_in_use',
					'transactions are linked to this budget line, so its category cannot change',
					{ field: 'category' }
				);
			Object.assign(line, draft, { description: draft.description.trim() });
			rebudget(s);
		}),
	budget_line_delete: (a) =>
		changed((s) => {
			s.budgetLines = s.budgetLines.filter((b) => b.id !== a.id);
			for (const l of s.txns.flatMap((t) => t.lines))
				if (l.budgetLine === a.id) l.budgetLine = null;
			rebudget(s);
		}),
	payouts_export_csv: () => null,
	transactions_export_csv: () => null,
	event_close: (a) =>
		changed((s) => {
			if (s.event.closedAt) return;
			const overrides = a.overrides as BlockerOverride[];
			const key = (x: unknown) => JSON.stringify(x);
			const blockers = s.openItems.filter(
				(i) => !overrides.some((o) => key(o.item) === key(i) && o.note.trim())
			);
			if (blockers.length)
				fail('closing_blocked', 'open items need to be resolved or accepted with a note');
			s.event.closedAt = new Date().toISOString();
			s.event.closingOverrides = overrides;
		}, true),
	event_reopen: () =>
		changed((s) => {
			s.event.closedAt = null;
			s.event.closingOverrides = [];
		}, true),
	payouts_mark_paid: () =>
		changed((s) => {
			const paid = new Set(s.payouts.payouts.filter((p) => p.iban).map((p) => p.person));
			for (const p of s.persons)
				if (paid.has(p.account)) p.summary = { ...p.summary, net: 0, status: 'settled' };
			s.payouts = {
				...s.payouts,
				payouts: s.payouts.payouts.filter((p) => !paid.has(p.person)),
				payableTotal: 0
			};
		}),
	iban_check: (a) => {
		const iban = (a.input as string).replace(/\s+/g, '').toUpperCase();
		if (!/^[A-Z]{2}\d{2}[A-Z0-9]{10,30}$/.test(iban))
			fail('wrong_length', 'an IBAN for this country has 22 characters');
		return { iban, formatted: iban.replace(/(.{4})/g, '$1 ').trim() };
	}
};

export async function invoke<T>(command: string, args: Args = {}): Promise<T> {
	const handler = commands[command];
	if (!handler) fail('internal', `the mock has no command ${command}`);
	await new Promise((r) => setTimeout(r, 10));
	return structuredClone(handler(args)) as T;
}

/** Stand-ins for the native dialogs: fixed paths, so flows can be clicked through. */
/** A one-page invoice, so the report has a PDF to draw with pdf.js. */
function invoicePdf(filename: string) {
	const text = `BT /F1 20 Tf 48 520 Td (Invoice) Tj /F1 11 Tf 0 -36 Td (Kulturhof Leipzig) Tj 0 -18 Td (Hall rent 12.-14.06.2026) Tj 0 -36 Td (Total 1.800,00 EUR) Tj 0 -60 Td (${filename}) Tj ET`;
	const pdf = [
		'%PDF-1.4',
		'1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj',
		'2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj',
		'3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 420 595]/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>endobj',
		`4 0 obj<</Length ${text.length}>>stream`,
		text,
		'endstream endobj',
		'5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj',
		'trailer<</Root 1 0 R>>',
		'%%EOF'
	].join('\n');
	return `data:application/pdf;base64,${btoa(pdf)}`;
}

/** A drawn receipt, so previews have something to show in the browser. */
export function attachmentUrl(attachment: { filename: string; mime?: string }) {
	if (attachment.mime === 'application/pdf') return invoicePdf(attachment.filename);
	const lines = [
		'METRO Leipzig',
		'13.06.2026  10:42',
		'',
		'Brötchen 60x   18,00',
		'Kaffee 2 kg    31,90',
		'Becher 200x    12,50',
		'',
		'SUMME         312,40'
	];
	const text = lines
		.map(
			(l, i) => `<text x="28" y="${48 + i * 26}" font-family="monospace" font-size="14">${l}</text>`
		)
		.join('');
	const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="300" height="460"><rect width="300" height="460" fill="white"/>${text}<text x="28" y="430" font-family="monospace" font-size="11" fill="#999">${attachment.filename}</text></svg>`;
	return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

export const dialogs = {
	open: async () => EXAMPLE,
	receipts: async () => [`${DIR}/scan-${String(Date.now()).slice(-4)}.jpg`],
	save: async (name: string) => `${DIR}/${name}`,
	reveal: async () => {}
};
