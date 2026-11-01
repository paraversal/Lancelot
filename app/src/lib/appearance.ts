// Light or dark. `theme.css` does the colours and follows the system by itself; a choice in
// Settings overrides it through `data-theme` on the root element.
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { Appearance } from './api/generated/Appearance';

export function applyAppearance(appearance: Appearance) {
	const root = document.documentElement;
	if (appearance === 'system') delete root.dataset.theme;
	else root.dataset.theme = appearance;
	if (isTauri()) void getCurrentWindow().setTheme(appearance === 'system' ? null : appearance);
}
