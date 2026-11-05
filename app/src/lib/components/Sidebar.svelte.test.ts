import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import type { OpenEvent } from '#lib/api/generated/OpenEvent.ts';
import example from '#lib/api/fixtures/example-event.json';
import Sidebar from './Sidebar.svelte';

vi.mock('$app/state', () => ({ page: { url: new URL('http://app/event/overview') } }));

const event: OpenEvent = {
	revision: 1,
	path: '/summer-festival-2026.lancelot',
	fileName: 'summer-festival-2026.lancelot',
	snapshot: structuredClone(example) as EventSnapshot,
	integrity: { issues: [] },
	backupError: null
};

describe('The sidebar', () => {
	it('offers Close file at its foot', async () => {
		const onclose = vi.fn();
		render(Sidebar, { event, onclose });

		const close = screen.getByRole('button', { name: 'Close file' });
		expect(close.getAttribute('title')).toContain('⌘W');

		await fireEvent.click(close);

		expect(onclose).toHaveBeenCalledOnce();
	});
});
