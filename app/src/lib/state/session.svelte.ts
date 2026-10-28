// The open event and the app settings, shared by all screens. Every command returns a fresh
// OpenEvent, which replaces the previous one wholesale; screens only derive from it.
import type { AppSettings } from '../api/generated/AppSettings';
import type { OpenEvent } from '../api/generated/OpenEvent';

import type { NumberFormat } from '../api/generated/NumberFormat';

class Session {
	event = $state<OpenEvent | null>(null);
	settings = $state<AppSettings | null>(null);
	/** When the integrity report in `event` was made: on opening or on "Check again". */
	checkedAt = $state<Date | null>(null);

	/** A closed event is read-only until it is reopened (R-EVT-2). */
	get closed(): boolean {
		return !!this.event?.snapshot.event.closedAt;
	}

	get format(): NumberFormat {
		return this.settings?.numberFormat ?? 'de';
	}

	/** Takes a new state from a command, ignoring one that arrives after a newer one. */
	update(next: OpenEvent, checked = false) {
		const current = this.event;
		if (current && current.path === next.path && next.revision < current.revision) return;
		if (checked || current?.path !== next.path) this.checkedAt = new Date();
		this.event = next;
	}

	/**
	 * Runs event commands one after another. Commands that send the whole event details must
	 * build them from the latest state, so `command` is called only when the previous one is
	 * done; two quick edits in different fields can then never overwrite each other.
	 */
	change(command: () => Promise<OpenEvent>, checked = false): Promise<void> {
		const next = this.#queue.then(async () => this.update(await command(), checked));
		this.#queue = next.catch(() => {});
		return next;
	}

	/** Like `change`, for commands that return more than the new state (e.g. a new id). */
	run<T extends { event: OpenEvent }>(command: () => Promise<T>): Promise<T> {
		const next = this.#queue.then(async () => {
			const result = await command();
			this.update(result.event);
			return result;
		});
		this.#queue = next.then(
			() => {},
			() => {}
		);
		return next;
	}

	#queue: Promise<void> = Promise.resolve();

	close() {
		this.event = null;
		this.checkedAt = null;
	}
}

export const session = new Session();
