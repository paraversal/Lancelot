// What an open item says, in words, on the Overview, in the close panel and in the report.
import type { EventSnapshot } from './api/generated/EventSnapshot';
import type { OpenItem } from './api/generated/OpenItem';

export interface ItemText {
	text: string;
	amount: number | null;
}

export function describe(item: OpenItem, snapshot: EventSnapshot): ItemText {
	const name = (id: number) => snapshot.accounts.find((a) => a.id === id)?.name ?? '';
	switch (item.type) {
		case 'personBalance':
			return {
				text: `${name(item.person)} ${item.net > 0 ? 'is owed' : 'holds'}`,
				amount: Math.abs(item.net)
			};
		case 'missingReceipt': {
			const txn = snapshot.txns.find((t) => t.id === item.txn);
			return { text: `No receipt: ${txn?.description || 'entry'}`, amount: txn?.amount ?? null };
		}
		case 'closingCountMissing':
			return { text: `${name(item.account)} not counted`, amount: null };
		case 'cashDifference':
			return { text: `${name(item.account)} count differs`, amount: item.difference };
	}
}
