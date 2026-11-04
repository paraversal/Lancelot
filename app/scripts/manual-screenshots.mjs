// Takes the screenshots for docs/manual by walking through the app's real flows on made-up events.
//
// Start the bridge and the UI first (`cargo run -p lancelot-bridge`, `pnpm dev`), and once
// `pnpm exec playwright-core install chromium-headless-shell`. Then:
//
//   pnpm manual:screenshots            every screenshot
//   pnpm manual:screenshots tickets    only the walks with this name
//
// A step that no longer works stops the walk with an error, so a change in the app that makes the
// manual wrong shows up here.
import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';

const BASE = 'http://localhost:5173';
const OUT = fileURLToPath(new URL('../../docs/manual/images/', import.meta.url));
mkdirSync(OUT, { recursive: true });

const browser = await chromium.launch();
const context = await browser.newContext({
	viewport: { width: 1280, height: 800 },
	deviceScaleFactor: 2,
	colorScheme: 'light',
	reducedMotion: 'reduce',
	locale: 'de-DE'
});
const page = await context.newPage();
page.setDefaultTimeout(5000);

const reset = (scenario) =>
	context.request.post(`${BASE}/__bridge/reset/${scenario}`, {
		headers: { 'X-Lancelot-Bridge': '1' }
	});

const goTo = async (path) => {
	await page.goto(BASE + path);
	await page.waitForLoadState('networkidle');
};

const button = (name) => page.getByRole('button', { name, exact: false });

/** The bridge's event files live in a temp folder. The screenshots show a folder a person would use. */
const tidyPaths = () =>
	page.evaluate(() => {
		const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
		for (let node = walker.nextNode(); node; node = walker.nextNode())
			if (node.nodeValue.includes('lancelot-bridge'))
				node.nodeValue = node.nodeValue.replace(/\S*lancelot-bridge[^/]*\/events/, '~/Events');
	});

async function shot(name, { height = 800 } = {}) {
	await page.setViewportSize({ width: 1280, height });
	await page.waitForTimeout(350);
	await tidyPaths();
	await page.mouse.move(0, 0);
	await page.screenshot({ path: `${OUT}${name}.png` });
	console.log(`  ${name}.png`);
}

/** Picks an option in a field that offers choices. */
const choose = (label, option) =>
	page.getByLabel(label, { exact: true }).first().selectOption({ label: option });

/** Saves the open panel and waits for the write, so the next step can't overtake it. */
async function save(name = 'Save') {
	await page.locator('aside.panel footer').getByRole('button', { name }).first().click();
	await page.waitForLoadState('networkidle');
	await page.waitForTimeout(250);
}

const person = (name) => page.getByRole('row', { name: new RegExp(`^${name}`) });

/** Bank details for someone: public example IBANs. */
async function addIban(name, iban) {
	await person(name).click();
	await button('Edit').click();
	const field = page.getByLabel('IBAN');
	await field.fill(iban);
	await field.blur();
	await page.waitForTimeout(250);
	await page.keyboard.press('Escape');
	await page.keyboard.press('Escape');
	await page.waitForTimeout(250);
}

/** Someone reimbursed another way than by bank transfer. */
async function addOtherMethod(name, method) {
	await person(name).click();
	await button('Edit').click();
	await page.getByRole('radiogroup', { name: 'Reimburse by' }).getByText('Other').click();
	const field = page.getByLabel('Method');
	await field.fill(method);
	await field.blur();
	await page.waitForTimeout(250);
	await page.keyboard.press('Escape');
	await page.keyboard.press('Escape');
	await page.waitForTimeout(250);
}

const walks = {
	async start() {
		await reset('start');
		await goTo('/');

		await button('New event').click();
		await page.getByLabel('Name', { exact: true }).first().fill('Summer Festival 2026');
		await page.getByLabel('From').fill('12.06.2026');
		await page.getByLabel('To', { exact: true }).fill('14.06.2026');
		await page.getByLabel('Location').fill('Jugendzentrum Nord');
		await page.getByPlaceholder('Name').fill('Bank account');
		await button('Account').click();
		await page.getByPlaceholder('Name').nth(1).fill('Cash box');
		await page.getByRole('radiogroup', { name: 'Cash box holds' }).getByText('Cash').click();
		await shot('new-event');

		await button('Create').click();
		await page.waitForURL('**/event/overview');
		await shot('overview-empty');

		// The first count and the first entry, as chapter 4 and 6 tell it.
		await button('Count').nth(1).click();
		await page.getByLabel('Amount').first().fill('150');
		await shot('count-opening');
		await save();
	},

	async entries() {
		await reset('example');
		await goTo('/event/ledger');
		await shot('ledger');

		await page.keyboard.press('n');
		await page.getByLabel('Amount').fill('1,29+1,49+45,20');
		await choose('Paid by', 'Anna');
		await page.locator('.category select').first().selectOption({ label: 'Kitchen' });
		await page.getByLabel('Description').fill('Spices and oil');
		await page.getByLabel('Counterparty').fill('Wholesale market');
		await shot('entry-expense', { height: 900 });

		await page.locator('.add-line').click();
		await page.locator('.line-amount input').first().fill('30');
		await page.locator('.category select').nth(1).selectOption({ label: 'General' });
		await page.locator('.line-amount input').nth(1).fill('17,98');
		await shot('entry-split', { height: 900 });
		await save();
		await page.getByRole('row', { name: /Spices and oil/ }).waitFor();

		await reset('example');
		await goTo('/event/people');
		await shot('people', { height: 560 });
		await person('Anna').click();
		await shot('person');
		await button('Settle').first().click();
		await shot('entry-settle');
	},

	async payouts() {
		await reset('example');
		await goTo('/event/people');
		await addIban('Anna', 'DE89370400440532013000');
		await addOtherMethod('Malte', 'PayPal: malte@example.org');
		await person('Anna').click();
		await button('Edit').click();
		await shot('person-edit');
		await goTo('/event/people/payouts');
		await button('Mark as paid').click();
		await shot('mark-paid', { height: 640 });
	},

	async plan() {
		await reset('example');
		await goTo('/event/budget');
		await shot('budget', { height: 900 });
		await page
			.getByRole('button', { name: /^Budget line/ })
			.first()
			.click();
		await page.getByLabel('Description').fill('Security');
		await page.getByRole('radiogroup', { name: 'Estimate' }).getByText('Quantity').click();
		await page.getByLabel('Quantity', { exact: true }).fill('4');
		await page.getByLabel('Unit price').fill('120');
		await shot('budget-line');
	},

	async overview() {
		await reset('example');
		await goTo('/event/overview');
		// The README and the manual's first figure share this one; the README also has it dark.
		await shot('readme-overview', { height: 720 });
		await page.emulateMedia({ colorScheme: 'dark' });
		await shot('readme-overview-dark', { height: 720 });
		await page.emulateMedia({ colorScheme: 'light' });

		await button('Count').nth(1).click();
		await shot('count-closing');
		await page.keyboard.press('Escape');
		await button('Statement').first().click();
		await shot('statement');
		await page.keyboard.press('Escape');

		await button('Close event').click();
		const notes = page.getByPlaceholder('Why it is fine');
		await notes.nth(0).fill('Paid after the event');
		await notes.nth(1).fill('Paid after the event');
		await notes.nth(2).fill('Bob pays it back next week');
		await notes.nth(3).fill('Kiosk gave no receipt');
		await notes.nth(4).fill('Rounding in the till');
		await shot('close');

		await goTo('/event/overview/report');
		await shot('report', { height: 1000 });
		const unread = await page.getByText('could not be read').count();
		if (unread) throw new Error(`the report could not read ${unread} PDF receipts`);
		await goTo('/event/history');
		await shot('history');
	},

	/** Steps the manual describes that no screenshot shows: they only have to keep working. */
	async steps() {
		await reset('empty');
		await goTo('/event/people');
		await button('Person').click();
		await page.getByLabel('Name', { exact: true }).fill('Dana');
		await page.keyboard.press('Enter');
		await page.getByRole('row', { name: /^Dana/ }).waitFor();
		await page.keyboard.press('Escape');

		await goTo('/event/settings');
		await page.getByLabel('New cost centre for goods').fill('Drinks');
		await button('Add cost centre').click();
		await page.getByRole('navigation').getByText('Drinks').click();
		await page.getByRole('button', { name: /^Product/ }).click();
		await page.keyboard.press('Escape');

		await reset('example');
		await goTo('/event/people/payouts');
		await button('Repaid').click();
		await page.getByRole('radio', { name: 'Repay', checked: true }).waitFor();

		// The Counterparty field offers the names already used, and one can be picked.
		await reset('example');
		await goTo('/event/ledger');
		await page.keyboard.press('n');
		const counterparty = page.getByLabel('Counterparty');
		await counterparty.fill('Hof');
		await page.getByRole('option', { name: 'Hoffmann' }).click();
		if ((await counterparty.inputValue()) !== 'Hoffmann') throw new Error('no suggestion picked');
		await page.keyboard.press('Escape');

		// Whoever is ticked gets paid; a bank transfer starts ticked, another method doesn't.
		await reset('example');
		await goTo('/event/people');
		await addOtherMethod('Anna', 'Cash');
		await addIban('Malte', 'DE02120300000000202051');
		await goTo('/event/people/payouts');
		await page.getByText('Cash').first().waitFor();
		await button('Mark as paid').click();
		const panel = page.locator('aside.panel');
		const anna = panel.getByRole('checkbox', { name: /Anna/ });
		if (await anna.isChecked()) throw new Error('another method started ticked');
		await panel.getByRole('checkbox', { name: /Malte/ }).uncheck();
		await anna.check();
		await panel.getByRole('button', { name: 'Mark as paid' }).click();
		await page.waitForTimeout(1200);
		await page.getByRole('row').or(page.getByText('Malte')).first().waitFor();

		// Dollars from the first entry on.
		await reset('start');
		await goTo('/');
		await button('New event').click();
		await page.getByLabel('Name', { exact: true }).first().fill('Street fair');
		await page.getByLabel('Currency').selectOption('USD');
		await page.getByPlaceholder('Name').fill('Bank');
		await button('Create').click();
		await page.waitForURL('**/event/overview');
		await page.getByText('$0.00').or(page.getByText('0,00 $')).first().waitFor();
	},

	/** One supplier per cost centre, as chapter 8 tells it. */
	async suppliers() {
		await reset('empty');
		await goTo('/event/settings');
		for (const name of ['Drinks Hoffmann', 'Drinks Krüger']) {
			await page.getByRole('button', { name: /^Cost centre/ }).click();
			await page.waitForLoadState('networkidle');
			await page.waitForTimeout(300);
			const field = page.getByLabel('Cost centre name').last();
			await field.fill(name);
			await field.blur();
			await page.waitForLoadState('networkidle');
			await page.waitForTimeout(300);
		}
		await page.getByRole('checkbox', { name: 'Drinks Hoffmann' }).check();
		await page.getByRole('checkbox', { name: 'Drinks Krüger' }).check();
		await page.waitForTimeout(300);
		await page.getByRole('checkbox', { name: 'Drinks Krüger' }).scrollIntoViewIfNeeded();
		await shot('drinks-suppliers', { height: 900 });

		for (const [centre, supplier] of [
			['Drinks Hoffmann', 'Hoffmann'],
			['Drinks Krüger', 'Krüger']
		]) {
			await page.getByRole('navigation').getByText(centre).click();
			await button('Supplier').click();
			await page.getByLabel('Supplier').fill(supplier);
			await save();
			await page.getByText(supplier).first().waitFor();
		}
		await page.getByRole('navigation').getByText('Drinks Hoffmann').click();
		await page.getByText('Hoffmann').first().waitFor();
		if (await page.getByText('Krüger', { exact: true }).count())
			throw new Error('the suppliers got mixed up');
	},

	async closed() {
		await reset('closed');
		await goTo('/event/overview');
		await shot('overview-closed');
	},

	async settings() {
		await reset('example');
		await goTo('/event/settings');
		await shot('settings', { height: 900 });
		await page.getByRole('heading', { name: 'Plugins' }).scrollIntoViewIfNeeded();
		await shot('settings-plugins', { height: 900 });
		await button('Set up').first().click();
		await button('Save and test').scrollIntoViewIfNeeded();
		await shot('data-source', { height: 900 });
	},

	async drinks() {
		await reset('example');
		await goTo('/event/overview');
		await page.getByRole('navigation').getByText('Drinks').click();
		await shot('drinks', { height: 960 });
		await page.getByText('Club-Mate 0,5').first().click();
		await shot('product');
	},

	async tickets() {
		await reset('empty');
		await goTo('/event/tickets');

		await page.getByRole('button', { name: 'Tier', exact: true }).click();
		await page.getByLabel('Name', { exact: true }).fill('Early Bird');
		await page.getByLabel('Price').fill('25');
		await page.getByLabel('Quota').fill('50');
		await shot('tier');
		await save();
		await page.keyboard.press('Escape');
		await page.getByRole('button', { name: 'Tier', exact: true }).click();
		await page.getByLabel('Name', { exact: true }).fill('Regular');
		await page.getByLabel('Price').fill('32');
		await page.getByLabel('Fee').fill('2,5');
		await save();
		await page.keyboard.press('Escape');

		// The first sheet has a tier the event doesn't know yet, which blocks the import.
		await button('Import sheet').click();
		await page.waitForTimeout(500);
		await shot('import-unknown');
		await button('New tier').click();
		await page.getByLabel('Price').fill('45');
		await save();
		await page.getByLabel('Snapshot date').fill('01.06.2026');
		await page.waitForTimeout(300);
		await save('Import');

		// The second sheet has orders gone, and a row with no order ID, which is listed first.
		await button('Import sheet').click();
		await page.waitForTimeout(500);
		await shot('sheet-problems');
		await button('Review import').click();
		await page.getByLabel('Snapshot date').fill('08.06.2026');
		await page.waitForTimeout(300);
		await shot('import-conflict');
		await save('Import');
		await shot('tickets-conflicts', { height: 1000 });

		const card = page.getByRole('article').filter({ hasText: 'Order 4711' });
		await card.getByLabel('Refund %').fill('50');
		await card.getByLabel('Note').fill('Cancelled after the deadline');
		await card.getByRole('button', { name: 'Resolve' }).click();
		await page.waitForTimeout(300);
		const moved = page.getByRole('article').filter({ hasText: 'Order 4702' });
		await moved.getByRole('button', { name: 'Resolve' }).click();
		await page.waitForTimeout(300);
		await shot('tickets', { height: 1000 });

		await button('Manual sale').click();
		await page.getByLabel('Count').fill('3');
		await shot('manual-sale');
		await page.keyboard.press('Escape');
		await page.keyboard.press('Escape');
	}
};

const asked = process.argv.slice(2);
let failed = false;
try {
	for (const [name, walk] of Object.entries(walks)) {
		if (asked.length && !asked.includes(name)) continue;
		console.log(name);
		try {
			await walk();
		} catch (e) {
			failed = true;
			console.error(`  ${name} stopped: ${e.message.split('\n').slice(0, 4).join(' | ')}`);
		}
	}
} finally {
	await browser.close();
}
process.exit(failed ? 1 : 0);
