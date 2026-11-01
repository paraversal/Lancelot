import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import AmountField from './AmountField.svelte';
import DateField from './DateField.svelte';
import IbanField from './IbanField.svelte';
import TextField from './TextField.svelte';

// The real parsers live in Rust; here they are replaced by fixed answers.
vi.mock('../api', async () => {
	const { ApiError, errorMessage } = await vi.importActual<typeof import('../api')>('../api');
	const amounts: Record<string, { cents: number; ambiguous: boolean; terms: number }> = {
		'1.234,5': { cents: 123450, ambiguous: false, terms: 1 },
		'1.234': { cents: 123400, ambiguous: true, terms: 1 },
		'1.234,50': { cents: 123450, ambiguous: false, terms: 1 },
		'1.2': { cents: 120, ambiguous: false, terms: 1 },
		'1.20': { cents: 120, ambiguous: false, terms: 1 },
		'1,20': { cents: 120, ambiguous: false, terms: 1 },
		'1,29+': { cents: 129, ambiguous: false, terms: 1 },
		'1,29+1,49': { cents: 278, ambiguous: false, terms: 2 }
	};
	const dates: Record<string, string> = { '6.12.26': '2026-12-06', '06.12.2026': '2026-12-06' };
	const messages: Record<string, string> = {
		bad_grouping: 'thousands separators must form groups of three',
		format: 'enter a date as 31.12.2026 or 2026-12-31',
		bad_checksum: 'checksum does not match; check for a typo'
	};
	const error = (code: string) =>
		new ApiError({ code, message: messages[code], field: null, report: null });
	return {
		ApiError,
		errorMessage,
		api: {
			parseSum: vi.fn(async (input: string) => {
				if (amounts[input]) return amounts[input];
				throw error('bad_grouping');
			}),
			checkIban: vi.fn(async (input: string) => {
				const iban = input.replace(/\s+/g, '').toUpperCase();
				if (iban !== 'DE89370400440532013000') throw error('bad_checksum');
				return { iban, formatted: 'DE89 3704 0044 0532 0130 00' };
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

describe('AmountField with a sum', () => {
	it('commits the total and keeps the sum on show to check', async () => {
		const oncommit = vi.fn();
		render(AmountField, { label: 'Amount', value: null, format: 'de', oncommit });
		const input = screen.getByLabelText('Amount') as HTMLInputElement;

		await fireEvent.focus(input);
		await type(input, '1,29+1,49');
		expect(screen.getByText(/^= 2,78.€$/)).toBeTruthy();

		await fireEvent.blur(input);
		await settle();
		expect(oncommit).toHaveBeenCalledWith(278);
		expect(input.value).toBe('1,29+1,49');
		expect(screen.getByText(/^= 2,78.€$/)).toBeTruthy();
	});

	it('tidies a sign left at the end', async () => {
		const oncommit = vi.fn();
		render(AmountField, { label: 'Amount', value: null, format: 'de', oncommit });
		const input = screen.getByLabelText('Amount') as HTMLInputElement;

		await type(input, '1,29+');
		expect(input.getAttribute('aria-invalid')).toBe('false');
		expect(screen.getByText(/^= 1,29.€$/)).toBeTruthy();

		await fireEvent.blur(input);
		await settle();
		expect(oncommit).toHaveBeenCalledWith(129);
		expect(input.value).toBe('1,29');
	});

	it('shows only the total when the entry is opened again', async () => {
		render(AmountField, { label: 'Amount', value: 278, format: 'de' });
		expect((screen.getByLabelText('Amount') as HTMLInputElement).value).toBe('2,78');
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

describe('error messages', () => {
	it('explain an invalid amount in a tooltip while typing and under the field after', async () => {
		render(AmountField, { label: 'Amount', value: null, format: 'de' });
		const input = screen.getByLabelText('Amount');
		await fireEvent.focus(input);
		await type(input, '1.23.4');
		const tip = screen.getByRole('tooltip');
		expect(tip.textContent).toBe('Thousands separators must form groups of three');
		expect(input.getAttribute('aria-describedby')).toBe(tip.id);
		await fireEvent.blur(input);
		await settle();
		expect(document.querySelector('.hint.error')?.textContent).toBe(
			'Thousands separators must form groups of three'
		);
	});

	it('keep an invalid date with its message instead of throwing it away', async () => {
		const oncommit = vi.fn();
		render(DateField, { label: 'Date', value: '2026-06-13', format: 'de', oncommit });
		const input = screen.getByLabelText('Date') as HTMLInputElement;
		await type(input, '13/06');
		await fireEvent.blur(input);
		await settle();
		expect(input.value).toBe('13/06');
		expect(oncommit).not.toHaveBeenCalled();
		expect(screen.getByRole('tooltip').textContent).toBe(
			'Enter a date as 31.12.2026 or 2026-12-31'
		);
	});
});

describe('Esc in a field', () => {
	it('undoes what was typed in a text field and goes no further', async () => {
		const oncommit = vi.fn();
		render(TextField, { label: 'Name', value: 'Clara', oncommit });
		const input = screen.getByLabelText<HTMLInputElement>('Name');
		await fireEvent.input(input, { target: { value: 'Clar' } });
		expect(await fireEvent.keyDown(input, { key: 'Escape' })).toBe(false);
		expect(input.value).toBe('Clara');
		expect(oncommit).not.toHaveBeenCalled();
	});

	it('reaches the panel from a text field with nothing to undo', async () => {
		render(TextField, { label: 'Name', value: 'Clara', oncommit() {} });
		expect(await fireEvent.keyDown(screen.getByLabelText('Name'), { key: 'Escape' })).toBe(true);
	});

	it('undoes what was typed in an IBAN field and goes no further', async () => {
		const oncommit = vi.fn();
		render(IbanField, { value: null, oncommit });
		const input = screen.getByRole<HTMLInputElement>('textbox');
		await type(input, 'DE89');
		expect(await fireEvent.keyDown(input, { key: 'Escape' })).toBe(false);
		expect(input.value).toBe('');
		await settle();
		expect(oncommit).not.toHaveBeenCalled();
	});
});

describe('IbanField', () => {
	it('previews the grouped IBAN and commits it normalised', async () => {
		const oncommit = vi.fn();
		render(IbanField, { value: null, oncommit });
		const input = screen.getByLabelText('IBAN') as HTMLInputElement;
		await fireEvent.focus(input);
		await type(input, 'de89370400440532013000');
		expect(screen.getByText('= DE89 3704 0044 0532 0130 00')).toBeTruthy();
		await fireEvent.blur(input);
		await settle();
		expect(oncommit).toHaveBeenCalledWith('DE89370400440532013000');
		expect(input.value).toBe('DE89 3704 0044 0532 0130 00');
	});

	it('does not save an IBAN with a typo', async () => {
		const oncommit = vi.fn();
		render(IbanField, { value: null, oncommit });
		const input = screen.getByLabelText('IBAN');
		await type(input, 'DE88370400440532013000');
		await fireEvent.blur(input);
		await settle();
		expect(oncommit).not.toHaveBeenCalled();
		expect(screen.getByRole('tooltip').textContent).toBe(
			'Checksum does not match; check for a typo'
		);
	});
});
