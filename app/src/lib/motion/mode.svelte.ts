// How much the app moves right now: full, or reduced when macOS asks for less motion or the
// person says so in Settings (`reducedMotion` overrides the system). The root element carries
// it as `data-motion`, so CSS can read it too.
import { FULL, REDUCED, type Idea, type Strength } from './choices';

/** What an effect does: one of the strengths, or nothing for an idea that is skipped. */
export type Level = Exclude<Strength, 'skip'> | 'off';

export const motion = $state<{ system: boolean; setting: boolean | null }>({
	system: false,
	setting: null
});

export const reduced = () => motion.setting ?? motion.system;

/** Follows the system's reduced-motion wish; returns a function that stops following. */
export function watchReducedMotion(): () => void {
	const query = matchMedia('(prefers-reduced-motion: reduce)');
	const apply = () => (motion.system = query.matches);
	apply();
	query.addEventListener('change', apply);
	return () => query.removeEventListener('change', apply);
}

/** How idea `id` moves. `strength` overrides the choice, as the trial page does. */
export function level(id: Idea, strength?: Strength): Level {
	const s = strength ?? (reduced() ? REDUCED : FULL)[id];
	return s === 'skip' ? 'off' : s;
}

export const isPlayful = (l: Level) => l === 'playful' || l === 'big';
