import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '../../api/index';
import example from '../../api/fixtures/example-event.json';
import type { EventSnapshot } from '../../api/generated/EventSnapshot';
import { session } from '../../state/session.svelte';
import CountPanel from '../../../routes/event/overview/CountPanel.svelte';
import { pluginContext } from '../host';
import { hiddenOf, offered, piecesOf, setHidden, setPieces, totalOf } from './records';
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

const record = (key: string, value: unknown) => ({
	id: 900,
	plugin: 'cash-count',
	key,
	scope: 'records' as const,
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
		expect(read({ 5000: -1 })).toEqual({});
		expect(read({ x: 1 })).toEqual({});
		expect(read([1])).toEqual({});
	});

	it('still offer a hidden denomination that was counted', () => {
		expect(offered([50000, 1], {})).not.toContain(50000);
		expect(offered([50000, 1], { 50000: 1 })).toContain(50000);
		expect(setHidden([1]).scope).toBe('setup');
		expect(hiddenOf([record('hidden', [1, 'x'])])).toEqual([1]);
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
		expect(screen.queryByText('Use pieces')).toBeNull();
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
	it('hide a denomination, for a template to copy', async () => {
		open();
		render(Settings, { ctx: pluginContext('cash-count') });
		await fireEvent.click(screen.getByLabelText('500,00 €'));
		expect(api.applyPluginRecords).toHaveBeenCalledWith('cash-count', [
			{ key: 'hidden', scope: 'setup', value: [50000] }
		]);
	});
});
