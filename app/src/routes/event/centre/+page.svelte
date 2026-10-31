<script lang="ts">
	// A cost centre (R-CC-1), the one named in the address. The panels and selection belong to one
	// centre, so another centre starts afresh.
	import { page } from '$app/state';
	import { session } from '#lib/state/session.svelte.ts';
	import Centre from './Centre.svelte';

	const id = $derived(Number(page.url.searchParams.get('id')));
	const centre = $derived(session.event!.snapshot.costCentres.find((c) => c.id === id));
</script>

{#key id}
	{#if centre}<Centre {centre} />{/if}
{/key}
