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

describe('A settle', () => {
	const person = (name: string) => snapshot.accounts.find((a) => a.name === name)!.id;
	const saved = (description: string) => snapshot.txns.find((t) => t.description === description)!;

	async function type(amount: number) {
		vi.spyOn(api, 'parseSum').mockResolvedValue({ cents: amount, ambiguous: false, terms: 1 });
		await fireEvent.input(screen.getByLabelText('Amount'), { target: { value: String(amount) } });
		await vi.waitFor(() => expect(api.parseSum).toHaveBeenCalled());
	}

	it('starts out settling the whole balance', () => {
		render(EntryPanel, { ...props, settle: person('Anna') });
		expect(screen.getByText('Anna is settled')).toBeTruthy();
	});

	it('shows what is left while typing', async () => {
		render(EntryPanel, { ...props, settle: person('Anna') });
		await type(5000);
		expect(await screen.findByText('Anna is still owed 162,40 €')).toBeTruthy();
	});

	it('shows what is left of a repayment', async () => {
		render(EntryPanel, { ...props, settle: person('Bob') });
		await type(5000);
		expect(await screen.findByText('Bob still owes 130,00 €')).toBeTruthy();
	});

	it('marks a reimbursement that pays out too much', async () => {
		render(EntryPanel, { ...props, settle: person('Anna') });
		await type(30000);
		const over = await screen.findByText('Too much: Anna would owe 87,60 €');
		expect(over.classList.contains('over')).toBe(true);
	});

	it('marks a repayment that takes back too much', async () => {
		render(EntryPanel, { ...props, settle: person('Bob') });
		await type(20000);
		expect(await screen.findByText('Too much: Bob would be owed 20,00 €')).toBeTruthy();
	});

	it('leaves out the entry being edited when it matches exactly', () => {
		render(EntryPanel, { ...props, txn: saved('Reimbursement Clara') });
		expect(screen.getByText('Clara is settled')).toBeTruthy();
		expect(screen.getByText('owed')).toBeTruthy();
	});

	it('leaves out the entry being edited when it changes', async () => {
		render(EntryPanel, { ...props, txn: saved('Reimbursement Clara') });
		await type(9400);
		expect(await screen.findByText('Clara is still owed 1,00 €')).toBeTruthy();
	});

	it('counts the entry being edited for anyone else', async () => {
		render(EntryPanel, { ...props, txn: saved('Reimbursement Clara') });
		await fireEvent.change(screen.getByLabelText('Person'), {
			target: { value: String(person('Malte')) }
		});
		expect(screen.getByText('Malte is still owed 15,00 €')).toBeTruthy();
	});
});

describe('Esc', () => {
	const esc = (init = {}) => fireEvent.keyDown(window, { key: 'Escape', ...init });
	const warning = () => screen.queryByText('Press Esc again to discard changes');
	const saved = () => snapshot.txns.find((t) => t.description === 'Reimbursement Clara')!;

	function changeNotes() {
		return fireEvent.input(screen.getByLabelText('Notes'), { target: { value: 'typed' } });
	}

	it('closes an untouched new entry', async () => {
		const onclose = vi.fn();
		render(EntryPanel, { ...props, onclose });
		await esc();
		expect(onclose).toHaveBeenCalledOnce();
	});

	it('closes an untouched saved entry', async () => {
		const onclose = vi.fn();
		render(EntryPanel, { ...props, txn: saved(), onclose });
		await esc();
		expect(onclose).toHaveBeenCalledOnce();
	});

	it('closes when only the kind of a new entry changed', async () => {
		const onclose = vi.fn();
		render(EntryPanel, { ...props, onclose });
		await fireEvent.click(screen.getByRole('radio', { name: 'Income' }));
		await esc();
		expect(onclose).toHaveBeenCalledOnce();
	});

	it('closes a settle filled in from the balance', async () => {
		const onclose = vi.fn();
		const person = snapshot.accounts.find((a) => a.name === 'Anna')!.id;
		render(EntryPanel, { ...props, settle: person, onclose });
		await esc();
		expect(onclose).toHaveBeenCalledOnce();
	});

	it('warns first when there are changes, and closes on the second press', async () => {
		const onclose = vi.fn();
		render(EntryPanel, { ...props, txn: saved(), onclose });
		await changeNotes();
		await esc();
		expect(onclose).not.toHaveBeenCalled();
		expect(warning()).toBeTruthy();
		await esc();
		expect(onclose).toHaveBeenCalledOnce();
	});

	it('warns about an amount that cannot be read', async () => {
		const onclose = vi.fn();
		vi.spyOn(api, 'parseSum').mockRejectedValue(new Error('unreadable'));
		render(EntryPanel, { ...props, onclose });
		await fireEvent.input(screen.getByLabelText('Amount'), { target: { value: 'abc' } });
		await vi.waitFor(() => expect(api.parseSum).toHaveBeenCalled());
		await new Promise((resolve) => setTimeout(resolve));
		await esc();
		expect(onclose).not.toHaveBeenCalled();
		expect(warning()).toBeTruthy();
	});

	it('closes when the changes were undone by hand', async () => {
		const onclose = vi.fn();
		render(EntryPanel, { ...props, txn: saved(), onclose });
		await changeNotes();
		await fireEvent.input(screen.getByLabelText('Notes'), { target: { value: '' } });
		await esc();
		expect(onclose).toHaveBeenCalledOnce();
	});

	it('starts over after anything else is done', async () => {
		const onclose = vi.fn();
		render(EntryPanel, { ...props, txn: saved(), onclose });
		await changeNotes();
		await esc();
		await fireEvent.keyDown(window, { key: 'a' });
		expect(warning()).toBeNull();
		await esc();
		expect(onclose).not.toHaveBeenCalled();
		await fireEvent.pointerDown(window);
		expect(warning()).toBeNull();
	});

	it('does not discard when Esc is held down', async () => {
		const onclose = vi.fn();
		render(EntryPanel, { ...props, txn: saved(), onclose });
		await changeNotes();
		await esc();
		await esc({ repeat: true });
		expect(onclose).not.toHaveBeenCalled();
	});

	it('warns about a receipt waiting to be attached', async () => {
		const onclose = vi.fn();
		render(EntryPanel, { ...props, pending: ['/tmp/receipt.pdf'], onclose });
		await esc();
		expect(onclose).not.toHaveBeenCalled();
	});

	it('closes an open receipt preview without a warning', async () => {
		const onescape = vi.fn();
		render(EntryPanel, { ...props, txn: saved(), previewing: 1, onescape });
		await changeNotes();
		await esc();
		expect(onescape).toHaveBeenCalledOnce();
		expect(warning()).toBeNull();
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
		vi.spyOn(api, 'parseSum').mockResolvedValue({ cents: 1200, ambiguous: false, terms: 1 });
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
		vi.spyOn(api, 'parseSum').mockResolvedValue({ cents: 1200, ambiguous: false, terms: 1 });
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
