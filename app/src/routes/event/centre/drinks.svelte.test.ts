import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import example from '#lib/api/fixtures/example-event.json';
import { session } from '#lib/state/session.svelte.ts';
import Centre from './Centre.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

function openDrinks() {
	const snapshot = session.event!.snapshot;
	const centre = snapshot.costCentres.find((c) => c.id === snapshot.drinkSettings.costCentre)!;
	render(Centre, { centre });
}

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

describe('A cost centre page', () => {
	it('shows the result, the records and an invoice that matches', () => {
		open();
		openDrinks();
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
		openDrinks();
		const text = screen.getByTestId('drinks-invoice').textContent?.replace(/\s+/g, ' ');
		expect(text).toContain('11,50 € more than the records');
	});

	it('leaves out the invoice until one is booked, and the actions once closed', () => {
		open((s) => {
			s.drinks.invoice = null;
			s.event.closedAt = '2026-06-20T10:00:00Z';
		});
		openDrinks();
		expect(screen.queryByTestId('drinks-invoice')).toBeNull();
		expect(screen.queryByText('Supplier invoice')).toBeNull();
	});

	it('has the records only on the drinks centre, and the tagged lines on each', () => {
		open((s) => {
			s.costCentres.push({ id: 99, name: 'Workshop' });
			s.budget.costCentres.push({
				costCentre: 99,
				categories: [],
				income: { estimate: 0, actual: 0, forecast: 0 },
				expense: { estimate: 0, actual: 1200, forecast: 1200 },
				result: { estimate: 0, actual: -1200, forecast: -1200 }
			});
			s.txns[0].lines[1].costCentre = 99;
			s.txns[0].description = 'Boards';
		});
		render(Centre, { centre: { id: 99, name: 'Workshop' } });
		expect(screen.getByRole('heading', { name: 'Workshop' })).toBeTruthy();
		expect(screen.getByText('Boards')).toBeTruthy();
		expect(screen.queryByTestId('drinks-records')).toBeNull();
		expect(screen.queryByText('Supplier invoice')).toBeNull();
	});
});
