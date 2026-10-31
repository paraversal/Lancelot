import type { Plugin } from '../api';
import Section from './Section.svelte';

/** Tickets from the spreadsheet of tickets sold. */
export const ticketSheet: Plugin = {
	id: 'ticket-sheet',
	name: 'Ticket sheet',
	description: 'Imports tickets from a spreadsheet of tickets sold: .ods, .xlsx or .csv.',
	TicketSection: Section
};
