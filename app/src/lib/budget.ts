// How the Budget page reads the core's figures (R-BUD-3, R-BUD-4).
import type { Assumption } from './api/generated/Assumption';
import type { AssumptionId } from './api/generated/AssumptionId';
import type { BudgetLine } from './api/generated/BudgetLine';
import type { BudgetLineFigures } from './api/generated/BudgetLineFigures';
import type { Estimate } from './api/generated/Estimate';
import type { NumberFormat } from './api/generated/NumberFormat';
import { formatAmount } from './format';

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

/** The budget lines whose quantity scales with an assumption (R-BUD-6). */
export function usersOf(lines: BudgetLine[], id: AssumptionId): BudgetLine[] {
	return lines.filter((l) => l.estimate.type === 'quantity' && l.estimate.per.includes(id));
}

/**
 * How a scaled estimate is made up, `2 × Guests × 4,50 €`, or null for one that scales with
 * nothing. A quantity of one is left out: `Guests × 25,00 €`.
 */
export function formula(
	estimate: Estimate,
	assumptions: Assumption[],
	format: NumberFormat
): string | null {
	if (estimate.type !== 'quantity' || !estimate.per.length) return null;
	const names = estimate.per.map((id) => assumptions.find((a) => a.id === id)?.name ?? '?');
	const count = estimate.quantity === 1 ? [] : [String(estimate.quantity)];
	return [...count, ...names, formatAmount(estimate.unitPrice, format)].join(' × ');
}
