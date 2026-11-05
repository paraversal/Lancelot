import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '#lib/api/index.ts';
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
