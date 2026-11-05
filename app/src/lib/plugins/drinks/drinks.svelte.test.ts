import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { goto } from '$app/navigation';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '../../api/index';
import type { EventSnapshot } from '../../api/generated/EventSnapshot';
import example from '../../api/fixtures/example-event.json';
import { session } from '../../state/session.svelte';
import { takeProposal } from '../host';
import Centre from '../../../routes/event/centre/Centre.svelte';
import { pluginContext } from '../host';
import { dataSources } from '../sources.svelte';
import { EXPLAINERS } from './figures';
import Settings from './Settings.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

const drinksCentre = () => session.event!.snapshot.costCentres.find((c) => c.name === 'Drinks')!;

function open(change: (s: EventSnapshot) => void = () => {}) {
	const snapshot = structuredClone(example) as unknown as EventSnapshot;
	change(snapshot);
	session.update({
		revision: Date.now(),
		path: '/x.lancelot',
		fileName: 'x.lancelot',
		snapshot,
		integrity: { issues: [] },
		backupError: null
	});
}

const text = (id: string) => screen.getByTestId(id).textContent?.replace(/\s+/g, ' ') ?? '';

beforeEach(() => {
	vi.restoreAllMocks();
	vi.clearAllMocks();
	takeProposal();
	dataSources.set({});
});

describe('The stock and sales section', () => {
	it('shows billed, left over, logged and unaccounted', () => {
		open();
		render(Centre, { centre: drinksCentre() });
		expect(screen.getByText('Club-Mate 0,5')).toBeTruthy();
		expect(screen.getByText('Hoffmann · on commission')).toBeTruthy();
		const total = text('plugin-drinks-total');
		expect(total).toContain('410');
		expect(total).toContain('420,00');
		expect(total).toContain('851,00');
	});

	it('compares what was logged with what was collected', () => {
		open();
		render(Centre, { centre: drinksCentre() });
		const collected = text('plugin-drinks-collected');
		expect(collected).toContain('851,00');
		expect(collected).toContain('812,50');
		expect(collected).toContain('38,50 € less than logged');
	});

	it('checks an invoice that matches, without deposit', () => {
		open();
		render(Centre, { centre: drinksCentre() });
		const invoice = text('plugin-drinks-invoice');
		expect(invoice).toContain('Invoice incl. deposit 32,00');
		expect(invoice).toContain('452,00');
		expect(invoice).not.toContain('than the records');
	});

	it('says how far an invoice is from the records', () => {
		open((s) => {
			const line = s.txns
				.find((t) => t.counterparty === 'Hoffmann')!
				.lines.find((l) => l.amount === 42000)!;
			line.amount = 43150;
		});
		render(Centre, { centre: drinksCentre() });
		expect(text('plugin-drinks-invoice')).toContain('11,50 € more than the records');
	});

	it('is not on a cost centre without the plugin, and has no actions once closed', () => {
		open((s) => {
			s.costCentres.push({ id: 99, name: 'Workshop' });
		});
		const { unmount } = render(Centre, { centre: { id: 99, name: 'Workshop' } });
		expect(screen.queryByTestId('plugin-drinks-total')).toBeNull();
		unmount();

		open((s) => {
			s.event.closedAt = '2026-06-20T10:00:00Z';
		});
		render(Centre, { centre: drinksCentre() });
		expect(screen.getByTestId('plugin-drinks-total')).toBeTruthy();
		expect(screen.queryByText('Book invoice')).toBeNull();
	});

	it('proposes the invoice as a normal entry, and saves nothing', async () => {
		open();
		const apply = vi.spyOn(api, 'applyPluginRecords');
		render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Book invoice'));

		expect(goto).toHaveBeenCalledWith('/event/ledger?proposal');
		const proposal = takeProposal()!;
		if (proposal.kind !== 'expense') throw new Error(proposal.kind);
		expect(proposal.counterparty).toBe('Hoffmann');
		expect(proposal.amount).toBe(42000);
		const deposit = session.event!.snapshot.accounts.find((a) => a.name === 'Deposit')!;
		expect(proposal.lines.map((l) => l.account)).not.toContain(deposit.id);
		expect(apply).not.toHaveBeenCalled();
	});

	it('explains the optional Deposit category', async () => {
		open();
		render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Hoffmann · on commission'));
		expect(screen.getByLabelText('Deposit category')).toBeTruthy();
		expect(screen.getByText(/^Optional\. Expenses in this category/)).toBeTruthy();
		expect(screen.getByRole('option', { name: 'None' })).toBeTruthy();
	});

	it('switches a supplier to goods bought outright', async () => {
		open();
		const apply = vi.spyOn(api, 'applyPluginRecords').mockResolvedValue(session.event!);
		render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Hoffmann · on commission'));
		await fireEvent.click(screen.getByLabelText('On commission'));
		await fireEvent.click(screen.getByText('Save'));

		await waitFor(() => expect(apply).toHaveBeenCalledOnce());
		expect(apply.mock.calls[0][1][0].value).toMatchObject({
			supplier: 'Hoffmann',
			commission: false
		});
	});

	it('has no Returned count for goods bought outright', async () => {
		open((s) => {
			const record = s.pluginRecords.find((r) => r.key.startsWith('centre:'))!;
			record.value = { ...(record.value as object), commission: false };
		});
		render(Centre, { centre: drinksCentre() });
		expect(screen.getByText('Hoffmann')).toBeTruthy();
		await fireEvent.click(screen.getByText('Club-Mate 0,5'));
		expect(screen.getByLabelText('Taken')).toBeTruthy();
		expect(screen.queryByLabelText('Returned')).toBeNull();
	});

	it('asks for the cost per unit without deposit', async () => {
		open();
		render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Club-Mate 0,5'));
		expect(screen.getByText('Without deposit.')).toBeTruthy();
	});

	it('goes straight on to the invoice after asking for the supplier', async () => {
		const withoutSupplier = (s: EventSnapshot) => {
			const record = s.pluginRecords.find((r) => r.key.startsWith('centre:'))!;
			record.value = { supplier: null, deposit: null };
		};
		open(withoutSupplier);
		const apply = vi.spyOn(api, 'applyPluginRecords').mockImplementation(async (_, changes) => {
			open((s) => {
				const record = s.pluginRecords.find((r) => r.key.startsWith('centre:'))!;
				record.value = changes[0].value;
			});
			return session.event!;
		});
		render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Book invoice'));
		expect(goto).not.toHaveBeenCalled();
		await fireEvent.input(screen.getByLabelText('Supplier'), { target: { value: 'Hoffmann' } });
		await fireEvent.click(screen.getByText('Continue'));

		await waitFor(() => expect(goto).toHaveBeenCalledWith('/event/ledger?proposal'));
		expect(apply).toHaveBeenCalledOnce();
		expect(takeProposal()).toMatchObject({ kind: 'expense', counterparty: 'Hoffmann' });
	});

	it('takes a tally with a donation and a logged count, without stock or cost', async () => {
		open();
		const apply = vi.spyOn(api, 'applyPluginRecords').mockResolvedValue(session.event!);
		render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Product'));
		await fireEvent.input(screen.getByLabelText('Product'), { target: { value: 'Tally' } });
		await fireEvent.input(screen.getByLabelText('Logged'), { target: { value: '700' } });
		await fireEvent.click(screen.getByText('Save'));

		await waitFor(() => expect(apply).toHaveBeenCalledOnce());
		const [, changes] = apply.mock.calls[0];
		expect(changes[0]).toMatchObject({ key: 'product:6', scope: 'setup' });
		expect(changes[0].value).toMatchObject({ name: 'Tally', cost: 0 });
		expect(changes[1]).toMatchObject({ key: 'count:6', scope: 'records' });
		expect(changes[1].value).toMatchObject({ taken: 0, manual: 700 });
	});

	it('still asks for the cost of what was taken', async () => {
		open();
		const apply = vi.spyOn(api, 'applyPluginRecords');
		render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Product'));
		await fireEvent.input(screen.getByLabelText('Product'), { target: { value: 'Pils' } });
		await fireEvent.input(screen.getByLabelText('Taken'), { target: { value: '24' } });
		await fireEvent.click(screen.getByText('Save'));
		expect(screen.getByText('enter a price')).toBeTruthy();
		expect(apply).not.toHaveBeenCalled();
	});

	it('explains the counts in the product panel', async () => {
		open();
		render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Club-Mate 0,5'));
		for (const line of Object.values(EXPLAINERS)) expect(screen.getByText(line)).toBeTruthy();
	});

	it('opens and closes its panel, and removes it with the page', async () => {
		open();
		const { unmount } = render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Club-Mate 0,5'));
		expect(screen.getByLabelText('Left over')).toBeTruthy();
		await fireEvent.click(screen.getByText('Lager 0,33'));
		expect(screen.getByRole('heading', { name: 'Lager 0,33' })).toBeTruthy();
		await fireEvent.click(screen.getByText('Book invoice'));
		unmount();
		expect(screen.queryByLabelText('Left over')).toBeNull();
	});

	it('waits for an amount typed just before saving', async () => {
		open();
		let finish = (_: { cents: number; ambiguous: boolean; terms: number }) => {};
		vi.spyOn(api, 'parseSum').mockImplementation(
			() => new Promise((resolve) => (finish = resolve))
		);
		const apply = vi.spyOn(api, 'applyPluginRecords').mockResolvedValue(session.event!);
		render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Product'));
		await fireEvent.input(screen.getByLabelText('Product'), { target: { value: 'Tally' } });
		await fireEvent.input(screen.getByLabelText('Donation per unit'), {
			target: { value: '0,50' }
		});
		await fireEvent.click(screen.getByText('Save'));
		expect(apply).not.toHaveBeenCalled();

		finish({ cents: 50, ambiguous: false, terms: 1 });
		await waitFor(() => expect(apply).toHaveBeenCalledOnce());
		expect(apply.mock.calls[0][1][0].value).toMatchObject({ name: 'Tally', donation: 50 });
	});

	it('does not save over an amount it cannot read', async () => {
		open();
		vi.spyOn(api, 'parseSum').mockRejectedValue(new Error('not an amount'));
		const apply = vi.spyOn(api, 'applyPluginRecords').mockResolvedValue(session.event!);
		render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Product'));
		await fireEvent.input(screen.getByLabelText('Product'), { target: { value: 'Tally' } });
		await fireEvent.input(screen.getByLabelText('Donation per unit'), {
			target: { value: 'abc' }
		});
		await fireEvent.click(screen.getByText('Save'));
		await waitFor(() => expect(screen.getAllByText(/not an amount/).length).toBeGreaterThan(0));
		expect(apply).not.toHaveBeenCalled();
	});

	it('refuses more left over than billed, and saves nothing', async () => {
		open();
		const apply = vi.spyOn(api, 'applyPluginRecords');
		render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Club-Mate 0,5'));
		await fireEvent.input(screen.getByLabelText('Left over'), { target: { value: '500' } });
		await fireEvent.click(screen.getByText('Save'));
		expect(screen.getByText('more left over than billed')).toBeTruthy();
		expect(apply).not.toHaveBeenCalled();
	});
});

describe('The plugin’s settings', () => {
	const ctx = () => pluginContext('drinks');

	it('sets up a cost centre, and tags the categories that are not in another one', async () => {
		open((s) => {
			s.costCentres = [];
			s.pluginRecords = [];
			s.accounts = s.accounts.filter((a) => a.name !== 'Drinks' && a.name !== 'Drink donations');
			for (const a of s.accounts) a.costCentre = null;
		});
		const context = ctx();
		const centre = vi.spyOn(api, 'addCostCentre').mockImplementation(async () => {
			open((s) => {
				s.costCentres = [{ id: 7, name: 'Drinks' }];
				s.pluginRecords = [];
				s.accounts = s.accounts.filter((a) => a.name !== 'Drinks' && a.name !== 'Drink donations');
				for (const a of s.accounts) a.costCentre = null;
			});
			return session.event!;
		});
		const add = vi
			.spyOn(api, 'addAccount')
			.mockImplementation(async () => ({ event: session.event!, id: 100 }));
		const tag = vi.spyOn(api, 'setCategoryCostCentre').mockResolvedValue(session.event!);
		const apply = vi.spyOn(api, 'applyPluginRecords').mockResolvedValue(session.event!);
		const deposit = session.event!.snapshot.accounts.find((a) => a.name === 'Deposit')!;

		render(Settings, { ctx: context });
		await fireEvent.input(screen.getByLabelText('New cost centre for goods'), {
			target: { value: ' Drinks ' }
		});
		await fireEvent.click(screen.getByText('Add cost centre'));

		await waitFor(() => expect(apply).toHaveBeenCalledOnce());
		expect(centre).toHaveBeenCalledWith('Drinks');
		expect(add.mock.calls.map((c) => c[1])).toEqual(['Drinks income', 'Drinks']);
		expect(tag).toHaveBeenCalledWith(deposit.id, 7);
		expect(apply.mock.calls[0][1][0]).toMatchObject({
			key: 'centre:7',
			value: { supplier: null, commission: true, deposit: deposit.id }
		});
	});

	it('asks for a name before adding a cost centre', async () => {
		const centre = vi.spyOn(api, 'addCostCentre');
		render(Settings, { ctx: ctx() });
		await fireEvent.click(screen.getByText('Add cost centre'));
		expect(await screen.findByText('Enter a name for the cost centre.')).toBeTruthy();
		expect(centre).not.toHaveBeenCalled();
	});

	it('names products whose cost centre was deleted, and moves them', async () => {
		open((s) => {
			s.costCentres = [{ id: 7, name: 'Bar' }];
		});
		const apply = vi.spyOn(api, 'applyPluginRecords').mockResolvedValue(session.event!);
		render(Settings, { ctx: ctx() });
		expect(screen.getByTestId('plugin-drinks-orphans').textContent).toContain('5');
		await fireEvent.change(screen.getByLabelText('Move to'), { target: { value: '7' } });

		await waitFor(() => expect(apply).toHaveBeenCalledOnce());
		const changes = apply.mock.calls[0][1];
		expect(changes).toHaveLength(5);
		expect(changes.every((c) => (c.value as { centre: number }).centre === 7)).toBe(true);
	});
});

describe('A cost centre’s own source', () => {
	const source = {
		label: 'Bar app',
		url: 'https://bar.example',
		authHeader: 'Authorization',
		list: 'drinks',
		fields: { name: 'name', quantity: 'qty' }
	};
	const read = {
		label: 'Bar app',
		sha256: 'ab',
		fields: ['name', 'quantity'],
		rows: [],
		problems: [],
		missing: []
	};

	it('pulls from the source of the cost centre it is on', async () => {
		open((s) => {
			s.costCentres.push({ id: 2, name: 'Merch' });
			s.pluginRecords.push({
				id: 900,
				plugin: 'drinks',
				key: 'centre:2',
				scope: 'setup',
				value: { supplier: null, commission: true, deposit: null }
			});
		});
		dataSources.set({
			'drinks:drinks': { source, hasToken: true },
			'drinks:merch': { source: { ...source, label: 'Merch app' }, hasToken: true }
		});
		const pull = vi.spyOn(api, 'pullDataSource').mockResolvedValue(read);
		render(Centre, { centre: drinksCentre() });
		await fireEvent.click(screen.getByText('Pull logged'));
		await waitFor(() => expect(pull).toHaveBeenCalledOnce());
		expect(pull.mock.calls[0]![0]).toBe('drinks:drinks');
	});

	it('offers no pull on a cost centre whose source is not set up', () => {
		open();
		dataSources.set({ 'drinks:merch': { source, hasToken: true } });
		render(Centre, { centre: drinksCentre() });
		expect(screen.queryByText('Pull logged')).toBeNull();
	});
});
