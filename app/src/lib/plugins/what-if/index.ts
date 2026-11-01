import type { Plugin } from '../api';
import Section from './Section.svelte';

/** Sliders against the forecast, for trying ideas out. Nothing they do is kept. */
export const whatIf: Plugin = {
	id: 'what-if',
	name: 'What if',
	description:
		'Try more tickets, other prices and other costs against the forecast. Nothing is saved.',
	onBudget: (ctx) => ctx.budget.lines.length > 0 || ctx.tiers.length > 0,
	BudgetSection: Section
};
