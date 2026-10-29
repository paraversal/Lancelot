import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { EventSnapshot } from '#lib/api/generated/EventSnapshot.ts';
import example from '#lib/api/mock/example-event.json';
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
});
