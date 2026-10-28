import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import AmountField from './AmountField.svelte';
import DateField from './DateField.svelte';

// The real parsers live in Rust; here they are replaced by fixed answers.
vi.mock('../api', async () => {
	const { ApiError } = await vi.importActual<typeof import('../api')>('../api');
	const amounts: Record<string, { cents: number; ambiguous: boolean }> = {
		'1.234,5': { cents: 123450, ambiguous: false },
		'1.234': { cents: 123400, ambiguous: true },
		'1.234,50': { cents: 123450, ambiguous: false },
		'1.2': { cents: 120, ambiguous: false },
		'1.20': { cents: 120, ambiguous: false },
		'1,20': { cents: 120, ambiguous: false }
	};
	const dates: Record<string, string> = { '6.12.26': '2026-12-06', '06.12.2026': '2026-12-06' };
	const error = (code: string) => new ApiError({ code, message: code, field: null, report: null });
	return {
		ApiError,
		api: {
			parseAmount: vi.fn(async (input: string) => {
				if (amounts[input]) return amounts[input];
				throw error('bad_grouping');
			}),
			parseDate: vi.fn(async (input: string) => {
				if (dates[input]) return { date: dates[input] };
				throw error('format');
			})
		}
	};
});

const settle = () => new Promise((r) => setTimeout(r, 0));

async function type(input: HTMLElement, text: string) {
	await fireEvent.input(input, { target: { value: text } });
	await settle();
}

describe('AmountField', () => {
	it('previews what was typed and commits it', async () => {
		const oncommit = vi.fn();
		render(AmountField, { label: 'Amount', value: null, format: 'de', oncommit });
		const input = screen.getByLabelText('Amount');

		await type(input, '1.234,5');
		expect(screen.getByText(/^= 1\.234,50.€$/)).toBeTruthy();

		await fireEvent.blur(input);
		await settle();
		expect(oncommit).toHaveBeenCalledWith(123450);
		expect((input as HTMLInputElement).value).toBe('1.234,50');
	});

	it('marks an ambiguous reading in attention', async () => {
		render(AmountField, { label: 'Amount', value: null, format: 'de' });
		await type(screen.getByLabelText('Amount'), '1.234');
		expect(screen.getByText(/^= 1\.234,00.€$/).classList.contains('ambiguous')).toBe(true);
	});

	it('flags invalid input and does not commit it', async () => {
		const oncommit = vi.fn();
		render(AmountField, { label: 'Amount', value: 500, format: 'de', oncommit });
		const input = screen.getByLabelText('Amount');
		await type(input, '1.23.4');
		expect(input.getAttribute('aria-invalid')).toBe('true');
		expect(screen.queryByText(/^=/)).toBeNull();
		await fireEvent.blur(input);
		await settle();
		expect(oncommit).not.toHaveBeenCalled();
	});
});

describe('AmountField with a live parent', () => {
	it('keeps the typed text when the parent passes the parsed value back', async () => {
		let value: number | null = null;
		const { rerender } = render(AmountField, {
			label: 'Amount',
			value,
			format: 'de',
			oninput: (v: number | null) => (value = v)
		});
		const input = screen.getByLabelText('Amount') as HTMLInputElement;
		// Halfway through, the input is not an amount yet and the field reports null.
		await type(input, '1.234,');
		await rerender({ value });
		expect(input.value).toBe('1.234,');
		await type(input, '1.234,5');
		await rerender({ value });
		expect(value).toBe(123450);
		expect(input.value).toBe('1.234,5');
		expect(screen.getByText(/^= 1\.234,50.€$/)).toBeTruthy();
	});
});

describe('AmountField previews every reading', () => {
	it('shows the preview for 1.2 and for 1.20 in De', async () => {
		let value: number | null = null;
		const { rerender } = render(AmountField, {
			label: 'Amount',
			value,
			format: 'de',
			oninput: (v: number | null) => (value = v)
		});
		const input = screen.getByLabelText('Amount') as HTMLInputElement;
		await type(input, '1.2');
		await rerender({ value });
		expect(screen.getByText(/^= 1,20.€$/)).toBeTruthy();
		await type(input, '1.20');
		await rerender({ value });
		expect(input.value).toBe('1.20');
		expect(screen.getByText(/^= 1,20.€$/)).toBeTruthy();
	});
});

describe('AmountField previews while typing', () => {
	it('confirms input that already looks like the result (1.20 in En, 1,20 in De)', async () => {
		for (const [format, text] of [
			['en', '1.20'],
			['de', '1,20']
		] as const) {
			const { unmount } = render(AmountField, { label: 'Amount', value: null, format });
			const input = screen.getByLabelText('Amount');
			await fireEvent.focus(input);
			await type(input, text);
			expect(screen.getByText(format === 'en' ? /^= €1\.20$/ : /^= 1,20.€$/)).toBeTruthy();
			await fireEvent.blur(input);
			await settle();
			expect(screen.queryByText(/^=/)).toBeNull();
			unmount();
		}
	});
});

describe('DateField', () => {
	it('previews a short date and commits it as ISO', async () => {
		const oncommit = vi.fn();
		render(DateField, { label: 'To', value: null, format: 'de', oncommit });
		const input = screen.getByLabelText('To');
		await type(input, '6.12.26');
		expect(screen.getByText('= 06.12.2026')).toBeTruthy();
		await fireEvent.blur(input);
		await settle();
		expect(oncommit).toHaveBeenCalledWith('2026-12-06');
	});

	it('keeps what is being typed when the parent re-renders with the same value', async () => {
		const { rerender } = render(DateField, {
			label: 'To',
			value: '2026-06-14',
			format: 'de',
			oncommit: () => {}
		});
		const input = screen.getByLabelText('To') as HTMLInputElement;
		await type(input, '6.12.26');
		await rerender({ value: '2026-06-14' });
		expect(input.value).toBe('6.12.26');
		await rerender({ value: '2026-06-15' });
		expect(input.value).toBe('15.06.2026');
	});
});
