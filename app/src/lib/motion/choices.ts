// The strength each animation has in the app (docs/design/motion.md). The trial page at
// /design/motion shows every strength of every idea; changing a line here changes the app.
export type Strength = 'regular' | 'playful' | 'big' | 'skip';

export const IDEAS = {
	A1: 'Switching pages',
	A2: 'Panels',
	A3: 'Pressing buttons',
	A4: 'Segmented controls',
	A5: 'Changing amounts',
	A6: 'Errors in fields',
	B1: 'A new entry',
	B2: 'Deleting an entry',
	B3: 'Filters and search',
	B4: 'Save & next',
	B5: 'Receipts landing',
	B6: 'Receipt preview',
	C1: 'A person settles',
	C2: 'Mark as paid',
	C3: 'A valid IBAN',
	D1: 'Arriving on the Overview',
	D2: 'In and out bars',
	D3: 'Open items',
	D4: 'The count matches',
	E1: 'Closing the event',
	E2: 'Reopening',
	E3: 'The last open item',
	F1: 'Opening an event',
	F2: 'Check again',
	F3: 'The report gets ready'
} as const;

export type Idea = keyof typeof IDEAS;

/** The strengths an idea comes in. Most have two; the big moments have three. */
export const STRENGTHS: Record<Idea, Strength[]> = Object.fromEntries(
	Object.keys(IDEAS).map((id) => [
		id,
		['E1', 'E3'].includes(id) ? ['regular', 'playful', 'big'] : ['regular', 'playful']
	])
) as Record<Idea, Strength[]>;

/** What the app uses. Proposed: playful where it answers a person, regular for the rest. */
export const CHOICES: Record<Idea, Strength> = {
	A1: 'regular',
	A2: 'regular',
	A3: 'playful',
	A4: 'playful',
	A5: 'regular',
	A6: 'playful',
	B1: 'playful',
	B2: 'regular',
	B3: 'regular',
	B4: 'regular',
	B5: 'playful',
	B6: 'regular',
	C1: 'playful',
	C2: 'playful',
	C3: 'playful',
	D1: 'regular',
	D2: 'regular',
	D3: 'regular',
	D4: 'playful',
	E1: 'playful',
	E2: 'regular',
	E3: 'playful',
	F1: 'regular',
	F2: 'regular',
	F3: 'playful'
};
