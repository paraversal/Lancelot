import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { EventSnapshot } from './api/generated/EventSnapshot.ts';
import example from './api/fixtures/example-event.json';
import { closeFile } from './actions';
import { session } from './state/session.svelte';

const { goto, release } = vi.hoisted(() => ({
	goto: vi.fn(),
	release: vi.fn()
}));
vi.mock('$app/navigation', () => ({ goto }));
vi.mock('./api', () => ({ api: { closeFile: release } }));
vi.mock('./api/dialogs', () => ({ chooseEventFile: vi.fn() }));

beforeEach(() => {
	session.close();
	goto.mockReset();
	release.mockReset().mockResolvedValue(undefined);
});

describe('Closing a file', () => {
	it('releases it and returns to the start screen', async () => {
		session.update({
			revision: 1,
			path: '/summer-festival-2026.lancelot',
			fileName: 'summer-festival-2026.lancelot',
			snapshot: structuredClone(example) as EventSnapshot,
			integrity: { issues: [] },
			backupError: null
		});

		await closeFile();

		expect(release).toHaveBeenCalledOnce();
		expect(session.event).toBeNull();
		expect(goto).toHaveBeenCalledWith('/');
	});
});
