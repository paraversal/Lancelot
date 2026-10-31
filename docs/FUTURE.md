# Lancelot — Future

Longer-term ideas and larger changes that are not planned yet. Nothing here is committed to: an item moves to the [roadmap](roadmap.md) when a real need appears. Until then, the rule is to keep the way open, not to build ahead.

## Ticket sources other than a sheet

Today tickets come from a sheet or a data source, each a plugin. Other groups may have only totals. The refactoring made tickets optional and removed the fixed Verein account, so no group is stuck with this workflow. Other sources can then follow without a redesign.

**The seam already exists.** `Book::preview_import` and `Book::import_tickets` take a `TicketBatch`, which is a label, a hash and one tier and order ID per ticket. Everything after that (conflicts, generated income, statistics) knows nothing about files. A new source only has to build a `TicketBatch`.

| Source | What it would take |
|---|---|
| Rows with no order ID | The core requires one. Conflicts are matched by (order ID, tier), so without it a removal cannot be traced to an order. Making it optional means turning conflict detection off for that event. |
| Totals per tier only | No orders at all, so no conflicts. Either manual sales cover it already, or a small form sets a count per tier. |

## Third-party plugins

Plugins are built in today (technical design §10). When another group wants its own, a loader follows: JavaScript in a sandboxed iframe, with a manifest of permissions, the context over messages, and network access through the data sources. The first step is testing that an iframe can't reach Tauri's commands. The Drinks plugin should then need little change.

## Motion trial

`lib/motion/choices.ts`, every strength of about 25 effects and the `/design/motion` trial page stay for now, so animations can still be tried out. Once the choices feel settled, keep the chosen effects and a full-or-reduced switch, and remove the rest (about 1,100 lines). Git keeps them. This changes nothing visible.

## Receipts before entries

`attachment.txn_id` is `NOT NULL`, so a receipt cannot exist before its entry. A stack of unbooked receipts would mean making it nullable and adding an open item for unassigned ones. Cheap, but a new feature, so it waits for real demand.

## New entry forms

A new form is one `Entry` variant, with no change to the load path. Candidates, each only when needed: a supplier credit note, a second payer on one receipt, and income net of a fee.

## Decided against

These do not fit a tool for one treasurer, and the changes above cover the realistic cases more simply: configurable workflows and event sourcing. Multi-event, multi-user and compliance features stay out of scope (requirements §1).
