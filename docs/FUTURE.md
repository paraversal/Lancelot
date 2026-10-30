# Lancelot — Future

Longer-term ideas and larger changes that are not planned yet. Nothing here is committed to: an item moves to the [roadmap](roadmap.md) when a real need appears. Until then, the rule is to keep the way open, not to build ahead.

## Ticket sources other than a sheet

Today tickets come from a sheet (`.ods`, `.xlsx` or `.csv`) with one row per ticket. Other groups may sell through a platform with an API, or have only totals. Stage 5 of the refactoring makes tickets optional and removes the fixed Verein account, so no group is stuck with this workflow. Other sources can then follow without a redesign.

**The seam already exists.** `Book::preview_import` and `Book::import_tickets` take a `Sheet`, which is a label, a hash and rows of tier and order ID. Everything after that (conflicts, generated income, statistics) knows nothing about files. A new source only has to build a `Sheet`.

| Source | What it would take |
|---|---|
| API pull | One function that fetches the orders and builds a `Sheet`. A pull is an import dated today, so repeated pulls work as re-imports do, including conflicts. `filename` and `sha256` become a source label and a hash of the content. The credentials belong in the app settings, not in the event file, which gets copied around. The source hands over only tier and order ID, so names and addresses stay out of the file (R-TKT-17). |
| Rows with no order ID | `sheet.rs` requires the column. Conflicts are matched by (order ID, tier), so without it a removal cannot be traced to an order. Making it optional means turning conflict detection off for that event. |
| Totals per tier only | No orders at all, so no conflicts. Either manual sales cover it already, or a small form sets a count per tier. |

**Do not build yet:** a provider trait or a source setting. With one source there is nothing to abstract, and the second one will show what the abstraction needs.

## Motion trial

`lib/motion/choices.ts`, every strength of about 25 effects and the `/design/motion` trial page stay for now, so animations can still be tried out. Once the choices feel settled, keep the chosen effects and a full-or-reduced switch, and remove the rest (about 1,100 lines). Git keeps them. This changes nothing visible.

## Receipts before entries

`attachment.txn_id` is `NOT NULL`, so a receipt cannot exist before its entry. A stack of unbooked receipts would mean making it nullable and adding an open item for unassigned ones. Cheap, but a new feature, so it waits for real demand.

## New entry forms

Once Stage 5 is done, a new form is one `Entry` variant, with no change to the load path. Candidates, each only when needed: a supplier credit note, a second payer on one receipt, and income net of a fee.

## Decided against

These do not fit a tool for one treasurer, and the changes above cover the realistic cases more simply: a plugin system, configurable workflows and event sourcing. Multi-event, multi-user and compliance features stay out of scope (requirements §1).
