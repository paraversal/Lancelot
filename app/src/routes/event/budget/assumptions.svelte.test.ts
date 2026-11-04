import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '#lib/api/index.ts';
import example from '#lib/api/fixtures/example-event.json';
import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import { formula, usersOf } from '#lib/budget.ts';
import { session } from '#lib/state/session.svelte.ts';
import Page from './+page.svelte';
import BudgetLinePanel from './BudgetLinePanel.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/state', () => ({ page: { url: new URL('http://x/event/budget') } }));

const snapshot = example as unknown as EventSnapshot;

function load(change: (s: EventSnapshot) => void = () => {}) {
	const copy = structuredClone(snapshot);
	change(copy);
	session.update({
		revision: Date.now(),
		path: '/x.lancelot',
		fileName: 'x.lancelot',
		snapshot: copy,
		integrity: { issues: [] },
		backupError: null
	});
}

/** Amounts keep the euro sign on their line with a non-breaking space. */
const plain = (text: string | null | undefined) => text?.replace(/\u00a0/g, ' ');
const strip = () => screen.getByRole('group', { name: 'Assumptions' });
const ticketSales = () => snapshot.budgetLines.find((l) => l.description === 'Ticket sales')!;

beforeEach(() => vi.restoreAllMocks());

describe('Assumptions', () => {
	it('say which lines scale with them and how', () => {
		const lines = snapshot.budgetLines;
		expect(usersOf(lines, 1).map((l) => l.description)).toEqual(['Ticket sales']);
		expect(plain(formula(ticketSales().estimate, snapshot.assumptions, 'de'))).toBe(
			'Guests × 25,00 €'
		);
		const twice = { type: 'quantity' as const, quantity: 2, unitPrice: 450, per: [1] };
		expect(plain(formula(twice, snapshot.assumptions, 'de'))).toBe('2 × Guests × 4,50 €');
		expect(formula({ ...twice, per: [] }, snapshot.assumptions, 'de')).toBeNull();
	});

	it('show on the Budget page, with the formula of each line that scales', () => {
		load();
		render(Page);
		expect(within(strip()).getByRole('button', { name: /Guests 260/ })).toBeTruthy();
		expect(screen.getByText('Guests × 25,00 €')).toBeTruthy();
	});

	it('change in place, and a value must be a whole number', async () => {
		load();
		const update = vi.spyOn(api, 'updateAssumption').mockResolvedValue(session.event!);
		render(Page);
		await fireEvent.click(within(strip()).getByRole('button', { name: /Guests 260/ }));
		const value = screen.getByRole('textbox', { name: 'Value' });
		// In use, so it can't be deleted: the editor says by how many lines instead.
		expect(screen.queryByRole('button', { name: 'Delete' })).toBeNull();
		expect(within(strip()).getByText('1 line')).toBeTruthy();

		await fireEvent.input(value, { target: { value: '2,5' } });
		await fireEvent.click(within(strip()).getByRole('button', { name: 'Save' }));
		expect(update).not.toHaveBeenCalled();
		expect(within(strip()).getByText('enter a whole number')).toBeTruthy();

		await fireEvent.input(value, { target: { value: '300' } });
		await fireEvent.keyDown(value, { key: 'Enter' });
		expect(update).toHaveBeenCalledWith(1, 'Guests', 300);
	});

	it('are added from the strip, and deleted while nothing uses them', async () => {
		load((s) => {
			s.assumptions.push({ id: 2, name: 'Days', value: 3 });
		});
		const add = vi.spyOn(api, 'addAssumption').mockResolvedValue(session.event!);
		const remove = vi.spyOn(api, 'deleteAssumption').mockResolvedValue(session.event!);
		render(Page);
		await fireEvent.click(within(strip()).getByRole('button', { name: 'Assumption' }));
		await fireEvent.input(screen.getByRole('textbox', { name: 'Name' }), {
			target: { value: 'Helpers' }
		});
		await fireEvent.input(screen.getByRole('textbox', { name: 'Value' }), {
			target: { value: '40' }
		});
		await fireEvent.click(within(strip()).getByRole('button', { name: 'Save' }));
		expect(add).toHaveBeenCalledWith('Helpers', 40);

		await fireEvent.click(within(strip()).getByRole('button', { name: /Days 3/ }));
		await fireEvent.click(within(strip()).getByRole('button', { name: 'Delete' }));
		expect(remove).toHaveBeenCalledWith(2);
	});

	it('stay but cannot change in a closed event', () => {
		load((s) => (s.event.closedAt = '2026-06-20T12:00:00Z'));
		render(Page);
		expect(within(strip()).getByRole('button', { name: /Guests 260/ })).toHaveProperty(
			'disabled',
			true
		);
		expect(within(strip()).queryByRole('button', { name: 'Assumption' })).toBeNull();
	});
});

describe('A budget line that scales', () => {
	it('picks its assumptions, previews the total and saves them', async () => {
		load((s) => {
			s.assumptions.push({ id: 2, name: 'Days', value: 3 });
		});
		const update = vi.spyOn(api, 'updateBudgetLine').mockResolvedValue(session.event!);
		render(BudgetLinePanel, { line: ticketSales(), category: null, onclose: () => {} });
		const guests = screen.getByRole('button', { name: /Guests/ });
		expect(guests.getAttribute('aria-pressed')).toBe('true');
		expect(plain(screen.getByTestId('budget-line-total').textContent)).toBe(
			'260 × 25,00 € = 6.500,00 €'
		);

		await fireEvent.click(screen.getByRole('button', { name: /Days/ }));
		expect(plain(screen.getByTestId('budget-line-total').textContent)).toBe(
			'260 × 3 × 25,00 € = 19.500,00 €'
		);
		await fireEvent.click(guests);
		expect(plain(screen.getByTestId('budget-line-total').textContent)).toBe(
			'3 × 25,00 € = 75,00 €'
		);

		await fireEvent.click(screen.getByRole('button', { name: /Save/ }));
		expect(update.mock.calls[0][1].estimate).toEqual({
			type: 'quantity',
			quantity: 1,
			unitPrice: 2500,
			per: [2]
		});
	});

	it('asks for a price per assumption, without a count', async () => {
		load();
		render(BudgetLinePanel, { line: ticketSales(), category: null, onclose: () => {} });
		expect(screen.getByRole('radio', { name: 'Per assumption' }).getAttribute('aria-checked')).toBe(
			'true'
		);
		expect(screen.queryByRole('textbox', { name: 'Count' })).toBeNull();
		expect(screen.getByRole('textbox', { name: 'Price per Guests' })).toBeTruthy();

		// A count of its own stops scaling.
		await fireEvent.click(screen.getByRole('radio', { name: 'Count × price' }));
		expect(screen.queryByRole('group', { name: 'Times' })).toBeNull();
		expect(screen.getByRole('textbox', { name: 'Count' })).toBeTruthy();
	});

	it('keeps a count that also scales as count × price, with its assumptions', () => {
		load();
		const line = {
			...ticketSales(),
			estimate: { type: 'quantity' as const, quantity: 2, unitPrice: 450, per: [1] }
		};
		render(BudgetLinePanel, { line, category: null, onclose: () => {} });
		expect(screen.getByRole('radio', { name: 'Count × price' }).getAttribute('aria-checked')).toBe(
			'true'
		);
		expect(screen.getByRole('button', { name: /Guests/ }).getAttribute('aria-pressed')).toBe(
			'true'
		);
		expect(plain(screen.getByTestId('budget-line-total').textContent)).toBe(
			'2 × 260 × 4,50 € = 2.340,00 €'
		);
	});
});
