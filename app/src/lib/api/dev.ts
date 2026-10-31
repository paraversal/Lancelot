// In a plain browser during development, the commands go to the dev bridge
// (crates/lancelot-bridge), which runs the real core on a real file. Vite proxies /__bridge to
// it (vite.config.ts). It is never part of a Tauri build: see ./index.ts and ./dialogs.ts.
import type { ApiError } from './generated/ApiError';

const BRIDGE = '/__bridge';

function unavailable(): ApiError {
	return {
		code: 'bridge_unavailable',
		message: 'The dev bridge is not running. Start it with: cargo run -p lancelot-bridge',
		field: null,
		report: null
	};
}

async function post<T>(path: string, body: BodyInit, type = 'application/json'): Promise<T> {
	const response = await fetch(`${BRIDGE}/${path}`, {
		method: 'POST',
		headers: { 'Content-Type': type, 'X-Lancelot-Bridge': '1' },
		body
	}).catch(() => {
		throw unavailable();
	});
	// Vite answers for the bridge when it cannot reach it, with no JSON to read.
	const data = await response.json().catch(() => {
		throw unavailable();
	});
	if (!response.ok) throw data;
	return data as T;
}

export const invoke = <T>(command: string, args: Record<string, unknown> = {}) =>
	post<T>(`invoke/${command}`, JSON.stringify(args));

/** Where the browser loads an attachment from, for previews. */
export const attachmentUrl = (id: number) => `${BRIDGE}/attachment/${id}`;

/** Keeps a dropped file in the bridge and returns its path; a browser does not know the path. */
export function upload(file: File): Promise<string> {
	const name = file.name.replace(/[^\w.-]/g, '_');
	return post<string>(`upload?name=${name}`, file, 'application/octet-stream');
}

/** Stand-ins for the native dialogs: they never ask, so flows can be clicked through. */
const dialog = <T>(kind: string, name?: string) =>
	post<T>(`dialog/${kind}`, JSON.stringify({ name }));

export const dialogs = {
	open: () => dialog<string>('open'),
	receipts: () => dialog<string[]>('receipts'),
	/** The ticket sheets in fixtures/tickets, in turn. */
	sheet: () => dialog<string>('sheet'),
	save: (name: string) => dialog<string>('save', name),
	reveal: async () => {}
};
