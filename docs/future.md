# Lancelot — Future

Longer-term ideas and larger changes that are not planned yet. Nothing here is committed to: an item moves to the [roadmap](roadmap.md) when a real need appears. Until then, the rule is to keep the way open, not to build ahead.

## Ticket sources other than a sheet

Today tickets come from a sheet or a data source, each a plugin. Other groups may have only totals. The refactoring made tickets optional and removed the fixed main bank account, so no group is stuck with this workflow. Other sources can then follow without a redesign.

**The seam already exists.** `Book::preview_import` and `Book::import_tickets` take a `TicketBatch`, which is a label, a hash and one tier and order ID per ticket. Everything after that (conflicts, generated income, statistics) knows nothing about files. A new source only has to build a `TicketBatch`.

| Source                | What it would take                                                                                                                                                                              |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rows with no order ID | The core requires one. Conflicts are matched by (order ID, tier), so without it a removal cannot be traced to an order. Making it optional means turning conflict detection off for that event. |
| Totals per tier only  | No orders at all, so no conflicts. Either manual sales cover it already, or a small form sets a count per tier.                                                                                 |

## More currencies

An event has one currency, set when it is created (R-COR-7). The core supports EUR, USD, GBP and CHF, and the New event list shows the other ISO codes greyed out. Another currency with two decimals takes a line in `Currency` and its symbol in `format.ts`. The arithmetic doesn't care: `Cents` is a whole number of the smallest unit.

A currency with none or three decimals, such as the yen or the Kuwaiti dinar, means formatting and the amount parser have to stop assuming two, including how they read an ambiguous `1.234`, so it is the first real test. Converting between currencies stays out. The payment reference on a bank transfer is German whatever the currency, which translating the app would cover.

The Cash box count below is the first plugin that would need the currency's denominations, so the two can share a list.

## Payouts as a plugin

The payout list could become an optional plugin, listing each person's method whatever it is. Reimbursement methods are in the core for now: a bank transfer or free text.

## Plugins

Proposals for plugins, each only when wanted. The seams they need are named; `readTable`, wider proposals and the report slot exist already.

### Cash box count

Count notes and coins by denomination at opening and closing, and get the total without a calculator.

- **Where:** in the panel that already counts an account (Overview → Where the money is → Count), under each amount field and only for a cash account. Counting is that panel's job, the total feeds its Amount field, and "Matches the books" appears right there. A page of its own would be one more place to look on a busy night, and a section on the Overview would crowd it.
- **Seam:** a `CountSection` slot, with `ctx`, the account, which count it is, and `setAmount(cents)`. It fills in the field and saves nothing, so the panel's Save stays the one action and the core's count stays the real one.
- **Kept:** the pieces counted, per account and count, in `records` scope, so a later look shows what was in the box. The denominations are in `setup` scope, so a template copies them and a group can leave out what it doesn't use.
- **Currencies:** a built-in euro list first, with the list as data, so another base-10 currency only needs its own values.

### Card and PayPal settlements

Pull a payout from SumUp, Stripe or PayPal, or read its export, and book it as gross income and fees rather than the net amount that reaches the bank.

- **Why:** card sales at a bar arrive days later, in batches, minus a fee. Booked net, the income is understated and the fee disappears, so the bank statement matches but the result doesn't explain itself.
- **Reading:** a data source for providers with an API, and `readTable` for an export. Both exist.
- **Seam:** a payout is an income and a fee expense, while `proposeEntry` opens one form. Either proposals become a queue that "Save & next" walks through, or the core gains the "income net of a fee" form from [New entry forms](#new-entry-forms). The second is simpler for the treasurer, a single entry with one receipt, but it is a change to the core.
- **Kept:** a hash per payout in `records` scope, so pulling twice doesn't propose the same one again. No customer data: only date, gross, fee and payout ID.

### Where your ticket went

A one-page poster showing how a ticket's price was spent, for the group's wall or newsletter: "Of your 32 €: 11,20 € venue, 8,40 € artists, 5,10 € kitchen…". It builds trust, and it shows only totals.

- **Where:** on the Report page, as a second document. Overview → Report gets a switch in its toolbar, Settlement or Where your ticket went, and printing works as it does now. It stays out of the settlement itself, which is the treasurer's proof with every entry and shouldn't turn into a poster, and out of the Overview, which is for working.
- **Seam:** a `Printout` slot next to `ReportSection`: a whole sheet with a name for the switch. Unlike every other slot, it can't wear the plugin frame, which would look wrong on a poster. Instead the toolbar marks it with the "Plugin" tag, and the rule that a plugin can't look like part of the app holds on screen.
- **Figures:** the categories' actuals, scaled to one ticket, and the tiers' prices. Neither is on the context yet, so it needs `ctx.budget` and `ctx.tiers`. The plugin's own figures are rounded so that they add up to the price exactly.
- **Questions to settle:** which ticket to show, the average or a chosen tier (the average is the honest default); what to do with other income such as drinks, which pays for part of the costs (show the share the tickets carried); how to say a loss or a surplus ("the rest went to next year" may not be true, so the treasurer names it); and whether it can print before the event closes (yes, marked "so far").

## Third-party plugins

Plugins are built in today (technical design §10). When another group wants its own, a loader follows: JavaScript in a sandboxed iframe, with a manifest of permissions, the context over messages, and network access through the data sources. The first step is testing that an iframe can't reach Tauri's commands. The Drinks plugin should then need little change.

## Data sources in the event file

A data source is set up per computer: its details are in `data-sources.json` in the app's data folder and its token in the keychain, so a shared event file can't pull until the source is set up again. Storing the details in the `.lancelot` file would let them travel with the event, but anyone given the file could read the provider's URL and field names. The token stays in the keychain either way.

## Motion trial

`lib/motion/choices.ts`, every strength of about 25 effects and the `/design/motion` trial page stay for now, so animations can still be tried out. Once the choices feel settled, keep the chosen effects and a full-or-reduced switch, and remove the rest (about 1,100 lines). Git keeps them. This changes nothing visible.

## Receipts before entries

`attachment.txn_id` is `NOT NULL`, so a receipt cannot exist before its entry. A stack of unbooked receipts would mean making it nullable and adding an open item for unassigned ones. Cheap, but a new feature, so it waits for real demand.

## New entry forms

A new form is one `Entry` variant, with no change to the load path. Candidates, each only when needed: a supplier credit note, a second payer on one receipt, and income net of a fee.

## Settles recorded by Mark as paid

Mark as paid (R-PER-5) writes ordinary settles, so nothing tells them apart from one typed by hand, and they can be edited like any other entry (R-TX-6). That stays: a settle records a transfer that already happened, and it has to be correctable when the record is wrong. The change log keeps every edit, and the payout list follows the new balance.

To guard against an accidental edit, every saved settle says that changing it corrects the record, not the transfer. If that turns out to be too vague, the entry can name its source: "Recorded by Mark as paid on 13.06." It needs a new `Origin` that remembers where the entry came from, which is a change to the file format.

## Architectural choices to revisit

Choices that work today but could shape later development. Each one says what would make it worth changing.

### Forms wait for field reads with no timeout

`AmountField` and `DateField` read every keystroke through the core, so a form waits for `amountsRead()` and `datesRead()` before it saves. Both wait until no read is in flight, with no timeout, and they wait for every field in the app, not only the form's own. If a read never returned, Save would do nothing and show no message, in every form that waits, until the app restarts.

This is acceptable while reads are local calls that return quickly. Revisit it if reading could ever hang, for example if parsing moved behind a network call or a plugin, or as soon as a stuck Save is reported. The likely change is a time limit that shows an error and refuses to save, since saving without the read would bring back the wrong value this wait prevents.

Each form also has to remember the wait and its `invalid` flags. If new forms keep forgetting them, a shared form helper that does both would be worth the larger change.

## Decided against

These do not fit a tool for one treasurer, and the changes above cover the realistic cases more simply: configurable workflows and event sourcing. Multi-event, multi-user and compliance features stay out of scope (requirements §1).

**Receipt OCR** was tried on 2026-11-01 with Donut and with small local vision models. It wasn't accurate enough, and loading a model and waiting for the answer takes far longer than typing the number. Receipts aren't itemised here either, so there is little to read beyond the amounts, and sums in amount fields (R-NF-8) cover adding them up.
