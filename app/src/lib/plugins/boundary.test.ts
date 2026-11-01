import { describe, expect, it } from 'vitest';

// Rule 4 of technical design §10: a plugin reaches the app only through its context, so that a
// sandboxed loader can run it later without rewriting it.
const sources = import.meta.glob('./*/**/*.{ts,svelte}', {
	query: '?raw',
	import: 'default',
	eager: true
}) as Record<string, string>;

const FORBIDDEN = [
	/state\/session/,
	/api\/index/,
	/api\/dialogs/,
	/\$app\//,
	/plugins\/host/,
	/host['"]/
];

describe('The plugin boundary', () => {
	it('finds the plugins to check', () => {
		expect(Object.keys(sources)).toContain('./drinks/Section.svelte');
	});

	for (const [path, source] of Object.entries(sources)) {
		if (path.includes('.test.')) continue;
		it(`${path} uses only the plugin context`, () => {
			const imports = source.match(/from\s+['"][^'"]+['"]|import\(['"][^'"]+['"]\)/g) ?? [];
			for (const line of imports) {
				for (const pattern of FORBIDDEN) expect(line).not.toMatch(pattern);
			}
		});
	}
});
