import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { PluginContext, PluginRecord, Table } from '../api';
import { columnsError, columnsOf, columnsRead, setColumns, sheetOf } from './records';
import Section from './Section.svelte';
import SheetPanel from './SheetPanel.svelte';

const batch = { source: 'a.ods', sha256: 'ab', tickets: [{ tier: 'Regular', order: '1' }] };
const table = (rows: string[][], errors: number[] = []): Table => ({
	source: 'a.ods',
	sha256: 'ab',
	rows: rows.map((cells, i) => ({ row: i + 2, cells })),
	errors
});

function setup(
	read: () => Promise<Table>,
	records: PluginRecord[] = [],
	ticketOrderIds: boolean | null = null
) {
	const readTable = vi.fn(read);
	const ctx = {
		records,
		ticketOrderIds,
		closed: false,
		chooseFile: vi.fn(async () => '/x/a.ods'),
		onFileDrop: () => () => {},
		readTable
	} as unknown as PluginContext;
	const proposeImport = vi.fn();
	const setPanel = vi.fn();
	render(Section, { ctx, proposeImport, setPanel });
	return { readTable, proposeImport, setPanel };
}

const record = (value: unknown): PluginRecord => ({
	id: 1,
	plugin: 'ticket-sheet',
	key: 'columns',
	scope: 'setup',
	value: value as PluginRecord['value']
});

const importSheet = () => fireEvent.click(screen.getByRole('button', { name: /Import sheet/ }));
const settled = () => new Promise((resolve) => setTimeout(resolve));

describe('The ticket sheet plugin', () => {
	it('reads the file with its columns and hands the tickets to the review', async () => {
		const { readTable, proposeImport, setPanel } = setup(async () => table([['Regular', '1']]));
		await importSheet();
		await settled();
		expect(readTable).toHaveBeenCalledWith('/x/a.ods', ['Ticket', 'Order ID']);
		expect(proposeImport).toHaveBeenCalledWith(batch);
		expect(setPanel).not.toHaveBeenCalledWith(expect.any(Function));
	});

	it('reads the tier column alone for a sheet without order IDs', async () => {
		const off = record({ tier: 'Ticket', order: 'Order ID', orderIds: false });
		const { readTable, proposeImport } = setup(async () => table([['Regular']]), [off]);
		await importSheet();
		await settled();
		expect(readTable).toHaveBeenCalledWith('/x/a.ods', ['Ticket']);
		expect(proposeImport).toHaveBeenCalledWith({
			...batch,
			tickets: [{ tier: 'Regular', order: null }]
		});
		expect(screen.getByText(/Reads Ticket · no order IDs/)).toBeTruthy();
	});

	it('reads sheets the way the event’s imports were made', async () => {
		const off = record({ tier: 'Ticket', order: 'Order ID', orderIds: false });
		const { readTable } = setup(async () => table([['Regular', '1']]), [off], true);
		await importSheet();
		await settled();
		expect(readTable).toHaveBeenCalledWith('/x/a.ods', ['Ticket', 'Order ID']);
	});

	it('stops at its panel when rows could not be read', async () => {
		const { proposeImport, setPanel } = setup(async () =>
			table([
				['Regular', '1'],
				['Regular', '']
			])
		);
		await importSheet();
		await settled();
		expect(proposeImport).not.toHaveBeenCalled();
		expect(setPanel).toHaveBeenCalledWith(expect.any(Function));
	});

	it('stops at its panel when the file does not fit the columns', async () => {
		const { proposeImport, setPanel } = setup(async () => {
			throw new Error('there is no column "Ticket"');
		});
		await importSheet();
		await settled();
		expect(proposeImport).not.toHaveBeenCalled();
		expect(setPanel).toHaveBeenCalledWith(expect.any(Function));
	});
});

describe('Its panel', () => {
	function panel(ticketOrderIds: boolean | null) {
		const apply = vi.fn(async () => {});
		const oncolumns = vi.fn();
		const ctx = { records: [], ticketOrderIds, closed: false, apply } as unknown as PluginContext;
		const noop = () => {};
		render(SheetPanel, {
			ctx,
			path: null,
			sheet: null,
			error: null,
			onchoose: noop,
			oncolumns,
			onreview: noop,
			onclose: noop
		});
		return {
			apply,
			oncolumns,
			box: screen.getByRole('checkbox', { name: 'Rows have an order ID' })
		};
	}

	it('switches order IDs off before the first import', async () => {
		const { apply, oncolumns, box } = panel(null);
		expect((box as HTMLInputElement).disabled).toBe(false);
		await fireEvent.click(box);
		await settled();
		expect(apply).toHaveBeenCalledWith([
			setColumns({ tier: 'Ticket', order: 'Order ID', orderIds: false })
		]);
		expect(oncolumns).toHaveBeenCalled();
	});

	it('locks the choice once the event has imports', () => {
		const { box } = panel(false);
		expect((box as HTMLInputElement).disabled).toBe(true);
		expect((box as HTMLInputElement).checked).toBe(false);
		expect(screen.getByText(/Delete the imports to change this/)).toBeTruthy();
		expect(screen.queryByLabelText('order')).toBeNull();
	});
});

describe('A sheet', () => {
	it('is one ticket per row, and lists the rows it cannot use in order', () => {
		const sheet = sheetOf(
			table(
				[
					['Regular', '1'],
					['Regular', ''],
					['', '2']
				],
				[9]
			)
		);
		expect(sheet.batch).toEqual(batch);
		expect(sheet.problems).toEqual([
			{ row: 3, problem: 'noOrder' },
			{ row: 4, problem: 'noTier' },
			{ row: 9, problem: 'cellError' }
		]);
	});

	it('needs no order without order IDs', () => {
		const sheet = sheetOf(table([['Regular'], ['']]), false);
		expect(sheet.batch.tickets).toEqual([{ tier: 'Regular', order: null }]);
		expect(sheet.problems).toEqual([{ row: 3, problem: 'noTier' }]);
	});
});

describe('The columns', () => {
	it('default until set, and ignore a record that does not fit', () => {
		const usual = { tier: 'Ticket', order: 'Order ID', orderIds: true };
		expect(columnsOf([])).toEqual(usual);
		expect(columnsOf([record({ tier: 3 })])).toEqual(usual);
		// Written before sheets could do without order IDs.
		expect(columnsOf([record({ tier: 'Kategorie', order: 'Nr' })])).toEqual({
			tier: 'Kategorie',
			order: 'Nr',
			orderIds: true
		});
		const off = [record({ tier: 'Ticket', order: 'Nr', orderIds: false })];
		expect(columnsOf(off).orderIds).toBe(false);
		expect(columnsRead(columnsOf(off))).toEqual(['Ticket']);
		// The event's imports decide once there are some.
		expect(columnsOf(off, true).orderIds).toBe(true);
		expect(columnsOf([], false).orderIds).toBe(false);
	});

	it('need two different names, and are copied by a template', () => {
		const on = { order: 'Nr', orderIds: true };
		expect(columnsError({ ...on, tier: ' ' })).not.toBeNull();
		expect(columnsError({ ...on, tier: 'Ticket', order: ' ticket ' })).not.toBeNull();
		expect(columnsError({ ...on, tier: 'Ticket' })).toBeNull();
		// Without order IDs, the order column isn't read.
		expect(columnsError({ tier: 'Ticket', order: ' ticket ', orderIds: false })).toBeNull();
		expect(columnsError({ tier: ' ', order: '', orderIds: false })).not.toBeNull();
		expect(setColumns({ ...on, tier: ' Ticket ' })).toEqual({
			key: 'columns',
			scope: 'setup',
			value: { tier: 'Ticket', order: 'Nr', orderIds: true }
		});
	});
});
