// Keyboard model (R-NF-7). In the app, New, Open, Close and Settings are native menu items with
// their shortcuts (app/src-tauri/src/lib.rs) and arrive as `menu` events; in a plain browser
// during development the same keys are handled here. ⌘1–9 go through the sidebar's pages and cost
// centres; Esc closes panels (components/Panel.svelte).
import { goto } from '$app/navigation';
import { isTauri } from '@tauri-apps/api/core';
import { chooseAndOpen, closeFile, newEvent } from './actions';
import { allLinks } from './nav';
import { session } from './state/session.svelte';

function run(id: string) {
	switch (id) {
		case 'new_event':
			return newEvent();
		case 'open_event':
			return chooseAndOpen();
		case 'close_file':
			return closeFile();
		case 'settings':
			return session.event ? goto('/event/settings') : undefined;
	}
}

function keydown(e: KeyboardEvent) {
	const mod = e.metaKey || e.ctrlKey;
	if (!mod) return;
	const page = Number(e.key);
	const link = session.event && page >= 1 ? allLinks(session.event.snapshot)[page - 1] : undefined;
	if (link) {
		e.preventDefault();
		goto(link);
		return;
	}
	if (isTauri()) return;
	const id =
		e.shiftKey && e.key.toLowerCase() === 'n'
			? 'new_event'
			: e.key === 'o'
				? 'open_event'
				: e.key === 'w'
					? 'close_file'
					: e.key === ','
						? 'settings'
						: null;
	if (id) {
		e.preventDefault();
		run(id);
	}
}

/** Installs the shortcuts; returns a function that removes them. */
export function installShortcuts(): () => void {
	window.addEventListener('keydown', keydown);
	let unlisten: (() => void) | undefined;
	if (isTauri()) {
		import('@tauri-apps/api/event').then(({ listen }) =>
			listen<string>('menu', (e) => run(e.payload)).then((u) => (unlisten = u))
		);
	}
	return () => {
		window.removeEventListener('keydown', keydown);
		unlisten?.();
	};
}
