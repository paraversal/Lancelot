// jsdom draws nothing, so it has no Web Animations API. Animations finish at once here.
if (!Element.prototype.animate) {
	Element.prototype.animate = function () {
		const done = Promise.resolve();
		return {
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
	};
	Element.prototype.getAnimations = () => [];
}
