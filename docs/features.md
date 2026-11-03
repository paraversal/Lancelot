# Lancelot — Features

What the app can do today. IDs in brackets lead to the [requirements](requirements.md), and what is planned is in the [roadmap](roadmap.md). A change that adds or removes a feature updates this list in the same commit.

No real event has been run in the app yet. That is the next test on the roadmap: an event alongside the Excel sheet, reaching the same final balance.

## Event files

- Create an event with a name, dates, location, notes and its accounts, and edit them later (R-EVT-1).
- Each event is one `.lancelot` file holding everything, receipts included (R-ISO-1, R-ATT-2, R-NF-4).
- The start screen lists recent events, with New and Open.
- A file that is already open elsewhere or damaged says so, instead of failing silently.
- Every open and save checks the file's integrity, and Settings can run the check on demand. Problems show in a banner (R-COR-5).
- Backups are kept automatically, the number kept is a setting, and Settings can restore one (R-NF-5).
- Start a new event from an existing one as a template, which copies the setup but no money (R-EVT-3).
- Copy people and budget lines from another event into an open one. It never overwrites, and the other file is only read (R-ISO-4).

## Entering money

- Four forms: expense, income, transfer between any two accounts, and settling a person (R-TX-3).
- One receipt can be split over several categories, and the lines must add up exactly (R-TX-2).
- Each line can link to a budget line and carry a cost centre. Choosing a budget line sets the line's category and cost centre (R-BUD-2, R-CC-1).
- People and categories can be added from the form itself.
- Entries can be edited and deleted until the event is closed. Every change is written to a change log, which the History page shows with its before and after values and a search (R-TX-6).
- Amount fields accept both number formats, add up sums like `1,29+1,49`, and show what they read before saving (R-NF-8).

## Ledger

- Every entry, grouped by day or by category with totals (R-TX-7).
- Filter by type, search, and show only the expenses with no receipt (R-ATT-4).
- Works from the keyboard: new entry, moving through the list, saving, and saving and starting the next (R-NF-7).

## Receipts

- Any number of files per entry: PNG, JPG, HEIC, PDF, or anything else, stored as it is (R-ATT-1).
- Preview in the app, open in another app, or export (R-ATT-3).
- Drop a file on the window to start an expense with it, or on an open entry to attach it (R-ATT-5).

## Accounts and categories

- Bank and cash accounts, named when an event is created and added or renamed in Settings. An event always has at least one. An account can count net movement only, for one whose real total doesn't matter (R-ACC-1, R-ISO-3).
- Income and expense categories, which can name the cost centre their new lines start in (R-ACC-2).
- Opening and closing counts per account, with the difference shown (R-ISO-3).
- Statement checkpoints: enter what a statement showed on a date, and any mismatch becomes an open item (R-ACC-4).
- Accounts and people that nothing refers to can be deleted.

## People and payouts

- A reimbursement method for each person: a bank transfer with the IBAN checked as you type, or another way in free text (R-PER-1, R-PER-2).
- Per person: advanced, held, settled and net balance, with each line leading to its entry (R-PER-3, R-TX-5).
- A settle shows the balance it leaves while you type, whether it is a new entry or a saved one being changed, and marks an amount that is too much (R-TX-4).
- A saved settle says that changing it corrects the record, not the transfer (R-TX-6).
- Payout list with reimbursement method and amount, and account holder and reference for a bank transfer, as CSV or PDF, with the advances behind it as an optional appendix (R-PER-4).
- Record the whole payout list as paid in one action (R-PER-5).
- Repayment list of the people who owe the event, with Repaid per person (R-PER-7).

## Budget

- Budget lines as an amount or as quantity × unit price, each of which can be marked complete (R-BUD-1).
- Plan against actual per line, category and cost centre, and in total, with what isn't budgeted (R-BUD-3).
- A cautious forecast of the result, on the Budget page and the Overview (R-BUD-4).
- How far the forecast is from the plan, and the three lines furthest from it, each opening the line.
- A bar per line of actual against estimate, and a forecast that only repeats the estimate is muted.
- Start a line from its category, and copy from another event while there are no lines yet.

## Tickets

Optional per event, and switched off in Settings while the event has no tiers (R-TKT-19).

- Tiers with a price, fee, quota and aliases (R-TKT-1, R-TKT-2, R-TKT-10).
- Income per tier generated from the imported tickets, read-only in the ledger, with fees as their own line (R-TKT-6).
- Every import goes through a review that shows what changed. A tier it doesn't know blocks the import until it is mapped (R-TKT-4, R-TKT-5).
- Each import is kept with its date, which can be changed. An import can be deleted, with a preview of what that changes (R-TKT-8).
- Manual sales for tickets sold outside the regular channels (R-TKT-7).
- A removed ticket becomes a conflict, grouped by order, with a badge in the navigation. It is resolved as a refund of 0–100 %, a match to a cancellation, or a correction, wholly or in part (R-TKT-12, R-TKT-13, R-TKT-16).
- Manual cancellations on an order or a manual sale, and a choice of who pays the refund (R-TKT-14, R-TKT-15).
- A sales chart over the imports, by places or net income, against the quota (R-TKT-8).
- Break-even: the amount missing and about how many tickets close it, overall and per tier, with the places left (R-TKT-9).
- Income typed into the Tickets category by hand shows as its own line, counted in Ticket income and leading to the category in the ledger. It adds no tickets to the tiers.

## Cost centres

- Add, rename and delete them in Settings. A new event has none (R-CC-2).
- Each has a page in the sidebar with income, expenses, result and its tagged lines (R-CC-1).

## Plugins

Built-in features switched per event in Settings. Each shows in a marked frame, and switching one off hides it and keeps its data in the file.

**Stock and sales**, for drinks or other goods from a supplier, attached to any cost centre:

- A new cost centre for goods can be added from its settings, with an income and an expense category of its name and a Deposit category.
- Per cost centre: the supplier, whether the goods are on commission or bought outright, and an optional deposit category (R-DRK-1, R-DRK-2).
- Per product: cost, donation, and units taken, returned (only on commission), left over and logged (R-DRK-5).
- Billed units, the gap against what was logged, and the cost per unit gone (R-DRK-5, R-DRK-9).
- Expected donations against what was collected. A paper tally is one product, `Tally` (R-DRK-7).
- A table per cost centre in the settlement report, which the report's toolbar can leave out.
- An invoice check, which leaves out the deposit category, and Book invoice, which opens the normal expense form filled in (R-DRK-2, R-DRK-8).
- Pull logged counts from a data source, reviewing the changes first. A name that matches no product must be mapped once and is then remembered (R-DRK-6). Not yet tried with a real provider.

**Ticket sheet** imports `.ods`, `.xlsx` or `.csv`. It reads only the columns you name, even with other lines above them, and the rest of the file is never stored (R-TKT-3, R-TKT-17).

**Ticket API** pulls tickets from a data source into the same review (R-TKT-3). Not yet tried with a real provider.

**Data sources** are set up with a form under the plugin, with Save and test. The details stay on this computer and the token in the system's keychain, so neither is in the event file. Pulling is the only use of the network (R-NF-3). Saving a token and pulling in the desktop app hasn't been fully tested yet.

## Settling and closing

- The Overview shows the result, money in and out by category, where the money is, the forecast and the tickets (R-SET-1, R-SET-4).
- Open items, each leading to its fix: non-zero balances, missing receipts, statement mismatches and ticket conflicts (R-SET-5).
- Closing is blocked by open items, and each can be accepted with a note. A closed event is read-only, though its receipts can still be previewed, and shows its final balance. It can be reopened, keeping the notes (R-EVT-2, R-SET-7, R-SET-8).
- The settlement report has the categories, plan against actual, each cost centre, the final balance, every entry and the receipts, and is saved as PDF from the print dialog (R-SET-2, R-SET-3, R-SET-6).
- All transactions export as CSV, one row per line (R-SET-6).

## Display and keyboard

- Light, dark, or following the system. Printing stays light (R-NF-9).
- Two number and date formats, defaulting to the system's (R-NF-6).
- A reduced motion setting.
- Shortcuts for new event, open, settings and close in the menu, ⌘1–9 for pages and cost centres, and Esc to close a panel, pressed twice if that would discard changes (R-NF-7).
