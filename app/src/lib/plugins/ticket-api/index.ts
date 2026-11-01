import type { Plugin } from '../api';
import Section from './Section.svelte';

/** Tickets from a provider that lists them over the network. */
export const ticketApi: Plugin = {
	id: 'ticket-api',
	name: 'Ticket API',
	description: 'Pulls the tickets sold from a provider, with one item per ticket.',
	TicketSection: Section,
	dataSource: {
		list: 'tickets',
		fields: [
			{ key: 'tier', label: 'Tier', usually: 'tier' },
			{ key: 'order', label: 'Order', usually: 'order' }
		]
	}
};
