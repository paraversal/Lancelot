import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { api, ApiError } from '#lib/api/index.ts';
import example from '#lib/api/fixtures/example-event.json';
import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import { dataSources } from '#lib/plugins/sources.svelte.ts';
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

const card = (name: string) =>
	screen.getByLabelText(`${name} on`).closest('[data-testid="plugin-card"]') as HTMLElement;

const settled = () => new Promise((resolve) => setTimeout(resolve));

const shop = {
	label: 'Shop',
	url: 'https://shop.example',
	authHeader: 'Authorization',
	list: 'tickets',
	fields: { tier: 'tier', order: 'order' }
};

/** Stock and sales reads the drinks from one source and the merch from another. */
const bar = {
	label: 'Bar app',
	url: 'https://bar.example',
	authHeader: 'Authorization',
	list: 'drinks',
	fields: { name: 'name', quantity: 'qty' }
};

function openTwoCentres() {
	open({
		costCentres: [...snapshot.costCentres, { id: 2, name: 'Merch' }],
		pluginRecords: [
			...snapshot.pluginRecords,
			{
				id: 900,
				plugin: 'drinks',
				key: 'centre:2',
				scope: 'setup' as const,
				value: { supplier: null, commission: true, deposit: null }
			}
		]
	});
}

beforeEach(() => {
	vi.spyOn(api, 'backups').mockResolvedValue([]);
});

afterEach(() => {
	dataSources.set({});
	vi.restoreAllMocks();
});

describe('The settings page', () => {
	it('groups categories by kind', () => {
		open();
		render(Page);
		const categories = block('Categories');
		expect(within(categories).getByText('Expenses')).toBeTruthy();
		expect(within(categories).getByText('Income')).toBeTruthy();
	});

	it('lists the categories without a cost centre beside each', () => {
		open();
		render(Page);
		const categories = block('Categories');
		expect(within(categories).getByText('Kitchen')).toBeTruthy();
		expect(within(categories).queryByRole('combobox')).toBeNull();
	});
});

describe('Adding a cost centre', () => {
	const name = () => screen.getByLabelText('New cost centre name') as HTMLInputElement;
	const start = async () => {
		await fireEvent.click(
			within(block('Cost centres')).getByRole('button', { name: 'Cost centre' })
		);
		return name();
	};

	it('starts empty, with a placeholder, the focus and nothing written', async () => {
		open();
		const add = vi.spyOn(api, 'addCostCentre');
		render(Page);
		const input = await start();
		expect(input.value).toBe('');
		expect(input.placeholder).toBe('Name');
		expect(document.activeElement).toBe(input);
		expect(add).not.toHaveBeenCalled();
		expect(within(block('Cost centres')).queryByRole('button', { name: 'Cost centre' })).toBeNull();
	});

	it('discards an empty draft on Esc', async () => {
		open();
		const add = vi.spyOn(api, 'addCostCentre');
		render(Page);
		await fireEvent.keyDown(await start(), { key: 'Escape' });
		expect(screen.queryByLabelText('New cost centre name')).toBeNull();
		expect(add).not.toHaveBeenCalled();
	});

	it('saves the typed name on Esc, and closes the draft', async () => {
		open();
		const add = vi.spyOn(api, 'addCostCentre').mockResolvedValue(session.event!);
		render(Page);
		const input = await start();
		await fireEvent.input(input, { target: { value: '  Merch ' } });
		await fireEvent.keyDown(input, { key: 'Escape' });
		await settled();
		expect(add).toHaveBeenCalledExactlyOnceWith('Merch');
		expect(screen.queryByLabelText('New cost centre name')).toBeNull();
	});

	it('saves the typed name on Enter', async () => {
		open();
		const add = vi.spyOn(api, 'addCostCentre').mockResolvedValue(session.event!);
		render(Page);
		const input = await start();
		await fireEvent.input(input, { target: { value: 'Merch' } });
		await fireEvent.keyDown(input, { key: 'Enter' });
		await settled();
		expect(add).toHaveBeenCalledExactlyOnceWith('Merch');
	});

	it('keeps a name the core refuses with its reason, and the next Esc discards it', async () => {
		open();
		const add = vi.spyOn(api, 'addCostCentre').mockRejectedValue(
			new ApiError({
				code: 'cost_centre_name',
				message: 'There is already a cost centre called Drinks',
				field: 'name',
				report: null
			})
		);
		render(Page);
		const input = await start();
		await fireEvent.input(input, { target: { value: 'Drinks' } });
		await fireEvent.keyDown(input, { key: 'Escape' });
		await settled();
		expect(screen.getByRole('alert').textContent).toContain('already a cost centre');
		expect(name().value).toBe('Drinks');
		await fireEvent.keyDown(name(), { key: 'Escape' });
		expect(screen.queryByLabelText('New cost centre name')).toBeNull();
		expect(add).toHaveBeenCalledTimes(1);
	});
});

describe('The cost centres', () => {
	const covers = (centre: string) => screen.getByRole('group', { name: `Categories of ${centre}` });

	it('lists the categories whose new lines start in each', () => {
		open();
		render(Page);
		const drinks = covers('Drinks');
		expect(within(drinks).getByText('Drink donations')).toBeTruthy();
		expect(within(drinks).getByText('Deposit')).toBeTruthy();
		expect(within(drinks).queryByText('Kitchen', { selector: '.tag' })).toBeNull();
	});

	it('adds a category to a cost centre', async () => {
		open();
		const tag = vi.spyOn(api, 'setCategoryCostCentre').mockResolvedValue(session.event!);
		render(Page);
		const pick = screen.getByLabelText('Add a category to Drinks') as HTMLSelectElement;
		await fireEvent.change(pick, { target: { value: '6' } });
		expect(tag).toHaveBeenCalledWith(6, 1);
	});

	it('moves a category from another cost centre, and says so first', async () => {
		openTwoCentres();
		const tag = vi.spyOn(api, 'setCategoryCostCentre').mockResolvedValue(session.event!);
		render(Page);
		const pick = screen.getByLabelText('Add a category to Merch') as HTMLSelectElement;
		const deposit = [...pick.options].find((o) => o.value === '13')!;
		expect(deposit.text).toBe('Deposit · now in Drinks');
		await fireEvent.change(pick, { target: { value: '13' } });
		expect(tag).toHaveBeenCalledWith(13, 2);
	});

	it('removes a category from its cost centre', async () => {
		open();
		const tag = vi.spyOn(api, 'setCategoryCostCentre').mockResolvedValue(session.event!);
		render(Page);
		await fireEvent.click(screen.getByRole('button', { name: 'Remove Deposit from Drinks' }));
		expect(tag).toHaveBeenCalledWith(13, null);
	});

	it('names the kind when an income and an expense share a name', () => {
		open({
			accounts: snapshot.accounts.map((a) => (a.id === 10 ? { ...a, costCentre: 1 } : a))
		});
		render(Page);
		expect(within(covers('Drinks')).getByText('General (expense)')).toBeTruthy();
	});

	it('cannot change while the event is closed', () => {
		open({ event: { ...snapshot.event, closedAt: '2026-07-01T12:00:00Z' } });
		render(Page);
		expect(within(covers('Drinks')).getByText('Deposit')).toBeTruthy();
		expect(screen.queryByLabelText('Add a category to Drinks')).toBeNull();
		expect(screen.queryByRole('button', { name: 'Remove Deposit from Drinks' })).toBeNull();
	});
});

describe('The plugin cards', () => {
	it('gives each plugin a card that says how it is set up', () => {
		open();
		dataSources.set({ 'ticket-api': { source: shop, hasToken: true } });
		render(Page);
		expect(screen.getAllByTestId('plugin-card')).toHaveLength(5);
		expect(within(card('Ticket API')).getByText('Pulls from Shop')).toBeTruthy();
		expect(within(card('Cash box count')).getByText('Denominations · EUR')).toBeTruthy();
		expect(within(card('What if')).getByText('Nothing to set up')).toBeTruthy();
	});

	it('says a data source is not set up yet', () => {
		open();
		render(Page);
		expect(within(card('Ticket API')).getByText('No data source yet')).toBeTruthy();
	});

	it('opens the plugin’s settings in the panel, and closes them', async () => {
		open();
		render(Page);
		await fireEvent.click(
			within(card('Cash box count')).getByRole('button', { name: 'Configure' })
		);
		const panel = screen.getByRole('complementary');
		expect(within(panel).getByRole('heading', { name: 'Cash box count' })).toBeTruthy();
		expect(within(panel).getByTestId('cash-settings-notes')).toBeTruthy();
		await fireEvent.click(within(panel).getByRole('button', { name: 'Close' }));
		expect(screen.queryByTestId('cash-settings-notes')).toBeNull();
	});

	it('opens a data source on the fields that configure it', async () => {
		open();
		render(Page);
		await fireEvent.click(within(card('Ticket API')).getByRole('button', { name: 'Configure' }));
		const panel = screen.getByRole('complementary');
		expect(within(panel).getByLabelText('URL')).toBeTruthy();
	});

	it('sets up a data source for each cost centre the plugin is shown on', async () => {
		openTwoCentres();
		dataSources.set({
			'drinks:drinks': { source: bar, hasToken: true },
			'drinks:merch': {
				source: { ...bar, label: 'Merch app', url: 'https://merch.example' },
				hasToken: true
			}
		});
		render(Page);
		await fireEvent.click(
			within(card('Stock and sales')).getByRole('button', { name: 'Configure' })
		);
		const panel = screen.getByRole('complementary');
		await settled();
		expect(
			within(panel)
				.getAllByRole('radio')
				.map((r) => r.textContent?.trim())
		).toEqual(['Drinks', 'Merch']);
		const url = () => within(panel).getByRole('textbox', { name: 'URL' }) as HTMLInputElement;
		expect(url().value).toBe('https://bar.example');
		await fireEvent.click(within(panel).getByRole('radio', { name: 'Merch' }));
		await settled();
		expect(url().value).toBe('https://merch.example');
	});

	it('keeps what is typed in a tab when another is chosen', async () => {
		openTwoCentres();
		render(Page);
		await fireEvent.click(
			within(card('Stock and sales')).getByRole('button', { name: 'Configure' })
		);
		const panel = screen.getByRole('complementary');
		await settled();
		const url = () => within(panel).getByRole('textbox', { name: 'URL' }) as HTMLInputElement;
		await fireEvent.input(url(), { target: { value: 'https://typed.example' } });
		await fireEvent.click(within(panel).getByRole('radio', { name: 'Merch' }));
		await settled();
		await fireEvent.click(within(panel).getByRole('radio', { name: 'Drinks' }));
		await settled();
		expect(url().value).toBe('https://typed.example');
	});

	it('saves a cost centre’s source under its own id', async () => {
		openTwoCentres();
		const save = vi.spyOn(api, 'setDataSource').mockResolvedValue({});
		vi.spyOn(api, 'pullDataSource').mockResolvedValue({
			label: 'Merch app',
			sha256: 'ab',
			fields: ['name', 'quantity'],
			rows: [],
			problems: [],
			missing: []
		});
		render(Page);
		await fireEvent.click(
			within(card('Stock and sales')).getByRole('button', { name: 'Configure' })
		);
		const panel = screen.getByRole('complementary');
		await settled();
		await fireEvent.click(within(panel).getByRole('radio', { name: 'Merch' }));
		await settled();
		await fireEvent.input(within(panel).getByRole('textbox', { name: 'URL' }), {
			target: { value: 'https://merch.example' }
		});
		await fireEvent.click(within(panel).getByRole('button', { name: 'Save and test' }));
		await settled();
		expect(save.mock.calls[0]![0]).toBe('drinks:merch');
	});

	it('says which cost centres pull, since each has a source of its own', () => {
		openTwoCentres();
		dataSources.set({
			'drinks:drinks': { source: bar, hasToken: true },
			'drinks:merch': { source: { ...bar, label: 'Merch app' }, hasToken: true }
		});
		render(Page);
		expect(within(card('Stock and sales')).getByText('Pulls for Drinks and Merch')).toBeTruthy();
	});

	it('switching one off closes its settings and says so', async () => {
		open();
		const off = { ...session.event!, snapshot: structuredClone(snapshot) };
		off.snapshot.pluginRecords.push({
			id: 999,
			plugin: 'cash-count',
			key: 'host:off',
			scope: 'setup',
			value: true
		});
		vi.spyOn(api, 'applyPluginRecords').mockResolvedValue(off);
		render(Page);
		await fireEvent.click(
			within(card('Cash box count')).getByRole('button', { name: 'Configure' })
		);
		expect(screen.getByTestId('cash-settings-notes')).toBeTruthy();

		await fireEvent.click(screen.getByLabelText('Cash box count on'));
		await settled();
		expect(screen.queryByTestId('cash-settings-notes')).toBeNull();
		expect(within(card('Cash box count')).getByText('Switched off')).toBeTruthy();
		expect(within(card('Cash box count')).queryByRole('button', { name: 'Configure' })).toBeNull();
	});
});
