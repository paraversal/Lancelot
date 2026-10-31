import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { PluginContext, PluginRecord, Sheet } from '../api';
import { columnsError, columnsOf, setColumns } from './records';
import Section from './Section.svelte';

const batch = { source: 'a.ods', sha256: 'ab', tickets: [{ tier: 'Regular', order: '1' }] };

function setup(read: () => Promise<Sheet>, records: PluginRecord[] = []) {
	const readTicketSheet = vi.fn(read);
	const ctx = {
		records,
		chooseFile: vi.fn(async () => '/x/a.ods'),
		onFileDrop: () => () => {},
		readTicketSheet
	} as unknown as PluginContext;
	const proposeImport = vi.fn();
	const setPanel = vi.fn();
	render(Section, { ctx, proposeImport, setPanel });
	return { readTicketSheet, proposeImport, setPanel };
}

const importSheet = () => fireEvent.click(screen.getByRole('button', { name: /Import sheet/ }));
const settled = () => new Promise((resolve) => setTimeout(resolve));

describe('The ticket sheet plugin', () => {
	it('reads the file with its columns and hands the tickets to the review', async () => {
		const { readTicketSheet, proposeImport, setPanel } = setup(async () => ({
			batch,
			problems: []
		}));
		await importSheet();
		await settled();
		expect(readTicketSheet).toHaveBeenCalledWith('/x/a.ods', { tier: 'Ticket', order: 'Order ID' });
		expect(proposeImport).toHaveBeenCalledWith(batch);
		expect(setPanel).not.toHaveBeenCalledWith(expect.any(Function));
	});

	it('stops at its panel when rows could not be read', async () => {
		const { proposeImport, setPanel } = setup(async () => ({
			batch,
			problems: [{ row: 3, problem: 'noOrder' }]
		}));
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

describe('The columns', () => {
	const record = (value: unknown): PluginRecord => ({
		id: 1,
		plugin: 'ticket-sheet',
		key: 'columns',
		scope: 'setup',
		value: value as PluginRecord['value']
	});

	it('default until set, and ignore a record that does not fit', () => {
		expect(columnsOf([])).toEqual({ tier: 'Ticket', order: 'Order ID' });
		expect(columnsOf([record({ tier: 3 })])).toEqual({ tier: 'Ticket', order: 'Order ID' });
		expect(columnsOf([record({ tier: 'Kategorie', order: 'Nr' })])).toEqual({
			tier: 'Kategorie',
			order: 'Nr'
		});
	});

	it('need two different names, and are copied by a template', () => {
		expect(columnsError({ tier: ' ', order: 'Nr' })).not.toBeNull();
		expect(columnsError({ tier: 'Ticket', order: ' ticket ' })).not.toBeNull();
		expect(columnsError({ tier: 'Ticket', order: 'Nr' })).toBeNull();
		expect(setColumns({ tier: ' Ticket ', order: 'Nr' })).toEqual({
			key: 'columns',
			scope: 'setup',
			value: { tier: 'Ticket', order: 'Nr' }
		});
	});
});
