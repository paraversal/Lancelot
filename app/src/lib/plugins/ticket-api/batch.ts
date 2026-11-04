// What a pull means as tickets: one per item (R-TKT-3). With order IDs, an item without one is
// listed, never dropped silently (R-TKT-4). Without them, the tickets count per tier (R-TKT-12).
import type { Pulled, TicketBatch } from '../api';

export type PulledTickets =
	{ batch: TicketBatch; problems: Pulled['problems'] } | { error: string };

/**
 * The tickets of a pull. Whether they have order IDs is what the event's imports have, or before
 * the first, whether the source has an Order field at all: left blank in Settings, it has none.
 */
export function ticketsOf(pulled: Pulled, eventOrderIds: boolean | null): PulledTickets {
	const sourceHas = pulled.fields.includes('order');
	const orderIds = eventOrderIds ?? sourceHas;
	if (orderIds && !sourceHas) {
		return {
			error:
				"The event's imports have order IDs, but this data source has no Order field. Set it in Settings → Plugins, or delete the imports."
		};
	}
	const tickets = pulled.rows
		.filter((row) => !orderIds || row.order)
		.map((row) => ({ tier: row.tier ?? '', order: orderIds ? row.order! : null }));
	const problems = [
		...pulled.problems,
		...(orderIds ? pulled.missing.filter((m) => m.field === 'order') : [])
	].sort((a, b) => a.row - b.row);
	return { batch: { source: pulled.label, sha256: pulled.sha256, tickets }, problems };
}
