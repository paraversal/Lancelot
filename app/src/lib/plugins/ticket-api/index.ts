import type { Plugin } from '../api';
import Section from './Section.svelte';

/** Tickets from a provider that lists them over the network. */
export const ticketApi: Plugin = {
	id: 'ticket-api',
	name: 'Ticket API',
	description: 'Pulls the tickets sold from a provider, with one item per ticket.',
	summary: (ctx) => {
		const source = ctx.dataSource();
		return source ? `Pulls from ${source}` : 'No data source yet';
	},
	TicketSection: Section,
	dataSource: {
		list: 'tickets',
		fields: [
			{ key: 'tier', label: 'Tier', usually: 'tier' },
			// Left blank, the tickets count per tier (R-TKT-12).
			{ key: 'order', label: 'Order', usually: 'order', optional: true }
		]
	}
};
