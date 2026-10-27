import { redirect } from '@sveltejs/kit';

// The start screen (recent events, New, Open) arrives in M2.
export function load() {
	redirect(307, '/event/overview');
}
