// The easing tokens for the Web Animations API, which cannot read `var(--spring-soft)`.
export type Easing = 'ease-out' | 'ease-in' | 'spring-soft' | 'spring-bouncy';

const FALLBACK: Record<Easing, string> = {
	'ease-out': 'cubic-bezier(0.2, 0.8, 0.2, 1)',
	'ease-in': 'cubic-bezier(0.5, 0, 0.8, 0.3)',
	'spring-soft': 'cubic-bezier(0.3, 1.25, 0.5, 1)',
	'spring-bouncy': 'cubic-bezier(0.3, 1.8, 0.5, 1)'
};

const cache = new Map<Easing, string>();

export function easing(name: Easing): string {
	let value = cache.get(name);
	if (!value) {
		value =
			getComputedStyle(document.documentElement).getPropertyValue(`--${name}`).trim() ||
			FALLBACK[name];
		// Older webviews know cubic-bezier() but not linear().
		if (value.startsWith('linear(') && !CSS.supports('animation-timing-function', value))
			value = FALLBACK[name];
		cache.set(name, value);
	}
	return value;
}

/** Svelte transitions take a function; this one follows a spring token closely enough. */
export function springFn(t: number, bounce = 1.25): number {
	// A damped overshoot: rises past 1 once, then settles.
	const s = 1 - Math.pow(1 - t, 3);
	return s + Math.sin(Math.PI * t) * (bounce - 1) * (1 - t);
}
