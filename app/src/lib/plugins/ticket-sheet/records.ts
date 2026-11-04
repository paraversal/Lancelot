// What the plugin keeps in the event file: the sheet's column names, and whether its rows have
// an order ID. A template copies them. And what a sheet means: one ticket per row, and a row it
// can't use is listed (R-TKT-4).
import type { BatchTicket } from '../../api/generated/BatchTicket';
import type { PluginChange, PluginRecord, Table, TicketBatch } from '../api';

/**
 * `orderIds` off, only the tier column is read: conflicts are then found per tier (R-TKT-12).
 * The order column's name is kept for when it is switched back on.
 */
export type TicketColumns = { tier: string; order: string; orderIds: boolean };

export type Problem = 'noTier' | 'noOrder' | 'cellError';

export type Sheet = { batch: TicketBatch; problems: { row: number; problem: Problem }[] };

export const SHEET_EXTENSIONS = ['ods', 'xlsx', 'csv'];

export const isSheet = (path: string) =>
	SHEET_EXTENSIONS.includes(path.split('.').pop()?.toLowerCase() ?? '');

const DEFAULT_COLUMNS: TicketColumns = { tier: 'Ticket', order: 'Order ID', orderIds: true };

/**
 * The columns the sheet is read from (R-TKT-3). A record that doesn't fit is ignored and kept; one
 * written before sheets could do without order IDs has them. Once the event has imports, `locked`
 * says whether they have order IDs, and the sheet is read the same way.
 */
export function columnsOf(records: PluginRecord[], locked: boolean | null = null): TicketColumns {
	const v = records.find((r) => r.key === 'columns')?.value as Partial<TicketColumns> | null;
	const columns =
		typeof v?.tier === 'string' && typeof v?.order === 'string'
			? { tier: v.tier, order: v.order, orderIds: v.orderIds !== false }
			: DEFAULT_COLUMNS;
	return { ...columns, orderIds: locked ?? columns.orderIds };
}

/** The names of the columns to read, the tier first. */
export const columnsRead = ({ tier, order, orderIds }: TicketColumns) =>
	orderIds ? [tier, order] : [tier];

/** What is wrong with these names, or null. Columns are matched ignoring case. */
export function columnsError({ tier, order, orderIds }: TicketColumns): string | null {
	if (!orderIds) return tier.trim() ? null : 'enter the tier column';
	if (!tier.trim() || !order.trim()) return 'enter both column names';
	if (tier.trim().toLowerCase() === order.trim().toLowerCase()) {
		return 'tier and order need different columns';
	}
	return null;
}

/** A table read with [`columnsRead`]: the tier column first, then the order column if any. */
export function sheetOf(table: Table, orderIds = true): Sheet {
	const tickets: BatchTicket[] = [];
	const problems: Sheet['problems'] = table.errors.map((row) => ({ row, problem: 'cellError' }));
	for (const { row, cells } of table.rows) {
		const [tier, order] = cells;
		if (!tier) problems.push({ row, problem: 'noTier' });
		else if (!orderIds) tickets.push({ tier, order: null });
		else if (!order) problems.push({ row, problem: 'noOrder' });
		else tickets.push({ tier, order });
	}
	problems.sort((a, b) => a.row - b.row);
	return { batch: { source: table.source, sha256: table.sha256, tickets }, problems };
}

export const setColumns = ({ tier, order, orderIds }: TicketColumns): PluginChange => ({
	key: 'columns',
	scope: 'setup',
	value: { tier: tier.trim(), order: order.trim(), orderIds }
});
