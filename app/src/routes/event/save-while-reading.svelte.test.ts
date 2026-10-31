import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError } from '#lib/api/index.ts';
import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import example from '#lib/api/fixtures/example-event.json';
import AmountField from '#lib/components/AmountField.svelte';
import DateField from '#lib/components/DateField.svelte';
import EntryPanel from '#lib/entry/EntryPanel.svelte';
import { session } from '#lib/state/session.svelte.ts';
import BudgetLinePanel from './budget/BudgetLinePanel.svelte';
import CountPanel from './overview/CountPanel.svelte';
import StatementPanel from './overview/StatementPanel.svelte';
import TierPanel from './tickets/TierPanel.svelte';

// Reading a field takes a round trip to the core. These hold that trip open across the click on
// Save: nothing may be saved until it returns, and then the value typed last must be.
const api = vi.hoisted(() => ({
	parseAmount: vi.fn(),
	parseDate: vi.fn(),
	saveTxn: vi.fn(),
	addTier: vi.fn(),
	addBudgetLine: vi.fn(),
	updateBudgetLine: vi.fn(),
	setCounts: vi.fn(),
	addReconciliation: vi.fn(),
	updateReconciliation: vi.fn()
}));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('#lib/api/index.ts', async () => {
	const actual = await vi.importActual<typeof import('#lib/api/index.ts')>('#lib/api/index.ts');
	return { ...actual, api };
});

const snapshot = example as unknown as EventSnapshot;
const settle = () => new Promise((r) => setTimeout(r, 0));
const cents = (n: number) => ({ cents: n, ambiguous: false });
const unreadable = () =>
	new ApiError({ code: 'bad_grouping', message: 'cannot read this', field: null, report: null });

/** The next read stays open until the returned functions answer it. */
function hold(parser: typeof api.parseAmount) {
	let resolve!: (value: unknown) => void;
	let reject!: (reason: unknown) => void;
	parser.mockImplementationOnce(
		() =>
			new Promise((res, rej) => {
				resolve = res;
				reject = rej;
			})
	);
	return { resolve: (v: unknown) => resolve(v), reject: (e: unknown) => reject(e) };
}

const type = (input: Element, value: string) => fireEvent.input(input, { target: { value } });
const clickSave = () => fireEvent.click(screen.getByRole('button', { name: /^Save ⌘/ }));
const amountInputs = () => [...document.querySelectorAll('input[inputmode="decimal"]')];

async function typeUnreadable(input: Element) {
	api.parseAmount.mockRejectedValueOnce(unreadable());
	await type(input, 'x');
	await settle();
}

async function typeUnreadableDate(input: Element) {
	api.parseDate.mockRejectedValueOnce(unreadable());
	await type(input, 'x');
	await settle();
}

const anyCall = () =>
	[
		api.saveTxn,
		api.addTier,
		api.addBudgetLine,
		api.updateBudgetLine,
		api.setCounts,
		api.addReconciliation,
		api.updateReconciliation
	].some((f) => f.mock.calls.length);

/** Save while the read is open: nothing happens until it returns. */
async function saveDuringRead(held: { resolve: (v: unknown) => void }, answer: unknown) {
	await clickSave();
	await settle();
	expect(anyCall()).toBe(false);
	held.resolve(answer);
	await settle();
}

beforeEach(() => {
	vi.clearAllMocks();
	api.parseAmount.mockReset();
	api.parseDate.mockReset();
	session.update({
		revision: 1,
		path: '/save.lancelot',
		fileName: 'save.lancelot',
		snapshot: structuredClone(snapshot),
		integrity: { issues: [] },
		backupError: null
	});
	const state = async () => session.event!;
	api.saveTxn.mockImplementation(async () => ({ event: session.event!, id: 1 }));
	for (const f of [
		api.addTier,
		api.addBudgetLine,
		api.updateBudgetLine,
		api.setCounts,
		api.addReconciliation,
		api.updateReconciliation
	])
		f.mockImplementation(state);
});

describe('The entry panel', () => {
	const open = (id: number) => {
		const txn = session.event!.snapshot.txns.find((t) => t.id === id)!;
		render(EntryPanel, {
			txn,
			previewing: null,
			onpreview: () => {},
			onclose: () => {},
			onsaved: () => {}
		});
		return txn;
	};
	const saved = () => api.saveTxn.mock.calls[0][0];
	const lineInputs = () => [...document.querySelectorAll('.line-amount input')];

	it('saves the amount typed just before Save, not the old one', async () => {
		open(6);
		const held = hold(api.parseAmount);
		await type(screen.getByLabelText('Amount'), '2000');
		await saveDuringRead(held, cents(200000));
		expect(saved().total).toBe(200000);
		expect(saved().entry.lines[0].amount).toBe(200000);
	});

	it('waits for the read that leaving the field starts', async () => {
		open(6);
		const input = screen.getByLabelText('Amount');
		const typing = hold(api.parseAmount);
		await type(input, '2000');
		const leaving = hold(api.parseAmount);
		await fireEvent.blur(input);
		await clickSave();
		await settle();
		typing.resolve(cents(200000));
		await settle();
		expect(api.saveTxn).not.toHaveBeenCalled();
		leaving.resolve(cents(200000));
		await settle();
		expect(saved().total).toBe(200000);
	});

	it('saves the split line typed just before Save', async () => {
		open(5);
		const held = hold(api.parseAmount);
		await type(lineInputs()[0], '1,23');
		await saveDuringRead(held, cents(123));
		expect(saved().entry.lines[0].amount).toBe(123);
	});

	it('saves the date typed just before Save', async () => {
		open(6);
		const held = hold(api.parseDate);
		await type(screen.getByLabelText('Date'), '1.7.26');
		await saveDuringRead(held, { date: '2026-07-01' });
		expect(saved().date).toBe('2026-07-01');
	});

	it('does not save when the amount that was typed turns out unreadable', async () => {
		open(6);
		const held = hold(api.parseAmount);
		await type(screen.getByLabelText('Amount'), 'x');
		await clickSave();
		held.reject(unreadable());
		await settle();
		expect(api.saveTxn).not.toHaveBeenCalled();
		expect(screen.getByLabelText('Amount').getAttribute('aria-invalid')).toBe('true');
	});

	it('does not save a split line it cannot read as zero', async () => {
		open(5);
		await typeUnreadable(lineInputs()[0]);
		await clickSave();
		await settle();
		expect(api.saveTxn).not.toHaveBeenCalled();
	});

	it('does not save the old date over one it cannot read', async () => {
		open(6);
		await typeUnreadableDate(screen.getByLabelText('Date'));
		await clickSave();
		await settle();
		expect(api.saveTxn).not.toHaveBeenCalled();
	});

	it('saves again once the input is fixed', async () => {
		open(6);
		const input = screen.getByLabelText('Amount');
		await typeUnreadable(input);
		api.parseAmount.mockResolvedValueOnce(cents(200000));
		await type(input, '2000');
		await settle();
		await clickSave();
		await settle();
		expect(saved().total).toBe(200000);
	});

	it('forgets an unreadable split line once the line is gone', async () => {
		open(5);
		await typeUnreadable(lineInputs()[1]);
		await fireEvent.click(screen.getAllByRole('button', { name: 'Remove line' })[1]);
		await settle();
		await clickSave();
		await settle();
		expect(api.saveTxn).toHaveBeenCalledOnce();
	});
});

describe('The tier panel', () => {
	const price = () => screen.getByLabelText('Price');

	it('saves the price typed just before Save', async () => {
		render(TierPanel, { id: null, onclose: () => {} });
		const held = hold(api.parseAmount);
		await type(price(), '12');
		await saveDuringRead(held, cents(1200));
		expect(api.addTier.mock.calls[0][0].price).toBe(1200);
	});

	it('does not save a price it cannot read', async () => {
		render(TierPanel, { id: null, name: 'Early', onclose: () => {} });
		await typeUnreadable(price());
		await clickSave();
		await settle();
		expect(api.addTier).not.toHaveBeenCalled();
	});
});

describe('The budget line panel', () => {
	const quantityLine = () => session.event!.snapshot.budgetLines[0];

	it('saves the amount typed just before Save', async () => {
		render(BudgetLinePanel, { line: null, category: 7, onclose: () => {} });
		const held = hold(api.parseAmount);
		await type(amountInputs()[0], '50');
		await saveDuringRead(held, cents(5000));
		expect(api.addBudgetLine.mock.calls[0][0].estimate).toEqual({ type: 'amount', amount: 5000 });
	});

	it('saves the unit price typed just before Save, not the old one', async () => {
		render(BudgetLinePanel, { line: quantityLine(), category: null, onclose: () => {} });
		const held = hold(api.parseAmount);
		await type(screen.getByLabelText('Unit price'), '30');
		await saveDuringRead(held, cents(3000));
		expect(api.updateBudgetLine.mock.calls[0][1].estimate).toEqual({
			type: 'quantity',
			quantity: 260,
			unitPrice: 3000
		});
	});

	it('does not save an estimate it cannot read', async () => {
		render(BudgetLinePanel, { line: quantityLine(), category: null, onclose: () => {} });
		await typeUnreadable(screen.getByLabelText('Unit price'));
		await clickSave();
		await settle();
		expect(api.updateBudgetLine).not.toHaveBeenCalled();
	});

	it('ignores an unreadable amount once the form has moved to quantity × price', async () => {
		render(BudgetLinePanel, { line: quantityLine(), category: null, onclose: () => {} });
		await fireEvent.click(screen.getByText('Amount'));
		await settle();
		await typeUnreadable(amountInputs()[0]);
		await fireEvent.click(screen.getByText('Quantity × price'));
		await settle();
		await clickSave();
		await settle();
		expect(api.updateBudgetLine).toHaveBeenCalledOnce();
	});
});

describe('A field that comes back', () => {
	it('starts readable, so the form says what is missing', async () => {
		render(BudgetLinePanel, { line: null, category: 7, onclose: () => {} });
		await typeUnreadable(amountInputs()[0]);
		await fireEvent.click(screen.getByText('Quantity × price'));
		await settle();
		await fireEvent.click(screen.getByText('Amount'));
		await settle();
		await clickSave();
		await settle();
		expect(screen.getByText('enter an amount')).toBeTruthy();
		expect(api.addBudgetLine).not.toHaveBeenCalled();
	});
});

describe('The count panel', () => {
	const closing = () => screen.getAllByLabelText('Amount')[1];

	it('saves the count typed just before Save, not the old one', async () => {
		render(CountPanel, { account: 2, onclose: () => {} });
		const held = hold(api.parseAmount);
		await type(closing(), '420');
		await saveDuringRead(held, cents(42000));
		expect(api.setCounts).toHaveBeenCalledWith(2, null, { date: '2026-06-14', amount: 42000 });
	});

	it('saves the date typed just before Save', async () => {
		render(CountPanel, { account: 2, onclose: () => {} });
		const held = hold(api.parseDate);
		await type(screen.getAllByLabelText('Date')[1], '15.6.26');
		await saveDuringRead(held, { date: '2026-06-15' });
		expect(api.setCounts).toHaveBeenCalledWith(2, null, { date: '2026-06-15', amount: 41000 });
	});

	it('does not clear a count it cannot read', async () => {
		render(CountPanel, { account: 2, onclose: () => {} });
		await typeUnreadable(closing());
		await clickSave();
		await settle();
		expect(api.setCounts).not.toHaveBeenCalled();
	});

	it('does not save the old date over one it cannot read', async () => {
		render(CountPanel, { account: 2, onclose: () => {} });
		await typeUnreadableDate(screen.getAllByLabelText('Date')[0]);
		await clickSave();
		await settle();
		expect(api.setCounts).not.toHaveBeenCalled();
	});
});

describe('The statement panel', () => {
	const open = async () => {
		render(StatementPanel, { account: 1, onclose: () => {} });
		await fireEvent.click(screen.getByText('In the books'));
		await settle();
	};
	const statement = () => screen.getByLabelText('Net movement');

	it('saves the amount typed just before Save, not the old one', async () => {
		await open();
		const held = hold(api.parseAmount);
		await type(statement(), '1800');
		await saveDuringRead(held, cents(180000));
		expect(api.updateReconciliation).toHaveBeenCalledWith(1, '2026-06-30', 180000);
	});

	it('adds a new statement after the read when saved from the keyboard', async () => {
		render(StatementPanel, { account: 1, onclose: () => {} });
		const held = hold(api.parseAmount);
		await type(statement(), '1800');
		await fireEvent.keyDown(window, { key: 'Enter', metaKey: true });
		await settle();
		expect(api.addReconciliation).not.toHaveBeenCalled();
		held.resolve(cents(180000));
		await settle();
		expect(api.addReconciliation).toHaveBeenCalledWith(1, '2026-06-14', 180000);
	});

	it('does not save an amount it cannot read', async () => {
		await open();
		await typeUnreadable(statement());
		await fireEvent.keyDown(window, { key: 'Enter', metaKey: true });
		await settle();
		expect(api.updateReconciliation).not.toHaveBeenCalled();
	});

	it('does not save the old date over one it cannot read', async () => {
		await open();
		await typeUnreadableDate(screen.getByLabelText('Date'));
		await clickSave();
		await settle();
		expect(api.updateReconciliation).not.toHaveBeenCalled();
	});
});

describe('A field that is gone', () => {
	it('ignores an amount read that returns afterwards', async () => {
		const oninput = vi.fn();
		const { unmount } = render(AmountField, { label: 'Late', value: null, format: 'de', oninput });
		const held = hold(api.parseAmount);
		await type(screen.getByLabelText('Late'), '5');
		unmount();
		held.resolve(cents(500));
		await settle();
		expect(oninput).not.toHaveBeenCalled();
	});

	it('ignores a date read that returns afterwards', async () => {
		const oninput = vi.fn();
		const { unmount } = render(DateField, {
			label: 'Late',
			value: null,
			format: 'de',
			oninput,
			oncommit: () => {}
		});
		const held = hold(api.parseDate);
		await type(screen.getByLabelText('Late'), '1.7.26');
		unmount();
		held.resolve({ date: '2026-07-01' });
		await settle();
		expect(oninput).not.toHaveBeenCalled();
	});
});
