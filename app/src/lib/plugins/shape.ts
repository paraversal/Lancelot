// The shape a provider has to answer with, drawn under the data source form (technical design
// §10). It is not a response already received: the form's names are the ones the provider uses, so
// the block changes while they are typed. One item is enough: the app reads the list item by item
// and ignores every field it didn't ask for.
import type { DataSourceSpec } from './api';

/** How a run of the preview reads: a key, a value, the punctuation around them, or a name to fill in. */
export type ShapeTone = 'plain' | 'key' | 'value' | 'missing';

export type ShapeToken = { text: string; tone: ShapeTone };

/** A run of spaces that indents a line. */
const indent = (width: number): ShapeToken => ({ text: ' '.repeat(width), tone: 'plain' });

/** The name as JSON shows it, or its label in brackets when it is still to be filled in. */
const name = (text: string, label: string): ShapeToken =>
	text
		? { text: JSON.stringify(text), tone: 'key' }
		: { text: JSON.stringify(`[${label}]`), tone: 'missing' };

/**
 * The data shape as JSON, one token per run of text. The list and field names are the form's. A
 * required name left blank shows its label in brackets, in `attention`, rather than a name the
 * provider doesn't have; an optional one left blank is left out of the item, since the app reads
 * items without it.
 */
export function sourceShape(
	spec: DataSourceSpec,
	list: string,
	names: Record<string, string>
): ShapeToken[] {
	const listName = list.trim();
	const lines: ShapeToken[][] = [
		[{ text: '{', tone: 'plain' }],
		[indent(2), name(listName, 'List'), { text: ': [', tone: 'plain' }]
	];

	// A field the provider has no name for is only read when the plugin made it optional.
	const read = spec.fields.filter((f) => !f.optional || names[f.key]?.trim());
	if (read.length === 0) {
		lines.push([indent(4), { text: '{}', tone: 'plain' }]);
	} else {
		lines.push([indent(4), { text: '{', tone: 'plain' }]);
		read.forEach((f, i) => {
			const line: ShapeToken[] = [
				indent(6),
				name(names[f.key]?.trim() ?? '', f.label),
				{ text: ': ', tone: 'plain' },
				{ text: JSON.stringify(f.example) ?? 'null', tone: 'value' }
			];
			if (i < read.length - 1) line.push({ text: ',', tone: 'plain' });
			lines.push(line);
		});
		lines.push([indent(4), { text: '}', tone: 'plain' }]);
	}

	lines.push([indent(2), { text: ']', tone: 'plain' }], [{ text: '}', tone: 'plain' }]);

	// A newline ends every line but the last, inside its last token, so the block copies as JSON.
	return lines.flatMap((tokens, i) =>
		i === lines.length - 1
			? tokens
			: tokens.map((token, j) =>
					j === tokens.length - 1 ? { ...token, text: `${token.text}\n` } : token
				)
	);
}
