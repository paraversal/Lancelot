import { describe, expect, it } from 'vitest';
import type { DataSourceSpec } from './api';
import { sourceShape, type ShapeToken } from './shape';

const spec: DataSourceSpec = {
	list: 'drinks',
	fields: [
		{ key: 'name', label: 'Name', usually: 'name', example: 'Club-Mate 0,5' },
		{ key: 'quantity', label: 'Quantity', usually: 'qty', example: 12 }
	]
};

const text = (tokens: ShapeToken[]) => tokens.map((t) => t.text).join('');
const tones = (tokens: ShapeToken[], run: string) =>
	tokens.filter((t) => t.text.includes(run)).map((t) => t.tone);

describe('The data shape', () => {
	it('draws the list and the names the form holds, with a value for each field', () => {
		expect(text(sourceShape(spec, 'drinks', { name: 'name', quantity: 'qty' }))).toBe(
			'{\n  "drinks": [\n    {\n      "name": "Club-Mate 0,5",\n      "qty": 12\n    }\n  ]\n}'
		);
	});

	it('leaves out an optional field left blank, which the app reads items without', () => {
		const tickets: DataSourceSpec = {
			list: 'tickets',
			fields: [
				{ key: 'tier', label: 'Tier', usually: 'tier', example: 'Regular' },
				{ key: 'order', label: 'Order', usually: 'order', optional: true, example: '1042' }
			]
		};
		expect(text(sourceShape(tickets, 'tickets', { tier: 'tier', order: ' ' }))).toBe(
			'{\n  "tickets": [\n    {\n      "tier": "Regular"\n    }\n  ]\n}'
		);
	});

	it('shows a required name still to fill in as its label between brackets', () => {
		const shape = sourceShape(spec, '', { name: '', quantity: 'qty' });
		expect(text(shape)).toBe(
			'{\n  "[List]": [\n    {\n      "[Name]": "Club-Mate 0,5",\n      "qty": 12\n    }\n  ]\n}'
		);
		expect(tones(shape, '[List]')).toEqual(['missing']);
		expect(tones(shape, '[Name]')).toEqual(['missing']);
		expect(tones(shape, '"qty"')).toEqual(['key']);
	});

	it('keeps the block JSON when a name has a quote in it', () => {
		const shape = sourceShape(spec, 'drinks', { name: 'na"me', quantity: 'qty' });
		expect(text(shape)).toContain('"na\\"me": "Club-Mate 0,5"');
	});

	it('draws an empty item when the plugin reads no field', () => {
		expect(text(sourceShape({ list: 'drinks', fields: [] }, 'drinks', {}))).toBe(
			'{\n  "drinks": [\n    {}\n  ]\n}'
		);
	});
});
