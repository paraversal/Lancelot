# Motion

How Lancelot moves. Every idea has a number (A1, B3 …), so a choice takes a few words: "A1 playful, B2 regular, skip D2".

Each idea comes in two strengths, and the big moments in three:

- **Regular**: it moves and it feels good, but nobody would call it a feature.
- **Playful**: someone smiles. Used where the motion answers something a person just did.
- **Big**: someone shows it to the person next to them. Only for closing the event and the last open item.

## How to choose

1. Run the app in a browser (`pnpm dev`) and open **`/design/motion`**. It shows every idea at every strength side by side, built from the same code the app uses. ▶ replays one strength, **Replay** replays them all. 
2. Each demo is marked **full** (what the app uses with full motion) or **reduced** (with reduced motion). Both come from [`app/src/lib/motion/choices.ts`](../../app/src/lib/motion/choices.ts), as `FULL` and `REDUCED`: changing a line there changes the app.
3. Nothing is deleted when a choice changes. Every strength stays on the trial page.

## Rules

1. **Motion answers something.** A press, a change, an arrival. Nothing loops while someone is reading numbers.
2. **Never in the way.** The state changes first; the animation only shows it. Nothing waits for an animation before it accepts the next click or key.
3. **Fast things are small, big things are rare.** A press answers within 100 ms, a panel takes about 200 ms. Only the big moments take longer, and they never block anything.
4. **Only cheap properties move.** `transform` and `opacity` (and `clip-path`), so the browser never lays out the page again mid-animation. Rows that open or collapse, and the room a panel takes, are the exception: both are short, and both have to move what is beside them.
5. **Springs, not ease-in-out.** Things that move overshoot a little and settle. The curves are tokens, written once.
6. **Reduced motion.** The app follows macOS's "reduce motion". Settings → Display → **Reduced motion** is a toggle that overrides it either way. There is no "off": the choice is full or reduced. Reduced uses the Regular strength of every idea, so nothing travels far, overshoots, shakes, stamps or throws confetti.
7. **The design rules still hold.** No gradients for glow, no captions that explain a joke, one font.

## Tokens

In [`tokens.css`](tokens.css), synced into the app with `pnpm tokens`.

| Token | Value | For |
|---|---|---|
| `--motion-instant` | 90 ms | Presses, small state changes |
| `--motion-quick` | 160 ms | Fades, small arrivals |
| `--motion-normal` | 240 ms | Panels, rows, pages |
| `--motion-slow` | 480 ms | Bars, stamps, the big moments |
| `--ease-out` | `cubic-bezier(0.2, 0.8, 0.2, 1)` | Things arriving |
| `--ease-in` | `cubic-bezier(0.5, 0, 0.8, 0.3)` | Things leaving |
| `--spring-soft` | `linear(…)`, a small overshoot | Pills, panels, receipts |
| `--spring-bouncy` | `linear(…)`, a bigger overshoot | Presses, the lock |

## Code

All of it is in [`app/src/lib/motion/`](../../app/src/lib/motion/):

- `choices.ts`: the ideas, their strengths, and the strengths the app uses with full and with reduced motion.
- `mode.svelte.ts`: whether motion is reduced (the system's wish, or the setting), and `level(id)`, the strength an idea plays at right now.
- `effects.ts`: one function per idea, each taking that level.
- `MovingAmount.svelte`, `Check.svelte`, `Lock.svelte`: an amount that moves to its new value, a check that draws itself, a lock whose shackle moves.
- `confetti.ts`: confetti and sparks for the big moments, on a canvas that exists only while they fall.

## The ideas

### A · Everywhere

| Id | Idea | Regular | Playful | Full motion |
|---|---|---|---|---|
| A1 | Switching pages | The page fades in and rises 6 px; the sidebar highlight slides to the new item | Rises with a spring; the highlight stretches as it travels | Regular |
| A2 | Panels | The page gives way as the panel's room opens, and the panel rides in with it; closing is immediate | The room opens with a small overshoot | Regular |
| A3 | Pressing buttons | The button sinks a little | It squashes and springs back | Regular |
| A4 | Segmented controls | The yellow pill slides to the new option | The pill stretches on the way and settles | Playful |
| A5 | Changing amounts | The new figure slides in from the direction it moved | The digits that changed roll, like an odometer | Playful |
| A6 | Errors in fields | The warning icon grows in | It pops in, and the field gives a short shake | Playful |

### B · Ledger

| Id | Idea | Regular | Playful | Full motion |
|---|---|---|---|---|
| B1 | A new entry | The row opens up with a yellow flash that fades | A highlighter stroke runs across the row and fades | Regular |
| B2 | Deleting an entry | The row collapses | A line strikes through it, then it collapses | Regular |
| B3 | Filters and search | Rows slide to their new places | With a spring | Regular |
| B4 | Save & next | The filled form slides away, a fresh one slides in | Like a page turning: further, slightly tilted | Regular |
| B5 | Receipts landing | A new receipt grows into the strip | It drops in like paper on a desk: crooked, then straight | Playful |
| B6 | Receipt preview | The preview fades in | It grows out of the thumbnail | Regular |

### C · People and payouts

| Id | Idea | Regular | Playful | Full motion |
|---|---|---|---|---|
| C1 | A person settles | The balance moves to zero; "Settled" fades in | "Settled" stamps in | Playful |
| C2 | Mark as paid | The rows turn grey in a quick wave; the total moves to zero | A check draws on each row, one after another | Playful |
| C3 | A valid IBAN | The check fades in | The check draws itself | Playful |

### D · Overview and counts

| Id | Idea | Regular | Playful | Full motion |
|---|---|---|---|---|
| D1 | Arriving on the Overview | The sections rise in one after another | And the result counts up from zero, once per opened event | Regular |
| D2 | In and out bars | The bars grow from the left | Segment after segment, with a spring | Playful |
| D3 | Open items | A fixed item collapses out; "Nothing open" fades in its check | The check draws itself | Regular |
| D4 | The count matches | "Matches the books" fades in | It pops in, and its check draws | Playful |

### E · The big moments

| Id | Idea | Regular | Playful | Big | Full motion |
|---|---|---|---|---|---|
| E1 | Closing the event | The lock closes; "Closed" fades in | "Closed" stamps onto the page | And confetti from the button | Big |
| E2 | Reopening | The lock opens | The shackle springs open | | Regular |
| E3 | The last open item | The item collapses; "Nothing open" fades in | A few sparks | Confetti | Playful |

### F · Start screen, files, report

| Id | Idea | Regular | Playful | Full motion |
|---|---|---|---|---|
| F1 | Opening an event | The event fades in and rises | The recent row grows into the window | Regular |
| F2 | Check again | The arrow turns once; the result fades in | The arrow turns with a spring; the check draws | Playful |
| F3 | The report gets ready | Receipts fade in as they load | And Print gives a little nudge once it can print | Playful |
