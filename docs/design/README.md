# Lancelot — Design

Source of truth: `UI.fig` (OpenPencil, local only and gitignored). Tokens: [`tokens.css`](tokens.css), generated from the file's variables. In the app, the components are on the `/design` page of a development build (`pnpm dev`, then open `/design`).

## Character

The app should feel like a well-kept paper ledger at a community event. It is calm and warm, with a few hand-made touches: a highlighter for selection, tally marks for Striche, and a type face with a single-storey *a*. It is functional without feeling corporate.

## Ground rules

1. **No gradients, no shadows.** Depth comes from rules (1 px lines) and the sunk sidebar tone.
2. **No coloured edge accents** (the "fingernail" strip on the side of cards or rows).
3. **One type face: Familjen Grotesk, weights 400 and 600.** Sizes: 12, 14, 16, 22, 40.
4. **Show, don't tell.** Labels name things. No sentences that explain a view, no empty-state prose, no "Here you can…" hints. A hint line exists only when it carries data (e.g. `= 1.234,50 €` under an amount, `9 tickets to break even`).
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
| Ledger / By category | The same list grouped by category with a total per category; a split transaction appears under each of its categories, tagged `SPLIT` |
| People | Advanced / holds / settled / net balance per person, payout list export, person panel with a running balance and the settle action |
| Tickets | Conflict resolution (refund %, match, correction), tiers with quota and fee, break-even, sales per import snapshot |
| Drinks | Drinks result, per-product commission records, Strichliste with tallies and unpaid amounts |
| Start | Recent events (name, dates, path, last opened; `Not found` for moved files), New event and Open |
| Start / New event | Name, dates (with the `= 06.12.2026` preview) and location in the right-hand panel |
| Start / States | An event open in another window, and a damaged file with its issues and backups to restore |
| Settings | Event details and the reimbursement account; number and date format; file path and integrity check; backups with Restore |
| Event / Integrity banner | One line per issue above the page, with Check again |

In the app, the entry panel's last text field is **Description** (shown in the ledger row), where the Ledger frame says *Note*.

Budget is not designed yet.
