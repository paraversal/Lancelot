// Native file dialogs and Finder, with the mock's stand-ins in a plain browser.
import { isTauri } from '@tauri-apps/api/core';

const FILTERS = [{ name: 'Lancelot event', extensions: ['lancelot'] }];

/** Asks for an event file to open. `null` when cancelled. */
export async function chooseEventFile(): Promise<string | null> {
	if (!isTauri()) return (await import('./mock')).dialogs.open();
	const { open } = await import('@tauri-apps/plugin-dialog');
	const path = await open({ multiple: false, directory: false, filters: FILTERS });
	return typeof path === 'string' ? path : null;
}

/** Asks where to save a new event file, suggesting a name. `null` when cancelled. */
export async function chooseNewEventFile(suggestedName: string): Promise<string | null> {
	const name = `${
		suggestedName
			.trim()
			.toLowerCase()
			.replace(/[^\p{L}\p{N}]+/gu, '-')
			.replace(/^-|-$/g, '') || 'event'
	}.lancelot`;
	if (!isTauri()) return (await import('./mock')).dialogs.save(name);
	const { save } = await import('@tauri-apps/plugin-dialog');
	const path = await save({ defaultPath: name, filters: FILTERS });
	if (!path) return null;
	return path.endsWith('.lancelot') ? path : `${path}.lancelot`;
}

/** Shows a file in Finder (or the system's file manager). */
export async function revealFile(path: string): Promise<void> {
	if (!isTauri()) return (await import('./mock')).dialogs.reveal();
	const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
	await revealItemInDir(path);
}
