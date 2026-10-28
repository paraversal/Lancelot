// Display formatting for amounts and dates. The one place money appears in TypeScript, and it
// only turns cents into text, never text into cents: parsing happens in Rust (R-COR-1).
// Both functions are tested against the same fixtures as their Rust counterparts.
import type { Cents } from './api/generated/Cents';
import type { NumberFormat } from './api/generated/NumberFormat';
import type { SignStyle } from './api/generated/SignStyle';

const MINUS = '−';
const NBSP = ' ';

function group(digits: string, separator: string): string {
	let out = '';
	for (let i = 0; i < digits.length; i++) {
		if (i > 0 && (digits.length - i) % 3 === 0) out += separator;
		out += digits[i];
	}
	return out;
}

/** `1.234,56 €` (De) or `€1,234.56` (En). Mirrors `format_amount` in the core. */
export function formatAmount(
	cents: Cents,
	format: NumberFormat,
	sign: SignStyle = 'plain'
): string {
	const magnitude = Math.abs(cents);
	const euros = group(String(Math.trunc(magnitude / 100)), format === 'de' ? '.' : ',');
	const rest = String(magnitude % 100).padStart(2, '0');
	let prefix = '';
	if (sign !== 'magnitude') {
		if (cents < 0) prefix = MINUS;
		else if (sign === 'always' && cents > 0) prefix = '+';
	}
	return format === 'de' ? `${prefix}${euros},${rest}${NBSP}€` : `${prefix}€${euros}.${rest}`;
}

/** `31.12.2026` (De) or `2026-12-31` (En), from an ISO date. Mirrors `format_date`. */
export function formatDate(iso: string, format: NumberFormat): string {
	const [y, m, d] = iso.split('-');
	return format === 'de' ? `${d}.${m}.${y}` : `${y}-${m}-${d}`;
}
