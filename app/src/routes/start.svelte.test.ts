import { render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError } from '#lib/api/index.ts';
import type { OpenEvent } from '#lib/api/generated/OpenEvent.ts';
import { session } from '#lib/state/session.svelte.ts';
import Start from './+page.svelte';

const { goto, openEvent, recentEvents } = vi.hoisted(() => ({
	goto: vi.fn(),
	openEvent: vi.fn(),
	recentEvents: vi.fn()
}));
vi.mock('$app/navigation', () => ({ goto }));
vi.mock('$app/state', () => ({
	get page() {
		return address;
	}
}));
vi.mock('#lib/api/index.ts', async () => {
	const actual = await vi.importActual<typeof import('#lib/api/index.ts')>('#lib/api/index.ts');
	return {
		...actual,
		api: { currentEvent: async () => null, recentEvents, openEvent, forgetRecent: vi.fn() }
	};
});

let address = $state({ url: new URL('http://app/') });
const ask = (path: string) => goto(`/?open=${encodeURIComponent(path)}`);
const failure = (code: string, message: string) =>
	new ApiError({ code, message, field: null, report: null });

describe('Opening a file from the start screen', () => {
	beforeEach(() => {
		address = { url: new URL('http://app/') };
		goto.mockReset().mockImplementation(async (to: string) => {
			address = { url: new URL(to, 'http://app/') };
		});
		openEvent.mockReset();
		recentEvents.mockReset().mockResolvedValue([]);
		session.close();
	});

	it('opens the chosen file while the screen is already showing', async () => {
		openEvent.mockResolvedValue({ path: '/e.lancelot', revision: 1 } as OpenEvent);
		render(Start);
		await waitFor(() => expect(recentEvents).toHaveBeenCalled());

		await ask('/Desktop/sample.lancelot');

		await waitFor(() => expect(openEvent).toHaveBeenCalledWith('/Desktop/sample.lancelot'));
		await waitFor(() => expect(goto).toHaveBeenCalledWith('/event/overview'));
	});

	it('says why a file that is not in the list failed, and lets it be chosen again', async () => {
		openEvent.mockRejectedValue(failure('not_an_event_file', 'this is not an event file'));
		render(Start);

		await ask('/Desktop/notes.lancelot');

		const shown = await screen.findByRole('alert');
		expect(shown.textContent).toContain('notes.lancelot: this is not an event file');

		await ask('/Desktop/notes.lancelot');

		await waitFor(() => expect(openEvent).toHaveBeenCalledTimes(2));
	});
});
