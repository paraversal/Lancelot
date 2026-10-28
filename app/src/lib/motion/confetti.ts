// Confetti and sparkles for the big moments (E1, E3): one canvas over the window that exists
// only while something falls. Paper in the app's own colours, no library.
const COLORS = [
	'--color-marker',
	'--color-plus',
	'--color-tier-2',
	'--color-tier-3',
	'--color-tier-4'
];

interface Piece {
	x: number;
	y: number;
	vx: number;
	vy: number;
	spin: number;
	angle: number;
	size: number;
	color: string;
}

/** Throws `count` pieces from a point (e.g. a button's centre). `spread` 1 is a burst. */
export function confetti(from: { x: number; y: number }, count = 120, spread = 1) {
	const canvas = document.createElement('canvas');
	const dpr = devicePixelRatio || 1;
	canvas.width = innerWidth * dpr;
	canvas.height = innerHeight * dpr;
	canvas.style.cssText =
		'position:fixed;inset:0;width:100vw;height:100vh;pointer-events:none;z-index:2000';
	canvas.setAttribute('aria-hidden', 'true');
	document.body.append(canvas);
	const ctx = canvas.getContext('2d')!;
	ctx.scale(dpr, dpr);
	const style = getComputedStyle(document.documentElement);
	const colors = COLORS.map((c) => style.getPropertyValue(c).trim() || '#F5D547');
	const pieces: Piece[] = Array.from({ length: count }, () => {
		const angle = -Math.PI / 2 + (Math.random() - 0.5) * Math.PI * spread;
		const speed = 6 + Math.random() * 9;
		return {
			x: from.x,
			y: from.y,
			vx: Math.cos(angle) * speed,
			vy: Math.sin(angle) * speed,
			spin: (Math.random() - 0.5) * 0.4,
			angle: Math.random() * Math.PI,
			size: 5 + Math.random() * 6,
			color: colors[Math.floor(Math.random() * colors.length)]
		};
	});
	const start = performance.now();
	const frame = (now: number) => {
		ctx.clearRect(0, 0, innerWidth, innerHeight);
		const age = now - start;
		for (const p of pieces) {
			p.vy += 0.32;
			p.vx *= 0.985;
			p.x += p.vx;
			p.y += p.vy;
			p.angle += p.spin;
			ctx.save();
			ctx.globalAlpha = Math.max(0, 1 - age / 2200);
			ctx.translate(p.x, p.y);
			ctx.rotate(p.angle);
			ctx.fillStyle = p.color;
			ctx.fillRect(-p.size / 2, -p.size / 4, p.size, p.size / 2);
			ctx.restore();
		}
		if (age < 2200) requestAnimationFrame(frame);
		else canvas.remove();
	};
	requestAnimationFrame(frame);
}

/** A few small sparks around a point: the playful version of a celebration. */
export const sparkle = (at: { x: number; y: number }) => confetti(at, 18, 2);

export const centreOf = (el: Element) => {
	const r = el.getBoundingClientRect();
	return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
};
