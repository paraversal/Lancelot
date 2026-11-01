import { render, screen, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import { api } from '../api/index';
import type { EventSnapshot } from '../api/generated/EventSnapshot';
import example from '../api/fixtures/example-event.json';
import { session } from '../state/session.svelte';
import Centre from '../../routes/event/centre/Centre.svelte';
import { pluginContext, pluginEnabled, setPluginEnabled } from './host';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

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

const switchedOff = (s: EventSnapshot) => {
	s.pluginRecords.push({
		id: 999,
		plugin: 'drinks',
		key: 'host:off',
		scope: 'setup',
		value: true
	});
};

const drinks = () => session.event!.snapshot.costCentres.find((c) => c.name === 'Drinks')!;

describe('A plugin on a cost centre page', () => {
	it('shows inside a frame that names it as a plugin', () => {
		open();
		render(Centre, { centre: drinks() });
		const frame = within(screen.getByTestId('plugin-frame'));
		expect(frame.getByText('Stock and sales')).toBeTruthy();
		expect(frame.getByText('Plugin')).toBeTruthy();
		expect(frame.getByText('Club-Mate 0,5')).toBeTruthy();
		// The core's own part stays outside.
		expect(frame.queryByText('Tagged lines')).toBeNull();
		expect(screen.getByText('Tagged lines')).toBeTruthy();
	});

	it('is gone when switched off, and the core’s figures stay', () => {
		open(switchedOff);
		render(Centre, { centre: drinks() });
		expect(screen.queryByTestId('plugin-frame')).toBeNull();
		expect(screen.queryByText('Club-Mate 0,5')).toBeNull();
		expect(screen.getByText('Tagged lines')).toBeTruthy();
		expect(screen.getByText(/Drink donations 812,50/)).toBeTruthy();
	});

	it('has no frame on a cost centre it is not attached to', () => {
		open((s) => {
			s.costCentres.push({ id: 99, name: 'Workshop' });
		});
		render(Centre, { centre: { id: 99, name: 'Workshop' } });
		expect(screen.queryByTestId('plugin-frame')).toBeNull();
	});
});

describe('The plugin switch', () => {
	it('is on unless switched off, and hides itself from the plugin', () => {
		open();
		expect(pluginEnabled('drinks')).toBe(true);
		open(switchedOff);
		expect(pluginEnabled('drinks')).toBe(false);
		expect(pluginContext('drinks').records.some((r) => r.key === 'host:off')).toBe(false);
	});

	it('stores the choice as the app’s own record', async () => {
		open();
		const apply = vi.spyOn(api, 'applyPluginRecords').mockResolvedValue(session.event!);
		await setPluginEnabled('drinks', false);
		expect(apply).toHaveBeenCalledWith('drinks', [
			{ key: 'host:off', scope: 'setup', value: true }
		]);
		await setPluginEnabled('drinks', true);
		expect(apply).toHaveBeenLastCalledWith('drinks', [
			{ key: 'host:off', scope: 'setup', value: null }
		]);
	});
});
