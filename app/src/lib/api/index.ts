// Typed wrappers around the Tauri commands (docs/technical-design.md §5).
// The types are generated from Rust by ts-rs; see ./generated.
import { invoke as tauriInvoke, isTauri } from '@tauri-apps/api/core';

import type { AccountId } from './generated/AccountId';
import type { ApiError as ApiErrorData } from './generated/ApiError';
import type { AppInfo } from './generated/AppInfo';
import type { AppSettings } from './generated/AppSettings';
import type { BackupInfo } from './generated/BackupInfo';
import type { EventDetails } from './generated/EventDetails';
import type { OpenEvent } from './generated/OpenEvent';
import type { Parsed } from './generated/Parsed';
import type { ParsedDate } from './generated/ParsedDate';
import type { RecentEvent } from './generated/RecentEvent';
import type { Report } from './generated/Report';

/** A failed command. `code` is stable; `message` is for people. */
export class ApiError extends Error {
	readonly code: string;
	readonly field: string | null;
	readonly report: Report | null;

	constructor(data: ApiErrorData) {
		super(data.message);
		this.name = 'ApiError';
		this.code = data.code;
		this.field = data.field;
		this.report = data.report;
	}
}

function isErrorData(value: unknown): value is ApiErrorData {
	return typeof value === 'object' && value !== null && 'code' in value && 'message' in value;
}

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

/** Tauri's invoke in the app; in a plain browser during development, the mock backend. */
async function backend(): Promise<Invoke> {
	if (isTauri()) return tauriInvoke;
	if (import.meta.env.DEV) return (await import('./mock')).invoke;
	throw new ApiError({
		code: 'internal',
		message: 'not running in Lancelot',
		field: null,
		report: null
	});
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
	const invoke = await backend();
	try {
		return await invoke<T>(command, args);
	} catch (e) {
		if (e instanceof ApiError) throw e;
		if (isErrorData(e)) throw new ApiError(e);
		throw new ApiError({ code: 'internal', message: String(e), field: null, report: null });
	}
}

export const api = {
	appInfo: () => call<AppInfo>('app_info'),

	settings: () => call<AppSettings>('settings_get'),
	setSettings: (settings: AppSettings) => call<AppSettings>('settings_set', { settings }),
	recentEvents: () => call<RecentEvent[]>('recent_events'),
	forgetRecent: (path: string) => call<RecentEvent[]>('recent_forget', { path }),

	createEvent: (path: string, details: EventDetails) =>
		call<OpenEvent>('event_create', { path, details }),
	openEvent: (path: string) => call<OpenEvent>('event_open', { path }),
	/** Closes the file and releases its lock; closing the event itself is `close` (M5). */
	closeFile: () => call<void>('file_close'),
	currentEvent: () => call<OpenEvent | null>('event_current'),
	updateEvent: (details: EventDetails) => call<OpenEvent>('event_update', { details }),
	setDefaultReimbursementAccount: (account: AccountId) =>
		call<OpenEvent>('event_set_default_reimbursement_account', { account }),

	checkIntegrity: () => call<OpenEvent>('integrity_check'),
	/** Backups of an event file: the open one, or one that cannot be opened. */
	backups: (event: string) => call<BackupInfo[]>('backups_list', { event }),
	restoreBackup: (event: string, backup: string, target: string) =>
		call<OpenEvent>('backup_restore', { event, backup, target }),

	parseAmount: (input: string, allowNegative: boolean) =>
		call<Parsed>('amount_parse', { input, allowNegative }),
	parseDate: (input: string) => call<ParsedDate>('date_parse', { input })
};
