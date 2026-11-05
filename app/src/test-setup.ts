// jsdom draws nothing, so it has no Web Animations API. Animations finish at once here: Svelte
// waits for `onfinish` before it takes a leaving element out of the page, so each animation
// reports finishing on the next microtask.
if (!Element.prototype.animate) {
	Element.prototype.animate = function () {
		const done = Promise.resolve();
		const animation = {
			finished: done,
			ready: done,
			cancel() {},
			finish() {},
			play() {},
			pause() {},
			reverse() {},
			commitStyles() {},
			addEventListener() {},
			removeEventListener() {},
			onfinish: null,
			oncancel: null
		} as unknown as Animation;
		// The handler is put on after the animation is made, so look it up when the microtask runs.
		queueMicrotask(() => animation.onfinish?.call(animation, new Event('finish') as never));
		return animation;
	};
	Element.prototype.getAnimations = () => [];
}
