import type { Plugin } from '../api';
import { centreSettings, stockedCentres } from './records';
import ReportSection from './ReportSection.svelte';
import Section from './Section.svelte';
import Settings from './Settings.svelte';

/** Goods from a supplier for a cost centre, on commission or bought, against what people logged and paid. */
export const drinks: Plugin = {
	id: 'drinks',
	name: 'Stock and sales',
	description:
		'Goods from a supplier, on commission or bought outright: what the supplier bills, what is left over, and what people logged and paid.',
	attachedTo: (ctx, centre) => centreSettings(ctx.records, centre) !== null,
	CentreSection: Section,
	SettingsSection: Settings,
	inReport: (ctx) => stockedCentres(ctx.records, ctx.costCentres).length > 0,
	ReportSection,
	dataSource: {
		list: 'drinks',
		fields: [
			{ key: 'name', label: 'Name', usually: 'name' },
			{ key: 'quantity', label: 'Quantity', usually: 'qty' }
		]
	}
};
