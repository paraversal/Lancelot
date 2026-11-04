import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError } from '#lib/api/index.ts';
import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import example from '#lib/api/fixtures/example-event.json';
import AmountField from '#lib/components/AmountField.svelte';
import DateField from '#lib/components/DateField.svelte';
import EntryPanel from '#lib/entry/EntryPanel.svelte';
import { session } from '#lib/state/session.svelte.ts';
import NewEventPanel from '../NewEventPanel.svelte';
import BudgetLinePanel from './budget/BudgetLinePanel.svelte';
import CountPanel from './overview/CountPanel.svelte';
import StatementPanel from './overview/StatementPanel.svelte';
import MarkPaidPanel from './people/payouts/MarkPaidPanel.svelte';
import CancellationPanel from './tickets/CancellationPanel.svelte';
import ConflictCard from './tickets/ConflictCard.svelte';
import ImportPanel from './tickets/ImportPanel.svelte';
import SalePanel from './tickets/SalePanel.svelte';
import TierPanel from './tickets/TierPanel.svelte';

// Reading a field takes a round trip to the core. These hold that trip open across the click on
// Save: nothing may be saved until it returns, and then the value typed last must be.
const api = vi.hoisted(() => ({
	parseSum: vi.fn(),
	parseDate: vi.fn(),
	saveTxn: vi.fn(),
	addTier: vi.fn(),
	addBudgetLine: vi.fn(),
	updateBudgetLine: vi.fn(),
	setCounts: vi.fn(),
	addReconciliation: vi.fn(),
	updateReconciliation: vi.fn(),
	markPayoutsPaid: vi.fn(),
	updateManualSale: vi.fn(),
	updateCancellation: vi.fn(),
	resolveConflict: vi.fn(),
	importTickets: vi.fn(),
	previewTickets: vi.fn(),
	createEvent: vi.fn(),
	ticketShare: vi.fn()
}));
const chooseNewEventFile = vi.hoisted(() => vi.fn());
vi.mock('#lib/api/dialogs.ts', () => ({
	chooseNewEventFile,
	chooseEventFile: vi.fn(),
	chooseFile: vi.fn()
}));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('#lib/api/index.ts', async () => {
	const actual = await vi.importActual<typeof import('#lib/api/index.ts')>('#lib/api/index.ts');
	return { ...actual, api };
});

const snapshot = example as unknown as EventSnapshot;
const settle = () => new Promise((r) => setTimeout(r, 0));
const cents = (n: number) => ({ cents: n, ambiguous: false, terms: 1 });
const unreadable = () =>
	new ApiError({ code: 'bad_grouping', message: 'cannot read this', field: null, report: null });

/** The next read stays open until the returned functions answer it. */
function hold(parser: typeof api.parseSum) {
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
	api.parseSum.mockRejectedValueOnce(unreadable());
	await type(input, 'x');
	await settle();
}

async function typeUnreadableDate(input: Element) {
	api.parseDate.mockRejectedValueOnce(unreadable());
	await type(input, 'x');
	await settle();
}

const savers = () => [
	api.addTier,
	api.addBudgetLine,
	api.updateBudgetLine,
	api.setCounts,
	api.addReconciliation,
	api.updateReconciliation,
	api.markPayoutsPaid,
	api.updateManualSale,
	api.updateCancellation,
	api.resolveConflict,
	api.importTickets,
	api.createEvent
];

const anyCall = () => [api.saveTxn, ...savers()].some((f) => f.mock.calls.length);

/** Save while the read is open: nothing happens until it returns. */
async function saveDuringRead(
	held: { resolve: (v: unknown) => void },
	answer: unknown,
	click: () => Promise<unknown> = clickSave
) {
	await click();
	await settle();
	expect(anyCall()).toBe(false);
	held.resolve(answer);
	await settle();
}

beforeEach(() => {
	vi.clearAllMocks();
	api.parseSum.mockReset();
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
	for (const f of savers()) f.mockImplementation(state);
	api.previewTickets.mockResolvedValue({
		filename: 'sheet.csv',
		sha256: 'abcdefgh',
		tickets: 0,
		tiers: [],
		unknown: [],
		removed: [],
		problems: [],
		receiptsLost: [],
		previous: null,
		sameFile: false,
		current: true
	});
	api.ticketShare.mockResolvedValue({ part: 0, rest: 0 });
	chooseNewEventFile.mockResolvedValue('/new.lancelot');
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
		const held = hold(api.parseSum);
		await type(screen.getByLabelText('Amount'), '2000');
		await saveDuringRead(held, cents(200000));
		expect(saved().total).toBe(200000);
		expect(saved().entry.lines[0].amount).toBe(200000);
	});

	it('waits for the read that leaving the field starts', async () => {
		open(6);
		const input = screen.getByLabelText('Amount');
		const typing = hold(api.parseSum);
		await type(input, '2000');
		const leaving = hold(api.parseSum);
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
		const held = hold(api.parseSum);
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
		const held = hold(api.parseSum);
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
		api.parseSum.mockResolvedValueOnce(cents(200000));
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
		const held = hold(api.parseSum);
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
		const held = hold(api.parseSum);
		await type(amountInputs()[0], '50');
		await saveDuringRead(held, cents(5000));
		expect(api.addBudgetLine.mock.calls[0][0].estimate).toEqual({ type: 'amount', amount: 5000 });
	});

	it('saves the unit price typed just before Save, not the old one', async () => {
		render(BudgetLinePanel, { line: quantityLine(), category: null, onclose: () => {} });
		const held = hold(api.parseSum);
		await type(screen.getByLabelText('Price per Guests'), '30');
		await saveDuringRead(held, cents(3000));
		expect(api.updateBudgetLine.mock.calls[0][1].estimate).toEqual({
			type: 'quantity',
			quantity: 1,
			unitPrice: 3000,
			// The example's ticket sales scale with its guests, and keep doing so.
			per: [1]
		});
	});

	it('does not save an estimate it cannot read', async () => {
		render(BudgetLinePanel, { line: quantityLine(), category: null, onclose: () => {} });
		await typeUnreadable(screen.getByLabelText('Price per Guests'));
		await clickSave();
		await settle();
		expect(api.updateBudgetLine).not.toHaveBeenCalled();
	});

	it('ignores an unreadable amount once the form has moved to quantity × price', async () => {
		render(BudgetLinePanel, { line: quantityLine(), category: null, onclose: () => {} });
		await fireEvent.click(screen.getByText('Amount'));
		await settle();
		await typeUnreadable(amountInputs()[0]);
		await fireEvent.click(screen.getByText('Count × price'));
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
		await fireEvent.click(screen.getByText('Count × price'));
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
		const held = hold(api.parseSum);
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
		const held = hold(api.parseSum);
		await type(statement(), '1800');
		await saveDuringRead(held, cents(180000));
		expect(api.updateReconciliation).toHaveBeenCalledWith(1, '2026-06-30', 180000);
	});

	it('adds a new statement after the read when saved from the keyboard', async () => {
		render(StatementPanel, { account: 1, onclose: () => {} });
		const held = hold(api.parseSum);
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

/** The event with one tier, one manual sale and one cancellation of it. */
function withTickets(change: (s: EventSnapshot) => void = () => {}) {
	const next = structuredClone(snapshot) as any;
	next.ticketSettings.account = 1;
	next.tickets.tiers = [
		{
			id: 1,
			name: 'Early',
			price: 2500,
			feeBp: 0,
			quota: null,
			aliases: [],
			sold: 0,
			sheet: 0,
			manual: 0,
			cancelled: 0,
			gross: 0,
			fees: 0,
			refunds: 0,
			net: 0,
			unresolved: 0
		}
	];
	next.tickets.manualSales = [
		{ id: 1, tier: 1, count: 2, destination: 2, date: '2026-06-13', reference: null, gross: 5000 }
	];
	next.tickets.cancellations = [
		{
			id: 1,
			order: 'A1',
			sale: null,
			tier: 1,
			count: 1,
			terms: { refundBp: 10000, date: '2026-06-13', payer: null },
			payer: 1,
			note: null
		}
	];
	change(next);
	session.update({ ...session.event!, snapshot: next });
}

describe('Mark as paid', () => {
	const open = () => {
		const next = structuredClone(snapshot);
		next.payouts.payouts[0].iban = 'DE89370400440532013000';
		session.update({ ...session.event!, snapshot: next });
		render(MarkPaidPanel, { onclose: () => {} });
	};
	const click = () => fireEvent.click(screen.getByRole('button', { name: /^Mark as paid/ }));

	it('pays on the date typed just before the click', async () => {
		open();
		const held = hold(api.parseDate);
		await type(screen.getByLabelText('Date'), '1.7.26');
		await saveDuringRead(held, { date: '2026-07-01' }, click);
		expect(api.markPayoutsPaid.mock.calls[0][1]).toBe('2026-07-01');
	});

	it('records who is ticked: bank transfers from the start, other methods by choice', async () => {
		const next = structuredClone(snapshot);
		const [anna, malte] = next.payouts.payouts;
		anna.iban = 'DE89370400440532013000';
		malte.otherMethod = 'PayPal: malte@example.invalid';
		session.update({ ...session.event!, snapshot: next });
		render(MarkPaidPanel, { onclose: () => {} });

		const box = (name: string) => screen.getByRole('checkbox', { name: new RegExp(name) });
		expect((box('Anna') as HTMLInputElement).checked).toBe(true);
		expect((box('Malte') as HTMLInputElement).checked).toBe(false);
		await fireEvent.click(box('Anna'));
		await fireEvent.click(box('Malte'));
		await click();
		await settle();
		expect(api.markPayoutsPaid.mock.calls[0][2]).toEqual([malte.person]);
	});

	it('does not pay on the old date when the new one cannot be read', async () => {
		open();
		await typeUnreadableDate(screen.getByLabelText('Date'));
		await click();
		await settle();
		expect(api.markPayoutsPaid).not.toHaveBeenCalled();
	});
});

describe('The manual sale panel', () => {
	const open = () => {
		withTickets();
		render(SalePanel, { id: 1, onopen: () => {}, onclose: () => {} });
	};

	it('saves the date typed just before Save, not the old one', async () => {
		open();
		const held = hold(api.parseDate);
		await type(screen.getByLabelText('Date'), '1.7.26');
		await saveDuringRead(held, { date: '2026-07-01' });
		expect(api.updateManualSale.mock.calls[0][1].date).toBe('2026-07-01');
	});

	it('does not save a date it cannot read', async () => {
		open();
		await typeUnreadableDate(screen.getByLabelText('Date'));
		await clickSave();
		await settle();
		expect(api.updateManualSale).not.toHaveBeenCalled();
	});
});

describe('The cancellation panel', () => {
	const open = () => {
		withTickets();
		render(CancellationPanel, { id: 1, onclose: () => {} });
	};

	it('saves the date typed just before Save, not the old one', async () => {
		open();
		const held = hold(api.parseDate);
		await type(screen.getByLabelText('Date'), '1.7.26');
		await saveDuringRead(held, { date: '2026-07-01' });
		expect(api.updateCancellation.mock.calls[0][1].terms.date).toBe('2026-07-01');
	});

	it('does not save a date it cannot read', async () => {
		open();
		await typeUnreadableDate(screen.getByLabelText('Date'));
		await clickSave();
		await settle();
		expect(api.updateCancellation).not.toHaveBeenCalled();
	});
});

describe('A conflict card', () => {
	const open = () => {
		withTickets();
		render(ConflictCard, {
			conflict: { order: 'A1', removed: [], added: [] },
			removed: { tier: 1, count: 1, since: 1, matches: [] }
		} as never);
	};
	const click = () => fireEvent.click(screen.getByRole('button', { name: /^Resolve$/ }));

	it('resolves with the date typed just before the click', async () => {
		open();
		const held = hold(api.parseDate);
		await type(screen.getByLabelText('Date'), '1.7.26');
		await saveDuringRead(held, { date: '2026-07-01' }, click);
		expect(api.resolveConflict.mock.calls[0][2].terms.date).toBe('2026-07-01');
	});

	it('does not resolve with a date it cannot read', async () => {
		open();
		await typeUnreadableDate(screen.getByLabelText('Date'));
		await click();
		await settle();
		expect(api.resolveConflict).not.toHaveBeenCalled();
	});
});

describe('The import panel', () => {
	const open = async () => {
		withTickets();
		render(ImportPanel, { batch: {} as never, onclose: () => {}, onnewtier: () => {} });
		await settle();
	};
	const date = () => screen.getByLabelText('Snapshot date');
	const click = () => fireEvent.click(screen.getByRole('button', { name: /^Import ⌘/ }));

	it('imports with the date typed just before the click', async () => {
		await open();
		const held = hold(api.parseDate);
		await type(date(), '1.7.26');
		await saveDuringRead(held, { date: '2026-07-01' }, click);
		expect(api.importTickets.mock.calls[0][1]).toBe('2026-07-01');
	});

	it('does not import with a date it cannot read', async () => {
		await open();
		await typeUnreadableDate(date());
		await click();
		await fireEvent.keyDown(window, { key: 'Enter', metaKey: true });
		await settle();
		expect(api.importTickets).not.toHaveBeenCalled();
	});
});

describe('The new event panel', () => {
	const click = () => fireEvent.click(screen.getByRole('button', { name: /^Create/ }));
	const open = async () => {
		render(NewEventPanel, { onclose: () => {} });
		await type(screen.getByLabelText('Name'), 'Fest');
		await type(screen.getByLabelText('Account name'), 'Bank');
		await settle();
		const from = screen.getByLabelText('From') as HTMLInputElement;
		from.focus();
		return from;
	};

	it('creates with the date typed just before the click, before asking for a file', async () => {
		const from = await open();
		const typing = hold(api.parseDate);
		await type(from, '1.7.26');
		const leaving = hold(api.parseDate);
		await click();
		await settle();
		expect(chooseNewEventFile).not.toHaveBeenCalled();
		typing.resolve({ date: '2026-07-01' });
		await settle();
		expect(chooseNewEventFile).not.toHaveBeenCalled();
		leaving.resolve({ date: '2026-07-01' });
		await settle();
		expect(api.createEvent.mock.calls[0][1].startsOn).toBe('2026-07-01');
		expect(api.createEvent.mock.calls[0][1].currency).toBe('EUR');
	});

	it('does not create an event without the date it could not read', async () => {
		const from = await open();
		await typeUnreadableDate(from);
		api.parseDate.mockRejectedValueOnce(unreadable());
		await click();
		await settle();
		await settle();
		expect(chooseNewEventFile).not.toHaveBeenCalled();
		expect(api.createEvent).not.toHaveBeenCalled();
	});

	it('needs a named account, and leaves empty rows out', async () => {
		await open();
		const create = () => screen.getByRole('button', { name: /^Create/ }) as HTMLButtonElement;
		expect(create().disabled).toBe(false);

		await fireEvent.click(screen.getByRole('button', { name: /Account$/ }));
		await settle();
		await click();
		await settle();
		await settle();
		expect(api.createEvent.mock.calls[0][2]).toEqual([{ name: 'Bank', assetType: 'bank' }]);

		const [first] = screen.getAllByLabelText('Account name');
		await fireEvent.input(first, { target: { value: '  ' } });
		await settle();
		expect(create().disabled).toBe(true);
	});

	it('says when two accounts have the same name, before asking for a file', async () => {
		await open();
		await fireEvent.click(screen.getByRole('button', { name: /Account$/ }));
		await settle();
		const [, second] = screen.getAllByLabelText('Account name');
		await fireEvent.input(second, { target: { value: 'BANK' } });
		await settle();
		expect(screen.getByText('Two accounts have the same name')).toBeTruthy();
		await click();
		await settle();
		await settle();
		expect(chooseNewEventFile).not.toHaveBeenCalled();
	});
});

describe('A field that is gone', () => {
	it('ignores an amount read that returns afterwards', async () => {
		const oninput = vi.fn();
		const { unmount } = render(AmountField, { label: 'Late', value: null, format: 'de', oninput });
		const held = hold(api.parseSum);
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
