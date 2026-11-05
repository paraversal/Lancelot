import type { Plugin } from '../api';
import { attachedCentres, centreSettings, sourceKey, stockedCentres } from './records';
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
	/** One source per cost centre: the drinks and the merch can come from different providers. */
	sourceTabs: (ctx) =>
		attachedCentres(ctx.records, ctx.costCentres).map((centre) => ({
			key: sourceKey(centre.name),
			title: centre.name
		})),
	summary: (ctx) => {
		const pulling = attachedCentres(ctx.records, ctx.costCentres).filter(
			(centre) => ctx.dataSource(sourceKey(centre.name)) !== null
		);
		const names = pulling.map((centre) => centre.name);
		if (!names.length) return 'No data source yet';
		if (names.length === 1) return `Pulls for ${names[0]}`;
		if (names.length === 2) return `Pulls for ${names[0]} and ${names[1]}`;
		return `Pulls for ${names.length} cost centres`;
	},
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
