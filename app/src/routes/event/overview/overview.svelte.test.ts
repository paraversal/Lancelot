import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import example from '#lib/api/mock/example-event.json';
import { session } from '#lib/state/session.svelte.ts';
import Overview from './+page.svelte';

const { goto, setCounts } = vi.hoisted(() => ({ goto: vi.fn(), setCounts: vi.fn() }));
vi.mock('$app/navigation', () => ({ goto }));
vi.mock('#lib/api/index.ts', async () => {
	const actual = await vi.importActual<typeof import('#lib/api/index.ts')>('#lib/api/index.ts');
	return { ...actual, api: { setCounts } };
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
		expect(text(cash)).toContain('−2,90 €');
		expect(text(cash)).toContain('412,90 €');
		expect(screen.getByText('Anna is owed')).toBeTruthy();
		expect(screen.getByText('Bob holds')).toBeTruthy();
		expect(screen.getByText('No receipt: Cups and ice')).toBeTruthy();
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
		expect(text(panel)).toContain('Difference −2,90 €');

		await fireEvent.click(within(panel).getByText('Save'));
		const id = snapshot.accounts.find((a) => a.name === 'Cash box')!.id;
		expect(setCounts).toHaveBeenCalledWith(id, null, { date: '2026-06-14', amount: 41000 });
	});
});
