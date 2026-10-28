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

/** `12.–14.06.2026` (De) or `2026-06-12 – 2026-06-14` (En); one date when the other is missing. */
export function formatDateRange(
	start: string | null,
	end: string | null,
	format: NumberFormat
): string {
	if (!start || !end) return start || end ? formatDate((start ?? end) as string, format) : '';
	if (start === end) return formatDate(start, format);
	if (format === 'en') return `${start} – ${end}`;
	const [sy, sm, sd] = start.split('-');
	const [ey, em] = end.split('-');
	if (sy === ey && sm === em) return `${sd}.–${formatDate(end, format)}`;
	if (sy === ey) return `${sd}.${sm}.–${formatDate(end, format)}`;
	return `${formatDate(start, format)}–${formatDate(end, format)}`;
}

/** A timestamp as the user's local time: `18:02` today, otherwise the date and time. */
export function formatTimestamp(iso: string, format: NumberFormat, now = new Date()): string {
	const t = new Date(iso);
	const time = `${String(t.getHours()).padStart(2, '0')}:${String(t.getMinutes()).padStart(2, '0')}`;
	const sameDay = t.toDateString() === now.toDateString();
	if (sameDay) return `Today ${time}`;
	const date = `${t.getFullYear()}-${String(t.getMonth() + 1).padStart(2, '0')}-${String(t.getDate()).padStart(2, '0')}`;
	return `${formatDate(date, format)} ${time}`;
}

/** `2,4 MB` (De) or `2.4 MB` (En). */
export function formatSize(bytes: number, format: NumberFormat): string {
	const units = ['B', 'KB', 'MB', 'GB'];
	let value = bytes;
	let unit = 0;
	while (value >= 1000 && unit < units.length - 1) {
		value /= 1000;
		unit++;
	}
	const text = unit === 0 ? String(value) : value.toFixed(1);
	return `${format === 'de' ? text.replace('.', ',') : text} ${units[unit]}`;
}
