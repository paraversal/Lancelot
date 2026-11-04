import { render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '#lib/api/index.ts';
import example from '#lib/api/fixtures/example-event.json';
import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import { session } from '#lib/state/session.svelte.ts';
import Page from './+page.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

const snapshot = example as unknown as EventSnapshot;

function open(changes: Partial<EventSnapshot> = {}) {
	session.update({
		revision: Date.now(),
		path: '/x.lancelot',
		fileName: 'x.lancelot',
		snapshot: { ...structuredClone(snapshot), ...changes },
		integrity: { issues: [] },
		backupError: null
	});
}

const block = (name: string) =>
	screen.getByRole('heading', { name, level: 2 }).closest('section') as HTMLElement;

beforeEach(() => {
	vi.spyOn(api, 'backups').mockResolvedValue([]);
});

describe('The settings page', () => {
	it('keeps categories apart from the cost centres', () => {
		open();
		render(Page);
		expect(
			within(block('Cost centres')).queryByLabelText('Kitchen (expense) starts in')
		).toBeNull();
		expect(within(block('Categories')).getByLabelText('Kitchen (expense) starts in')).toBeTruthy();
	});

	it('groups categories by kind', () => {
		open();
		render(Page);
		const categories = block('Categories');
		expect(within(categories).getByText('Expenses')).toBeTruthy();
		expect(within(categories).getByText('Income')).toBeTruthy();
	});

	it('asks for no cost centre while the event has none', () => {
		open({ costCentres: [] });
		render(Page);
		const categories = block('Categories');
		expect(within(categories).getByText('Kitchen')).toBeTruthy();
		expect(within(categories).queryByLabelText('Kitchen (expense) starts in')).toBeNull();
	});
});
