# Lancelot — Writing a plugin

A plugin adds records, screens and proposals for one kind of group's workflow. Plugins are built in: their code is in `app/src/lib/plugins/<id>/` and ships with the app, and their data is in the event file. The reasons behind this guide are in [technical design §10](technical-design.md#10-plugins), and the types are in [`api.ts`](../app/src/lib/plugins/api.ts), which is the source of truth.

## The rules

1. **The event works without you.** It opens and settles correctly with any plugin missing. So a plugin's data never takes part in a balance, a result or a closing check.
2. **No code in event files.** A file holds a plugin's data, never its code.
3. **Money moves only through what the treasurer confirms.** Propose an entry or a ticket batch. The normal form or review opens, and nothing is saved until the treasurer does.
4. **Use only `PluginContext`.** Import `plugins/api.ts`, shared components and formatting, never the session or the command API. `boundary.test.ts` enforces this, so that a sandboxed loader can run plugins later.
5. **Validate your own records.** The core checks a record's shape and stores its value as JSON. It doesn't know what the value means. A record that doesn't fit is ignored and kept, not deleted.

## Adding one

1. Create `app/src/lib/plugins/<id>/index.ts`, exporting a `Plugin`:

   ```ts
   import type { Plugin } from "../api";
   import Section from "./Section.svelte";

   export const example: Plugin = {
     id: "example", // Stored with its records, so it never changes.
     name: "Example",
     description: "One sentence for Settings: what it adds to the app.",
     BudgetSection: Section,
   };
   ```

2. Add it to `plugins` in `registry.ts`. It is on for every event until switched off in Settings → Plugins.
3. Keep figures and record parsing in plain `.ts` files, so they are easy to test.
4. Add a test beside it, and a line under Plugins in [features](features.md).

The app draws the frame around every slot, with the plugin's name and a "Plugin" tag, so a plugin can't look like part of the app. A panel from `setPanel` sits outside the frame.

## Slots

A plugin that is switched off for the event shows none of these, and its records stay in the file.

| Slot              | Shows                                                   | Shown when                                                     | Props                                                       |
| ----------------- | ------------------------------------------------------- | -------------------------------------------------------------- | ----------------------------------------------------------- |
| `CentreSection`   | On a cost centre's page                                 | `attachedTo(ctx, centre)` holds                                | `ctx`, `centre`, `setPanel`                                 |
| `TicketSection`   | On the Tickets page                                     | The event is open and its data source is set                   | `ctx`, `proposeImport`, `setPanel`                          |
| `BudgetSection`   | On the Budget page, under the forecast                  | The event is open and `onBudget(ctx)` holds                    | `ctx`                                                       |
| `CountSection`    | Under each amount in the Count panel                    | `onCount(ctx, account)` holds                                  | `ctx`, `account`, `which`, `amount`, `setAmount`, `onSaved` |
| `SettingsSection` | In the panel its Configure opens, above the data source | Always                                                         | `ctx`                                                       |
| `ReportSection`   | In the settlement report                                | `inReport(ctx)` holds, unless left out in the report's toolbar | `ctx`                                                       |

`onBudget`, `onCount` and `inReport` default to yes when left out. A count section fills in the amount with `setAmount` and saves nothing. To keep its own records, it registers a function with `onSaved`, which runs once the panel's Save has saved the count, so Cancel leaves nothing. A report section prints on the report's sheet, so it uses `pt` sizes, avoids breaking inside a row and changes nothing.

## The context

### What the event holds

`closed`, `format`, `currency`, `costCentres`, `accounts`, `txns`, `budget`, `budgetLines`, `assumptions`, `tiers` and `figuresOf(centre)` read the event as it is now. They are reactive, so read them in `$derived`. People's bank details and ticket orders are left out on purpose. `ticketOrderIds` says only whether the event's ticket imports have order IDs, or `null` before the first one with tickets: a ticket source builds its batch the same way, since one of the other kind is refused.

A closed event refuses changes, so hide what would change it when `ctx.closed` holds.

### Your records

`ctx.records` holds this plugin's records, and `ctx.apply(changes)` applies changes together, through the change log and backups like any other change. A `null` value removes a record.

The `scope` says what a template does with a record: `setup` is copied, like a price list, and `records` isn't, like counts. Templates keep ids, so a `setup` value may refer to accounts and cost centres.

### Proposing an entry

`ctx.proposeEntry` opens the normal form on the Ledger:

```ts
ctx.proposeEntry({
  kind: "income", // or 'expense' or 'transfer'
  counterparty: "SumUp",
  amount: 15000,
  date: "2026-06-13", // Optional, like the description and notes.
  destination: bank, // Optional: `payer` for an expense. The default account otherwise.
  lines: [
    { account: sales, costCentre: bar, amount: 10000 }, // An amount per line for a split.
    { account: other, costCentre: null, amount: 5000 },
  ],
});
```

A transfer takes `from` and `to` instead of `counterparty` and `lines`. A settle can't be proposed. Amounts are integer cents.

### Setting up categories

`createCostCentre`, `createCategory` and `setCategoryCentre` add to the event's setup, as Stock and sales does when it adds a cost centre for goods.

### Reading a file

Ask for a file with `chooseFile` or listen for one with `onFileDrop`, then read only the columns you need:

```ts
const path = await ctx.chooseFile("Bank statement", ["csv", "xlsx", "ods"]);
if (!path) return;
const table = await ctx.readTable(path, ["Buchungstag", "Betrag"]);
for (const { row, cells } of table.rows) {
  const [date, amount] = cells;
  const cents = await ctx.parseAmount(amount);
  const day = await ctx.parseDate(date);
}
```

- Nothing but the named columns reaches the plugin, so ask for as little as you can (R-TKT-17).
- The header is the first row holding all the columns, so lines above it don't matter. A column that isn't there rejects, naming it.
- Cells come in the order you asked for, and an empty one is `''`. Blank rows are left out.
- Rows with a spreadsheet error such as `#REF!` are in `table.errors`. List them, and any rows you can't use, rather than dropping them (R-TKT-4). `ticket-sheet` shows how.
- Number cells come in the app's number format and date cells as ISO, so `parseAmount` and `parseDate` read them. CSV cells come as written, and a CSV in another number format can misread, so show what was read before anything is saved.
- `table.sha256` identifies the content, and `table.source` is the file's name. Don't keep the path.

### Pulling from a provider

Declare `dataSource` on the plugin. The app draws the form in Settings, keeps the details on this computer or, when ticked, in the event file as the plugin's record `data-source` (`data-source:<tab key>` for a tab), keeps the token in the keychain, and makes the request in Rust. Don't use those record keys for anything else. `ctx.pull(fields)` returns only those fields, so a plugin never sees the token. `ctx.dataSource()` is the source's name, or `null` until it is set up.

A plugin that needs one source per thing it is attached to declares `sourceTabs`, as Stock and sales does with one per cost centre: the Configure panel gets a tab for each, and `ctx.dataSource(key)` and `ctx.pull(fields, key)` ask for the one with that tab's key. Keep the key stable, since it is what the source is saved under. A cost centre's name is the natural one; renaming the cost centre leaves its source under the old name, to be set up again.

A field marked `optional: true` may be left blank in Settings, when the provider doesn't have it, and missing from an item. An item without a required field is a problem and isn't read. One without an optional field is read without it, and listed in `missing` in case the plugin needs it after all. `pulled.fields` says which fields the source has, so a plugin can tell a provider without the field from items that lack it. Ticket API's Order field works this way. A provider's details never go in the repository, and its token never goes in an event file.

Each field also carries an `example`: what one value looks like, such as a product's name or a count. Under the form, the shape preview draws the list with one item, field by field, using the names as they are typed. Give a number for a count and a string for text, so it reads like the provider's JSON; an optional field left blank is left out of the item.

## Your own figures

Every figure of the core comes from the core, and the UI only formats it. A plugin's own figures are the one exception, allowed because they never affect the ledger. Keep them in integer cents, say so at the top of the file, and test them. `drinks/figures.ts` and `what-if/scenario.ts` are good examples.

## Design

Follow the [design notes](design/README.md): labels name things, and a hint line carries data or says what to type, never what a view is for. Use shared components, the design tokens and `formatAmount`. Add `data-testid="plugin-<id>-…"` to what a test needs to find.

## Testing

Render through the real page where the slot lives, with the example event, as `what-if/what-if.svelte.test.ts` does, or pass a stub context, as `ticket-sheet/sheet.svelte.test.ts` does. Check that proposing saves nothing, and that a closed event hides what would change it. `scripts/check.sh` runs everything, including the boundary test.
