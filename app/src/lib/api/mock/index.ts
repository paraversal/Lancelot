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
import type { EventDetails } from '../generated/EventDetails';
import type { EventSnapshot } from '../generated/EventSnapshot';
import type { OpenEvent } from '../generated/OpenEvent';
import type { RecentEvent } from '../generated/RecentEvent';
import empty from './empty-event.json';
import example from './example-event.json';

const DIR = '/Users/you/Documents/Lancelot';
const EXAMPLE = `${DIR}/sommerfest-2026.lancelot`;
const DAMAGED = `${DIR}/winterfest-2025.lancelot`;
const LOCKED = `${DIR}/sommerfest-2025.lancelot`;

let settings: AppSettings = { numberFormat: 'de', backupCount: 20 };
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

function changed(f: (s: EventSnapshot) => void): OpenEvent {
	if (!open) fail('no_event_open', 'no event is open');
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

type Args = Record<string, unknown>;

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
	date_parse: (a) => parseDate(a.input as string)
};

export async function invoke<T>(command: string, args: Args = {}): Promise<T> {
	const handler = commands[command];
	if (!handler) fail('internal', `the mock has no command ${command}`);
	await new Promise((r) => setTimeout(r, 10));
	return structuredClone(handler(args)) as T;
}

/** Stand-ins for the native dialogs: fixed paths, so flows can be clicked through. */
export const dialogs = {
	open: async () => EXAMPLE,
	save: async (name: string) => `${DIR}/${name}`,
	reveal: async () => {}
};
