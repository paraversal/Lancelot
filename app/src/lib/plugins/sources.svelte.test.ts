import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { api, ApiError } from '../api/index';
import type { Pulled } from '../api/generated/Pulled';
import { ticketApi } from './ticket-api/index';
import type { PluginContext } from './api';
import DataSourceForm from './DataSourceForm.svelte';
import { dataSources } from './sources.svelte';
import TicketApiSection from './ticket-api/Section.svelte';

const spec = ticketApi.dataSource!;
const source = {
	label: 'Shop',
	url: 'https://x.example',
	authHeader: 'Authorization',
	list: 'tickets',
	fields: { tier: 'tier', order: 'order' }
};
const pulled = (rows: Record<string, string>[], over: Partial<Pulled> = {}): Pulled => ({
	label: 'Shop',
	sha256: 'ab',
	fields: ['tier', 'order'],
	rows,
	problems: [],
	missing: [],
	...over
});
const settled = () => new Promise((resolve) => setTimeout(resolve));
const button = (name: RegExp) => screen.getByRole('button', { name });

afterEach(() => {
	dataSources.set({});
	vi.restoreAllMocks();
});

describe('The data source form', () => {
	it('fills in the usual names and saves the token apart, then tries the source', async () => {
		const views = { 'ticket-api': { source, hasToken: true } };
		const save = vi.spyOn(api, 'setDataSource').mockResolvedValue(views);
		const saveToken = vi.spyOn(api, 'setDataSourceToken').mockResolvedValue(views);
		const pull = vi
			.spyOn(api, 'pullDataSource')
			.mockResolvedValue(pulled([{ tier: 'Regular', order: '1' }]));
		render(DataSourceForm, { id: 'ticket-api', spec });
		await fireEvent.click(button(/Set up/));
		await fireEvent.input(screen.getByLabelText('Token'), { target: { value: 'Bearer x' } });
		await fireEvent.click(button(/Save and test/));
		await settled();
		expect(save.mock.calls[0]![1]).toMatchObject({
			authHeader: 'Authorization',
			list: 'tickets',
			fields: { tier: 'tier', order: 'order' }
		});
		expect(JSON.stringify(save.mock.calls[0])).not.toContain('Bearer');
		expect(saveToken).toHaveBeenCalledWith('ticket-api', 'Bearer x');
		expect(pull).toHaveBeenCalledWith('ticket-api', ['tier', 'order'], ['order']);
		expect(screen.getByTestId('data-source-result').textContent).toContain('Regular · 1');
	});

	it('takes an optional field left blank, but not a required one', async () => {
		const views = { 'ticket-api': { source, hasToken: true } };
		const save = vi.spyOn(api, 'setDataSource').mockResolvedValue(views);
		vi.spyOn(api, 'pullDataSource').mockResolvedValue(pulled([{ tier: 'Regular' }]));
		render(DataSourceForm, { id: 'ticket-api', spec });
		await fireEvent.click(button(/Set up/));
		expect(screen.getByText(/Optional. Leave it empty/)).toBeTruthy();

		await fireEvent.input(screen.getByLabelText('Tier'), { target: { value: ' ' } });
		await fireEvent.click(button(/Save and test/));
		await settled();
		expect(save).not.toHaveBeenCalled();
		expect(screen.getByText("enter the provider's field name")).toBeTruthy();

		await fireEvent.input(screen.getByLabelText('Tier'), { target: { value: 'tier' } });
		await fireEvent.input(screen.getByLabelText('Order'), { target: { value: '' } });
		await fireEvent.click(button(/Save and test/));
		await settled();
		expect(save).toHaveBeenCalled();
		expect(screen.getByTestId('data-source-result').textContent).toContain('Regular');
	});

	it('starts on its fields when the panel that opens it means to configure it', async () => {
		render(DataSourceForm, { id: 'ticket-api', spec, open: true });
		await settled();
		expect(screen.getByLabelText('URL')).toBeTruthy();
		expect(button(/Save and test/)).toBeTruthy();
	});

	it('says when there are changes the panel would lose on closing', async () => {
		const ondirty = vi.fn();
		render(DataSourceForm, { id: 'ticket-api', spec, open: true, ondirty });
		await settled();
		expect(ondirty).toHaveBeenLastCalledWith(false);

		await fireEvent.input(screen.getByLabelText('URL'), { target: { value: 'https://y' } });
		expect(ondirty).toHaveBeenLastCalledWith(true);
	});

	it('shows a saved token only as saved', async () => {
		dataSources.set({ 'ticket-api': { source, hasToken: true } });
		render(DataSourceForm, { id: 'ticket-api', spec });
		await fireEvent.click(button(/Edit/));
		expect(screen.queryByLabelText('Token')).toBeNull();
		expect(button(/Replace/)).toBeTruthy();
	});

	it('shows what the source said when the test fails', async () => {
		const views = { 'ticket-api': { source, hasToken: false } };
		vi.spyOn(api, 'setDataSource').mockResolvedValue(views);
		vi.spyOn(api, 'pullDataSource').mockRejectedValue(
			new ApiError({
				code: 'data_source_status',
				message: 'Shop answered 401',
				field: null,
				report: null
			})
		);
		render(DataSourceForm, { id: 'ticket-api', spec });
		await fireEvent.click(button(/Set up/));
		await fireEvent.click(button(/Save and test/));
		await settled();
		expect(screen.getByRole('alert').textContent).toContain('401');
	});
});

describe('The ticket API plugin', () => {
	function setup(pull: PluginContext['pull'], ticketOrderIds: boolean | null = null) {
		const ctx = { dataSource: () => 'Shop', pull, ticketOrderIds } as unknown as PluginContext;
		const proposeImport = vi.fn();
		render(TicketApiSection, { ctx, proposeImport, setPanel: vi.fn() });
		return proposeImport;
	}

	it('hands what it pulled to the review, named after the source', async () => {
		const proposeImport = setup(async () => pulled([{ tier: 'Regular', order: '1' }]));
		await fireEvent.click(button(/Pull tickets/));
		await settled();
		expect(proposeImport).toHaveBeenCalledWith({
			source: 'Shop',
			sha256: 'ab',
			tickets: [{ tier: 'Regular', order: '1' }]
		});
	});

	it('lists what it could not read before going on', async () => {
		const proposeImport = setup(async () =>
			pulled([{ tier: 'Regular', order: '1' }, { tier: 'Regular' }], {
				problems: [{ row: 3, field: 'tier' }],
				missing: [{ row: 2, field: 'order' }]
			})
		);
		await fireEvent.click(button(/Pull tickets/));
		await settled();
		expect(proposeImport).not.toHaveBeenCalled();
		const listed = screen.getByTestId('plugin-ticket-api-problems').textContent;
		expect(listed).toContain('Item 2: no order');
		expect(listed).toContain('Item 3: no tier');
		await fireEvent.click(button(/Review the rest/));
		expect(proposeImport).toHaveBeenCalledWith({
			source: 'Shop',
			sha256: 'ab',
			tickets: [{ tier: 'Regular', order: '1' }]
		});
	});

	it('has tickets without order IDs from a source without an Order field', async () => {
		const proposeImport = setup(async () =>
			pulled([{ tier: 'Regular' }], { fields: ['tier'], missing: [{ row: 1, field: 'order' }] })
		);
		await fireEvent.click(button(/Pull tickets/));
		await settled();
		expect(proposeImport).toHaveBeenCalledWith({
			source: 'Shop',
			sha256: 'ab',
			tickets: [{ tier: 'Regular', order: null }]
		});
	});

	it('reads tickets the way the event’s imports were made', async () => {
		// Imports without order IDs: a source's orders are left out.
		const without = setup(async () => pulled([{ tier: 'Regular', order: '1' }]), false);
		await fireEvent.click(button(/Pull tickets/));
		await settled();
		expect(without.mock.calls[0]![0].tickets).toEqual([{ tier: 'Regular', order: null }]);
	});

	it('says what to do when the event needs order IDs and the source has none', async () => {
		const proposeImport = setup(
			async () => pulled([{ tier: 'Regular' }], { fields: ['tier'] }),
			true
		);
		await fireEvent.click(button(/Pull tickets/));
		await settled();
		expect(proposeImport).not.toHaveBeenCalled();
		expect(screen.getByText(/has no Order field/)).toBeTruthy();
	});
});
