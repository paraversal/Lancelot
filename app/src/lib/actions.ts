// What the menu, the keyboard and the buttons do: open, create and close event files.
import { goto } from '$app/navigation';
import { api } from './api';
import { chooseEventFile } from './api/dialogs';
import { session } from './state/session.svelte';

/** Opens an event file and shows its overview. Errors are for the caller to show. */
export async function openEvent(path: string) {
	session.update(await api.openEvent(path));
	await goto('/event/overview');
}

/** Asks for a file and opens it. Returns the error, if any, for the start screen to show. */
export async function chooseAndOpen() {
	const path = await chooseEventFile();
	if (!path) return;
	await closeFile(false);
	await goto(`/?open=${encodeURIComponent(path)}`);
}

/** Closes the file (releasing its lock) and returns to the start screen. */
export async function closeFile(navigate = true) {
	if (session.event) {
		await api.closeFile();
		session.close();
	}
	if (navigate) await goto('/');
}

/** Goes to the start screen with the new-event panel open. */
export async function newEvent() {
	await closeFile(false);
	await goto('/?new');
}
