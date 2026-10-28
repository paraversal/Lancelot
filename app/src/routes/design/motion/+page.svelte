<script lang="ts">
	// The motion trial page (docs/design/motion.md): every idea at every strength, side by side,
	// built from the effects the app uses. The strength the app uses is marked; choices.ts
	// decides it. Replay remounts a demo; the Motion switch shows Calm and Off.
	import type { Motion } from '#lib/api/generated/Motion.ts';
	import Segmented from '#lib/components/Segmented.svelte';
	import { CHOICES, IDEAS, STRENGTHS, type Idea, type Strength } from '#lib/motion/choices.ts';
	import { motion } from '#lib/motion/mode.svelte.ts';
	import Demo from './Demo.svelte';

	const GROUPS: { key: string; title: string }[] = [
		{ key: 'A', title: 'Everywhere' },
		{ key: 'B', title: 'Ledger' },
		{ key: 'C', title: 'People and payouts' },
		{ key: 'D', title: 'Overview and counts' },
		{ key: 'E', title: 'The big moments' },
		{ key: 'F', title: 'Start screen, files, report' }
	];
	const ids = Object.keys(IDEAS) as Idea[];
	const LABEL: Record<Strength, string> = {
		regular: 'Regular',
		playful: 'Playful',
		big: 'Big',
		skip: 'Skip'
	};

	let plays = $state<Record<string, number>>({});
	const replay = (key: string) => (plays[key] = (plays[key] ?? 0) + 1);
	const replayAll = (id: Idea) => STRENGTHS[id].forEach((s) => replay(`${id}-${s}`));
</script>

<div class="trial">
	<header>
		<h1>Motion</h1>
		<Segmented
			label="Motion"
			value={motion.setting}
			onchange={(v: Motion) => (motion.setting = v)}
			options={[
				{ value: 'full', label: 'Full' },
				{ value: 'calm', label: 'Calm' },
				{ value: 'off', label: 'Off' }
			]}
		/>
	</header>

	{#each GROUPS as g (g.key)}
		<section>
			<h2>{g.key} · {g.title}</h2>
			{#each ids.filter((id) => id.startsWith(g.key)) as id (id)}
				<article>
					<div class="idea">
						<strong>{id}</strong>
						<span>{IDEAS[id]}</span>
						<button type="button" class="replay" onclick={() => replayAll(id)}>Replay</button>
					</div>
					<div class="cells">
						{#each STRENGTHS[id] as s (s)}
							{@const key = `${id}-${s}`}
							<div class="cell" class:chosen={CHOICES[id] === s}>
								<div class="cell-head">
									<span>{LABEL[s]}</span>
									{#if CHOICES[id] === s}<span class="badge">in the app</span>{/if}
									<button type="button" class="replay" onclick={() => replay(key)}>▶</button>
								</div>
								{#key `${plays[key] ?? 0}-${motion.setting}`}
									<Demo {id} strength={s} />
								{/key}
							</div>
						{/each}
					</div>
				</article>
			{/each}
		</section>
	{/each}
</div>

<style>
	.trial {
		height: 100%;
		overflow: auto;
		padding: var(--spacing-10);
	}

	header {
		position: sticky;
		top: calc(-1 * var(--spacing-10));
		z-index: 3;
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin: calc(-1 * var(--spacing-10)) calc(-1 * var(--spacing-10)) 0;
		padding: var(--spacing-6) var(--spacing-10);
		border-bottom: 1px solid var(--color-rule);
		background: var(--color-paper);
	}

	h1 {
		margin: 0;
		font-size: var(--type-lg);
		font-weight: 600;
	}

	h2 {
		margin: 36px 0 12px;
		font-size: var(--type-md);
		font-weight: 600;
	}

	article {
		display: grid;
		grid-template-columns: 180px 1fr;
		gap: 20px;
		padding: 18px 0;
		border-top: 1px solid var(--color-rule);
	}

	.idea {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 4px;
	}

	.cells {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
		gap: 16px;
	}

	.cell {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 12px;
		border-radius: var(--radius-md);
		background: var(--color-paper-sunk);
	}

	.cell.chosen {
		outline: 2px solid var(--color-marker);
	}

	.cell-head {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 12px;
		font-weight: 600;
		color: var(--color-ink-muted);
	}

	.badge {
		padding: 1px 6px;
		border-radius: var(--radius-pill);
		background: var(--color-marker);
		color: var(--color-ink);
	}

	.cell-head .replay {
		margin-left: auto;
	}

	.replay {
		padding: 2px 8px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-paper);
		font: inherit;
		font-size: 12px;
		cursor: pointer;
	}
</style>
