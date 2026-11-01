import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import example from '#lib/api/fixtures/example-event.json';
import { session } from '#lib/state/session.svelte.ts';
import Report from './+page.svelte';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('#lib/api/index.ts', async () => {
	const actual = await vi.importActual<typeof import('#lib/api/index.ts')>('#lib/api/index.ts');
	return {
		...actual,
		api: { print: vi.fn() },
		attachmentUrl: vi.fn(async (a: { filename: string }) => `about:blank#${a.filename}`)
	};
});
vi.mock('#lib/report/pdf.ts', () => ({
	renderPdf: vi.fn(async () => ['data:image/png;base64,p1', 'data:image/png;base64,p2'])
}));

const snapshot = example as EventSnapshot;
const settle = () => new Promise((r) => setTimeout(r, 0));

describe('Settlement report', () => {
	it('lists every entry and prints every receipt, PDFs page by page', async () => {
		session.update({
			revision: 1,
			path: '/r.lancelot',
			fileName: 'r.lancelot',
			snapshot: structuredClone(snapshot),
			integrity: { issues: [] },
			backupError: null
		});
		render(Report);
		expect(screen.getByTestId('report-result').textContent?.replace(/\s+/g, ' ').trim()).toBe(
			'+2.027,50 €'
		);
		expect(document.querySelectorAll('.entries tbody tr').length).toBe(snapshot.txns.length);
		const plan = document.querySelector('.budget')!;
		const row = (name: string) =>
			[...plan.querySelectorAll('tr')]
				.map((r) =>
					[...r.querySelectorAll('td')].map((c) => c.textContent!.replace(/\s+/g, ' ').trim())
				)
				.find((cells) => cells[0] === name);
		expect(row('Venue')).toEqual(['Venue', '1.800,00 €', '1.800,00 €', '1.800,00 €']);
		expect(row('Mobility')).toEqual(['Mobility', '150,00 €', '–', '150,00 €']);
		expect(row('Expenses')).toEqual(['Expenses', '4.895,00 €', '5.362,50 €', '5.512,50 €']);
		expect(row('Result')).toEqual(['Result', '+2.305,00 €', '+2.027,50 €', '+1.765,00 €']);

		const centre = screen.getByTestId('report-cost-centre').textContent?.replace(/\s+/g, ' ');
		expect(centre).toContain('Drinks');
		expect(centre).toContain('812,50 €');
		expect(centre).toContain('484,00 €');
		expect(centre).toContain('+328,50 €');

		await settle();
		await settle();
		const files = snapshot.txns.flatMap((t) => t.attachments);
		const pdfs = files.filter((a) => a.mime === 'application/pdf').length;
		expect(pdfs).toBe(2);
		// Photos take one page each, the PDFs two pages each in this test.
		expect(document.querySelectorAll('.receipt').length).toBe(files.length - pdfs + 2 * pdfs);
		const print = screen.getByText('Print').closest('button')!;
		expect(print.disabled).toBe(false);
	});

	function open(change: (s: EventSnapshot) => void = () => {}) {
		const copy = structuredClone(snapshot);
		change(copy);
		session.update({
			revision: Date.now(),
			path: '/r.lancelot',
			fileName: 'r.lancelot',
			snapshot: copy,
			integrity: { issues: [] },
			backupError: null
		});
	}

	it('prints what a plugin adds, in its frame, and can leave it out', async () => {
		open();
		render(Report);
		const frame = screen.getByTestId('report-plugin');
		expect(frame.textContent).toContain('Stock and sales');
		expect(frame.textContent).toContain('Plugin');
		const table = screen.getByTestId('plugin-drinks-report').textContent!;
		expect(table).toContain('Club-Mate 0,5');
		expect(table).toContain('851,00');
		expect(table).toContain('812,50');

		await fireEvent.click(screen.getByLabelText('Stock and sales'));
		expect(screen.queryByTestId('report-plugin')).toBeNull();
	});

	it('leaves out a plugin that is switched off or has nothing to print', () => {
		open((s) => {
			s.pluginRecords = s.pluginRecords.filter((r) => !r.key.startsWith('product:'));
		});
		render(Report);
		expect(screen.queryByTestId('report-plugin')).toBeNull();
		expect(screen.queryByLabelText('Stock and sales')).toBeNull();
	});
});
