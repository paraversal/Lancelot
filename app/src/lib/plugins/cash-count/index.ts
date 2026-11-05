import type { Plugin } from '../api';
import Section from './Section.svelte';
import Settings from './Settings.svelte';

/** Counting a cash account by its notes, coins and custom denominations, instead of adding them up
 * by hand. */
export const cashCount: Plugin = {
	id: 'cash-count',
	name: 'Cash box count',
	description: 'Count notes, coins and custom denominations when counting a cash account.',
	// A net-movement account holds only the event's movement, not what is in the box (R-ISO-2).
	onCount: (ctx, account) => {
		const details = ctx.accounts.find((a) => a.id === account);
		return details?.assetType === 'cash' && !details.netMovement;
	},
	CountSection: Section,
	SettingsSection: Settings
};
