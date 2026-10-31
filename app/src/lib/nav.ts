// Where the sidebar and ⌘1–9 go: the pages, then one per cost centre (R-CC-1).
import type { EventSnapshot } from './api/generated/EventSnapshot';

export const centreLink = (id: number) => `/event/centre?id=${id}`;

export function pageLinks(snapshot: EventSnapshot): { href: string; label: string }[] {
	return [
		{ href: '/event/overview', label: 'Overview' },
		{ href: '/event/ledger', label: 'Ledger' },
		{ href: '/event/people', label: 'People' },
		...(snapshot.ticketSettings.enabled ? [{ href: '/event/tickets', label: 'Tickets' }] : []),
		{ href: '/event/budget', label: 'Budget' }
	];
}

/** Every link in the sidebar's order. */
export function allLinks(snapshot: EventSnapshot): string[] {
	return [
		...pageLinks(snapshot).map((p) => p.href),
		...snapshot.costCentres.map((c) => centreLink(c.id))
	];
}
