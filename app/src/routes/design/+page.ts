import { error } from '@sveltejs/kit';
import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';

// The component sheet exists only in development builds. The example event is loaded inside
// the DEV branch, so production builds leave it out entirely.
export async function load() {
	if (import.meta.env.DEV) {
		const example = await import('#lib/api/mock/example-event.json');
		return { snapshot: example.default as EventSnapshot };
	}
	error(404);
}
