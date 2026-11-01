import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '../api/index';
import example from '../api/fixtures/example-event.json';
import type { EventSnapshot } from '../api/generated/EventSnapshot';
import { session } from '../state/session.svelte';
import EntryPanel from './EntryPanel.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

const snapshot = example as unknown as EventSnapshot;
const props = { txn: null, previewing: null, onpreview() {}, onclose() {}, onsaved() {} };

const budgetLine = (description: string) =>
	snapshot.budgetLines.find((b) => b.description === description)!;

const category = () => screen.getAllByRole<HTMLSelectElement>('combobox')[1];
const budget = () => screen.getByLabelText<HTMLSelectElement>('Budget line');
const centre = () => screen.getByLabelText<HTMLSelectElement>('Cost centre');

const groupsOf = (select: HTMLSelectElement) =>
	[...select.querySelectorAll('optgroup')].map((g) => g.label);

beforeEach(() => {
	vi.restoreAllMocks();
	session.update({
		revision: Date.now(),
		path: '/x.lancelot',
		fileName: 'x.lancelot',
		snapshot: structuredClone(snapshot),
		integrity: { issues: [] },
		backupError: null
	});
});

describe('A closed event', () => {
	const withReceipt = () => snapshot.txns.find((t) => t.attachments.length)!;

	beforeEach(() => {
		const closed = structuredClone(snapshot);
		closed.event.closedAt = '2026-06-20T12:00:00Z';
		session.update({ ...session.event!, snapshot: closed, revision: Date.now() });
	});

	it('still previews a receipt', async () => {
		const txn = withReceipt();
		const onpreview = vi.fn();
		render(EntryPanel, { ...props, txn, onpreview });
		const receipt = screen.getByRole<HTMLButtonElement>('button', {
			name: new RegExp(txn.attachments[0].filename)
		});
		expect(receipt.disabled).toBe(false);
		await fireEvent.click(receipt);
		expect(onpreview).toHaveBeenCalledWith(txn.attachments[0]);
	});

	it('offers no way to change the receipts', () => {
		const txn = withReceipt();
		render(EntryPanel, { ...props, txn });
		const remove = `Remove ${txn.attachments[0].filename}`;
		expect(screen.queryByRole('button', { name: remove })).toBeNull();
		expect(screen.queryByRole('button', { name: /Drop receipt/ })).toBeNull();
	});

	it('keeps the fields locked', () => {
		render(EntryPanel, { ...props, txn: withReceipt() });
		expect(screen.getByLabelText<HTMLInputElement>('Amount').matches(':disabled')).toBe(true);
	});
});

describe('A split line', () => {
	it('offers every budget line before a category is chosen', () => {
		render(EntryPanel, props);
		expect(groupsOf(budget())).toContain('Mobility');
		expect(groupsOf(budget())).not.toContain('Tickets');
	});

	it('lists the budget lines of its own category first, outside the groups', async () => {
		render(EntryPanel, props);
		const van = budgetLine('Van rental');
		await fireEvent.change(category(), { target: { value: String(van.category) } });
		expect(groupsOf(budget())).not.toContain('Mobility');
		expect(budget().options[1].text).toBe('Van rental');
	});

	it('takes the category and cost centre of a budget line from another category', async () => {
		render(EntryPanel, props);
		const crates = budgetLine('Drink crates');
		await fireEvent.change(budget(), { target: { value: String(crates.id) } });
		expect(category().value).toBe(String(crates.category));
		expect(centre().value).toBe(String(crates.costCentre));
		expect(centre().closest('fieldset')?.disabled).toBe(true);
	});

	it('stops showing a refused payer once one is chosen', async () => {
		vi.spyOn(api, 'parseAmount').mockResolvedValue({ cents: 1200, ambiguous: false });
		render(EntryPanel, props);
		await fireEvent.input(screen.getByLabelText('Amount'), { target: { value: '12' } });
		await fireEvent.click(screen.getByRole('button', { name: /^Save(?! &)/ }));
		await screen.findAllByText('Choose who');
		await fireEvent.change(screen.getByLabelText('Paid by'), {
			target: { value: String(snapshot.accounts.find((a) => a.kind === 'asset')!.id) }
		});
		expect(screen.queryByText('Choose who')).toBeNull();
	});

	it('saves the budget line it was given', async () => {
		const save = vi.spyOn(api, 'saveTxn').mockResolvedValue({
			...session.event!,
			revision: Date.now()
		} as never);
		vi.spyOn(api, 'parseAmount').mockResolvedValue({ cents: 1200, ambiguous: false });
		render(EntryPanel, props);
		const van = budgetLine('Van rental');
		await fireEvent.change(screen.getByLabelText('Paid by'), {
			target: { value: String(snapshot.accounts.find((a) => a.kind === 'asset')!.id) }
		});
		await fireEvent.input(screen.getByLabelText('Amount'), { target: { value: '12' } });
		await fireEvent.change(budget(), { target: { value: String(van.id) } });
		await fireEvent.click(screen.getByRole('button', { name: /^Save(?! &)/ }));
		await vi.waitFor(() => expect(save).toHaveBeenCalled());
		const entry = save.mock.calls[0][0].entry;
		expect(entry).toMatchObject({
			type: 'expense',
			lines: [{ account: van.category, budgetLine: van.id }]
		});
	});
});

// Last: saving is remembered for the next entry.
describe('A proposal from a plugin', () => {
	const saved = () => vi.mocked(api.saveTxn).mock.calls[0][0];
	const asset = (i: number) => snapshot.accounts.filter((a) => a.kind === 'asset')[i].id;
	const income = () => snapshot.accounts.find((a) => a.kind === 'income')!.id;

	beforeEach(() => {
		vi.spyOn(api, 'saveTxn').mockResolvedValue({
			...session.event!,
			revision: Date.now()
		} as never);
	});

	async function save() {
		await fireEvent.click(screen.getByRole('button', { name: /^Save(?! &)/ }));
		await vi.waitFor(() => expect(api.saveTxn).toHaveBeenCalled());
	}

	it('fills in an income with its date, destination and split, and saves only on Save', async () => {
		render(EntryPanel, {
			...props,
			proposal: {
				kind: 'income',
				counterparty: 'SumUp',
				amount: 1500,
				date: '2026-06-13',
				description: 'Card sales',
				destination: asset(1),
				lines: [
					{ account: income(), costCentre: null, amount: 1000 },
					{ account: income(), costCentre: null, amount: 500 }
				]
			}
		});
		expect(api.saveTxn).not.toHaveBeenCalled();
		await save();
		const draft = saved();
		expect(draft.date).toBe('2026-06-13');
		expect(draft.description).toBe('Card sales');
		expect(draft.counterparty).toBe('SumUp');
		expect(draft.total).toBe(1500);
		const entry = draft.entry;
		if (entry.type !== 'income') throw new Error(entry.type);
		expect(entry.destination).toBe(asset(1));
		expect(entry.lines.map((l) => l.amount)).toEqual([1000, 500]);
	});

	it('fills in a transfer between two accounts', async () => {
		render(EntryPanel, {
			...props,
			proposal: { kind: 'transfer', amount: 2000, from: asset(0), to: asset(1) }
		});
		await save();
		expect(saved().entry).toEqual({ type: 'transfer', from: asset(0), to: asset(1), amount: 2000 });
	});

	it('takes the default account for an expense that names no payer', async () => {
		render(EntryPanel, {
			...props,
			proposal: {
				kind: 'expense',
				counterparty: 'Hoffmann',
				amount: 700,
				lines: [
					{ account: snapshot.accounts.find((a) => a.kind === 'expense')!.id, costCentre: null }
				]
			}
		});
		await save();
		const entry = saved().entry;
		if (entry.type !== 'expense') throw new Error(entry.type);
		expect(entry.payer).toBe(snapshot.event.defaultReimbursementAccount);
		expect(entry.lines[0].amount).toBe(700);
	});
});
