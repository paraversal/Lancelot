# Lancelot — Design

Source of truth: `UI.fig` (OpenPencil, local only and gitignored). Tokens: [`tokens.css`](tokens.css), generated from the file's variables. In the app, the components are on the `/design` page of a development build (`pnpm dev`, then open `/design`).

**Install Familjen Grotesk before opening `UI.fig`:** `brew install --cask font-familjen-grotesk`. Without it, OpenPencil downloads Google Fonts subsets that hold only the characters it needs, with their glyphs renumbered, and text comes out garbled ("Budget" as "Bt dgeß"). Saved outlines can carry that into the file. If it happens: quit OpenPencil, run `scripts/openpencil-purge-font.py "Familjen Grotesk"` to drop the cached subsets, then `node scripts/openpencil-reshape-text.mjs UI.fig` to have every text layer shaped again, open the file and save it once.

## Character

The app should feel like a well-kept paper ledger at a community event. It is calm and warm, with a few hand-made touches: a highlighter for selection, tally marks, and a type face with a single-storey _a_. It is functional without feeling corporate.

## Ground rules

1. **No gradients, no shadows.** Depth comes from rules (1 px lines) and the sunk sidebar tone.
2. **No coloured edge accents** (the "fingernail" strip on the side of cards or rows).
3. **One type face: Familjen Grotesk, weights 400 and 600.** Sizes: 12, 14, 16, 22, 40.
4. **Show, don't tell.** Labels name things. No sentences that explain a view, no empty-state prose, no "Here you can…" hints. A hint line exists only when it carries data (e.g. `= 1.234,50 €` under an amount, `~9 tickets`), or when the words alone don't say what to type (the counts of a drinks product, the deposit category).
5. **Rules and whitespace instead of cards.** A card is allowed only when it holds more than one figure and is a unit of work. Example: the open ticket conflict, which has its own controls. A plugin always sits in its own card: white, a rule around it, radius 16, its name and a "Plugin" tag in the header. That rounder corner belongs to plugins alone, so they read as added to the app rather than part of it.
6. **Inviting, a little quirky, never forced.** The quirks are the marker highlight, the tally marks and the warm paper tone.

## Colour semantics

| Token                  | Use                                                                                                                   |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `paper` / `paper-sunk` | Content background / sidebar and segmented-control track                                                              |
| `ink` / `ink-muted`    | Primary text and primary buttons / labels and secondary values                                                        |
| `rule`                 | All dividers and input borders                                                                                        |
| `marker`               | **The one thing that is selected or active.** Nothing else.                                                           |
| `plus` / `minus`       | Gains and losses only (results, income)                                                                               |
| `attention`            | Anything that needs doing (owes, missing receipt, conflicts)                                                          |
| `tier-1…5`             | Data series: expense categories and ticket tiers. Fills only, never text. Income categories use `plus` for their dot. |
| `field`                | Input fill                                                                                                            |

Expenses are shown in ink with a leading `−`, income in `plus` with a leading `+`, and transfers in `ink-muted` without a sign.

## Layout

- Light or dark: Settings → Display, following the system by default. The dark values are in `app/src/lib/styles/theme.css`, not in `UI.fig`. Print and the report's sheet stay light.
- Window 1280 × 800. Sidebar 220 px with event name and dates, navigation (counts on the right, a badge in `attention` when something needs doing), and at the bottom History and the file name, which opens Settings.
- Content padding 40. Spacing scale 4 · 8 · 12 · 16 · 24 · 32 · 40 · 48. Radius 4 / 6 / 8, plus pills for filter chips.
- A list or table on the left. Editing and detail happen in a **right-hand panel** (380 px) separated by a rule. There are no modals for routine work.
- Tables: 36–40 px rows, a hairline under each row, numbers right-aligned in fixed-width columns, and day headers in the ledger.

## Motion

Snappy, with motion that answers what a person did: presses, panels, amounts that move to their new value, a stamp when the event closes. The plan, its rules and the choice per idea are in [motion.md](motion.md); every strength can be tried at `/design/motion`.

## Components

| Component         | Notes                                                                                                                                                                                                         |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Button            | `primary` (ink), `ghost` (rule border), optional icon and shortcut hint (`⌘↵`, `N`)                                                                                                                           |
| Filter chip       | Pill; active is an ink fill                                                                                                                                                                                   |
| Segmented control | Sunk track; active segment uses `marker`                                                                                                                                                                      |
| Amount input      | Lenient parsing with a live `= …` preview, which also adds up sums such as `1,29+1,49`. Ambiguous input shows the preview in `attention`; invalid input gets a `minus` border and a warning icon. See R-NF-8. |
| Balance cell      | `owed 212,40 €` / `owes 180,00 €` (attention) / `✓ settled`                                                                                                                                                   |
| Receipt indicator | Paperclip when present, `attention` warning when missing                                                                                                                                                      |
| Tally             | Groups of four strokes crossed by a fifth (a tally sheet). In `UI.fig` only; the app doesn't draw it.                                                                                                           |
| Stacked bar       | In vs. out composition, and ticket snapshots per import                                                                                                                                                       |

## Screens in UI.fig

| Screen                   | Content                                                                                                                                                                                                                                      |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| System                   | Tokens, type scale, controls, input states, value styles, rules                                                                                                                                                                              |
| Overview                 | Result, forecast and tickets; in/out composition; where the money is (reconciles to the result); open items                                                                                                                                  |
| Ledger                   | Transactions grouped by day, with filters, plus the entry panel (type switch, amount, payer with live balance, split lines, receipt)                                                                                                         |
| Ledger / Entry panels    | Income (with a category being added), Transfer (delete confirmation), Settle (direction and amount from the person's balance)                                                                                                                |
| Ledger / Empty           | The first entry, with its payer added on the spot                                                                                                                                                                                            |
| Ledger / Receipt preview | A receipt in place of the table, with Open and Export; the panel stays                                                                                                                                                                       |
| Ledger / Drop            | Dragging a file over the window: a new expense, or attach to the open entry                                                                                                                                                                  |
| Ledger / By category     | The same list grouped by category with a total per category; a split transaction appears under each of its categories, tagged `SPLIT`                                                                                                        |
| People                   | Advanced / holds / settled / net balance per person, payout list export, person panel with a running balance and the settle action                                                                                                           |
| People / Payouts         | The payout list (reimbursement method, reference, amount, total) and the repayment list with Repaid; PDF, CSV, Mark as paid                                                                                                                                  |
| People / Mark as paid    | A tick box per reimbursement to record, people with no method, the account and the date                                                                                                                                                               |
| People / Edit person     | The person's details in the panel; Reimburse by switches between bank details and a free-text method; the IBAN previews its grouped form while typing                                                                                                                                                           |
| Tickets                  | Conflict resolution (refund %, match, correction), tiers with quota and fee, break-even, sales per import snapshot                                                                                                                           |
| Tickets / Statistics     | The states of Break-even (too few places, no tiers, only free tiers, reached), Sales by revenue, and both under the tiers while a panel is open                                                                                              |
| Tickets / Import         | The sheet in the right-hand panel: file and hash, the columns read, the snapshot date, an unknown tier to map or add, the changes per tier (removals lead to conflicts) and the rows not read; Import stays blocked while a tier is unmapped |
| Tickets / Edit tier      | Name, price, fee (`= 0,80 € per ticket`), quota and the sheet's names for the tier; sold, fees and net; a tier in use cannot be deleted                                                                                                      |
| Tickets / Manual sale    | Tier, count, the amount, who received the money (with their balance before and after), date and reference; the earlier manual sales                                                                                                          |
| Tickets / Empty          | No tiers yet: Import sheet and `+ Tier`; Manual sale is disabled                                                                                                                                                                             |
| Drinks                   | Drinks result, per-product commission records, tally sheet with tally marks and unpaid amounts                                                                                                                                                   |
| Start                    | Recent events (name, dates, path, last opened; `Not found` for moved files), New event and Open                                                                                                                                              |
| Start / New event        | Name, dates (with the `= 06.12.2026` preview) and location in the right-hand panel                                                                                                                                                           |
| Start / States           | An event open in another window, and a damaged file with its issues and backups to restore                                                                                                                                                   |
| Settings                 | Event details and the reimbursement account; number and date format; file path and integrity check; backups with Restore                                                                                                                     |
| Event / Integrity banner | One line per issue above the page, with Check again                                                                                                                                                                                          |

In the app, the entry panel's last text field is **Description** (shown in the ledger row), where the Ledger frame says _Note_.

The app's Tickets page follows the Tickets frames. Where it differs: the Edit tier panel saves with **Save ⌘↵** instead of Done, like the other panels; "New tier" for an unknown name in the Import panel opens the tier panel and returns to the import once saved; the header's import line opens the earlier imports with their snapshot dates; the totals also show the ticket fees, and **Other income in the ledger** when income was typed into the Tickets category by hand, which the Ticket income total includes. In the Import panel, an order's conflict sits on its own line under the tier.

Sales draws places against the quota, so its label matches the Sold column, with the cancelled on top in `on-ink-muted`, a neutral fill that belongs to no tier. The line appears only when every tier has a quota. Revenue is net income; only its last column carries an amount, and the others have theirs in the tooltip. Date labels thin out when they no longer fit, and the last always shows. Below 920 px of content, Break-even and Sales move under the tiers.

The conflict card has more fields than the frame, which only shows the refund and the payer: the count of a partly resolved conflict, the date and a note (R-TKT-13), as labelled fields under the switch. **Match** is offered only when a cancellation is waiting; then the card opens on it, with **Confirm** and "Resolve another way". The default refund is 100 % (R-TKT-13), where the frame shows 50 %. Not in the frames: a warning under the totals while conflicts are open (R-TKT-16), and a **Cancellations** list under the tiers with `+ Cancellation`, which opens a right-hand panel (R-TKT-14). The navigation's Tickets count turns into a badge while conflicts are open.

The app's Overview follows the Overview frame, whose hero has the result, the forecast and the tickets. Where it differs: Income and Expenses stay in the hero unless both the forecast and the tickets are there; the Tickets figure counts places, like the Tickets page, without a quota or bar when a tier has none; the forecast's note is the plan once there is no gap to close, and the break-even reads `~9 tickets to break even`, with the places left in `attention` when they fall short.

The sidebar's Cost centres section, with a separator, a label and one item per centre, is in no frame. Each centre's page reuses the Drinks hero and table rows for its result, costs, income and tagged lines. A split line in the entry panel keeps its Budget line and Cost centre side by side under its category, with a rule between lines, so each line's details read as its own. Its label says Category until there is a second line, then Split. Budget line always shows and also lists the other categories' lines, so a treasurer who thinks of the line first can start there; choosing one sets the category.

Not in any frame: the entry panel's Notes and free Transfer fields, the Sells tickets and Net movement settings, the entry panel's and the budget panel's Cost centre field, Cost centres in the report, and the Cost centres and Categories sections in Settings.

Settings has its two columns, then two sections across the full width. An ink rule opens each section, so it reads apart from the hairlines between rows. **Categories** flows over two columns, expenses then income, with a category's cost centre beside it, and shows no choice while the event has no cost centres. A category's Delete shows when its row is hovered or focused. **Plugins** comes last under a double rule, with a larger heading and its cards in two columns.

The Drinks frame predates the drinks plugin and logging each drink, so the plugin departs from it. **Stock and sales** sits in its plugin card on a cost centre's page, with no tally sheet or tally marks. A paper list is one product, `Tally`. Its table has Billed, Left over, Logged, Gap, Cost and Expected per product, and drops columns beside a panel. Its caption opens a panel for the supplier and the deposit category, and beside it are the cost per drink, Pull logged, Book invoice and `+ Product`. Under the table are expected against collected and the invoice check, where a difference reads `11,50 € more than the records` in `attention`.

Plugins are in no frame beyond the card in the ground rules. Settings → Plugins has a card with an On switch per plugin, and under it the plugin's settings and its data source form: label, URL, header, token, list and fields, with Save and test. On the Tickets page, **Ticket sheet** and **Ticket API** each have a card that starts an import, and the Import frame's panel, called Review import and with the ticket account added, comes after that. Naming the sheet's columns happens in the plugin's own panel. Appearance under Display is in no frame either.

History is in no frame. It sits apart from the pages at the bottom of the sidebar, and ⌘1–9 skip it, since it's where you look when something seems off rather than a place you work. It follows the ledger's day headers and time column, one block per command, and each row shows the fields that moved as `old → new`.

Reconciliation and templates are in no frame. **Statement** on each account in Where the money is opens a right-hand panel like Count: date and amount (`Net movement` for the Bank account, `Statement shows` otherwise) over the account's checkpoints with the books' figure and any difference. The note beside the account shows its latest mismatch, else its latest checkpoint. The New event panel has a **Template** field, with the counts of what comes along once an event is chosen, and Settings has a **Copy from another event** section with what is new and what is already here.

Budget is designed on the file's **Update** page, apart from the older frames on Page 1, and the app's page follows it. Where it differs: the panel uses the app's shared fields, so the category's kind sits under its field, the segmented control is as wide as its options and Delete is next to Save. A closed event, in no frame, shows the result and the plan in the hero.

The hero has the forecast, the plan with how far the forecast is from it, the result so far and the three lines furthest from plan, which go beside a panel. One column header stays at the top while scrolling, over Income, Expenses, Result and Cost centres, each opened by an ink rule with its totals on the same row. Under them, categories carry the dot they have on the Overview. Each line has a bar of actual against estimate in its category's colour, in `attention` once it is over, and the bars also go beside a panel. A forecast that only repeats the estimate is in `ink-muted`, so the figures already decided by real amounts stand out. While nothing is booked, the hero shows the plan with income and expenses instead. A category's `+` appears on hover and starts a line in that category. With no lines, the page has **Copy from another event**, which leads to that section in Settings, next to `+ Budget line`.

| Frame              | Content                                                                                       |
| ------------------ | --------------------------------------------------------------------------------------------- |
| Budget             | During the event: hero, column header, Income and the start of Expenses                       |
| Budget / Edit line | A line over its estimate selected; figures, quantity × price, Complete and the linked entries |
| Budget / Planning  | Before the event: no actuals, a new line being added from a category                          |
| Budget / Empty     | No lines yet                                                                                  |
| Budget / Scrolled  | The end of the page: the column header held at the top, Result and Cost centres               |
