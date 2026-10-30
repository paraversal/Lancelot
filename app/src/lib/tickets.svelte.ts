// The fee or refund a form is about to save, from the core's rule (R-COR-6).
import { api } from './api';
import type { TicketShare } from './api/generated/TicketShare';

export type ShareInput = { price: number; bp: number; count: number } | null;

/** Follows `input`; `current` is null without an input, and when the core refuses it. */
export function ticketShare(input: () => ShareInput) {
	let current = $state<TicketShare | null>(null);
	let seq = 0;
	$effect(() => {
		const i = input();
		const mine = ++seq;
		if (!i) {
			current = null;
			return;
		}
		api.ticketShare(i.price, i.bp, i.count).then(
			(share) => {
				if (mine === seq) current = share;
			},
			() => {
				if (mine === seq) current = null;
			}
		);
	});
	return {
		get current() {
			return current;
		}
	};
}
