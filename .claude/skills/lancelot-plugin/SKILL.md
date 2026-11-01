---
name: lancelot-plugin
description: Add or change a built-in Lancelot plugin in app/src/lib/plugins, or add a slot or method to its PluginContext. Use for work on the drinks, ticket-sheet, ticket-api, what-if or cash-count plugins, or a new one.
---

Read `docs/plugins.md` first: it has the rules, the slots and the context. `app/src/lib/plugins/api.ts` is the source of truth for the types.

Hold to these while working:

- A plugin imports `plugins/api.ts`, shared components and formatting, never the session or the command API.
- A plugin's data never takes part in a balance, a result or a closing check. Money moves only through `proposeEntry` or `proposeImport`, which the treasurer confirms.
- A plugin never sees people's bank details or ticket orders. If a feature seems to need them, do that part in the host or the core, and ask first.
- A provider's real details never go in the repository or an event file, or in tests and docs.

When adding a plugin:

1. Add `<id>/index.ts`, and register it in `registry.ts`. Its `id` never changes once released.
2. Keep figures and record parsing in plain `.ts` files, and test them. Records that don't fit are ignored and kept.
3. Add a test through the page where its slot lives. Check that proposing saves nothing and a closed event hides what would change it.
4. Add it to `docs/features.md` under Plugins, in the same commit.

When adding a slot or a context method:

1. Add it to `api.ts` and `host.ts`, or to the page that hosts the slot, wrapped in `PluginFrame`.
2. Add a real plugin that uses it, unless the user asked for the seam alone. A sandboxed loader has to be able to implement it too, so pass data rather than the app's objects.
3. Update `docs/plugins.md` and technical design §10 in the same commit.

Finish with `scripts/check.sh`, and look at the slot in the running app with the `ui` and `bridge` preview servers. The bridge's scenarios are in technical design §5: `example` has a budget and drinks, and `conflict` also has ticket tiers.
