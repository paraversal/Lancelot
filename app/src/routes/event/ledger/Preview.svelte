<script lang="ts">
	// A receipt, large, in place of the table; the panel stays beside it (R-ATT-3). Images,
	// PDFs and HEIC photos are shown by WebKit itself; anything else can be opened.
	import type { Attachment } from '#lib/api/generated/Attachment.ts';
	import { api, attachmentUrl } from '#lib/api/index.ts';
	import { chooseExportPath } from '#lib/api/dialogs.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import { formatSize } from '#lib/format.ts';
	import { session } from '#lib/state/session.svelte.ts';

	let { attachment, onclose }: { attachment: Attachment; onclose: () => void } = $props();

	let error = $state<string | null>(null);
	const kind = $derived(
		attachment.mime.startsWith('image/')
			? 'image'
			: attachment.mime === 'application/pdf'
				? 'pdf'
				: 'other'
	);

	async function run(f: () => Promise<unknown>) {
		try {
			error = null;
			await f();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	const exportIt = () =>
		run(async () => {
			const target = await chooseExportPath(attachment.filename);
			if (target) await api.exportAttachment(attachment.id, target);
		});
</script>

<section class="preview">
	<header>
		<h2>{attachment.filename}</h2>
		<span class="size">{formatSize(attachment.size, session.format)}</span>
		<span class="grow"></span>
		<Button size="sm" onclick={() => run(() => api.openAttachment(attachment.id))}>Open</Button>
		<Button size="sm" onclick={exportIt}>Export</Button>
		<button type="button" class="close" aria-label="Close preview" onclick={onclose}
			><Icon name="x" size={18} /></button
		>
	</header>
	<div class="stage">
		{#await attachmentUrl(attachment) then url}
			{#if kind === 'image'}
				<img src={url} alt={attachment.filename} />
			{:else if kind === 'pdf'}
				<iframe src={url} title={attachment.filename}></iframe>
			{:else}
				<span class="other"><Icon name="file" size={28} /></span>
			{/if}
		{/await}
	</div>
	{#if error}<p class="error">{error}</p>{/if}
</section>

<style>
	.preview {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-4);
		flex: 1;
		min-width: 0;
		padding: var(--spacing-10);
	}

	header {
		display: flex;
		align-items: center;
		gap: 10px;
		min-height: 40px;
	}

	h2 {
		margin: 0;
		overflow: hidden;
		font-size: var(--type-md);
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.size {
		font-size: 13px;
		color: var(--color-ink-muted);
	}

	.grow {
		flex: 1;
	}

	.close {
		display: flex;
		padding: 0;
		border: none;
		background: none;
		color: var(--color-ink-muted);
		cursor: pointer;
	}

	.stage {
		display: flex;
		align-items: center;
		justify-content: center;
		flex: 1;
		min-height: 0;
		border-radius: var(--radius-md);
		background: var(--color-paper-sunk);
		overflow: hidden;
	}

	img {
		max-width: calc(100% - 64px);
		max-height: calc(100% - 64px);
		object-fit: contain;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-sm);
		background: var(--color-field);
	}

	iframe {
		width: 100%;
		height: 100%;
		border: none;
	}

	.other {
		display: flex;
		color: var(--color-ink-muted);
	}

	.error {
		margin: 0;
		font-size: var(--type-xs);
		color: var(--color-minus);
	}
</style>
