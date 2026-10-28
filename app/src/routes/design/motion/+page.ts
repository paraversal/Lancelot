import { error } from '@sveltejs/kit';

// The motion trial page exists only in development builds, like the component sheet.
export function load() {
	if (!import.meta.env.DEV) error(404);
}
