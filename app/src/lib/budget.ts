// How the Budget page reads the core's figures (R-BUD-3, R-BUD-4).
import type { BudgetLineFigures } from './api/generated/BudgetLineFigures';

/** An expense over its estimate, or income short of it once complete. */
export function isOver(
	kind: 'income' | 'expense',
	figures: { estimate: number; actual: number },
	complete: boolean
): boolean {
	return kind === 'expense'
		? figures.actual > figures.estimate
		: complete && figures.actual < figures.estimate;
}

/** Whether a line's forecast only repeats its estimate, with no real amount behind it yet. */
export function isWaiting(
	figures: { estimate: number; actual: number; forecast: number },
	complete: boolean
): boolean {
	return !complete && figures.forecast === figures.estimate && figures.actual !== figures.forecast;
}

/** The lines that moved the result most from its plan, in either direction. */
export function furthestFromPlan(lines: BudgetLineFigures[], count = 3): BudgetLineFigures[] {
	return lines
		.filter((l) => l.fromPlan !== 0)
		.sort((a, b) => Math.abs(b.fromPlan) - Math.abs(a.fromPlan))
		.slice(0, count);
}
