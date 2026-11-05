import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import example from '#lib/api/fixtures/example-event.json';
import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import { furthestFromPlan, isOver, isWaiting } from '#lib/budget.ts';
import { session } from '#lib/state/session.svelte.ts';
import Page from './+page.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/state', () => ({ page: { url: new URL('http://x/event/budget') } }));

const snapshot = example as unknown as EventSnapshot;

function open(change: (s: EventSnapshot) => void = () => {}) {
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
	render(Page);
}

const noFigures = { estimate: 0, actual: 0, forecast: 0 };

beforeEach(() => vi.clearAllMocks());

describe('The Budget page', () => {
	it('heads the table with the sections and the result', () => {
		open();
		const table = screen.getByRole('table', { name: 'Budget' });
		for (const name of ['Income', 'Expenses', 'Result']) {
			expect(within(table).getByRole('rowgroup', { name })).toBeTruthy();
		}
		expect(screen.getByText('Forecast', { selector: '.form-label' })).toBeTruthy();
	});

	it('shows the plan on top while nothing is booked', () => {
		open((s) => {
			s.budget.income = { ...s.budget.income, actual: 0 };
			s.budget.expense = { ...s.budget.expense, actual: 0 };
		});
		expect(screen.queryByText('Forecast', { selector: '.form-label' })).toBeNull();
		expect(screen.getByText('Planned', { selector: '.form-label' })).toBeTruthy();
	});

	it('leads with the result once the event is closed', () => {
		open((s) => {
			s.event.closedAt = '2026-06-20T12:00:00Z';
		});
		expect(screen.getByText('Result', { selector: '.form-label' })).toBeTruthy();
		expect(screen.queryByText('Forecast', { selector: '.form-label' })).toBeNull();
		expect(screen.queryByText('Furthest from plan')).toBeNull();
	});

	it('offers to copy from another event while it has nothing to show', () => {
		open((s) => {
			s.budgetLines = [];
			s.budget.lines = [];
			s.budget.costCentres = [];
			s.budget.categories = s.budget.categories.map((c) => ({ ...c, ...noFigures, unbudgeted: 0 }));
		});
		expect(screen.getByRole('button', { name: 'Copy from another event' })).toBeTruthy();
		expect(screen.queryByRole('rowgroup')).toBeNull();
	});

	it('opens a line from a category with that category chosen', async () => {
		open();
		const category = snapshot.accounts.find(
			(a) => a.kind === 'expense' && snapshot.budgetLines.some((b) => b.category === a.id)
		)!;
		await fireEvent.click(
			screen.getByRole('button', { name: `New budget line in ${category.name}` })
		);
		expect(screen.getByText('New budget line')).toBeTruthy();
		expect((screen.getByLabelText('Category') as HTMLInputElement).value).toBe(category.name);
	});
});

describe('Reading the figures', () => {
	const line = (id: number, fromPlan: number) => ({ line: id, fromPlan, ...noFigures });

	it('finds the lines furthest from plan in either direction', () => {
		const lines = [line(1, -100), line(2, 0), line(3, 300), line(4, -200), line(5, 50)];
		expect(furthestFromPlan(lines).map((l) => l.line)).toEqual([3, 4, 1]);
	});

	it('calls an expense over its estimate, and income only once complete', () => {
		expect(isOver('expense', { estimate: 100, actual: 101 }, false)).toBe(true);
		expect(isOver('expense', { estimate: 100, actual: 100 }, true)).toBe(false);
		expect(isOver('income', { estimate: 100, actual: 50 }, false)).toBe(false);
		expect(isOver('income', { estimate: 100, actual: 50 }, true)).toBe(true);
	});

	it('marks a forecast that only repeats the estimate', () => {
		expect(isWaiting({ estimate: 100, actual: 40, forecast: 100 }, false)).toBe(true);
		expect(isWaiting({ estimate: 100, actual: 100, forecast: 100 }, false)).toBe(false);
		expect(isWaiting({ estimate: 100, actual: 140, forecast: 140 }, false)).toBe(false);
		expect(isWaiting({ estimate: 100, actual: 40, forecast: 40 }, true)).toBe(false);
	});
});
