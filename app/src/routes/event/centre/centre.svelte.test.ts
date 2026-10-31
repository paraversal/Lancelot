import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import example from '#lib/api/fixtures/example-event.json';
import { session } from '#lib/state/session.svelte.ts';
import Centre from './Centre.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

function open(change: (s: EventSnapshot) => void = () => {}) {
	const snapshot = structuredClone(example) as unknown as EventSnapshot;
	change(snapshot);
	session.update({
		revision: 1,
		path: '/x.lancelot',
		fileName: 'x.lancelot',
		snapshot,
		integrity: { issues: [] },
		backupError: null
	});
}

describe('A cost centre page', () => {
	it('shows the result and the tagged lines without any plugin', () => {
		open((s) => {
			s.pluginRecords = [];
		});
		const snapshot = session.event!.snapshot;
		render(Centre, { centre: snapshot.costCentres.find((c) => c.name === 'Drinks')! });
		expect(screen.getByRole('heading', { name: 'Drinks' })).toBeTruthy();
		expect(screen.getAllByText('Hoffmann drinks')).toHaveLength(2);
		expect(screen.queryByTestId('plugin-drinks-total')).toBeNull();
	});

	it('has the tagged lines on every centre', () => {
		open((s) => {
			s.costCentres.push({ id: 99, name: 'Workshop' });
			s.budget.costCentres.push({
				costCentre: 99,
				categories: [],
				income: { estimate: 0, actual: 0, forecast: 0 },
				expense: { estimate: 0, actual: 1200, forecast: 1200 },
				result: { estimate: 0, actual: -1200, forecast: -1200 }
			});
			s.txns[0].lines[1].costCentre = 99;
			s.txns[0].description = 'Boards';
		});
		render(Centre, { centre: { id: 99, name: 'Workshop' } });
		expect(screen.getByRole('heading', { name: 'Workshop' })).toBeTruthy();
		expect(screen.getByText('Boards')).toBeTruthy();
		expect(screen.queryByTestId('plugin-drinks-total')).toBeNull();
	});
});
