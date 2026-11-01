import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { api, ApiError } from '../api/index';
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
		const pull = vi.spyOn(api, 'pullDataSource').mockResolvedValue({
			label: 'Shop',
			sha256: 'ab',
			rows: [{ tier: 'Regular', order: '1' }],
			problems: []
		});
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
		expect(pull).toHaveBeenCalledWith('ticket-api', ['tier', 'order']);
		expect(screen.getByTestId('data-source-result').textContent).toContain('Regular · 1');
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
	function setup(pull: PluginContext['pull']) {
		const ctx = { dataSource: 'Shop', pull } as unknown as PluginContext;
		const proposeImport = vi.fn();
		render(TicketApiSection, { ctx, proposeImport, setPanel: vi.fn() });
		return proposeImport;
	}

	it('hands what it pulled to the review, named after the source', async () => {
		const proposeImport = setup(async () => ({
			label: 'Shop',
			sha256: 'ab',
			rows: [{ tier: 'Regular', order: '1' }],
			problems: []
		}));
		await fireEvent.click(button(/Pull tickets/));
		await settled();
		expect(proposeImport).toHaveBeenCalledWith({
			source: 'Shop',
			sha256: 'ab',
			tickets: [{ tier: 'Regular', order: '1' }]
		});
	});

	it('lists what it could not read before going on', async () => {
		const proposeImport = setup(async () => ({
			label: 'Shop',
			sha256: 'ab',
			rows: [],
			problems: [{ row: 2, field: 'order' }]
		}));
		await fireEvent.click(button(/Pull tickets/));
		await settled();
		expect(proposeImport).not.toHaveBeenCalled();
		expect(screen.getByTestId('plugin-ticket-api-problems').textContent).toContain(
			'Item 2: no order'
		);
		await fireEvent.click(button(/Review the rest/));
		expect(proposeImport).toHaveBeenCalled();
	});
});
