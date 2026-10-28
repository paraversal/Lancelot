# Motion

How Lancelot moves. Every idea has a number (A1, B3 …), so a choice takes a few words: "A1 playful, B2 regular, skip D2".

Each idea comes in two strengths, and the big moments in three:

- **Regular**: it moves and it feels good, but nobody would call it a feature.
- **Playful**: someone smiles. Used where the motion answers something a person just did.
- **Big**: someone shows it to the person next to them. Only for closing the event and the last open item.

## How to choose

1. Run the app in a browser (`pnpm dev`) and open **`/design/motion`**. It shows every idea at every strength side by side, built from the same code the app uses. ▶ replays one strength, **Replay** replays them all. The switch at the top shows **Calm** and **Off**.
2. The strength the app uses is marked **in the app**. It comes from [`app/src/lib/motion/choices.ts`](../../app/src/lib/motion/choices.ts): changing a line there changes the app.
3. Nothing is deleted when a choice changes. Every strength stays on the trial page.

## Rules

1. **Motion answers something.** A press, a change, an arrival. Nothing loops while someone is reading numbers.
2. **Never in the way.** The state changes first; the animation only shows it. Nothing waits for an animation before it accepts the next click or key.
3. **Fast things are small, big things are rare.** A press answers within 100 ms, a panel takes about 200 ms. Only the big moments take longer, and they never block anything.
4. **Only cheap properties move.** `transform` and `opacity` (and `clip-path`), so the browser never lays out the page again mid-animation. Rows that open or collapse are the exception, and they are short.
5. **Springs, not ease-in-out.** Things that move overshoot a little and settle. The curves are tokens, written once.
6. **Calm on request.** Settings → Display → **Motion**: *Full*, *Calm* (short fades, nothing big) or *Off*. When macOS asks to reduce motion, *Full* behaves like *Calm*.
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

- `choices.ts`: the ideas, their strengths, and the strength the app uses.
- `mode.svelte.ts`: the Motion setting with reduce motion, and `level(id)`, the level an idea plays at right now (a strength, `calm` or `off`).
- `effects.ts`: one function per idea, each taking that level.
- `MovingAmount.svelte`, `Check.svelte`, `Lock.svelte`: an amount that moves to its new value, a check that draws itself, a lock whose shackle moves.
- `confetti.ts`: confetti and sparks for the big moments, on a canvas that exists only while they fall.

## The ideas

### A · Everywhere

| Id | Idea | Regular | Playful | In the app |
|---|---|---|---|---|
| A1 | Switching pages | The page fades in and rises 6 px; the sidebar highlight slides to the new item | Rises with a spring; the highlight stretches as it travels | Regular |
| A2 | Panels | Slide in from the right and settle; closing is faster | Slide further with a small overshoot | Regular |
| A3 | Pressing buttons | The button sinks a little | It squashes and springs back | Playful |
| A4 | Segmented controls | The yellow pill slides to the new option | The pill stretches on the way and settles | Playful |
| A5 | Changing amounts | The new figure slides in from the direction it moved | The digits that changed roll, like an odometer | Regular |
| A6 | Errors in fields | The warning icon grows in | It pops in, and the field gives a short shake | Playful |

### B · Ledger

| Id | Idea | Regular | Playful | In the app |
|---|---|---|---|---|
| B1 | A new entry | The row opens up with a yellow flash that fades | A highlighter stroke runs across the row and fades | Playful |
| B2 | Deleting an entry | The row collapses | A line strikes through it, then it collapses | Regular |
| B3 | Filters and search | Rows slide to their new places | With a spring | Regular |
| B4 | Save & next | The filled form slides away, a fresh one slides in | Like a page turning: further, slightly tilted | Regular |
| B5 | Receipts landing | A new receipt grows into the strip | It drops in like paper on a desk: crooked, then straight | Playful |
| B6 | Receipt preview | The preview fades in | It grows out of the thumbnail | Regular |

### C · People and payouts

| Id | Idea | Regular | Playful | In the app |
|---|---|---|---|---|
| C1 | A person settles | The balance moves to zero; "Settled" fades in | "Settled" stamps in | Playful |
| C2 | Mark as paid | The rows turn grey in a quick wave; the total moves to zero | A check draws on each row, one after another | Playful |
| C3 | A valid IBAN | The check fades in | The check draws itself | Playful |

### D · Overview and counts

| Id | Idea | Regular | Playful | In the app |
|---|---|---|---|---|
| D1 | Arriving on the Overview | The sections rise in one after another | And the result counts up from zero, once per opened event | Regular |
| D2 | In and out bars | The bars grow from the left | Segment after segment, with a spring | Regular |
| D3 | Open items | A fixed item collapses out; "Nothing open" fades in its check | The check draws itself | Regular |
| D4 | The count matches | "Matches the books" fades in | It pops in, and its check draws | Playful |

### E · The big moments

| Id | Idea | Regular | Playful | Big | In the app |
|---|---|---|---|---|---|
| E1 | Closing the event | The lock closes; "Closed" fades in | "Closed" stamps onto the page | And confetti from the button | Playful |
| E2 | Reopening | The lock opens | The shackle springs open | | Regular |
| E3 | The last open item | The item collapses; "Nothing open" fades in | A few sparks | Confetti | Playful |

### F · Start screen, files, report

| Id | Idea | Regular | Playful | In the app |
|---|---|---|---|---|
| F1 | Opening an event | The event fades in and rises | The recent row grows into the window | Regular |
| F2 | Check again | The arrow turns once; the result fades in | The arrow turns with a spring; the check draws | Regular |
| F3 | The report gets ready | Receipts fade in as they load | And Print gives a little nudge once it can print | Playful |
