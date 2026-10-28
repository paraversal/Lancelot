// Typed wrappers around the Tauri commands (docs/technical-design.md §5).
// The types are generated from Rust by ts-rs; see ./generated.
import { convertFileSrc, invoke as tauriInvoke, isTauri } from '@tauri-apps/api/core';

import type { AccountId } from './generated/AccountId';
import type { AccountKind } from './generated/AccountKind';
import type { Attachment } from './generated/Attachment';
import type { AssetType } from './generated/AssetType';
import type { AttachmentId } from './generated/AttachmentId';
import type { Added } from './generated/Added';
import type { ApiError as ApiErrorData } from './generated/ApiError';
import type { AppInfo } from './generated/AppInfo';
import type { AppSettings } from './generated/AppSettings';
import type { BackupInfo } from './generated/BackupInfo';
import type { CheckedIban } from './generated/CheckedIban';
import type { CountInput } from './generated/CountInput';
import type { EventDetails } from './generated/EventDetails';
import type { OpenEvent } from './generated/OpenEvent';
import type { Parsed } from './generated/Parsed';
import type { Person } from './generated/Person';
import type { ParsedDate } from './generated/ParsedDate';
import type { RecentEvent } from './generated/RecentEvent';
import type { Report } from './generated/Report';
import type { SavedTxn } from './generated/SavedTxn';
import type { TxnDraft } from './generated/TxnDraft';
import type { TxnId } from './generated/TxnId';

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
	parseDate: (input: string) => call<ParsedDate>('date_parse', { input }),

	/** Saves a transaction with the files to attach (paths) and the attachments to remove. */
	saveTxn: (draft: TxnDraft, id: TxnId | null, files: string[] = [], remove: AttachmentId[] = []) =>
		call<SavedTxn>('txn_save', { draft, id, files, remove }),
	openAttachment: (id: AttachmentId) => call<void>('attachment_open', { id }),
	exportAttachment: (id: AttachmentId, target: string) =>
		call<void>('attachment_export', { id, target }),
	deleteTxn: (id: TxnId) => call<OpenEvent>('txn_delete', { id }),
	addAccount: (kind: AccountKind, name: string) => call<Added>('account_add', { kind, name }),
	renameAccount: (id: AccountId, name: string) => call<OpenEvent>('account_rename', { id, name }),
	addAsset: (name: string, assetType: AssetType) => call<Added>('asset_add', { name, assetType }),
	setAssetType: (id: AccountId, assetType: AssetType) =>
		call<OpenEvent>('asset_set_type', { id, assetType }),
	/** Replaces both counts of an asset account; `null` removes one. */
	setCounts: (account: AccountId, opening: CountInput | null, closing: CountInput | null) =>
		call<OpenEvent>('counts_set', { account, opening, closing }),
	addPerson: (name: string) => call<Added>('person_add', { name }),
	updatePerson: (id: AccountId, name: string, person: Person) =>
		call<OpenEvent>('person_update', { id, name, person }),
	checkIban: (input: string) => call<CheckedIban>('iban_check', { input }),

	exportPayoutsCsv: (target: string) => call<void>('payouts_export_csv', { target }),
	markPayoutsPaid: (account: AccountId, date: string) =>
		call<OpenEvent>('payouts_mark_paid', { account, date }),
	/** The print dialog, e.g. to save the payout list as PDF. */
	print: async () => (isTauri() ? call<void>('print_window') : window.print())
};

/** Where the webview loads an attachment from, for previews (`lancelot://` in the app). */
export async function attachmentUrl(attachment: Attachment): Promise<string> {
	if (isTauri()) return convertFileSrc(`attachment/${attachment.id}`, 'lancelot');
	return (await import('./mock')).attachmentUrl(attachment);
}
