// What the plugin keeps in the event file: the sheet's column names. A template copies them.
import type { PluginChange, PluginRecord, TicketColumns } from '../api';

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

export const setColumns = ({ tier, order }: TicketColumns): PluginChange => ({
	key: 'columns',
	scope: 'setup',
	value: { tier: tier.trim(), order: order.trim() }
});
