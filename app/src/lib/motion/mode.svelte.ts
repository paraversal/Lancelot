// How much the app moves right now: the Motion setting, made calm when macOS asks for less
// motion. The root element carries it as `data-motion`, so CSS can read it too.
import type { Motion } from '../api/generated/Motion';
import { CHOICES, type Idea, type Strength } from './choices';

/** What an effect does: one of the strengths, a short fade, or nothing. */
export type Level = Exclude<Strength, 'skip'> | 'calm' | 'off';

export const motion = $state<{ setting: Motion; reduced: boolean }>({
	setting: 'full',
	reduced: false
});

export function effectiveMotion(): Motion {
	return motion.setting === 'full' && motion.reduced ? 'calm' : motion.setting;
}

/** Follows the system's reduced-motion wish; returns a function that stops following. */
export function watchReducedMotion(): () => void {
	const query = matchMedia('(prefers-reduced-motion: reduce)');
	const apply = () => (motion.reduced = query.matches);
	apply();
	query.addEventListener('change', apply);
	return () => query.removeEventListener('change', apply);
}

/** How idea `id` moves. `strength` overrides the choice, as the trial page does. */
export function level(id: Idea, strength?: Strength): Level {
	const mode = effectiveMotion();
	if (mode === 'off') return 'off';
	if (mode === 'calm') return 'calm';
	const s = strength ?? CHOICES[id];
	return s === 'skip' ? 'off' : s;
}

export const moves = (l: Level) => l !== 'off';
export const isPlayful = (l: Level) => l === 'playful' || l === 'big';
