// What the plugin keeps in the event file: the sheet's column names. A template copies them. And
// what a sheet means: one ticket per row, and a row it can't use is listed (R-TKT-4).
import type { BatchTicket } from '../../api/generated/BatchTicket';
import type { PluginChange, PluginRecord, Table, TicketBatch } from '../api';

export type TicketColumns = { tier: string; order: string };

export type Problem = 'noTier' | 'noOrder' | 'cellError';

export type Sheet = { batch: TicketBatch; problems: { row: number; problem: Problem }[] };

export const SHEET_EXTENSIONS = ['ods', 'xlsx', 'csv'];

export const isSheet = (path: string) =>
	SHEET_EXTENSIONS.includes(path.split('.').pop()?.toLowerCase() ?? '');

const DEFAULT_COLUMNS: TicketColumns = { tier: 'Ticket', order: 'Order ID' };

/** The columns the sheet is read from (R-TKT-3). A record that doesn't fit is ignored and kept. */
export function columnsOf(records: PluginRecord[]): TicketColumns {
	const v = records.find((r) => r.key === 'columns')?.value as Partial<TicketColumns> | null;
	return typeof v?.tier === 'string' && typeof v?.order === 'string'
		? { tier: v.tier, order: v.order }
		: DEFAULT_COLUMNS;
}

/** What is wrong with these names, or null. Columns are matched ignoring case. */
export function columnsError({ tier, order }: TicketColumns): string | null {
	if (!tier.trim() || !order.trim()) return 'enter both column names';
	if (tier.trim().toLowerCase() === order.trim().toLowerCase()) {
		return 'tier and order need different columns';
	}
	return null;
}

/** A table read with the tier column first and the order column second. */
export function sheetOf(table: Table): Sheet {
	const tickets: BatchTicket[] = [];
	const problems: Sheet['problems'] = table.errors.map((row) => ({ row, problem: 'cellError' }));
	for (const { row, cells } of table.rows) {
		const [tier, order] = cells;
		if (!tier) problems.push({ row, problem: 'noTier' });
		else if (!order) problems.push({ row, problem: 'noOrder' });
		else tickets.push({ tier, order });
	}
	problems.sort((a, b) => a.row - b.row);
	return { batch: { source: table.source, sha256: table.sha256, tickets }, problems };
}

export const setColumns = ({ tier, order }: TicketColumns): PluginChange => ({
	key: 'columns',
	scope: 'setup',
	value: { tier: tier.trim(), order: order.trim() }
});
