<script lang="ts">
	// The panel's receipts: saved attachments (click to preview), files waiting to be saved,
	// and a place to drop or pick more.
	import type { Attachment } from '#lib/api/generated/Attachment.ts';
	import { attachmentUrl } from '#lib/api/index.ts';
	import { chooseReceipts } from '#lib/api/dialogs.ts';
	import Icon from '#lib/components/Icon.svelte';
	import { formatSize } from '#lib/format.ts';
	import { receiptIn } from '#lib/motion/effects.ts';
	import { level } from '#lib/motion/mode.svelte.ts';
	import { session } from '#lib/state/session.svelte.ts';

	let {
		attachments,
		pending,
		previewing,
		onpreview,
		onremove,
		onremovePending,
		onadd
	}: {
		attachments: Attachment[];
		/** Paths of files that are attached when the entry is saved. */
		pending: string[];
		previewing: number | null;
		onpreview: (a: Attachment) => void;
		onremove: (a: Attachment) => void;
		onremovePending: (index: number) => void;
		onadd: (paths: string[]) => void;
	} = $props();

	const name = (path: string) => path.split(/[\\/]/).pop() ?? path;
	const isImage = (a: Attachment) => a.mime.startsWith('image/');
</script>

<div class="receipts">
	<span class="label">Receipt</span>
	{#each attachments as a (a.id)}
		<div class="item" class:selected={previewing === a.id} in:receiptIn={{ level: level('B5') }}>
			<button type="button" class="open" onclick={() => onpreview(a)}>
				<span class="thumb" data-attachment={a.id}>
					{#if isImage(a)}
						{#await attachmentUrl(a) then url}<img src={url} alt="" />{/await}
					{:else}
						<Icon name="file" size={18} />
					{/if}
				</span>
				<span class="text">
					<strong>{a.filename}</strong>
					<small>{formatSize(a.size, session.format)}</small>
				</span>
			</button>
			<button
				type="button"
				class="remove"
				aria-label="Remove {a.filename}"
				onclick={() => onremove(a)}
			>
				<Icon name="x" size={14} />
			</button>
		</div>
	{/each}
	{#each pending as path, i (path)}
		<div class="item" in:receiptIn={{ level: level('B5') }}>
			<span class="open">
				<span class="thumb"><Icon name="file" size={18} /></span>
				<span class="text"><strong>{name(path)}</strong></span>
			</span>
			<button
				type="button"
				class="remove"
				aria-label="Remove {name(path)}"
				onclick={() => onremovePending(i)}
			>
				<Icon name="x" size={14} />
			</button>
		</div>
	{/each}
	<button type="button" class="drop" onclick={async () => onadd(await chooseReceipts())}>
		<Icon name="upload" />Drop receipt
	</button>
</div>

<style>
	.receipts {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.label {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.item {
		display: flex;
		align-items: center;
		gap: var(--spacing-2);
		padding: 6px;
		border-radius: var(--radius-field);
	}

	.item.selected {
		background: var(--color-marker);
	}

	.open {
		display: flex;
		align-items: center;
		gap: 10px;
		flex: 1;
		min-width: 0;
		padding: 0;
		border: none;
		background: none;
		text-align: left;
		cursor: pointer;
	}

	span.open {
		cursor: default;
	}

	.thumb {
		display: flex;
		align-items: center;
		justify-content: center;
		flex: none;
		width: 36px;
		height: 44px;
		overflow: hidden;
		border-radius: var(--radius-sm);
		background: var(--color-paper-sunk);
		color: var(--color-ink-muted);
	}

	.thumb img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	strong {
		overflow: hidden;
		font-size: 13px;
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	small {
		font-size: var(--type-xs);
		color: var(--color-ink-muted);
	}

	.remove {
		display: flex;
		padding: 0;
		border: none;
		background: none;
		color: var(--color-ink-muted);
		cursor: pointer;
	}

	.drop {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-2);
		height: 44px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: none;
		font-size: 13px;
		color: var(--color-ink-muted);
		cursor: pointer;
	}
</style>
