// What the plugin keeps in the event file. A template copies the setup (a cost centre's settings
// and the price list) and leaves out the counts. A record that doesn't fit is ignored and kept.
import type { CostCentre, PluginChange, PluginRecord } from '../api';

export type CentreSettings = {
	supplier: string | null;
	/** The expense category of deposit, left out of the invoice check (R-DRK-2). */
	deposit: number | null;
};

export type Product = {
	id: number;
	centre: number;
	name: string;
	/** What a unit costs from the supplier, in cents. */
	cost: number;
	/** What people are asked to give per unit, in cents. A paper list's tally is a product too. */
	donation: number | null;
	/** Other names a data source uses for it. */
	aliases?: string[];
};

export type Counts = {
	taken: number;
	returned: number;
	/** Billed but still here, like an opened crate. */
	leftOver: number | null;
	/** Typed in. It can be negative, to correct an import. */
	manual: number | null;
	/** From a sales import. */
	imported: number | null;
};

export const NO_COUNTS: Counts = {
	taken: 0,
	returned: 0,
	leftOver: null,
	manual: null,
	imported: null
};

const isObject = (v: unknown): v is Record<string, unknown> =>
	typeof v === 'object' && v !== null && !Array.isArray(v);
const isCount = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;
const isWhole = (v: unknown): v is number => Number.isSafeInteger(v);
const optional = (v: unknown, check: (v: unknown) => boolean) => v === null || check(v);

const find = (records: PluginRecord[], key: string) => records.find((r) => r.key === key)?.value;

export function centreSettings(records: PluginRecord[], centre: number): CentreSettings | null {
	const v = find(records, `centre:${centre}`);
	if (!isObject(v)) return null;
	return {
		supplier: typeof v.supplier === 'string' ? v.supplier : null,
		deposit: isWhole(v.deposit) ? v.deposit : null
	};
}

function readProduct(r: PluginRecord): Product | null {
	const id = Number(r.key.slice('product:'.length));
	const v = r.value;
	if (!r.key.startsWith('product:') || !isWhole(id) || !isObject(v)) return null;
	if (!isWhole(v.centre) || typeof v.name !== 'string' || !isCount(v.cost)) return null;
	if (!optional(v.donation, isCount)) return null;
	return {
		id,
		centre: v.centre,
		name: v.name,
		cost: v.cost,
		donation: v.donation as number | null,
		...(Array.isArray(v.aliases) ? { aliases: v.aliases.filter((a) => typeof a === 'string') } : {})
	};
}

export const allProducts = (records: PluginRecord[]) =>
	records.flatMap((r) => readProduct(r) ?? []);

export function countsOf(records: PluginRecord[], product: number): Counts {
	const v = find(records, `count:${product}`);
	if (
		!isObject(v) ||
		!isCount(v.taken) ||
		!isCount(v.returned) ||
		!optional(v.leftOver, isCount) ||
		!optional(v.manual, isWhole) ||
		!optional(v.imported, isCount)
	) {
		return NO_COUNTS;
	}
	return {
		taken: v.taken,
		returned: v.returned,
		leftOver: v.leftOver as number | null,
		manual: v.manual as number | null,
		imported: v.imported as number | null
	};
}

/** The cost centres the plugin is attached to that have products, with what it knows of them. */
export function stockedCentres(records: PluginRecord[], centres: CostCentre[]) {
	return centres.flatMap((centre) => {
		const settings = centreSettings(records, centre.id);
		const products = allProducts(records)
			.filter((p) => p.centre === centre.id)
			.map((product) => ({ product, counts: countsOf(records, product.id) }));
		return settings && products.length ? [{ centre, settings, products }] : [];
	});
}

/** Ids stay unique among the products that exist; a removed product's counts go with it. */
export const nextProductId = (records: PluginRecord[]) =>
	Math.max(0, ...allProducts(records).map((p) => p.id)) + 1;

export const setCentre = (centre: number, settings: CentreSettings | null): PluginChange => ({
	key: `centre:${centre}`,
	scope: 'setup',
	value: settings
});

export const setProduct = ({ id, ...value }: Product): PluginChange => ({
	key: `product:${id}`,
	scope: 'setup',
	value
});

export const setCounts = (product: number, counts: Counts): PluginChange => ({
	key: `count:${product}`,
	scope: 'records',
	value: counts
});

export const removeProduct = (id: number): PluginChange[] => [
	{ key: `product:${id}`, scope: 'setup', value: null },
	{ key: `count:${id}`, scope: 'records', value: null }
];
