<script lang="ts">
	// One idea of the motion plan at one strength, built from the effects the app uses. Each demo
	// plays once when it appears (the page remounts it to replay) and can be played by hand.
	import { onMount, untrack } from 'svelte';
	import { flip } from 'svelte/animate';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import type { Idea, Strength } from '#lib/motion/choices.ts';
	import Check from '#lib/motion/Check.svelte';
	import Lock from '#lib/motion/Lock.svelte';
	import MovingAmount from '#lib/motion/MovingAmount.svelte';
	import { centreOf, confetti, sparkle } from '#lib/motion/confetti.ts';
	import * as fx from '#lib/motion/effects.ts';
	import { level as levelOf } from '#lib/motion/mode.svelte.ts';
	import { formatAmount } from '#lib/format.ts';

	let { id, strength }: { id: Idea; strength: Strength } = $props();
	const level = $derived(levelOf(id, strength));
	const later = (ms: number, f: () => void) => setTimeout(f, ms);

	// A1 · pages and the sidebar marker
	const PAGES = ['Overview', 'Ledger', 'People'];
	let page = $state(0);
	let marker = $state<HTMLSpanElement>();
	let navItems: HTMLElement[] = $state([]);
	function go(i: number) {
		if (i === page) return;
		const from = navItems[page].getBoundingClientRect();
		page = i;
		queueMicrotask(
			() => marker && fx.markerTravel(marker, from, navItems[i].getBoundingClientRect(), level)
		);
	}

	// A2 · panel
	let panel = $state(false);

	// A4 · segmented
	let kind = $state<'expense' | 'income' | 'transfer'>('expense');

	// A5, C1, C2, D4 · amounts
	let amount = $state(41290);
	let owed = $state(21240);
	let settled = $state(false);

	// A6 · field error
	let invalid = $state(false);
	let fieldBox = $state<HTMLElement>();
	function commitInvalid() {
		invalid = false;
		queueMicrotask(() => {
			invalid = true;
			if (fieldBox) fx.shake(fieldBox, level);
		});
	}

	// B1, B2, B3 · rows
	let next = 4;
	let rows = $state([
		{ id: 1, text: 'Hall rent', cents: -180000, kind: 'expense' },
		{ id: 2, text: 'Ticket sheet', cents: 622750, kind: 'income' },
		{ id: 3, text: 'Groceries', cents: -31240, kind: 'expense' }
	]);
	let filter = $state<'all' | 'expense'>('all');
	const visible = $derived(rows.filter((r) => filter === 'all' || r.kind === filter));
	const addRow = () =>
		(rows = [{ id: next++, text: 'Bakery', cents: -22635, kind: 'expense' }, ...rows]);
	const removeRow = (rid: number) => (rows = rows.filter((r) => r.id !== rid));

	// B4 · form
	let form = $state(1);

	// B5, B6 · receipts
	let receipts = $state([1]);
	let thumb = $state<HTMLElement>();
	let preview = $state<HTMLElement>();
	let previewing = $state(false);
	function openPreview() {
		const from = thumb?.getBoundingClientRect() ?? null;
		previewing = true;
		queueMicrotask(() => preview && fx.growFrom(preview, from, level));
	}

	// C2 · payouts
	let paid = $state(false);

	// D3, E3 · open items
	let items = $state(['Anna Krüger is owed', 'Cups and ice: no receipt']);
	let lastItem = $state(['Cash box: 2,90 € less than recorded']);
	let celebrate = $state<HTMLElement>();
	function fixLast() {
		lastItem = [];
		if (!celebrate) return;
		if (level === 'big') confetti(centreOf(celebrate), 90, 0.9);
		else if (level === 'playful') sparkle(centreOf(celebrate));
	}

	// D4 · count
	let counted = $state(41000);
	const diff = $derived(counted - 41290);

	// E1, E2 · closing
	// E2 starts closed so it can reopen; the page remounts a demo when its idea changes.
	let closed = $state(untrack(() => id === 'E2'));
	let closeButton = $state<HTMLElement>();
	function close() {
		closed = true;
		if (level === 'big' && closeButton) confetti(centreOf(closeButton));
	}

	// F1 · opening an event
	let opened = $state(false);
	let recentRow = $state<HTMLElement>();
	async function openEvent() {
		if (recentRow) await fx.expandInto(recentRow, level);
		opened = true;
	}

	// F2 · check again
	let checking = $state(0);
	let arrow = $state<HTMLElement>();
	function check() {
		if (arrow) fx.spin(arrow, level);
		checking++;
	}

	// F3 · report
	let ready = $state(0);
	let printButton = $state<HTMLElement>();
	function loadReport() {
		ready = 0;
		for (let i = 1; i <= 4; i++)
			later(i * 260, () => {
				ready = i;
				if (i === 4 && printButton) fx.nudge(printButton, level);
			});
	}

	// Play each demo once on arrival.
	onMount(() => {
		const plays: Partial<Record<Idea, () => void>> = {
			A1: () => go(1),
			A2: () => (panel = true),
			A4: () => (kind = 'income'),
			A5: () => (amount += 1250),
			A6: commitInvalid,
			B1: addRow,
			B2: () => removeRow(2),
			B3: () => (filter = 'expense'),
			B4: () => form++,
			B5: () => (receipts = [...receipts, receipts.length + 1]),
			B6: openPreview,
			C1: () => ((owed = 0), (settled = true)),
			C2: () => (paid = true),
			D3: () => (items = items.slice(1)),
			D4: () => (counted = 41290),
			E1: close,
			E2: () => (closed = false),
			E3: fixLast,
			F1: openEvent,
			F2: check,
			F3: loadReport
		};
		const t = later(450, () => plays[id]?.());
		return () => clearTimeout(t);
	});
</script>

<div class="demo">
	{#if id === 'A1'}
		<div class="mini-app">
			<nav>
				<span class="marker" bind:this={marker} style:top="{6 + page * 30}px"></span>
				{#each PAGES as p, i (p)}
					<button type="button" class:on={page === i} bind:this={navItems[i]} onclick={() => go(i)}
						>{p}</button
					>
				{/each}
			</nav>
			{#key page}
				<div class="page" in:fx.pageIn={{ level }}>
					<strong>{PAGES[page]}</strong>
					<span class="line"></span><span class="line short"></span>
				</div>
			{/key}
		</div>
	{:else if id === 'A2'}
		<div class="mini-app">
			<div class="page">
				<Button size="sm" onclick={() => (panel = !panel)}>{panel ? 'Close' : 'Open'} panel</Button>
			</div>
			{#if panel}
				<aside in:fx.panelIn={{ level }} out:fx.panelOut={{ level }}>
					<strong>Groceries</strong>
					<span class="line"></span><span class="line short"></span>
				</aside>
			{/if}
		</div>
	{:else if id === 'A3'}
		<div class="row-of">
			<Button kind="primary" icon="plus" {strength}>New entry</Button>
			<Button {strength}>Cancel</Button>
		</div>
	{:else if id === 'A4'}
		<Segmented
			bind:value={kind}
			{strength}
			options={[
				{ value: 'expense', label: 'Expense' },
				{ value: 'income', label: 'Income' },
				{ value: 'transfer', label: 'Transfer' }
			]}
		/>
	{:else if id === 'A5'}
		<div class="figure"><MovingAmount value={amount} format="de" sign="always" {level} /></div>
		<div class="row-of">
			<Button size="sm" onclick={() => (amount += 1250)}>+ 12,50</Button>
			<Button size="sm" onclick={() => (amount -= 300)}>− 3,00</Button>
		</div>
	{:else if id === 'A6'}
		<div class="field" class:invalid bind:this={fieldBox}>
			<span>1.2.3</span>
			{#if invalid}
				<span class="warn" in:fx.warnIn={{ level }}><Icon name="warn" size={14} /></span>
			{/if}
		</div>
		<Button size="sm" onclick={commitInvalid}>Commit</Button>
	{:else if id === 'B1' || id === 'B2' || id === 'B3'}
		{#if id === 'B3'}
			<div class="row-of">
				<Button size="sm" onclick={() => (filter = 'all')}>All</Button>
				<Button size="sm" onclick={() => (filter = 'expense')}>Expenses</Button>
			</div>
		{/if}
		<ul class="rows">
			{#each id === 'B3' ? visible : rows as r (r.id)}
				<li
					animate:flip={fx.flipOptions(id === 'B3' ? level : 'off')}
					in:fx.rowIn={{ level: id === 'B1' ? level : 'off' }}
					out:fx.rowOut={{ level: id === 'B2' || id === 'B3' ? level : 'off' }}
				>
					<span class="sweep"></span>
					<span class="strike"></span>
					<span>{r.text}</span>
					<span>{formatAmount(r.cents, 'de')}</span>
					{#if id === 'B2'}
						<button type="button" class="x" onclick={() => removeRow(r.id)} aria-label="Delete"
							><Icon name="x" size={14} /></button
						>
					{/if}
				</li>
			{/each}
		</ul>
		{#if id === 'B1'}<Button size="sm" icon="plus" onclick={addRow}>Entry</Button>{/if}
	{:else if id === 'B4'}
		<div class="form-stage">
			{#key form}
				<div class="form" in:fx.formIn={{ level }} out:fx.formOut={{ level }}>
					<span class="label">Entry {form}</span>
					<span class="input">{form % 2 ? 'Bakery' : 'Metro'}</span>
					<span class="input">{form % 2 ? '22,35' : '58,10'}</span>
				</div>
			{/key}
		</div>
		<Button size="sm" onclick={() => form++}>Save & next</Button>
	{:else if id === 'B5'}
		<div class="strip">
			{#each receipts as r (r)}
				<span class="receipt" in:fx.receiptIn={{ level }}>
					<span class="line"></span><span class="line short"></span><span class="line"></span>
				</span>
			{/each}
		</div>
		<Button size="sm" onclick={() => (receipts = [...receipts, receipts.length + 1])}
			>Drop a receipt</Button
		>
	{:else if id === 'B6'}
		<div class="stage">
			<button
				type="button"
				class="receipt"
				aria-label="Open the receipt"
				bind:this={thumb}
				onclick={openPreview}
			>
				<span class="line"></span><span class="line short"></span>
			</button>
			{#if previewing}
				<button
					type="button"
					class="preview"
					bind:this={preview}
					onclick={() => (previewing = false)}
				>
					<strong>METRO Leipzig</strong>
					<span class="line"></span><span class="line short"></span><span class="line"></span>
				</button>
			{/if}
		</div>
	{:else if id === 'C1'}
		<div class="person">
			<span>Anna Krüger</span>
			{#if settled}
				<span class="stamp" in:fx.stampIn={{ level }}>Settled</span>
			{:else}
				<MovingAmount value={owed} format="de" {level} />
			{/if}
		</div>
		<Button size="sm" onclick={() => ((settled = false), (owed = 21240))}>Owed again</Button>
	{:else if id === 'C2'}
		<ul class="rows">
			{#each [['Anna Krüger', 21240], ['Malte Brandt', 11000], ['Kim Laue', 5850]] as [name, cents], i (name)}
				<li class:done={paid}>
					<span>{name}</span>
					<span class="pay">
						{#if paid}<span class="ok"><Check {level} delay={fx.wave(level, i)} /></span>{/if}
						{formatAmount(cents as number, 'de')}
					</span>
				</li>
			{/each}
		</ul>
		<div class="total">
			<MovingAmount value={paid ? 0 : 38090} format="de" {level} />
		</div>
	{:else if id === 'C3'}
		{#key checking}
			<div class="field">
				<span>DE89 3704 0044 0532 0130 00</span>
				{#if checking}<span class="ok"><Check {level} /></span>{/if}
			</div>
		{/key}
		<Button size="sm" onclick={() => checking++}>Check IBAN</Button>
	{:else if id === 'D1'}
		<div class="sections">
			<div class="section" in:fx.riseIn={{ level, i: 0 }}>
				<span class="label">Result so far</span>
				<span class="big"
					><MovingAmount value={202750} format="de" sign="always" {level} countUp /></span
				>
			</div>
			<div class="section" in:fx.riseIn={{ level, i: 1 }}><span class="line"></span></div>
			<div class="section" in:fx.riseIn={{ level, i: 2 }}>
				<span class="line"></span><span class="line short"></span>
			</div>
		</div>
	{:else if id === 'D2'}
		{#each [[['var(--color-plus)', 70], ['var(--color-plus)', 9]], [['var(--color-tier-1)', 26], ['var(--color-tier-3)', 6], ['var(--color-tier-4)', 22], ['var(--color-tier-5)', 8]]] as bar, b (b)}
			<div class="bar">
				{#each bar as [color, w], i (i)}
					<span
						style:background={color}
						style:width="{w}%"
						{@attach (node) => fx.growBar(node, level, i)}
					></span>
				{/each}
			</div>
		{/each}
	{:else if id === 'D3' || id === 'E3'}
		<ul class="rows" bind:this={celebrate}>
			{#each id === 'D3' ? items : lastItem as item (item)}
				<li out:fx.rowOut={{ level: level === 'big' ? 'playful' : level }}>
					<span class="dot"></span>
					<span>{item}</span>
					<button
						type="button"
						class="x"
						onclick={() => (id === 'D3' ? (items = items.filter((x) => x !== item)) : fixLast())}
						>Fix</button
					>
				</li>
			{:else}
				<li class="nothing"><span class="ok"><Check {level} delay={160} /></span>Nothing open</li>
			{/each}
		</ul>
	{:else if id === 'D4'}
		<div class="sums">
			<div><span>In the books</span><span>412,90 €</span></div>
			<div><span>Counted</span><MovingAmount value={counted} format="de" {level} /></div>
			<div class="total">
				{#if diff === 0}
					<span class="match" in:fx.popIn={{ level }}
						><span class="ok"><Check {level} delay={120} /></span>Matches the books</span
					>
				{:else}
					<strong>Less than recorded</strong><strong class="minus"
						>{formatAmount(diff, 'de', 'magnitude')}</strong
					>
				{/if}
			</div>
		</div>
		<Button size="sm" onclick={() => (counted = counted === 41290 ? 41000 : 41290)}
			>Count again</Button
		>
	{:else if id === 'E1' || id === 'E2'}
		<div class="head">
			<strong>Overview</strong>
			{#if closed}
				<span class="chip" in:fx.stampIn={{ level: id === 'E1' ? level : 'calm' }}
					><Lock closed {level} size={14} />Closed 21.06.2026</span
				>
			{/if}
			<span class="grow"></span>
			<span bind:this={closeButton}>
				{#if closed}
					<Button size="sm" onclick={() => (closed = false)}
						><span class="with-lock"><Lock closed={false} {level} size={14} />Reopen</span></Button
					>
				{:else}
					<Button size="sm" onclick={close}
						><span class="with-lock"><Lock closed={false} {level} size={14} />Close event</span
						></Button
					>
				{/if}
			</span>
		</div>
	{:else if id === 'F1'}
		<div class="stage">
			{#if opened}
				<div class="page" in:fx.pageIn={{ level }}>
					<strong>Sommerfest 2026</strong>
					<span class="line"></span><span class="line short"></span>
					<Button size="sm" onclick={() => (opened = false)}>Back</Button>
				</div>
			{:else}
				<button type="button" class="recent" bind:this={recentRow} onclick={openEvent}>
					<strong>Sommerfest 2026</strong><small>12.–14.06.2026</small>
				</button>
			{/if}
		</div>
	{:else if id === 'F2'}
		<div class="head">
			{#key checking}
				<span class="ok-line">
					{#if checking}<span class="ok"><Check {level} delay={240} /></span>{/if}
					<strong>No issues</strong>
				</span>
			{/key}
			<span class="grow"></span>
			<button type="button" class="ghost" onclick={check}>
				<span class="spin" bind:this={arrow}><Icon name="refresh" size={14} /></span>Check again
			</button>
		</div>
	{:else if id === 'F3'}
		<div class="head">
			<span class="muted">Receipts {ready < 4 ? `${ready}/` : ''}4</span>
			<span class="grow"></span>
			<span bind:this={printButton}>
				<Button kind="primary" size="sm" icon="file" disabled={ready < 4}>Print</Button>
			</span>
		</div>
		<div class="strip">
			{#each Array.from({ length: ready }, (_, i) => i) as r (r)}
				<span class="receipt" in:fx.riseIn={{ level: level === 'playful' ? 'regular' : level }}>
					<span class="line"></span><span class="line short"></span>
				</span>
			{/each}
		</div>
		<Button size="sm" onclick={loadReport}>Load again</Button>
	{/if}
</div>

<style>
	.demo {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 10px;
		min-height: 120px;
		font-size: 13px;
	}

	.row-of,
	.head {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
	}

	.grow {
		flex: 1;
	}

	.muted,
	.label,
	small {
		color: var(--color-ink-muted);
		font-size: 12px;
	}

	.line {
		display: block;
		width: 100%;
		height: 6px;
		border-radius: 3px;
		background: var(--color-rule);
	}

	.line.short {
		width: 60%;
	}

	.mini-app {
		position: relative;
		display: flex;
		width: 100%;
		height: 120px;
		overflow: hidden;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-md);
	}

	nav {
		position: relative;
		display: flex;
		flex-direction: column;
		width: 90px;
		padding: 6px;
		background: var(--color-paper-sunk);
	}

	nav button {
		position: relative;
		height: 30px;
		padding: 0 8px;
		border: none;
		background: none;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	nav button.on {
		font-weight: 600;
	}

	.marker {
		position: absolute;
		left: 6px;
		right: 6px;
		height: 30px;
		margin-top: 0;
		border-radius: var(--radius-field);
		background: var(--color-marker);
		transform-origin: center;
	}

	.page {
		display: flex;
		flex-direction: column;
		gap: 8px;
		flex: 1;
		padding: 12px;
	}

	aside {
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: 45%;
		padding: 12px;
		border-left: 1px solid var(--color-rule);
		background: var(--color-paper);
	}

	.figure {
		font-size: 26px;
		font-weight: 600;
	}

	.field {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		width: 100%;
		height: 34px;
		padding: 0 10px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-field);
	}

	.field.invalid {
		border-color: var(--color-minus);
	}

	.warn {
		display: flex;
		color: var(--color-minus);
	}

	.ok {
		display: inline-flex;
		color: var(--color-plus);
	}

	.rows {
		width: 100%;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.rows li {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		height: 34px;
		padding: 0 6px;
		border-bottom: 1px solid var(--color-rule);
	}

	.rows li.done {
		color: var(--color-ink-muted);
		transition: color 300ms;
	}

	.sweep,
	.strike {
		position: absolute;
		pointer-events: none;
		transform-origin: left center;
	}

	.sweep {
		inset: 4px 0;
		border-radius: 3px;
		background: var(--color-marker);
		opacity: 0;
		z-index: -1;
	}

	.strike {
		left: 4px;
		right: 4px;
		top: 50%;
		height: 1.5px;
		background: var(--color-ink);
		transform: scaleX(0);
	}

	.x {
		display: flex;
		padding: 2px 6px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: none;
		font: inherit;
		font-size: 12px;
		cursor: pointer;
	}

	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--color-attention);
	}

	.nothing {
		justify-content: flex-start !important;
		color: var(--color-plus);
	}

	.form-stage,
	.stage {
		position: relative;
		width: 100%;
		height: 96px;
		overflow: hidden;
	}

	.form {
		position: absolute;
		inset: 0;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.input {
		display: block;
		height: 24px;
		padding: 3px 8px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: var(--color-field);
	}

	.strip {
		display: flex;
		gap: 8px;
		min-height: 64px;
	}

	.receipt {
		display: flex;
		flex-direction: column;
		gap: 5px;
		width: 46px;
		height: 60px;
		padding: 8px 6px;
		border: 1px solid var(--color-rule);
		border-radius: 3px;
		background: white;
		cursor: pointer;
	}

	.preview {
		position: absolute;
		inset: 0 0 0 64px;
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 10px;
		border: 1px solid var(--color-rule);
		background: white;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.person,
	.sums > div {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		height: 32px;
	}

	.sums {
		width: 100%;
	}

	.sums .total {
		border-top: 1px solid var(--color-rule);
	}

	.match {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-weight: 600;
		color: var(--color-plus);
	}

	.minus {
		color: var(--color-minus);
	}

	.stamp {
		padding: 2px 8px;
		border: 1.5px solid var(--color-plus);
		border-radius: var(--radius-sm);
		font-weight: 600;
		color: var(--color-plus);
	}

	.pay {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}

	.total {
		align-self: flex-end;
		font-weight: 600;
	}

	.sections {
		display: flex;
		flex-direction: column;
		gap: 10px;
		width: 100%;
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.big {
		font-size: 26px;
		font-weight: 600;
		color: var(--color-plus);
	}

	.bar {
		display: flex;
		gap: 2px;
		width: 100%;
	}

	.bar span {
		height: 12px;
		transform-origin: left center;
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 26px;
		padding: 0 8px;
		border-radius: var(--radius-sm);
		background: var(--color-paper-sunk);
		font-weight: 600;
		color: var(--color-ink-muted);
	}

	.recent {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 2px;
		width: 100%;
		padding: 8px 10px;
		border: none;
		border-radius: var(--radius-field);
		background: var(--color-marker);
		font: inherit;
		cursor: pointer;
	}

	.ok-line {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}

	.ghost {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 28px;
		padding: 0 10px;
		border: 1px solid var(--color-rule);
		border-radius: var(--radius-field);
		background: none;
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}

	.spin {
		display: flex;
	}

	.with-lock {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
</style>
