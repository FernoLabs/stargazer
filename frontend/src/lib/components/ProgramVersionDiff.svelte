<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { MergeView } from '@codemirror/merge';
	import { EditorState } from '@codemirror/state';
	import { EditorView, lineNumbers } from '@codemirror/view';

	let {
		beforeText,
		afterText,
		beforeLabel = 'Before',
		afterLabel = 'After',
		mode
	}: {
		beforeText: string;
		afterText: string;
		beforeLabel?: string;
		afterLabel?: string;
		mode: 'assembly' | 'hex';
	} = $props();

	let container: HTMLDivElement;
	let mergeView: MergeView | null = null;
	let mounted = false;

	const editorTheme = EditorView.theme({
		'&': {
			height: '72vh',
			fontSize: '12.5px',
			backgroundColor: '#fff'
		},
		'.cm-scroller': {
			fontFamily: "'IBM Plex Mono', 'SFMono-Regular', monospace",
			lineHeight: '1.45'
		},
		'.cm-gutters': {
			backgroundColor: '#f7f6f3',
			color: '#7a8793',
			borderRight: '1px solid rgba(23, 32, 42, 0.1)'
		},
		'.cm-line': {
			padding: '0 0.7rem'
		},
		'.cm-content': {
			padding: '0.5rem 0',
			minWidth: 'max-content'
		}
	});

	function editorExtensions() {
		return [
			lineNumbers(),
			EditorState.readOnly.of(true),
			EditorView.editable.of(false),
			editorTheme
		];
	}

	function render() {
		if (!mounted || !container) return;
		mergeView?.destroy();
		container.innerHTML = '';
		mergeView = new MergeView({
			a: {
				doc: beforeText || '',
				extensions: editorExtensions()
			},
			b: {
				doc: afterText || '',
				extensions: editorExtensions()
			},
			parent: container,
			collapseUnchanged: {
				margin: mode === 'hex' ? 2 : 5,
				minSize: mode === 'hex' ? 12 : 8
			},
			diffConfig: {
				scanLimit: mode === 'hex' ? 500 : 5_000
			}
		});
	}

	$effect(() => {
		beforeText;
		afterText;
		mode;
		render();
	});

	onMount(() => {
		mounted = true;
		render();
	});

	onDestroy(() => {
		mergeView?.destroy();
	});
</script>

<div class="diff-shell">
	<div class="diff-labels" aria-hidden="true">
		<span>{beforeLabel}</span>
		<span>{afterLabel}</span>
	</div>
	<div class="diff-host" bind:this={container}></div>
</div>

<style>
	.diff-shell {
		display: grid;
		gap: 0;
	}

	.diff-labels {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-bottom: 0;
		border-radius: 8px 8px 0 0;
		overflow: hidden;
		background: var(--fw-border);
	}

	.diff-labels span {
		min-width: 0;
		padding: 0.48rem 0.68rem;
		background: #f7f6f3;
		color: #596775;
		font-size: 0.78rem;
		font-weight: 800;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.diff-host {
		min-height: 72vh;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 0 0 8px 8px;
		overflow: hidden;
		background: #fff;
	}

	:global(.cm-mergeView) {
		height: 72vh;
	}

	:global(.cm-mergeViewEditors) {
		height: 100%;
	}

	:global(.cm-mergeViewEditor) {
		height: 100%;
	}

	:global(.cm-editor) {
		height: 100%;
	}

	:global(.cm-scroller) {
		overflow: auto;
	}

	:global(.cm-changedLine) {
		background: rgba(189, 74, 23, 0.08);
	}

	:global(.cm-deletedChunk) {
		background: rgba(175, 35, 20, 0.08);
	}

	:global(.cm-insertedChunk) {
		background: rgba(61, 140, 99, 0.1);
	}

	@media (max-width: 860px) {
		.diff-labels {
			grid-template-columns: 1fr;
		}

		:global(.cm-mergeViewEditors) {
			flex-direction: column;
		}
	}
</style>
