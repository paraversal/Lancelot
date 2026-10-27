// Copies the design tokens generated from UI.fig into the app. Run with `--check` in CI.
import { readFileSync, writeFileSync } from 'node:fs';

const source = new URL('../../docs/design/tokens.css', import.meta.url);
const target = new URL('../src/lib/styles/tokens.css', import.meta.url);
const tokens = readFileSync(source, 'utf8');

if (process.argv.includes('--check')) {
	if (readFileSync(target, 'utf8') !== tokens) {
		console.error('src/lib/styles/tokens.css is out of date. Run `pnpm tokens`.');
		process.exit(1);
	}
} else {
	writeFileSync(target, tokens);
}
