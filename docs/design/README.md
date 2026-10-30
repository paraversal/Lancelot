# Lancelot — Design

Source of truth: `UI.fig` (OpenPencil, local only and gitignored). Tokens: [`tokens.css`](tokens.css), generated from the file's variables. In the app, the components are on the `/design` page of a development build (`pnpm dev`, then open `/design`).

## Character

The app should feel like a well-kept paper ledger at a community event. It is calm and warm, with a few hand-made touches: a highlighter for selection, tally marks for Striche, and a type face with a single-storey *a*. It is functional without feeling corporate.

## Ground rules

1. **No gradients, no shadows.** Depth comes from rules (1 px lines) and the sunk sidebar tone.
2. **No coloured edge accents** (the "fingernail" strip on the side of cards or rows).
3. **One type face: Familjen Grotesk, weights 400 and 600.** Sizes: 12, 14, 16, 22, 40.
4. **Show, don't tell.** Labels name things. No sentences that explain a view, no empty-state prose, no "Here you can…" hints. A hint line exists only when it carries data (e.g. `= 1.234,50 €` under an amount, `~9 tickets`).
5. **Rules and whitespace instead of cards.** A card is allowed only when it holds more than one figure and is a unit of work. Example: the open ticket conflict, which has its own controls.
6. **Inviting, a little quirky, never forced.** The quirks are the marker highlight, the tally marks and the warm paper tone.

## Colour semantics

| Token | Use |
|---|---|
| `paper` / `paper-sunk` | Content background / sidebar and segmented-control track |
| `ink` / `ink-muted` | Primary text and primary buttons / labels and secondary values |
| `rule` | All dividers and input borders |
| `marker` | **The one thing that is selected or active.** Nothing else. |
| `plus` / `minus` | Gains and losses only (results, income) |
| `attention` | Anything that needs doing (owes, missing receipt, conflicts) |
| `tier-1…5` | Data series: expense categories and ticket tiers. Fills only, never text. Income categories use `plus` for their dot. |
| `field` | Input fill |

Expenses are shown in ink with a leading `−`, income in `plus` with a leading `+`, and transfers in `ink-muted` without a sign.

## Layout

- Window 1280 × 800. Sidebar 220 px with event name and dates, navigation (counts on the right, a badge in `attention` when something needs doing), and the file name at the bottom, which opens Settings.
- Content padding 40. Spacing scale 4 · 8 · 12 · 16 · 24 · 32 · 40 · 48. Radius 4 / 6 / 8, plus pills for filter chips.
- A list or table on the left. Editing and detail happen in a **right-hand panel** (380 px) separated by a rule. There are no modals for routine work.
- Tables: 36–40 px rows, a hairline under each row, numbers right-aligned in fixed-width columns, and day headers in the ledger.

## Motion

Snappy, with motion that answers what a person did: presses, panels, amounts that move to their new value, a stamp when the event closes. The plan, its rules and the choice per idea are in [motion.md](motion.md); every strength can be tried at `/design/motion`.

## Components

| Component | Notes |
|---|---|
| Button | `primary` (ink), `ghost` (rule border), optional icon and shortcut hint (`⌘↵`, `N`) |
| Filter chip | Pill; active is an ink fill |
| Segmented control | Sunk track; active segment uses `marker` |
| Amount input | Lenient parsing with a live `= …` preview. Ambiguous input shows the preview in `attention`; invalid input gets a `minus` border and a warning icon. See R-NF-8. |
| Balance cell | `owed 212,40 €` / `owes 180,00 €` (attention) / `✓ settled` |
| Receipt indicator | Paperclip when present, `attention` warning when missing |
| Tally | Groups of four strokes crossed by a fifth (Strichliste) |
| Stacked bar | In vs. out composition, and ticket snapshots per import |

## Screens in UI.fig

| Screen | Content |
|---|---|
| System | Tokens, type scale, controls, input states, value styles, rules |
| Overview | Result, forecast and tickets; in/out composition; where the money is (reconciles to the result); open items |
| Ledger | Transactions grouped by day, with filters, plus the entry panel (type switch, amount, payer with live balance, split lines, receipt) |
| Ledger / Entry panels | Income (with a category being added), Transfer (delete confirmation), Settle (direction and amount from the person's balance) |
| Ledger / Empty | The first entry, with its payer added on the spot |
| Ledger / Receipt preview | A receipt in place of the table, with Open and Export; the panel stays |
| Ledger / Drop | Dragging a file over the window: a new expense, or attach to the open entry |
| Ledger / By category | The same list grouped by category with a total per category; a split transaction appears under each of its categories, tagged `SPLIT` |
| People | Advanced / holds / settled / net balance per person, payout list export, person panel with a running balance and the settle action |
| People / Payouts | The payout list (IBAN, reference, amount, total) and the repayment list with Repaid; PDF, CSV, Mark as paid |
| People / Mark as paid | The reimbursements to record, people without an IBAN, the account and the date |
| People / Edit person | The person's details in the panel; the IBAN previews its grouped form while typing |
| Tickets | Conflict resolution (refund %, match, correction), tiers with quota and fee, break-even, sales per import snapshot |
| Tickets / Statistics | The states of Break-even (too few places, no tiers, only free tiers, reached), Sales by revenue, and both under the tiers while a panel is open |
| Tickets / Import | The sheet in the right-hand panel: file and hash, the columns read, the snapshot date, an unknown tier to map or add, the changes per tier (removals lead to conflicts) and the rows not read; Import stays blocked while a tier is unmapped |
| Tickets / Edit tier | Name, price, fee (`= 0,80 € per ticket`), quota and the sheet's names for the tier; sold, fees and net; a tier in use cannot be deleted |
| Tickets / Manual sale | Tier, count, the amount, who received the money (with their balance before and after), date and reference; the earlier manual sales |
| Tickets / Empty | No tiers yet: Import sheet and `+ Tier`; Manual sale is disabled |
| Drinks | Drinks result, per-product commission records, Strichliste with tallies and unpaid amounts |
| Start | Recent events (name, dates, path, last opened; `Not found` for moved files), New event and Open |
| Start / New event | Name, dates (with the `= 06.12.2026` preview) and location in the right-hand panel |
| Start / States | An event open in another window, and a damaged file with its issues and backups to restore |
| Settings | Event details and the reimbursement account; number and date format; file path and integrity check; backups with Restore |
| Event / Integrity banner | One line per issue above the page, with Check again |

In the app, the entry panel's last text field is **Description** (shown in the ledger row), where the Ledger frame says *Note*.

The app's Tickets page follows the Tickets frames. Where it differs: the Edit tier panel saves with **Save ⌘↵** instead of Done, like the other panels; "New tier" for an unknown name in the Import panel opens the tier panel and returns to the import once saved; the header's import line opens the earlier imports with their snapshot dates; the totals also show the ticket fees.

Sales draws places against the quota, so its label matches the Sold column, with the cancelled on top in `on-ink-muted`, a neutral fill that belongs to no tier. The line appears only when every tier has a quota. Revenue is net income; only its last column carries an amount, and the others have theirs in the tooltip. Date labels thin out when they no longer fit, and the last always shows. Below 920 px of content, Break-even and Sales move under the tiers.

The conflict card has more fields than the frame, which only shows the refund and the payer: the count of a partly resolved conflict, the date and a note (R-TKT-13), as labelled fields under the switch. **Match** is offered only when a cancellation is waiting; then the card opens on it, with **Confirm** and "Resolve another way". The default refund is 100 % (R-TKT-13), where the frame shows 50 %. Not in the frames: a warning under the totals while conflicts are open (R-TKT-16), and a **Cancellations** list under the tiers with `+ Cancellation`, which opens a right-hand panel (R-TKT-14). The navigation's Tickets count turns into a badge while conflicts are open.

The app's Overview follows the Overview frame, whose hero has the result, the forecast and the tickets. Where it differs: Income and Expenses stay in the hero unless both the forecast and the tickets are there; the Tickets figure counts places, like the Tickets page, without a quota or bar when a tier has none; the forecast's note is the plan once there is no gap to close, and the break-even reads `~9 tickets to break even`, with the places left in `attention` when they fall short.

The app's Drinks page follows the Drinks frame without the Strichliste, its Sync button and the `% of Striche` under Donations, which come with M11; the records take the full width. Where it differs: `+ Product` beside Supplier invoice, a panel per product and a panel for the supplier and the price per Strich, opened from the caption, a row with the kept units and their cost, the cost per drink beside the price per Strich, and category names in the Cost note as they are in the event. A difference on the invoice reads `11,50 € more than the records` in `attention`. Not in any frame: the entry panel's and the budget panel's Cost centre field, Cost centres on the Budget page and in the report, and the Cost centres section in Settings.

Budget is not designed yet. Until it is, the app's Budget page follows the People page: a table of estimate, actual and forecast with a right-hand panel per budget line.
