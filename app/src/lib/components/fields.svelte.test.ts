import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import AmountField from './AmountField.svelte';
import ChoiceField from './ChoiceField.svelte';
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

describe('TextField with suggestions', () => {
	const suggestions = ['Hoffmann', 'Metro', 'Hofladen'];
	const open = async (text: string) => {
		const oncommit = vi.fn();
		render(TextField, { label: 'Counterparty', value: null, suggestions, oncommit });
		const input = screen.getByLabelText('Counterparty');
		await fireEvent.focus(input);
		await fireEvent.input(input, { target: { value: text } });
		return { input, oncommit };
	};
	const options = () => screen.queryAllByRole('option').map((o) => o.textContent?.trim());

	it('offers what matches what is typed', async () => {
		await open('hof');
		expect(options()).toEqual(['Hoffmann', 'Hofladen']);
	});

	it('picks one by arrow and Enter', async () => {
		const { input, oncommit } = await open('hof');
		await fireEvent.keyDown(input, { key: 'ArrowDown' });
		await fireEvent.keyDown(input, { key: 'ArrowDown' });
		await fireEvent.keyDown(input, { key: 'Enter' });
		expect(oncommit).toHaveBeenCalledWith('Hofladen');
		expect(options()).toEqual([]);
	});

	it('picks one by clicking, without committing what was half typed', async () => {
		const { oncommit } = await open('hof');
		await fireEvent.mouseDown(screen.getByRole('option', { name: 'Hoffmann' }));
		expect(oncommit.mock.calls).toEqual([['Hoffmann']]);
	});

	it('keeps a new name, and Esc only closes the list', async () => {
		const { input, oncommit } = await open('Ho');
		await fireEvent.keyDown(input, { key: 'Escape' });
		expect(options()).toEqual([]);
		expect((input as HTMLInputElement).value).toBe('Ho');
		await fireEvent.blur(input);
		expect(oncommit).toHaveBeenCalledWith('Ho');
	});

	it('has nothing to offer for an exact match', async () => {
		await open('metro');
		expect(options()).toEqual([]);
	});
});

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
	it('saves what was typed in a text field and lets the panel see the same Esc', async () => {
		const oncommit = vi.fn();
		render(TextField, { label: 'Name', value: 'Clara', oncommit });
		const input = screen.getByLabelText<HTMLInputElement>('Name');
		await fireEvent.input(input, { target: { value: 'Clar' } });
		expect(await fireEvent.keyDown(input, { key: 'Escape' })).toBe(true);
		expect(input.value).toBe('Clar');
		expect(oncommit.mock.calls).toEqual([['Clar']]);
	});

	it('reaches the panel from a text field with nothing to save', async () => {
		render(TextField, { label: 'Name', value: 'Clara', oncommit() {} });
		expect(await fireEvent.keyDown(screen.getByLabelText('Name'), { key: 'Escape' })).toBe(true);
	});

	it('keeps an invalid IBAN in the field and lets the panel take the Esc', async () => {
		const oncommit = vi.fn();
		render(IbanField, { value: null, oncommit });
		const input = screen.getByRole<HTMLInputElement>('textbox');
		input.focus();
		await type(input, 'DE89');
		expect(await fireEvent.keyDown(input, { key: 'Escape' })).toBe(true);
		expect(input.value).toBe('DE89');
		await settle();
		expect(oncommit).not.toHaveBeenCalled();
	});

	it('saves a valid IBAN on Esc before the panel can close', async () => {
		const oncommit = vi.fn();
		render(IbanField, { value: null, oncommit });
		const input = screen.getByRole<HTMLInputElement>('textbox');
		input.focus();
		await type(input, 'de89370400440532013000');
		expect(await fireEvent.keyDown(input, { key: 'Escape' })).toBe(false);
		await settle();
		await settle();
		expect(oncommit).toHaveBeenCalledWith('DE89370400440532013000');
		expect(input.value).toBe('DE89 3704 0044 0532 0130 00');
	});
});

describe('IbanField', () => {
	it('previews the grouped IBAN and commits it normalised', async () => {
		const oncommit = vi.fn();
		render(IbanField, { value: null, oncommit });
		const input = screen.getByLabelText('IBAN') as HTMLInputElement;
		input.focus();
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

describe('ChoiceField', () => {
	const accounts = [
		{ value: 1, label: 'Bank account' },
		{ value: 2, label: 'Anna', group: 'People' },
		{ value: 3, label: 'Nils', group: 'People' },
		{ value: 4, label: 'Nora', group: 'People' }
	];

	function open(value: number | null = null) {
		const onchange = vi.fn();
		const onadd = vi.fn(async () => 9);
		render(ChoiceField, {
			label: 'Paid by',
			value,
			options: accounts,
			addLabel: 'New person…',
			onchange,
			onadd
		});
		const field = screen.getByLabelText<HTMLInputElement>('Paid by');
		return { field, onchange, onadd };
	}

	const shown = () =>
		screen.queryAllByRole('option').map((o) => o.textContent?.replace(/\s+/g, ' ').trim());

	it('opens every account and person when focused', async () => {
		const { field } = open();
		await fireEvent.focus(field);
		expect(shown()).toEqual(['Bank account', 'Anna', 'Nils', 'Nora']);
	});

	it('narrows to the names a letter matches instead of adding a person', async () => {
		const { field, onchange, onadd } = open();
		await fireEvent.focus(field);
		await fireEvent.input(field, { target: { value: 'N' } });
		// People that start with N before the rest that only contain it; adding is last.
		expect(shown()).toEqual(['Nils', 'Nora', 'Anna', 'Bank account', 'New person “N”']);
		await fireEvent.keyDown(field, { key: 'Enter' });
		expect(onchange).toHaveBeenCalledWith(3);
		expect(onadd).not.toHaveBeenCalled();
	});

	it('adds the typed name only when nothing matches it', async () => {
		const { field, onchange, onadd } = open();
		await fireEvent.focus(field);
		await fireEvent.input(field, { target: { value: 'Frida' } });
		expect(shown()).toEqual(['New person “Frida”']);
		await fireEvent.keyDown(field, { key: 'Enter' });
		await vi.waitFor(() => expect(onchange).toHaveBeenCalledWith(9));
		expect(onadd).toHaveBeenCalledWith('Frida');
	});

	it('offers nothing to add for a name that exists', async () => {
		const { field } = open();
		await fireEvent.focus(field);
		await fireEvent.input(field, { target: { value: 'Nora' } });
		expect(shown()).toEqual(['Nora']);
	});

	it('picks a match by arrow and Enter', async () => {
		const { field, onchange } = open();
		await fireEvent.focus(field);
		await fireEvent.input(field, { target: { value: 'N' } });
		await fireEvent.keyDown(field, { key: 'ArrowDown' });
		await fireEvent.keyDown(field, { key: 'Enter' });
		expect(onchange).toHaveBeenCalledWith(4);
	});

	it('leaves the save shortcuts to the panel', async () => {
		const { field, onchange } = open();
		await fireEvent.focus(field);
		await fireEvent.input(field, { target: { value: 'N' } });
		await fireEvent.keyDown(field, { key: 'Enter', metaKey: true });
		await fireEvent.keyDown(field, { key: 'Enter', metaKey: true, shiftKey: true });
		expect(onchange).not.toHaveBeenCalled();
		expect(shown().length).toBeGreaterThan(0);
	});

	it('picks a match by clicking it', async () => {
		const { field, onchange } = open();
		await fireEvent.focus(field);
		await fireEvent.input(field, { target: { value: 'ann' } });
		await fireEvent.mouseDown(screen.getByRole('option', { name: 'Anna' }));
		expect(onchange).toHaveBeenCalledWith(2);
		expect(shown()).toEqual([]);
	});

	it('closes only the list on Esc, and leaves the chosen name alone', async () => {
		const { field, onchange } = open(2);
		await fireEvent.focus(field);
		expect(shown().length).toBeGreaterThan(0);
		expect(await fireEvent.keyDown(field, { key: 'Escape' })).toBe(false);
		expect(shown()).toEqual([]);
		expect(field.value).toBe('Anna');
		expect(onchange).not.toHaveBeenCalled();
		// With nothing open, the panel gets the Esc.
		expect(await fireEvent.keyDown(field, { key: 'Escape' })).toBe(true);
	});

	it('leaves the field with the name it was showing', async () => {
		const { field } = open(2);
		await fireEvent.focus(field);
		await fireEvent.input(field, { target: { value: 'Nil' } });
		await fireEvent.blur(field);
		expect(field.value).toBe('Anna');
	});
});
