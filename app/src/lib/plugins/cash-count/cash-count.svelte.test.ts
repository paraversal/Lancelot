import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '../../api/index';
import example from '../../api/fixtures/example-event.json';
import type { Currency } from '../../api/generated/Currency';
import type { EventSnapshot } from '../../api/generated/EventSnapshot';
import { session } from '../../state/session.svelte';
import CountPanel from '../../../routes/event/overview/CountPanel.svelte';
import { pluginContext } from '../host';
import {
	DENOMINATIONS,
	customOf,
	dropCustomPieces,
	newCustomId,
	piecesOf,
	setCustom,
	setPieces,
	totalOf
} from './records';
import Settings from './Settings.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

const snapshot = example as unknown as EventSnapshot;
const cash = () => snapshot.accounts.find((a) => a.assetType === 'cash')!.id;
const bank = () => snapshot.accounts.find((a) => a.assetType === 'bank')!.id;

function open(change: (s: EventSnapshot) => void = () => {}) {
	const copy = structuredClone(snapshot);
	change(copy);
	session.update({
		revision: Date.now(),
		path: '/x.lancelot',
		fileName: 'x.lancelot',
		snapshot: copy,
		integrity: { issues: [] },
		backupError: null
	});
}

const record = (key: string, value: unknown, scope: 'records' | 'setup' = 'records') => ({
	id: 900,
	plugin: 'cash-count',
	key,
	scope,
	value: value as never
});

const frames = () => screen.queryAllByTestId('plugin-frame');
const amounts = () => screen.getAllByLabelText<HTMLInputElement>('Amount');

async function openClosing() {
	const frame = frames()[1];
	await fireEvent.click(within(frame).getByRole('button', { name: 'Count' }));
	return frame;
}

const type = (frame: HTMLElement, label: string, value: string) =>
	fireEvent.input(within(frame).getByLabelText(`Pieces of ${label}`), { target: { value } });

beforeEach(() => {
	vi.restoreAllMocks();
	vi.spyOn(api, 'setCounts').mockImplementation(async () => session.event!);
	vi.spyOn(api, 'applyPluginRecords').mockImplementation(async () => session.event!);
	vi.spyOn(api, 'parseAmount').mockImplementation(async (input) => ({
		cents: Number(input.replace(/\D/g, '')),
		ambiguous: false
	}));
});

describe('The denominations', () => {
	it('cover every currency the app supports', () => {
		for (const currency of ['EUR', 'USD', 'GBP', 'CHF'] as Currency[]) {
			const { notes, coins } = DENOMINATIONS[currency];
			expect(notes.length).toBeGreaterThan(0);
			expect(coins.length).toBeGreaterThan(0);
			expect([...notes, ...coins].every((value) => Number.isSafeInteger(value) && value > 0)).toBe(
				true
			);
		}
	});
});

describe('The pieces', () => {
	it('add up in cents, and go when there are none', () => {
		expect(totalOf({ 5000: 3, 50: 7, 1: 2 })).toBe(15352);
		expect(setPieces(4, 'closing', { 5000: 0 })).toMatchObject({
			key: 'pieces:4:closing',
			value: null
		});
		expect(setPieces(4, 'closing', { 5000: 1 }).scope).toBe('records');
	});

	it('ignore a record that does not fit', () => {
		const read = (value: unknown) => piecesOf([record('pieces:4:closing', value)], 4, 'closing');
		expect(read({ 5000: 2 })).toEqual({ 5000: 2 });
		expect(read({ 'custom:rolls': 3 })).toEqual({ 'custom:rolls': 3 });
		expect(read({ 5000: -1 })).toEqual({});
		expect(read({ x: 1 })).toEqual({});
		expect(read({ 'custom:Bad ID': 1 })).toEqual({});
		expect(read([1])).toEqual({});
	});
});

describe('The custom denominations', () => {
	const rolls = { id: 'rolls', name: 'Geldrollen', value: 2500 };

	it('have a name and what one is worth, and a template copies them', () => {
		expect(customOf([record('custom', [rolls], 'setup')])).toEqual([rolls]);
		expect(customOf([record('custom', [{ id: 'rolls', name: ' ', value: 2500 }])])).toEqual([]);
		expect(customOf([record('custom', [{ id: 'rolls', name: 'Geldrollen', value: -1 }])])).toEqual(
			[]
		);
		expect(setCustom([rolls])).toEqual({ key: 'custom', scope: 'setup', value: [rolls] });
		expect(setCustom([]).value).toBeNull();
	});

	it('count apart from the notes and coins', () => {
		expect(totalOf({ 'custom:rolls': 2, 2000: 1 }, [rolls])).toBe(7000);
		expect(totalOf({ 'custom:gone': 2 }, [rolls])).toBe(0);
		expect(
			piecesOf([record('pieces:4:closing', { 'custom:rolls': 2, 2000: 1 })], 4, 'closing')
		).toEqual({ 'custom:rolls': 2, 2000: 1 });
	});

	it('leave no pieces behind when one is removed', () => {
		expect(
			dropCustomPieces(
				[
					record('pieces:4:closing', { 2000: 1, 'custom:rolls': 2 }),
					record('pieces:4:opening', { 'custom:rolls': 2 })
				],
				'rolls'
			)
		).toEqual([
			{ key: 'pieces:4:closing', scope: 'records', value: { 2000: 1 } },
			{ key: 'pieces:4:opening', scope: 'records', value: null }
		]);
	});

	it('get an id no other custom denomination has', () => {
		const taken = { ...rolls };
		const id = newCustomId([taken]);
		expect(id).toMatch(/^[a-z0-9-]+$/);
		expect(id).not.toBe('rolls');
	});
});

describe('The Count panel', () => {
	it('offers the pieces under both counts of a cash account only', () => {
		open();
		const shown = render(CountPanel, { account: cash(), onclose() {} });
		expect(frames().length).toBe(2);
		shown.unmount();

		render(CountPanel, { account: bank(), onclose() {} });
		expect(frames().length).toBe(0);
	});

	it('is gone when switched off', () => {
		open((s) => s.pluginRecords.push({ ...record('host:off', true), scope: 'setup' }));
		render(CountPanel, { account: cash(), onclose() {} });
		expect(frames().length).toBe(0);
	});

	it('shows the notes and coins apart, with their icons', async () => {
		open();
		render(CountPanel, { account: cash(), onclose() {} });
		const frame = await openClosing();
		const notes = within(frame).getByTestId('cash-notes');
		const coins = within(frame).getByTestId('cash-coins');

		expect(within(notes).getByText('500,00 €')).toBeTruthy();
		expect(within(notes).getByText('5,00 €')).toBeTruthy();
		expect(within(coins).getByText('2,00 €')).toBeTruthy();
		expect(within(coins).getByText('0,01 €')).toBeTruthy();
		expect(notes.querySelector('svg')).toBeTruthy();
		expect(coins.querySelector('svg')).toBeTruthy();
	});

	it('follows the currency of the event', async () => {
		open((s) => {
			s.event.currency = 'USD';
		});
		render(CountPanel, { account: cash(), onclose() {} });
		const frame = await openClosing();
		const notes = within(frame).getByTestId('cash-notes');
		const coins = within(frame).getByTestId('cash-coins');

		expect(within(notes).getByText('100,00 $')).toBeTruthy();
		expect(within(coins).getByText('0,25 $')).toBeTruthy();
		expect(within(frame).queryByText('2,00 €')).toBeNull();

		await type(frame, '0,25 $', '4');
		expect(amounts()[1].value).toBe('1,00');
	});

	it('counts a custom denomination apart from the currency’s own', async () => {
		open((s) =>
			s.pluginRecords.push(
				record('custom', [{ id: 'rolls', name: 'Geldrollen', value: 2500 }], 'setup')
			)
		);
		render(CountPanel, { account: cash(), onclose() {} });
		const frame = await openClosing();
		const custom = within(frame).getByTestId('cash-custom');

		expect(within(custom).getByText('Geldrollen')).toBeTruthy();
		expect(within(custom).getByText('25,00 €')).toBeTruthy();

		await type(frame, 'Geldrollen (25,00 €)', '3');
		expect(amounts()[1].value).toBe('75,00');
		await fireEvent.click(screen.getByRole('button', { name: /^Save/ }));

		await waitFor(() => expect(api.applyPluginRecords).toHaveBeenCalled());
		expect(api.applyPluginRecords).toHaveBeenCalledWith('cash-count', [
			{ key: `pieces:${cash()}:closing`, scope: 'records', value: { 'custom:rolls': 3 } }
		]);
	});

	it('fills in the amount as pieces are typed, and saves nothing until Save', async () => {
		open();
		render(CountPanel, { account: cash(), onclose() {} });
		const frame = await openClosing();
		await type(frame, '50,00 €', '8');
		await type(frame, '0,50 €', '3');

		expect(amounts()[1].value).toBe('401,50');
		expect(screen.getByTestId('plugin-cash-total').textContent).toContain('401,50');
		expect(api.applyPluginRecords).not.toHaveBeenCalled();
		expect(api.setCounts).not.toHaveBeenCalled();
	});

	it('keeps the pieces after the count is saved, not before', async () => {
		open();
		const onclose = vi.fn();
		render(CountPanel, { account: cash(), onclose });
		const frame = await openClosing();
		await type(frame, '20,00 €', '2');
		await fireEvent.click(screen.getByRole('button', { name: /^Save/ }));

		await waitFor(() => expect(onclose).toHaveBeenCalled());
		const [counted] = vi.mocked(api.setCounts).mock.invocationCallOrder;
		const [kept] = vi.mocked(api.applyPluginRecords).mock.invocationCallOrder;
		expect(counted).toBeLessThan(kept);
		expect(vi.mocked(api.setCounts).mock.calls[0][2]).toMatchObject({ amount: 4000 });
		expect(api.applyPluginRecords).toHaveBeenCalledWith('cash-count', [
			{ key: `pieces:${cash()}:closing`, scope: 'records', value: { 2000: 2 } }
		]);
	});

	it('keeps nothing on Cancel, and writes nothing when the pieces did not change', async () => {
		open();
		const first = render(CountPanel, { account: cash(), onclose() {} });
		await type(await openClosing(), '20,00 €', '2');
		await fireEvent.click(screen.getByRole('button', { name: /^Cancel/ }));
		expect(api.applyPluginRecords).not.toHaveBeenCalled();
		first.unmount();

		const onclose = vi.fn();
		render(CountPanel, { account: cash(), onclose });
		await fireEvent.click(screen.getByRole('button', { name: /^Save/ }));
		await waitFor(() => expect(onclose).toHaveBeenCalled());
		expect(api.applyPluginRecords).not.toHaveBeenCalled();
	});

	it('says when the amount no longer matches the pieces, and can take them back', async () => {
		open((s) => s.pluginRecords.push(record(`pieces:${cash()}:closing`, { 5000: 1 })));
		render(CountPanel, { account: cash(), onclose() {} });
		expect(screen.getByText('Use pieces')).toBeTruthy();
		await fireEvent.click(screen.getByText('Use pieces'));
		expect(amounts()[1].value).toBe('50,00');
		// The row closes rather than vanishing (A7), so it takes a moment to leave the page.
		await waitFor(() => expect(screen.queryByText('Use pieces')).toBeNull());
	});

	it('clears the amount when the last piece goes', async () => {
		open();
		render(CountPanel, { account: cash(), onclose() {} });
		const frame = await openClosing();
		await type(frame, '20,00 €', '2');
		await type(frame, '20,00 €', '');
		expect(amounts()[1].value).toBe('');
	});
});

describe('The settings', () => {
	it('lists the notes and coins, without a way to leave one out', () => {
		open();
		render(Settings, { ctx: pluginContext('cash-count') });
		const notes = within(screen.getByTestId('cash-settings-notes'));
		const coins = within(screen.getByTestId('cash-settings-coins'));

		expect(notes.getByText('500,00 €')).toBeTruthy();
		expect(notes.getByText('5,00 €')).toBeTruthy();
		expect(coins.getByText('2,00 €')).toBeTruthy();
		expect(coins.getByText('0,01 €')).toBeTruthy();
		expect(screen.queryByRole('checkbox')).toBeNull();
	});

	it('lists them in the event currency', () => {
		open((s) => {
			s.event.currency = 'CHF';
		});
		render(Settings, { ctx: pluginContext('cash-count') });

		expect(screen.getByText('Denominations · CHF')).toBeTruthy();
		expect(within(screen.getByTestId('cash-settings-coins')).getByText('5,00 CHF')).toBeTruthy();
	});

	it('adds a custom denomination, which a template copies', async () => {
		open();
		render(Settings, { ctx: pluginContext('cash-count') });

		await fireEvent.input(screen.getByLabelText('Name'), {
			target: { value: 'Geldrollen' }
		});
		await fireEvent.input(screen.getByLabelText('Value each'), {
			target: { value: '25,00' }
		});
		await fireEvent.click(screen.getByRole('button', { name: 'Add' }));

		await waitFor(() => expect(api.applyPluginRecords).toHaveBeenCalled());
		expect(api.applyPluginRecords).toHaveBeenCalledWith('cash-count', [
			{
				key: 'custom',
				scope: 'setup',
				value: [{ id: expect.stringMatching(/^[a-z0-9-]+$/), name: 'Geldrollen', value: 2500 }]
			}
		]);
	});

	it('needs a name and what one is worth', async () => {
		open();
		render(Settings, { ctx: pluginContext('cash-count') });

		await fireEvent.click(screen.getByRole('button', { name: 'Add' }));
		await waitFor(() => expect(screen.getByRole('alert').textContent).toMatch(/name/i));
		expect(api.applyPluginRecords).not.toHaveBeenCalled();
	});

	it('removes a custom denomination with the pieces counted against it', async () => {
		open((s) =>
			s.pluginRecords.push(
				record('custom', [{ id: 'rolls', name: 'Geldrollen', value: 2500 }], 'setup'),
				record(`pieces:${cash()}:closing`, { 2000: 2, 'custom:rolls': 1 })
			)
		);
		render(Settings, { ctx: pluginContext('cash-count') });

		await fireEvent.click(screen.getByRole('button', { name: 'Remove Geldrollen' }));

		await waitFor(() => expect(api.applyPluginRecords).toHaveBeenCalled());
		expect(api.applyPluginRecords).toHaveBeenCalledWith('cash-count', [
			{ key: 'custom', scope: 'setup', value: null },
			{ key: `pieces:${cash()}:closing`, scope: 'records', value: { 2000: 2 } }
		]);
	});
});
