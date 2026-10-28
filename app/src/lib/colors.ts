// Colours of categories (data series: fills only, never text). The defaults follow UI.fig:
// Kitchen, Drinks, Pfand, Mobility, Venue, Tech, General. Categories added later cycle through
// the series. Income categories use `plus`.
import type { AccountView } from './api/generated/AccountView';

const DEFAULTS = ['tier-1', 'tier-3', 'tier-3', 'tier-2', 'tier-4', 'tier-5', 'ink-muted'];
const SERIES = ['tier-1', 'tier-2', 'tier-3', 'tier-4', 'tier-5'];

export function categoryColor(account: AccountView, accounts: AccountView[]): string {
	if (account.kind === 'income') return 'var(--color-plus)';
	const index = accounts.filter((a) => a.kind === 'expense').findIndex((a) => a.id === account.id);
	const token =
		index < DEFAULTS.length ? DEFAULTS[index] : SERIES[(index - DEFAULTS.length) % SERIES.length];
	return `var(--color-${token ?? 'ink-muted'})`;
}
