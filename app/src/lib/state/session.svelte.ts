// The open event and the app settings, shared by all screens. Every command returns a fresh
// OpenEvent, which replaces the previous one wholesale; screens only derive from it.
import type { AppSettings } from '../api/generated/AppSettings';
import type { OpenEvent } from '../api/generated/OpenEvent';

class Session {
	event = $state<OpenEvent | null>(null);
	settings = $state<AppSettings | null>(null);

	/** Takes a new state from a command, ignoring one that arrives after a newer one. */
	update(next: OpenEvent) {
		const current = this.event;
		if (current && current.path === next.path && next.revision < current.revision) return;
		this.event = next;
	}

	close() {
		this.event = null;
	}
}

export const session = new Session();
