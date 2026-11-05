import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import example from '#lib/api/fixtures/example-event.json';
import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import { session } from '#lib/state/session.svelte.ts';
import BudgetLinePanel from './BudgetLinePanel.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

const snapshot = example as unknown as EventSnapshot;
const account = (kind: string) => snapshot.accounts.find((a) => a.kind === kind)!;

beforeEach(() => {
	session.update({
		revision: Date.now(),
		path: '/x.lancelot',
		fileName: 'x.lancelot',
		snapshot: structuredClone(snapshot),
		integrity: { issues: [] },
		backupError: null
	});
});

describe('The budget line panel', () => {
	it('says whether the chosen category is income or an expense', async () => {
		render(BudgetLinePanel, { line: null, category: account('income').id, onclose() {} });
		expect(screen.getByText('Income')).toBeTruthy();
		const field = screen.getByLabelText('Category');
		await fireEvent.focus(field);
		await fireEvent.input(field, { target: { value: account('expense').name } });
		await fireEvent.keyDown(field, { key: 'Enter' });
		expect(screen.getByText('Expense')).toBeTruthy();
	});
});
