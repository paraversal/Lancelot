// Files dragged onto the window (R-ATT-5). In the app, Tauri reports the paths; in a plain
// browser during development, the dev bridge keeps the files and gives their paths.
import { isTauri } from '@tauri-apps/api/core';

export interface DropHandlers {
	/** A file is being dragged over the window; the names, when known. */
	over: (names: string[]) => void;
	leave: () => void;
	drop: (paths: string[]) => void;
}

const name = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** Starts listening; returns a function that stops. */
export function onFileDrop(handlers: DropHandlers): () => void {
	if (isTauri()) {
		let stop: (() => void) | undefined;
		let stopped = false;
		import('@tauri-apps/api/webview').then(({ getCurrentWebview }) =>
			getCurrentWebview()
				.onDragDropEvent((e) => {
					const p = e.payload;
					if (p.type === 'enter') handlers.over(p.paths.map(name));
					else if (p.type === 'leave') handlers.leave();
					else if (p.type === 'drop') handlers.drop(p.paths);
				})
				.then((u) => (stopped ? u() : (stop = u)))
		);
		return () => {
			stopped = true;
			stop?.();
		};
	}
	const files = (e: DragEvent) => e.dataTransfer?.types.includes('Files');
	const over = (e: DragEvent) => {
		if (!files(e)) return;
		e.preventDefault();
		handlers.over([...(e.dataTransfer?.items ?? [])].map(() => ''));
	};
	const leave = (e: DragEvent) => {
		if (e.relatedTarget === null) handlers.leave();
	};
	const drop = (e: DragEvent) => {
		if (!files(e)) return;
		e.preventDefault();
		const dropped = [...(e.dataTransfer?.files ?? [])];
		import('./api/dev').then(({ upload }) => Promise.all(dropped.map(upload))).then(handlers.drop);
	};
	window.addEventListener('dragover', over);
	window.addEventListener('dragleave', leave);
	window.addEventListener('drop', drop);
	return () => {
		window.removeEventListener('dragover', over);
		window.removeEventListener('dragleave', leave);
		window.removeEventListener('drop', drop);
	};
}
