// Receipts that are PDFs, drawn into the settlement report page by page with pdf.js (it runs in
// a worker, loaded on first use).
import type * as PdfJs from 'pdfjs-dist';

let library: Promise<typeof PdfJs> | null = null;

function pdfjs() {
	library ??= Promise.all([
		import('pdfjs-dist'),
		import('pdfjs-dist/build/pdf.worker.min.mjs?url')
	]).then(([lib, worker]) => {
		lib.GlobalWorkerOptions.workerSrc = worker.default;
		return lib;
	});
	return library;
}

/** Every page of a PDF as a PNG data URL, wide enough for print (about 200 dpi on A4). */
export async function renderPdf(url: string, width = 1400): Promise<string[]> {
	const lib = await pdfjs();
	const data = new Uint8Array(await (await fetch(url)).arrayBuffer());
	const task = lib.getDocument({ data });
	const pages: string[] = [];
	try {
		const doc = await task.promise;
		for (let n = 1; n <= doc.numPages; n++) {
			const page = await doc.getPage(n);
			const base = page.getViewport({ scale: 1 });
			const viewport = page.getViewport({ scale: width / base.width });
			const canvas = document.createElement('canvas');
			canvas.width = Math.round(viewport.width);
			canvas.height = Math.round(viewport.height);
			// The print intent renders in one go, without waiting for animation frames that a
			// hidden window never gets.
			await page.render({ canvas, viewport, intent: 'print' }).promise;
			pages.push(canvas.toDataURL('image/png'));
			page.cleanup();
		}
	} finally {
		await task.destroy();
	}
	return pages;
}
