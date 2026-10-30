// Every animation in Lancelot, by idea (docs/design/motion.md). Each takes the level it plays
// at, from `level()` in mode.svelte.ts. Only transform, opacity and clip-path move, and nothing
// here delays the work: the state has already changed when an effect starts.
import { cubicOut } from 'svelte/easing';
import type { TransitionConfig } from 'svelte/transition';
import { easing, springFn } from './easing';
import { isPlayful, type Level } from './mode.svelte';

const NONE: TransitionConfig = { duration: 0 };
const fade = (duration = 120): TransitionConfig => ({
	duration,
	css: (t) => `opacity: ${t}`
});

function play(node: Element, keyframes: Keyframe[], options: KeyframeAnimationOptions) {
	return node.animate(keyframes, { fill: 'backwards', ...options });
}

// --- A · Everywhere ----------------------------------------------------------------------

/** A1: a page arriving. */
export function pageIn(_node: Element, { level }: { level: Level }): TransitionConfig {
	if (level === 'off') return NONE;
	const rise = isPlayful(level) ? 10 : 6;
	return {
		duration: isPlayful(level) ? 320 : 180,
		easing: isPlayful(level) ? (t) => springFn(t, 1.08) : cubicOut,
		css: (t, u) => `opacity: ${Math.min(1, t * 1.6)}; transform: translateY(${u * rise}px)`
	};
}

/** A1: the sidebar's highlight travels from the item it was on to the new one. */
export function markerTravel(marker: HTMLElement, from: DOMRect, to: DOMRect, level: Level) {
	if (level === 'off') return;
	const dy = from.top - to.top;
	if (!dy) return;
	const frames: Keyframe[] = isPlayful(level)
		? [
				{ transform: `translateY(${dy}px) scaleY(1)` },
				{ transform: `translateY(${dy * 0.45}px) scaleY(1.35)`, offset: 0.45 },
				{ transform: 'translateY(0) scaleY(1)' }
			]
		: [{ transform: `translateY(${dy}px)` }, { transform: 'translateY(0)' }];
	marker.animate(frames, {
		duration: isPlayful(level) ? 360 : 200,
		easing: isPlayful(level) ? easing('spring-soft') : easing('ease-out')
	});
}

/** A2: a panel sliding in from the right. */
export function panelIn(_node: Element, { level }: { level: Level }): TransitionConfig {
	if (level === 'off') return NONE;
	const playful = isPlayful(level);
	const from = playful ? 28 : 16;
	return {
		duration: playful ? 340 : 200,
		easing: playful ? (t) => springFn(t, 1.06) : cubicOut,
		css: (t, u) => `opacity: ${Math.min(1, t * 2)}; transform: translateX(${u * from}px)`
	};
}

export function panelOut(_node: Element, { level }: { level: Level }): TransitionConfig {
	if (level === 'off') return NONE;
	return {
		duration: 120,
		css: (t, u) => `opacity: ${t}; transform: translateX(${u * 10}px)`
	};
}

/** A3: a press. Regular sinks (in CSS, base.css); playful squashes and springs back. */
export function press(node: HTMLElement, getLevel: () => Level) {
	const down = () => {
		if (!isPlayful(getLevel())) return;
		node.animate([{ transform: 'scale(1)' }, { transform: 'scale(0.92, 0.88)' }], {
			duration: 90,
			easing: easing('ease-out'),
			fill: 'forwards'
		});
	};
	const up = () => {
		if (!isPlayful(getLevel())) return;
		node.getAnimations().forEach((a) => a.cancel());
		node.animate([{ transform: 'scale(0.92, 0.88)' }, { transform: 'scale(1)' }], {
			duration: 380,
			easing: easing('spring-bouncy')
		});
	};
	node.addEventListener('pointerdown', down);
	node.addEventListener('pointerup', up);
	node.addEventListener('pointerleave', up);
	return {
		destroy() {
			node.removeEventListener('pointerdown', down);
			node.removeEventListener('pointerup', up);
			node.removeEventListener('pointerleave', up);
		}
	};
}

/** A4: the active option's pill moves from the old option to the new one. */
export function pillTravel(pill: HTMLElement, from: DOMRect, to: DOMRect, level: Level) {
	if (level === 'off') return;
	const dx = from.left - to.left;
	const sx = from.width / Math.max(1, to.width);
	if (!dx && sx === 1) return;
	pill.animate(
		[
			{ transform: `translateX(${dx}px) scaleX(${sx})` },
			...(isPlayful(level)
				? [{ transform: `translateX(${dx * 0.3}px) scaleX(${(1 + sx) / 2 + 0.12})`, offset: 0.4 }]
				: []),
			{ transform: 'translateX(0) scaleX(1)' }
		],
		{
			duration: isPlayful(level) ? 340 : 180,
			easing: isPlayful(level) ? easing('spring-soft') : easing('ease-out')
		}
	);
}

/** A6: the warning icon of a field that did not accept its input. */
export function warnIn(node: Element, { level }: { level: Level }): TransitionConfig {
	if (level === 'off') return NONE;
	return {
		duration: isPlayful(level) ? 360 : 160,
		easing: isPlayful(level) ? (t) => springFn(t, 1.3) : cubicOut,
		css: (t) => `opacity: ${Math.min(1, t * 2)}; transform: scale(${0.4 + 0.6 * t})`
	};
}

/** A6 playful: a short shake of the field when an invalid value is committed. */
export function shake(node: Element, level: Level) {
	if (!isPlayful(level)) return;
	node.animate(
		[
			{ transform: 'translateX(0)' },
			{ transform: 'translateX(-5px)' },
			{ transform: 'translateX(4px)' },
			{ transform: 'translateX(-2px)' },
			{ transform: 'translateX(0)' }
		],
		{ duration: 280, easing: 'ease-out' }
	);
}

// --- B · Ledger --------------------------------------------------------------------------

/** B1: a new row opens up; the highlighter (a `.sweep` child) runs across and fades. */
export function rowIn(node: HTMLElement, { level }: { level: Level }): TransitionConfig {
	if (level === 'off') return NONE;
	const sweep = node.querySelector<HTMLElement>('.sweep');
	if (sweep)
		isPlayful(level)
			? sweep.animate(
					[
						{ transform: 'scaleX(0)', opacity: 1 },
						{ transform: 'scaleX(1)', opacity: 1, offset: 0.35 },
						{ transform: 'scaleX(1)', opacity: 0 }
					],
					{ duration: 1100, easing: 'ease-out', fill: 'both' }
				)
			: sweep.animate([{ opacity: 0.9 }, { opacity: 0 }], {
					duration: 900,
					easing: 'ease-out',
					fill: 'both'
				});
	const h = node.offsetHeight;
	return {
		duration: 200,
		easing: cubicOut,
		css: (t) => `opacity: ${t}; height: ${t * h}px; overflow: hidden`
	};
}

/** B2: a deleted row collapses; playful strikes it through first (a `.strike` child). */
export function rowOut(node: HTMLElement, { level }: { level: Level }): TransitionConfig {
	if (level === 'off') return NONE;
	const h = node.offsetHeight;
	const strike = isPlayful(level);
	const line = node.querySelector<HTMLElement>('.strike');
	if (strike && line)
		line.animate([{ transform: 'scaleX(0)' }, { transform: 'scaleX(1)' }], {
			duration: 220,
			easing: easing('ease-out'),
			fill: 'forwards'
		});
	return {
		duration: strike ? 480 : 200,
		css: (t) => {
			// Playful: the first half is the strike, the second the collapse.
			const c = strike ? Math.min(1, t * 2) : t;
			return `opacity: ${c}; height: ${c * h}px; overflow: hidden`;
		}
	};
}

/** B3: rows moving to their new places (animate:flip takes these options). */
export function flipOptions(level: Level) {
	if (level === 'off') return { duration: 0 };
	return isPlayful(level)
		? { duration: 360, easing: (t: number) => springFn(t, 1.08) }
		: { duration: 200, easing: cubicOut };
}

/** B4: the filled form leaves to the left, a fresh one arrives from the right. */
export function formOut(_node: Element, { level }: { level: Level }): TransitionConfig {
	if (level === 'off') return NONE;
	const playful = isPlayful(level);
	return {
		duration: playful ? 220 : 140,
		easing: cubicOut,
		css: (t, u) =>
			`opacity: ${t}; transform: translateX(${-u * (playful ? 40 : 20)}px) rotate(${playful ? -u * 2 : 0}deg)`
	};
}

export function formIn(_node: Element, { level }: { level: Level }): TransitionConfig {
	if (level === 'off') return NONE;
	const playful = isPlayful(level);
	return {
		duration: playful ? 340 : 180,
		delay: playful ? 120 : 60,
		easing: playful ? (t) => springFn(t, 1.06) : cubicOut,
		css: (t, u) =>
			`opacity: ${Math.min(1, t * 2)}; transform: translateX(${u * (playful ? 40 : 20)}px) rotate(${playful ? u * 2 : 0}deg)`
	};
}

/** B5: a receipt landing in the strip; playful drops it like paper on a desk. */
export function receiptIn(_node: Element, { level }: { level: Level }): TransitionConfig {
	if (level === 'off') return NONE;
	if (isPlayful(level))
		return {
			duration: 520,
			css: (t, u) => {
				const s = springFn(t, 1.12);
				return `opacity: ${Math.min(1, t * 3)}; transform: translateY(${-u * 26}px) rotate(${-7 * (1 - s)}deg) scale(${1.08 - 0.08 * s})`;
			}
		};
	return {
		duration: 180,
		easing: cubicOut,
		css: (t) => `opacity: ${t}; transform: scale(${0.9 + 0.1 * t})`
	};
}

/** B6: the preview grows out of the thumbnail it was opened from. */
export function growFrom(node: HTMLElement, from: DOMRect | null, level: Level) {
	if (level === 'off') return;
	if (!from || !isPlayful(level)) {
		play(
			node,
			[
				{ opacity: 0, transform: 'scale(0.97)' },
				{ opacity: 1, transform: 'none' }
			],
			{
				duration: 180,
				easing: easing('ease-out')
			}
		);
		return;
	}
	const to = node.getBoundingClientRect();
	const sx = from.width / to.width;
	const sy = from.height / to.height;
	play(
		node,
		[
			{
				transformOrigin: '0 0',
				transform: `translate(${from.left - to.left}px, ${from.top - to.top}px) scale(${sx}, ${sy})`,
				opacity: 0.6
			},
			{ transformOrigin: '0 0', transform: 'none', opacity: 1 }
		],
		{ duration: 320, easing: easing('spring-soft') }
	);
}

// --- C · People --------------------------------------------------------------------------

/** C1, E1: a stamp landing; regular just fades in. */
export function stampIn(_node: Element, { level }: { level: Level }): TransitionConfig {
	if (level === 'off') return NONE;
	if (!isPlayful(level)) return fade(160);
	return {
		duration: 420,
		css: (t) => {
			const s = springFn(t, 1.2);
			return `opacity: ${Math.min(1, t * 4)}; transform: scale(${1.7 - 0.7 * s}) rotate(${-9 + 7 * s}deg)`;
		}
	};
}

/** C2: rows ticking off one after another. Returns the delay for row `i`. */
export const wave = (level: Level, i: number) =>
	level === 'off' ? 0 : i * (isPlayful(level) ? 70 : 35);

/** C3, D3, D4, F2: a check that draws itself (an SVG path with pathLength="1"). */
export function drawCheck(path: SVGPathElement, level: Level, delay = 0) {
	if (level === 'off') return;
	if (!isPlayful(level)) {
		play(path, [{ opacity: 0 }, { opacity: 1 }], { duration: 160, delay });
		return;
	}
	play(path, [{ strokeDashoffset: 1 }, { strokeDashoffset: 0 }], {
		duration: 320,
		delay,
		easing: easing('ease-out')
	});
}

// --- D · Overview ------------------------------------------------------------------------

/** D1: sections rising in one after another; `i` is the section's place. */
export function riseIn(_node: Element, { level, i = 0 }: { level: Level; i?: number }) {
	if (level === 'off') return NONE;
	return {
		duration: 260,
		delay: i * (isPlayful(level) ? 70 : 45),
		easing: cubicOut,
		css: (t: number, u: number) => `opacity: ${t}; transform: translateY(${u * 10}px)`
	};
}

/** D2: a bar growing from the left. */
export function growBar(node: HTMLElement, level: Level, i = 0) {
	if (level === 'off') return;
	const playful = isPlayful(level);
	play(node, [{ transform: 'scaleX(0)' }, { transform: 'scaleX(1)' }], {
		duration: playful ? 520 : 360,
		delay: playful ? i * 60 : 0,
		easing: playful ? easing('spring-soft') : easing('ease-out')
	});
}

/** D2: a column growing from the bottom, as the sales per import do. */
export function growColumn(node: Element, level: Level, i = 0, count = 1) {
	if (level === 'off') return;
	const playful = isPlayful(level);
	play(node, [{ transform: 'scaleY(0)' }, { transform: 'scaleY(1)' }], {
		duration: playful ? 520 : 360,
		delay: playful ? i * Math.min(40, 240 / count) : 0,
		easing: playful ? easing('spring-soft') : easing('ease-out')
	});
}

/** D4: "Matches the books" popping in. */
export function popIn(_node: Element, { level }: { level: Level }): TransitionConfig {
	if (level === 'off') return NONE;
	if (!isPlayful(level)) return fade(160);
	return {
		duration: 380,
		css: (t) => `opacity: ${Math.min(1, t * 3)}; transform: scale(${0.6 + 0.4 * springFn(t, 1.25)})`
	};
}

// --- E, F · Big moments, files, report ---------------------------------------------------

/** E2: a section that lingers just long enough for its lock to open, then fades. */
export function lingerOut(_node: Element, { level }: { level: Level }): TransitionConfig {
	if (level === 'off') return NONE;
	const linger = isPlayful(level) ? 420 : 220;
	return { delay: linger, duration: 140, css: (t) => `opacity: ${t}` };
}

/** E1, E2: the lock's shackle (an SVG group) closing or opening. */
export function lockShackle(shackle: SVGGElement, closing: boolean, level: Level) {
	if (level === 'off') return;
	const open = 'translateY(-2px) rotate(-22deg)';
	const frames: Keyframe[] = closing
		? [{ transform: open }, { transform: 'none' }]
		: [{ transform: 'none' }, { transform: open }];
	shackle.animate(frames, {
		duration: isPlayful(level) ? 420 : 220,
		easing: isPlayful(level) ? easing('spring-bouncy') : easing('ease-out'),
		fill: 'forwards'
	});
}

/** F1: the recent row growing into the window. */
export function expandInto(row: HTMLElement, level: Level): Promise<void> {
	if (level === 'off' || !isPlayful(level)) return Promise.resolve();
	const box = row.getBoundingClientRect();
	const copy = row.cloneNode(true) as HTMLElement;
	copy.style.cssText = `position:fixed;left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px;margin:0;z-index:1000;pointer-events:none;background:var(--color-paper);transform-origin:0 0`;
	document.body.append(copy);
	const sx = innerWidth / box.width;
	const sy = innerHeight / box.height;
	const a = copy.animate(
		[
			{ transform: 'none', opacity: 1 },
			{
				transform: `translate(${-box.left}px, ${-box.top}px) scale(${sx}, ${sy})`,
				opacity: 0
			}
		],
		{ duration: 360, easing: easing('ease-out') }
	);
	return a.finished.then(() => copy.remove());
}

/** F2: the refresh arrow turning once. */
export function spin(node: Element, level: Level) {
	if (level === 'off') return;
	node.animate([{ transform: 'rotate(0)' }, { transform: 'rotate(360deg)' }], {
		duration: isPlayful(level) ? 620 : 420,
		easing: isPlayful(level) ? easing('spring-soft') : easing('ease-out')
	});
}

/** F3: Print waking up once every receipt is ready. */
export function nudge(node: Element, level: Level) {
	if (!isPlayful(level)) return;
	node.animate(
		[
			{ transform: 'scale(1)' },
			{ transform: 'scale(1.08) rotate(-2deg)', offset: 0.35 },
			{ transform: 'scale(0.98) rotate(1deg)', offset: 0.7 },
			{ transform: 'scale(1)' }
		],
		{ duration: 460, easing: 'ease-out' }
	);
}
