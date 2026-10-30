import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import example from '#lib/api/fixtures/example-event.json';
import { session } from '#lib/state/session.svelte.ts';
import Drinks from './+page.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

function open(change: (s: EventSnapshot) => void = () => {}) {
	const snapshot = structuredClone(example) as unknown as EventSnapshot;
	change(snapshot);
	session.update({
		revision: 1,
		path: '/x.lancelot',
		fileName: 'x.lancelot',
		snapshot,
		integrity: { issues: [] },
		backupError: null
	});
}

describe('Drinks page', () => {
	it('shows the result, the records and an invoice that matches', () => {
		open();
		render(Drinks);
		expect(screen.getByText('Club-Mate 0,5')).toBeTruthy();
		expect(screen.getByText('Hoffmann · commission')).toBeTruthy();
		const invoice = screen.getByTestId('drinks-invoice').textContent ?? '';
		expect(invoice).toContain('Invoice incl. Pfand 32,00');
		expect(invoice).toContain('452,00');
		expect(invoice).not.toContain('than the records');
		expect(screen.getByTestId('drinks-records').textContent).toContain('420,00');
	});

	it('says how far an invoice is from the records', () => {
		open((s) => {
			s.drinks.invoice = { amount: 46350, pfand: 3200, difference: 1150 };
		});
		render(Drinks);
		const text = screen.getByTestId('drinks-invoice').textContent?.replace(/\s+/g, ' ');
		expect(text).toContain('11,50 € more than the records');
	});

	it('leaves out the invoice until one is booked, and the actions once closed', () => {
		open((s) => {
			s.drinks.invoice = null;
			s.event.closedAt = '2026-06-20T10:00:00Z';
		});
		render(Drinks);
		expect(screen.queryByTestId('drinks-invoice')).toBeNull();
		expect(screen.queryByText('Supplier invoice')).toBeNull();
	});
});
