// Display formatting for amounts and dates. The one place money appears in TypeScript, and it
// only turns cents into text, never text into cents: parsing happens in Rust (R-COR-1).
// Both functions are tested against the same fixtures as their Rust counterparts.
import type { Cents } from './api/generated/Cents';
import type { Currency } from './api/generated/Currency';
import type { NumberFormat } from './api/generated/NumberFormat';
import type { SignStyle } from './api/generated/SignStyle';

const MINUS = '−';
const NBSP = ' ';

const SYMBOLS: Record<Currency, string> = { EUR: '€', USD: '$', GBP: '£', CHF: 'CHF' };

let eventCurrency: Currency = 'EUR';

/** Amounts are in the open event's currency, and one window holds one event. The session sets it. */
export function setEventCurrency(currency: Currency) {
	eventCurrency = currency;
}

function group(digits: string, separator: string): string {
	let out = '';
	for (let i = 0; i < digits.length; i++) {
		if (i > 0 && (digits.length - i) % 3 === 0) out += separator;
		out += digits[i];
	}
	return out;
}

function digits(magnitude: number, format: NumberFormat): string {
	const units = group(String(Math.trunc(magnitude / 100)), format === 'de' ? '.' : ',');
	const rest = String(magnitude % 100).padStart(2, '0');
	return `${units}${format === 'de' ? ',' : '.'}${rest}`;
}

/**
 * `1.234,56 €` (De) or `€1,234.56` (En), in the event's currency unless another is named. The
 * symbol's place follows the number format, and one of letters keeps a space. Mirrors
 * `format_amount` in the core.
 */
export function formatAmount(
	cents: Cents,
	format: NumberFormat,
	sign: SignStyle = 'plain',
	currency: Currency = eventCurrency
): string {
	const amount = digits(Math.abs(cents), format);
	const symbol = SYMBOLS[currency];
	let prefix = '';
	if (sign !== 'magnitude') {
		if (cents < 0) prefix = MINUS;
		else if (sign === 'always' && cents > 0) prefix = '+';
	}
	if (format === 'de') return `${prefix}${amount}${NBSP}${symbol}`;
	return `${prefix}${symbol}${symbol.length > 1 ? NBSP : ''}${amount}`;
}

/** An amount without its symbol, as an input holds it or a column of the same currency shows it. */
export function formatPlain(cents: Cents, format: NumberFormat): string {
	return `${cents < 0 ? MINUS : ''}${digits(Math.abs(cents), format)}`;
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

/** A count against the books: `2,90 € less than recorded`, `7,50 € more than recorded`. */
export function differenceText(cents: Cents, format: NumberFormat): string {
	const amount = formatAmount(cents, format, 'magnitude');
	return `${amount} ${cents < 0 ? 'less' : 'more'} than recorded`;
}
