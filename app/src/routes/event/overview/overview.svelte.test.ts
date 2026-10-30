import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import example from '#lib/api/mock/example-event.json';
import { session } from '#lib/state/session.svelte.ts';
import Overview from './+page.svelte';

const { goto, setCounts, closeEvent, deleteReconciliation } = vi.hoisted(() => ({
	goto: vi.fn(),
	setCounts: vi.fn(),
	closeEvent: vi.fn(),
	deleteReconciliation: vi.fn()
}));
vi.mock('$app/navigation', () => ({ goto }));
vi.mock('#lib/api/index.ts', async () => {
	const actual = await vi.importActual<typeof import('#lib/api/index.ts')>('#lib/api/index.ts');
	return { ...actual, api: { setCounts, closeEvent, deleteReconciliation, parseAmount: vi.fn() } };
});

// jsdom does not lay out; the bars only need it to exist.
globalThis.ResizeObserver ??= class {
	observe() {}
	unobserve() {}
	disconnect() {}
} as unknown as typeof ResizeObserver;

const snapshot = example as EventSnapshot;
const text = (el: HTMLElement) => el.textContent?.replace(/\s+/g, ' ').trim();

beforeEach(() => {
	goto.mockClear();
	setCounts.mockReset();
	closeEvent.mockReset();
	session.update({
		revision: 1,
		path: '/e.lancelot',
		fileName: 'e.lancelot',
		snapshot: structuredClone(snapshot),
		integrity: { issues: [] },
		backupError: null
	});
});

describe('Overview', () => {
	it('shows the result, where the money is and what is open', () => {
		render(Overview);
		expect(text(screen.getByTestId('result'))).toBe('+2.027,50 €');
		const cash = screen.getByText('Cash box').closest('li')!;
		expect(text(cash)).toContain('counted 410,00 €');
		expect(text(cash)).toContain('2,90 € less than recorded');
		expect(text(cash)).toContain('412,90 €');
		expect(screen.getByText('Anna is owed')).toBeTruthy();
		expect(screen.getByText('Bob holds')).toBeTruthy();
		expect(screen.getByText('No receipt: Cups and ice')).toBeTruthy();
	});

	describe('statements', () => {
		const mismatched = () => {
			const next = structuredClone(snapshot);
			const r = next.reconciliations[0];
			r.amount += 1200;
			r.difference = 1200;
			next.openItems.push({
				type: 'reconciliationMismatch',
				checkpoint: r.id,
				account: r.account,
				difference: 1200
			});
			session.update({ ...session.event!, snapshot: next });
			return r;
		};

		it('shows that the Verein account matches its statement', () => {
			render(Overview);
			const verein = screen.getByText('Verein account').closest('li')!;
			expect(text(verein)).toContain('statement 30.06.2026 ✓');
		});

		it('leads from a mismatch to the panel, which asks for the net movement', async () => {
			mismatched();
			render(Overview);
			await fireEvent.click(
				screen.getByText('Verein account, 30.06.2026: 12,00 € more than recorded')
			);
			expect(screen.getByText('Net movement')).toBeTruthy();
			expect(
				screen.getByText('12,00 € more than recorded', { selector: 'li button span' })
			).toBeTruthy();
		});

		it('asks for the statement on another account', async () => {
			render(Overview);
			const cash = screen.getByText('Cash box').closest('li')!;
			await fireEvent.click(within(cash).getByRole('button', { name: 'Statement' }));
			expect(screen.getByText('Statement shows')).toBeTruthy();
		});

		it('deletes a checkpoint after asking', async () => {
			const r = mismatched();
			deleteReconciliation.mockResolvedValue(session.event);
			render(Overview);
			const verein = screen.getByText('Verein account').closest('li')!;
			await fireEvent.click(within(verein).getByRole('button', { name: 'Statement' }));
			await fireEvent.click(screen.getByText('In the books'));
			await fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
			expect(deleteReconciliation).not.toHaveBeenCalled();
			await fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
			expect(deleteReconciliation).toHaveBeenCalledWith(r.id);
		});
	});

	it('shows the forecast and leads to the budget', async () => {
		render(Overview);
		const forecast = screen.getByTestId('forecast');
		expect(text(forecast)).toBe('Forecast +1.765,00 € planned +2.305,00 €');
		await fireEvent.click(forecast);
		expect(goto).toHaveBeenCalledWith('/event/budget');
	});

	it('shows no forecast without a budget', () => {
		const next = structuredClone(snapshot);
		next.budgetLines = [];
		session.update({ ...session.event!, snapshot: next });
		render(Overview);
		expect(screen.queryByTestId('forecast')).toBeNull();
	});

	describe('with tickets', () => {
		const tier = (id: number, quota: number | null, places: number) =>
			({
				...structuredClone(snapshot.tickets.total),
				id,
				name: `T${id}`,
				price: 2000,
				feeBp: 0,
				quota,
				aliases: [],
				places
			}) as EventSnapshot['tickets']['tiers'][number];
		const open = (
			tiers: EventSnapshot['tickets']['tiers'],
			profitability: Partial<NonNullable<EventSnapshot['profitability']>> = {}
		) => {
			const next = structuredClone(snapshot);
			next.tickets.tiers = tiers;
			next.tickets.total.places = tiers.reduce((n, t) => n + t.places, 0);
			next.profitability = { ...next.profitability!, ...profitability };
			session.update({ ...session.event!, snapshot: next });
			render(Overview);
		};

		it('shows places against the quota and leads to the tickets', async () => {
			open([tier(1, 200, 150), tier(2, 100, 64)]);
			const figure = screen.getByTestId('tickets');
			expect(text(figure)).toBe('Tickets 214 / 300');
			expect(figure.querySelector<HTMLElement>('.bar span')!.style.width).toBe(
				`${(214 / 300) * 100}%`
			);
			await fireEvent.click(figure);
			expect(goto).toHaveBeenCalledWith('/event/tickets');
		});

		it('has no quota or bar when a tier has no quota', () => {
			open([tier(1, 200, 150), tier(2, null, 64)]);
			const figure = screen.getByTestId('tickets');
			expect(text(figure)).toBe('Tickets 214');
			expect(figure.querySelector('.bar')).toBeNull();
		});

		it('is absent without tiers', () => {
			open([]);
			expect(screen.queryByTestId('tickets')).toBeNull();
			expect(screen.getByText('Income')).toBeTruthy();
		});

		it('shows the break-even under the forecast while there is a gap', () => {
			open([tier(1, 200, 150)], { gap: 21200, tickets: 9, placesLeft: 50 });
			expect(text(screen.getByTestId('forecast'))).toBe(
				'Forecast +1.765,00 € ~9 tickets to break even'
			);
		});

		it('says when the places left fall short', () => {
			open([tier(1, 200, 199)], { gap: 21200, tickets: 9, placesLeft: 1 });
			expect(text(screen.getByTestId('forecast'))).toContain(
				'~9 tickets to break even · 1 place left'
			);
		});

		it('keeps the plan once break-even is reached', () => {
			open([tier(1, 200, 150)], { gap: 0, tickets: 0, placesLeft: 50 });
			expect(text(screen.getByTestId('forecast'))).toBe('Forecast +1.765,00 € planned +2.305,00 €');
		});

		it('leaves income and expenses to the bars beside forecast and tickets', () => {
			open([tier(1, 200, 150)]);
			expect(screen.queryByText('Income')).toBeNull();
			expect(screen.queryByText('Expenses')).toBeNull();
		});

		it('keeps them when the forecast is gone', () => {
			const next = structuredClone(snapshot);
			next.tickets.tiers = [tier(1, 200, 150)];
			next.tickets.total.places = 150;
			next.budgetLines = [];
			next.profitability = null;
			session.update({ ...session.event!, snapshot: next });
			render(Overview);
			expect(screen.getByTestId('tickets')).toBeTruthy();
			expect(screen.getByText('Income')).toBeTruthy();
		});
	});

	it('leads from an open item to its fix', async () => {
		render(Overview);
		await fireEvent.click(screen.getByText('Anna is owed'));
		const anna = snapshot.persons.find((p) => p.name === 'Anna')!.account;
		expect(goto).toHaveBeenCalledWith(`/event/people?person=${anna}`);
	});

	it('counts a cash account in the panel', async () => {
		setCounts.mockResolvedValue(session.event);
		render(Overview);
		const cash = screen.getByText('Cash box').closest('li')!;
		await fireEvent.click(within(cash).getByText('Count'));
		const panel = document.querySelector('aside')!;
		expect(text(panel)).toContain('In the books412,90 €');
		expect(text(panel)).toContain('Less than recorded 2,90 €');

		await fireEvent.click(within(panel).getByText('Save'));
		const id = snapshot.accounts.find((a) => a.name === 'Cash box')!.id;
		expect(setCounts).toHaveBeenCalledWith(id, null, { date: '2026-06-14', amount: 41000 });
	});

	it('closes only once every open item has a note', async () => {
		closeEvent.mockImplementation(async () => {
			const next = structuredClone($state.snapshot(session.event!));
			next.revision += 1;
			next.snapshot.event.closedAt = '2026-06-21T18:00:00Z';
			next.snapshot.event.closingOverrides = snapshot.openItems.map((item) => ({
				item,
				note: 'ok'
			}));
			return next;
		});
		render(Overview);
		await fireEvent.click(screen.getByText('Close event'));
		const panel = document.querySelector('aside')!;
		const close = within(panel).getByRole('button', { name: /Close event/ }) as HTMLButtonElement;
		expect(close.disabled).toBe(true);
		expect(text(panel)).toContain(`${snapshot.openItems.length} items without a note`);

		for (const input of panel.querySelectorAll('input'))
			await fireEvent.input(input, { target: { value: 'ok' } });
		expect(close.disabled).toBe(false);
		await fireEvent.click(close);
		await new Promise((r) => setTimeout(r, 0));

		expect(closeEvent).toHaveBeenCalledWith(
			snapshot.openItems.map((item) => ({ item, note: 'ok' }))
		);
		expect(text(screen.getByText(/^Closed /))).toBe('Closed 21.06.2026');
		expect(screen.queryByText('Count')).toBeNull();
		expect(screen.queryByText('New entry')).toBeNull();
		expect(screen.getByText('Accepted when closing')).toBeTruthy();
		expect(screen.queryByTestId('forecast')).toBeNull();
	});
});
